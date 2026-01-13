//! Codex - Main orchestration interface
//!
//! This module provides the high-level interface to the Codex system, operating
//! as a queue pair where you send submissions and receive events.
//!
//! The implementation uses DashFlow StateGraph for agent workflow orchestration
//! while maintaining API compatibility with the original Codex CLI interface.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::compact::{
    build_compacted_history_with_limit, collect_user_messages, create_compaction_prompt,
    estimate_history_tokens, should_compact, CompactConfig,
};
use crate::config::Config;
use crate::execpolicy::ExecPolicy;
use crate::ghost_commit::{
    create_ghost_commit, restore_ghost_commit, CreateGhostCommitOptions, GhostCommit,
};
use crate::graph::build_agent_graph;
use crate::llm::{LlmClient, LlmConfig};
use crate::state::{AgentState, CompletionStatus, Message, MessageRole};
use crate::streaming::StreamCallback;
use crate::Result;

/// Channel capacity for submissions
pub const SUBMISSION_CHANNEL_CAPACITY: usize = 64;

/// Operations that can be submitted to the Codex agent.
///
/// These are the commands that control agent behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum Op {
    /// Abort the current task.
    /// The agent sends [`Event::TurnAborted`] in response.
    Interrupt,

    /// Input from the user to process.
    UserInput {
        /// The user's message or query
        message: String,
        /// Optional context items to include
        #[serde(default)]
        context: Vec<ContextItem>,
    },

    /// Similar to [`Op::UserInput`], but includes turn-level configuration.
    UserTurn {
        /// The user's message
        message: String,
        /// Working directory for sandbox and tool calls
        #[serde(default)]
        cwd: Option<PathBuf>,
        /// Policy to use for command approval
        #[serde(default)]
        approval_policy: ApprovalPolicy,
        /// Policy to use for sandboxing
        #[serde(default)]
        sandbox_policy: SandboxPolicy,
        /// Model to use for this turn
        #[serde(default)]
        model: Option<String>,
    },

    /// Override parts of the persistent turn context for subsequent turns.
    OverrideTurnContext {
        /// Updated working directory
        #[serde(default)]
        cwd: Option<PathBuf>,
        /// Updated approval policy
        #[serde(default)]
        approval_policy: Option<ApprovalPolicy>,
        /// Updated sandbox policy
        #[serde(default)]
        sandbox_policy: Option<SandboxPolicy>,
        /// Updated model
        #[serde(default)]
        model: Option<String>,
    },

    /// Approve or deny a pending shell command execution.
    ExecApproval {
        /// ID of the approval request
        id: String,
        /// The decision
        decision: ApprovalDecision,
    },

    /// Approve or deny a pending patch application.
    PatchApproval {
        /// ID of the approval request
        id: String,
        /// The decision
        decision: ApprovalDecision,
    },

    /// Add text to the conversation history without triggering a turn.
    AddToHistory {
        /// Text to add
        text: String,
    },

    /// Request conversation compaction to reduce token count.
    Compact,

    /// Undo the last agent action.
    Undo,

    /// Request a code review.
    Review {
        /// The review request details
        request: ReviewRequest,
    },

    /// Shutdown the agent gracefully.
    Shutdown,

    /// List available models.
    ListModels,

    /// List available MCP tools.
    ListMcpTools,

    /// Run a user shell command (not agent-initiated).
    RunUserShellCommand {
        /// The command to run
        command: String,
    },
}

/// Context item that can be included with user input.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContextItem {
    /// A file to include as context
    File { path: PathBuf },
    /// A URL to fetch and include
    Url { url: String },
    /// Raw text context
    Text { content: String },
}

/// Policy for when to ask for approval before execution.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    /// Never ask for approval (automatic execution)
    Never,
    /// Ask on first unknown command, then remember
    #[default]
    OnUnknown,
    /// Always ask for approval
    Always,
}

/// Policy for sandboxing command execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SandboxPolicy {
    /// No sandboxing
    None,
    /// Use platform-native sandbox (Seatbelt on macOS, Landlock on Linux)
    #[default]
    Native,
    /// Use Docker container for isolation
    Docker { image: Option<String> },
}

/// Decision for approval requests.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Approve the action
    Approve,
    /// Deny the action
    Deny,
    /// Approve and remember for similar future actions
    ApproveAndRemember,
    /// Deny and remember for similar future actions
    DenyAndRemember,
}

impl std::fmt::Display for ApprovalPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalPolicy::Never => write!(f, "never"),
            ApprovalPolicy::OnUnknown => write!(f, "on-unknown"),
            ApprovalPolicy::Always => write!(f, "always"),
        }
    }
}

impl std::fmt::Display for SandboxPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxPolicy::None => write!(f, "none"),
            SandboxPolicy::Native => write!(f, "native"),
            SandboxPolicy::Docker { image } => {
                if let Some(img) = image {
                    write!(f, "docker:{}", img)
                } else {
                    write!(f, "docker")
                }
            }
        }
    }
}

impl std::fmt::Display for ApprovalDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalDecision::Approve => write!(f, "approve"),
            ApprovalDecision::Deny => write!(f, "deny"),
            ApprovalDecision::ApproveAndRemember => write!(f, "approve-remember"),
            ApprovalDecision::DenyAndRemember => write!(f, "deny-remember"),
        }
    }
}

/// A request for code review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRequest {
    /// Type of review
    #[serde(default)]
    pub review_type: ReviewType,
    /// Files to review (if empty, reviews recent changes)
    #[serde(default)]
    pub files: Vec<PathBuf>,
    /// Focus areas for the review
    #[serde(default)]
    pub focus: Vec<String>,
}

/// Type of code review to perform.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewType {
    /// Full code review
    #[default]
    Full,
    /// Security-focused review
    Security,
    /// Performance-focused review
    Performance,
    /// Style and formatting review
    Style,
}

/// A submission to the Codex agent.
#[derive(Debug, Clone)]
pub struct Submission {
    /// Unique ID for this submission
    pub id: String,
    /// The operation to perform
    pub op: Op,
}

/// Events emitted by the Codex agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum Event {
    /// Session has been configured and is ready.
    SessionConfigured {
        /// Unique session ID
        session_id: String,
        /// Model being used
        model: String,
    },

    /// A turn has started processing.
    TurnStarted {
        /// Submission ID that triggered this turn
        submission_id: String,
        /// Turn number
        turn: u32,
    },

    /// Agent is reasoning/thinking.
    ReasoningStarted {
        /// Turn number
        turn: u32,
    },

    /// Partial reasoning content (streaming).
    ReasoningDelta {
        /// Partial content
        content: String,
    },

    /// Reasoning completed.
    ReasoningComplete {
        /// Turn number
        turn: u32,
        /// Total reasoning time in milliseconds
        duration_ms: u64,
        /// Whether tool calls were generated
        has_tool_calls: bool,
    },

    /// Tool execution started.
    ToolStarted {
        /// Tool name
        tool: String,
        /// Tool call ID
        call_id: String,
    },

    /// Tool execution completed.
    ToolComplete {
        /// Tool name
        tool: String,
        /// Call ID
        call_id: String,
        /// Whether execution succeeded
        success: bool,
        /// Result or error message
        result: String,
    },

    /// Request for approval before executing a command.
    ExecApprovalRequest {
        /// Request ID (use in ExecApproval op)
        id: String,
        /// Command to be executed
        command: String,
        /// Assessment of the command
        assessment: CommandAssessment,
    },

    /// Request for approval before applying a patch.
    PatchApprovalRequest {
        /// Request ID (use in PatchApproval op)
        id: String,
        /// File being patched
        file: PathBuf,
        /// The patch content
        patch: String,
    },

    /// A turn has completed.
    TurnComplete {
        /// Submission ID
        submission_id: String,
        /// Turn number
        turn: u32,
        /// Final response from the agent
        response: String,
    },

    /// A turn was aborted.
    TurnAborted {
        /// Submission ID
        submission_id: String,
        /// Reason for abort
        reason: AbortReason,
    },

    /// The session has completed.
    SessionComplete {
        /// Session ID
        session_id: String,
        /// Total turns executed
        total_turns: u32,
        /// Final status
        status: String,
    },

    /// Shutdown completed.
    ShutdownComplete,

    /// An error occurred.
    Error {
        /// Submission ID (if applicable)
        submission_id: Option<String>,
        /// Error message
        message: String,
        /// Whether the error is recoverable
        recoverable: bool,
    },

    /// Token usage information.
    TokenUsage {
        /// Input tokens used
        input_tokens: u64,
        /// Output tokens used
        output_tokens: u64,
        /// Estimated cost in USD
        cost_usd: Option<f64>,
    },

    /// List of available models.
    ModelsAvailable {
        /// Available models
        models: Vec<ModelInfo>,
    },

    /// Conversation was compacted.
    Compacted {
        /// Original token count
        original_tokens: u64,
        /// New token count
        new_tokens: u64,
    },

    /// Undo operation started.
    UndoStarted,

    /// Undo operation completed.
    UndoComplete {
        /// Description of what was undone
        description: String,
    },

    /// MCP tools list response.
    McpToolsAvailable {
        /// Available MCP tools from all connected servers
        tools: Vec<McpToolInfo>,
    },

    /// Code review completed.
    ReviewComplete {
        /// Review output
        output: crate::review::ReviewOutputEvent,
    },

    /// User shell command completed.
    ShellCommandComplete {
        /// Command that was executed
        command: String,
        /// Exit code
        exit_code: i32,
        /// Standard output
        stdout: String,
        /// Standard error
        stderr: String,
    },

    /// Exec approval response (command approved/denied).
    ExecApprovalResponse {
        /// Request ID
        id: String,
        /// Whether the command was approved
        approved: bool,
    },

    /// Patch approval response.
    PatchApprovalResponse {
        /// Request ID
        id: String,
        /// Whether the patch was approved
        approved: bool,
    },
}

/// Assessment of a command's safety.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandAssessment {
    /// Risk level
    pub risk: RiskLevel,
    /// Why this assessment was made
    pub reason: String,
    /// Whether the command is known-safe
    pub known_safe: bool,
}

/// Risk level for command assessment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Safe to execute
    Safe,
    /// Low risk
    Low,
    /// Medium risk, review recommended
    Medium,
    /// High risk, approval required
    High,
    /// Critical risk, should not execute
    Critical,
}

/// Reason for turn abort.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbortReason {
    /// User requested interrupt
    UserInterrupt,
    /// Turn limit reached
    TurnLimit,
    /// Error during execution
    Error { message: String },
    /// Approval was denied
    ApprovalDenied,
    /// Shutdown requested
    Shutdown,
}

/// Information about an available model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Model provider
    pub provider: String,
    /// Whether model supports reasoning
    pub supports_reasoning: bool,
    /// Maximum context tokens
    pub max_context: Option<u64>,
}

/// Information about an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    /// Qualified tool name (mcp__server__tool)
    pub qualified_name: String,
    /// Original tool name
    pub name: String,
    /// Server providing this tool
    pub server: String,
    /// Tool description
    pub description: Option<String>,
}

/// The high-level interface to the Codex system.
///
/// Operates as a queue pair where you send submissions and receive events.
/// Uses DashFlow StateGraph for agent workflow orchestration internally.
pub struct Codex {
    /// Next submission ID
    next_id: AtomicU64,
    /// Channel to send submissions
    tx_sub: Sender<Submission>,
    /// Channel to receive events
    rx_event: Receiver<Event>,
    /// Session ID
    session_id: String,
}

/// Result of spawning a Codex instance.
pub struct CodexSpawnOk {
    /// The spawned Codex instance
    pub codex: Codex,
    /// The session ID
    pub session_id: String,
}

/// Internal session state
#[allow(dead_code)] // Fields used for future features
struct Session {
    /// Session ID
    session_id: String,
    /// Configuration
    config: Arc<Config>,
    /// Event sender
    tx_event: Sender<Event>,
    /// Current agent state
    state: Mutex<AgentState>,
    /// Stream callback for telemetry
    stream_callback: Arc<dyn StreamCallback>,
    /// Execution policy
    exec_policy: Arc<ExecPolicy>,
    /// Turn context settings
    turn_settings: Mutex<TurnSettings>,
    /// Whether shutdown was requested
    shutdown_requested: Mutex<bool>,
    /// Stack of ghost commits for undo support
    ghost_commits: Mutex<Vec<GhostCommit>>,
    /// Pending approval responses (keyed by request ID)
    pending_approvals:
        Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<ApprovalDecision>>>,
    /// MCP client for tool listing (initialized lazily)
    mcp_client: Mutex<Option<Arc<codex_dashflow_mcp::McpClient>>>,
}

/// Settings that persist across turns
#[derive(Debug, Clone)]
struct TurnSettings {
    cwd: PathBuf,
    approval_policy: ApprovalPolicy,
    sandbox_policy: SandboxPolicy,
    model: Option<String>,
}

impl Default for TurnSettings {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            approval_policy: ApprovalPolicy::default(),
            sandbox_policy: SandboxPolicy::default(),
            model: None,
        }
    }
}

impl Codex {
    /// Spawn a new Codex instance with the given configuration.
    ///
    /// This creates the submission/event channels and starts the agent loop.
    pub async fn spawn(
        config: Config,
        stream_callback: Arc<dyn StreamCallback>,
    ) -> Result<CodexSpawnOk> {
        let (tx_sub, rx_sub) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
        let (tx_event, rx_event) = async_channel::unbounded();

        // Generate unique session ID
        let session_id = uuid::Uuid::new_v4().to_string();

        // Load exec policy
        let exec_policy = Arc::new(ExecPolicy::default());

        let config = Arc::new(config);

        let session = Arc::new(Session {
            session_id: session_id.clone(),
            config: config.clone(),
            tx_event: tx_event.clone(),
            state: Mutex::new(AgentState::new()),
            stream_callback,
            exec_policy,
            turn_settings: Mutex::new(TurnSettings::default()),
            shutdown_requested: Mutex::new(false),
            ghost_commits: Mutex::new(Vec::new()),
            pending_approvals: Mutex::new(std::collections::HashMap::new()),
            mcp_client: Mutex::new(None),
        });

        // Spawn the submission processing loop
        let session_clone = session.clone();
        tokio::spawn(async move {
            submission_loop(session_clone, rx_sub).await;
        });

        // Send session configured event
        let _ = tx_event
            .send(Event::SessionConfigured {
                session_id: session_id.clone(),
                model: config.model.clone(),
            })
            .await;

        let codex = Codex {
            next_id: AtomicU64::new(0),
            tx_sub,
            rx_event,
            session_id: session_id.clone(),
        };

        Ok(CodexSpawnOk { codex, session_id })
    }

    /// Submit an operation to the agent.
    ///
    /// Returns the unique submission ID that can be used to correlate events.
    pub async fn submit(&self, op: Op) -> Result<String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst).to_string();
        let sub = Submission { id: id.clone(), op };
        self.submit_with_id(sub).await?;
        Ok(id)
    }

    /// Submit with a specific ID (use sparingly, prefer `submit()`).
    pub async fn submit_with_id(&self, sub: Submission) -> Result<()> {
        self.tx_sub
            .send(sub)
            .await
            .map_err(|_| crate::Error::AgentShutdown)?;
        Ok(())
    }

    /// Receive the next event from the agent.
    pub async fn next_event(&self) -> Result<Event> {
        self.rx_event
            .recv()
            .await
            .map_err(|_| crate::Error::AgentShutdown)
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Check if there are pending events.
    pub fn has_pending_events(&self) -> bool {
        !self.rx_event.is_empty()
    }

    /// Create a Codex instance from existing channels.
    ///
    /// This is used by delegate code to create bridged Codex instances.
    pub fn from_channels(tx_sub: Sender<Submission>, rx_event: Receiver<Event>) -> Self {
        Self {
            next_id: AtomicU64::new(0),
            tx_sub,
            rx_event,
            session_id: String::new(), // Delegate instances don't have their own session ID
        }
    }

    /// Get a clone of the ops sender channel.
    ///
    /// Used by delegate code for forwarding operations.
    pub fn ops_sender(&self) -> Sender<Submission> {
        self.tx_sub.clone()
    }
}

/// Main submission processing loop.
async fn submission_loop(session: Arc<Session>, rx_sub: Receiver<Submission>) {
    debug!(session_id = %session.session_id, "Starting submission loop");

    while let Ok(sub) = rx_sub.recv().await {
        debug!(id = %sub.id, op = ?std::mem::discriminant(&sub.op), "Processing submission");

        match sub.op.clone() {
            Op::Interrupt => {
                handle_interrupt(&session, &sub.id).await;
            }
            Op::UserInput { message, context } => {
                handle_user_input(&session, &sub.id, message, context).await;
            }
            Op::UserTurn {
                message,
                cwd,
                approval_policy,
                sandbox_policy,
                model,
            } => {
                // Update turn settings
                {
                    let mut settings = session.turn_settings.lock().await;
                    if let Some(cwd) = cwd {
                        settings.cwd = cwd;
                    }
                    settings.approval_policy = approval_policy;
                    settings.sandbox_policy = sandbox_policy;
                    settings.model = model;
                }
                handle_user_input(&session, &sub.id, message, vec![]).await;
            }
            Op::OverrideTurnContext {
                cwd,
                approval_policy,
                sandbox_policy,
                model,
            } => {
                let mut settings = session.turn_settings.lock().await;
                if let Some(cwd) = cwd {
                    settings.cwd = cwd;
                }
                if let Some(policy) = approval_policy {
                    settings.approval_policy = policy;
                }
                if let Some(policy) = sandbox_policy {
                    settings.sandbox_policy = policy;
                }
                if let Some(model) = model {
                    settings.model = Some(model);
                }
            }
            Op::ExecApproval { id, decision } => {
                handle_exec_approval(&session, &id, decision).await;
            }
            Op::PatchApproval { id, decision } => {
                handle_patch_approval(&session, &id, decision).await;
            }
            Op::AddToHistory { text } => {
                handle_add_to_history(&session, text).await;
            }
            Op::Compact => {
                handle_compact(&session, &sub.id).await;
            }
            Op::Undo => {
                handle_undo(&session, &sub.id).await;
            }
            Op::Review { request } => {
                handle_review(&session, &sub.id, request).await;
            }
            Op::Shutdown => {
                if handle_shutdown(&session, &sub.id).await {
                    break;
                }
            }
            Op::ListModels => {
                handle_list_models(&session, &sub.id).await;
            }
            Op::ListMcpTools => {
                handle_list_mcp_tools(&session, &sub.id).await;
            }
            Op::RunUserShellCommand { command } => {
                handle_run_user_shell_command(&session, &sub.id, command).await;
            }
        }
    }

    debug!(session_id = %session.session_id, "Submission loop exited");
}

// Handler implementations

async fn handle_interrupt(session: &Session, _sub_id: &str) {
    info!(session_id = %session.session_id, "Interrupt requested");
    let mut state = session.state.lock().await;
    state.status = CompletionStatus::Interrupted;
}

async fn handle_user_input(
    session: &Session,
    sub_id: &str,
    message: String,
    _context: Vec<ContextItem>,
) {
    // Capture ghost commit before agent operations for undo support
    capture_ghost_commit(session).await;

    let turn;
    {
        let mut state = session.state.lock().await;
        state.turn_count += 1;
        turn = state.turn_count;
        state.messages.push(crate::state::Message {
            role: MessageRole::User,
            content: message.clone(),
            tool_call_id: None,
            tool_calls: vec![],
        });
    }

    // Emit turn started event
    let _ = session
        .tx_event
        .send(Event::TurnStarted {
            submission_id: sub_id.to_string(),
            turn,
        })
        .await;

    // Build and run the agent graph
    let graph = match build_agent_graph() {
        Ok(g) => g,
        Err(e) => {
            error!("Failed to build agent graph: {}", e);
            let _ = session
                .tx_event
                .send(Event::Error {
                    submission_id: Some(sub_id.to_string()),
                    message: format!("Failed to build agent graph: {}", e),
                    recoverable: false,
                })
                .await;
            return;
        }
    };

    // Get current state (messages already updated above)
    let state = session.state.lock().await.clone();

    // Run the graph
    let start = std::time::Instant::now();
    match graph.invoke(state).await {
        Ok(result) => {
            let duration_ms = start.elapsed().as_millis() as u64;

            // Update session state
            {
                let mut session_state = session.state.lock().await;
                *session_state = result.final_state.clone();
            }

            // Emit completion events
            let _ = session
                .tx_event
                .send(Event::ReasoningComplete {
                    turn,
                    duration_ms,
                    has_tool_calls: !result.final_state.tool_results.is_empty(),
                })
                .await;

            let response = result.final_state.last_response.clone().unwrap_or_default();

            let _ = session
                .tx_event
                .send(Event::TurnComplete {
                    submission_id: sub_id.to_string(),
                    turn,
                    response,
                })
                .await;
        }
        Err(e) => {
            error!("Graph execution failed: {}", e);
            let _ = session
                .tx_event
                .send(Event::TurnAborted {
                    submission_id: sub_id.to_string(),
                    reason: AbortReason::Error {
                        message: e.to_string(),
                    },
                })
                .await;
        }
    }
}

async fn handle_exec_approval(session: &Session, id: &str, decision: ApprovalDecision) {
    info!(
        session_id = %session.session_id,
        request_id = %id,
        decision = %decision,
        "Processing exec approval"
    );

    // Look up and remove pending approval request
    let sender = {
        let mut pending = session.pending_approvals.lock().await;
        pending.remove(id)
    };

    let approved = matches!(
        decision,
        ApprovalDecision::Approve | ApprovalDecision::ApproveAndRemember
    );

    // If we have a pending sender, notify it
    if let Some(tx) = sender {
        if tx.send(decision).is_err() {
            warn!(
                session_id = %session.session_id,
                request_id = %id,
                "Pending approval receiver dropped"
            );
        }
    }

    // Send response event
    let _ = session
        .tx_event
        .send(Event::ExecApprovalResponse {
            id: id.to_string(),
            approved,
        })
        .await;
}

async fn handle_patch_approval(session: &Session, id: &str, decision: ApprovalDecision) {
    info!(
        session_id = %session.session_id,
        request_id = %id,
        decision = %decision,
        "Processing patch approval"
    );

    // Look up and remove pending approval request
    let sender = {
        let mut pending = session.pending_approvals.lock().await;
        pending.remove(id)
    };

    let approved = matches!(
        decision,
        ApprovalDecision::Approve | ApprovalDecision::ApproveAndRemember
    );

    // If we have a pending sender, notify it
    if let Some(tx) = sender {
        if tx.send(decision).is_err() {
            warn!(
                session_id = %session.session_id,
                request_id = %id,
                "Pending patch approval receiver dropped"
            );
        }
    }

    // Send response event
    let _ = session
        .tx_event
        .send(Event::PatchApprovalResponse {
            id: id.to_string(),
            approved,
        })
        .await;
}

async fn handle_add_to_history(session: &Session, text: String) {
    let mut state = session.state.lock().await;
    state.messages.push(crate::state::Message {
        role: MessageRole::System,
        content: text,
        tool_call_id: None,
        tool_calls: vec![],
    });
}

async fn handle_compact(session: &Session, sub_id: &str) {
    info!(session_id = %session.session_id, "Compacting conversation history");

    // Get current messages and estimate tokens
    let (messages, original_tokens) = {
        let state = session.state.lock().await;
        let tokens = estimate_history_tokens(&state.messages) as u64;
        (state.messages.clone(), tokens)
    };

    // Check if compaction is needed (threshold: 50k tokens)
    const COMPACTION_THRESHOLD: usize = 50_000;
    if !should_compact(&messages, COMPACTION_THRESHOLD) && messages.len() < 20 {
        info!(
            session_id = %session.session_id,
            tokens = original_tokens,
            "Compaction not needed, history is small enough"
        );
        let _ = session
            .tx_event
            .send(Event::Compacted {
                original_tokens,
                new_tokens: original_tokens,
            })
            .await;
        return;
    }

    // Extract initial context (system messages at the start)
    let mut initial_context: Vec<Message> = Vec::new();
    for msg in &messages {
        if msg.role == MessageRole::System {
            initial_context.push(msg.clone());
        } else {
            break;
        }
    }

    // Collect user messages for the summary
    let user_messages = collect_user_messages(&messages);

    // Create compaction prompt
    let compaction_prompt = create_compaction_prompt(&messages, None);

    // Call LLM to generate summary
    let summary = match generate_summary(session, &compaction_prompt).await {
        Ok(s) => s,
        Err(e) => {
            warn!(
                session_id = %session.session_id,
                error = %e,
                "Failed to generate summary, using fallback"
            );
            // Fallback: use last assistant message as summary
            messages
                .iter()
                .rev()
                .find(|m| m.role == MessageRole::Assistant)
                .map(|m| m.content.clone())
                .unwrap_or_else(|| "(conversation summary unavailable)".to_string())
        }
    };

    // Build compacted history
    let config = CompactConfig::default();
    let compacted = build_compacted_history_with_limit(
        initial_context,
        &user_messages,
        &summary,
        config.user_message_max_tokens,
    );

    let new_tokens = estimate_history_tokens(&compacted) as u64;

    // Update session state with compacted history
    {
        let mut state = session.state.lock().await;
        state.messages = compacted;
    }

    info!(
        session_id = %session.session_id,
        original_tokens,
        new_tokens,
        savings = original_tokens.saturating_sub(new_tokens),
        "Conversation compacted"
    );

    let _ = session
        .tx_event
        .send(Event::Compacted {
            original_tokens,
            new_tokens,
        })
        .await;

    let _ = sub_id;
}

/// Generate a summary using the LLM
async fn generate_summary(session: &Session, prompt: &str) -> Result<String> {
    // Get model from config or use default
    let model = &session.config.model;

    let llm_config = LlmConfig::with_model(model);
    let client = LlmClient::with_config(llm_config).with_temperature(0.3);

    // Create a simple message for summarization
    let messages = vec![crate::state::Message {
        role: MessageRole::User,
        content: prompt.to_string(),
        tool_call_id: None,
        tool_calls: vec![],
    }];

    let response = client.generate(&messages, Some(&[])).await?;

    response.content.ok_or_else(|| {
        crate::error::Error::LlmApi("LLM returned no content for summary".to_string())
    })
}

async fn handle_undo(session: &Session, sub_id: &str) {
    info!(session_id = %session.session_id, "Undo requested");
    let _ = session.tx_event.send(Event::UndoStarted).await;

    // Get the working directory
    let cwd = {
        let settings = session.turn_settings.lock().await;
        settings.cwd.clone()
    };

    // Pop the most recent ghost commit
    let ghost_commit = {
        let mut commits = session.ghost_commits.lock().await;
        commits.pop()
    };

    match ghost_commit {
        Some(commit) => {
            // Restore to the ghost commit
            match restore_ghost_commit(&cwd, &commit) {
                Ok(()) => {
                    info!(
                        session_id = %session.session_id,
                        commit_id = %commit.id(),
                        "Successfully restored to ghost commit"
                    );
                    let _ = session
                        .tx_event
                        .send(Event::UndoComplete {
                            description: format!("Restored to snapshot {}", &commit.id()[..8]),
                        })
                        .await;
                }
                Err(e) => {
                    warn!(
                        session_id = %session.session_id,
                        error = %e,
                        "Failed to restore ghost commit"
                    );
                    let _ = session
                        .tx_event
                        .send(Event::UndoComplete {
                            description: format!("Undo failed: {}", e),
                        })
                        .await;
                }
            }
        }
        None => {
            info!(
                session_id = %session.session_id,
                "No ghost commits available for undo"
            );
            let _ = session
                .tx_event
                .send(Event::UndoComplete {
                    description: "No previous state to restore".to_string(),
                })
                .await;
        }
    }

    let _ = sub_id;
}

/// Create a ghost commit before agent operations for undo support
async fn capture_ghost_commit(session: &Session) {
    let cwd = {
        let settings = session.turn_settings.lock().await;
        settings.cwd.clone()
    };

    let options = CreateGhostCommitOptions::new(&cwd);

    match create_ghost_commit(&options) {
        Ok(commit) => {
            let mut commits = session.ghost_commits.lock().await;
            // Limit the stack to prevent unbounded growth
            const MAX_GHOST_COMMITS: usize = 10;
            if commits.len() >= MAX_GHOST_COMMITS {
                commits.remove(0);
            }
            debug!(
                session_id = %session.session_id,
                commit_id = %commit.id(),
                "Captured ghost commit for undo"
            );
            commits.push(commit);
        }
        Err(e) => {
            // Non-fatal: just log and continue
            debug!(
                session_id = %session.session_id,
                error = %e,
                "Failed to capture ghost commit (undo will be unavailable)"
            );
        }
    }
}

async fn handle_review(session: &Session, sub_id: &str, request: ReviewRequest) {
    use crate::review::{
        generate_review_prompt, review_target_hint, ReviewOutputEvent, ReviewTarget,
    };

    info!(
        session_id = %session.session_id,
        review_type = ?request.review_type,
        "Starting code review"
    );

    // Determine review target from request
    let target = if request.files.is_empty() {
        ReviewTarget::UncommittedChanges
    } else {
        // Use custom review with file list
        let files_list = request
            .files
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        ReviewTarget::Custom {
            instructions: format!("Review the following files: {}", files_list),
        }
    };

    // Generate the review prompt
    let prompt = match generate_review_prompt(&target, None) {
        Ok(p) => p,
        Err(e) => {
            warn!(
                session_id = %session.session_id,
                error = %e,
                "Failed to generate review prompt"
            );
            let _ = session
                .tx_event
                .send(Event::ReviewComplete {
                    output: ReviewOutputEvent {
                        findings: vec![],
                        overall_correctness: "error".to_string(),
                        overall_explanation: format!("Failed to generate review prompt: {}", e),
                        overall_confidence_score: 0.0,
                    },
                })
                .await;
            return;
        }
    };

    // Add focus areas if provided
    let full_prompt = if !request.focus.is_empty() {
        format!("{}\n\nFocus areas: {}", prompt, request.focus.join(", "))
    } else {
        prompt
    };

    // Get the model and create LLM client
    let model = &session.config.model;
    let llm_config = LlmConfig::with_model(model);
    let client = LlmClient::with_config(llm_config).with_temperature(0.2);

    // Create a review request message
    let messages = vec![crate::state::Message {
        role: MessageRole::User,
        content: format!(
            "You are a code reviewer. {}\n\n\
            Respond with a JSON object containing:\n\
            - \"findings\": array of findings with title, body, confidence_score, priority, and code_location\n\
            - \"overall_correctness\": one of \"correct\", \"needs-work\", \"incorrect\"\n\
            - \"overall_explanation\": brief explanation of your assessment\n\
            - \"overall_confidence_score\": 0.0 to 1.0",
            full_prompt
        ),
        tool_call_id: None,
        tool_calls: vec![],
    }];

    // Call LLM for review
    let review_output = match client.generate(&messages, Some(&[])).await {
        Ok(response) => {
            if let Some(content) = response.content {
                // Try to parse JSON response
                match serde_json::from_str::<ReviewOutputEvent>(&content) {
                    Ok(output) => output,
                    Err(_) => {
                        // Fallback: use content as explanation
                        ReviewOutputEvent {
                            findings: vec![],
                            overall_correctness: "reviewed".to_string(),
                            overall_explanation: content,
                            overall_confidence_score: 0.7,
                        }
                    }
                }
            } else {
                ReviewOutputEvent {
                    findings: vec![],
                    overall_correctness: "error".to_string(),
                    overall_explanation: "LLM returned no content".to_string(),
                    overall_confidence_score: 0.0,
                }
            }
        }
        Err(e) => {
            warn!(
                session_id = %session.session_id,
                error = %e,
                "Failed to get review from LLM"
            );
            ReviewOutputEvent {
                findings: vec![],
                overall_correctness: "error".to_string(),
                overall_explanation: format!("Review failed: {}", e),
                overall_confidence_score: 0.0,
            }
        }
    };

    info!(
        session_id = %session.session_id,
        findings_count = review_output.findings.len(),
        overall = %review_output.overall_correctness,
        hint = %review_target_hint(&target),
        "Code review complete"
    );

    let _ = session
        .tx_event
        .send(Event::ReviewComplete {
            output: review_output,
        })
        .await;

    let _ = sub_id;
}

async fn handle_shutdown(session: &Session, _sub_id: &str) -> bool {
    info!(session_id = %session.session_id, "Shutdown requested");
    *session.shutdown_requested.lock().await = true;
    let _ = session.tx_event.send(Event::ShutdownComplete).await;
    true
}

async fn handle_list_models(session: &Session, _sub_id: &str) {
    // Return a basic list of models
    let models = vec![
        ModelInfo {
            id: "gpt-4".to_string(),
            name: "GPT-4".to_string(),
            provider: "openai".to_string(),
            supports_reasoning: true,
            max_context: Some(8192),
        },
        ModelInfo {
            id: "gpt-4-turbo".to_string(),
            name: "GPT-4 Turbo".to_string(),
            provider: "openai".to_string(),
            supports_reasoning: true,
            max_context: Some(128000),
        },
        ModelInfo {
            id: "claude-3-opus".to_string(),
            name: "Claude 3 Opus".to_string(),
            provider: "anthropic".to_string(),
            supports_reasoning: true,
            max_context: Some(200000),
        },
    ];
    let _ = session
        .tx_event
        .send(Event::ModelsAvailable { models })
        .await;
}

async fn handle_list_mcp_tools(session: &Session, _sub_id: &str) {
    info!(session_id = %session.session_id, "Listing MCP tools");

    // Get or initialize MCP client
    let mcp_client = {
        let mut client_guard = session.mcp_client.lock().await;
        if client_guard.is_none() {
            *client_guard = Some(Arc::new(codex_dashflow_mcp::McpClient::new()));
        }
        client_guard.as_ref().cloned().unwrap()
    };

    // Get tools from all connected servers
    let mcp_tools = mcp_client.list_tools().await;

    // Convert to McpToolInfo
    let tools: Vec<McpToolInfo> = mcp_tools
        .into_iter()
        .map(|t| McpToolInfo {
            qualified_name: t.qualified_name,
            name: t.name,
            server: t.server,
            description: t.description,
        })
        .collect();

    info!(
        session_id = %session.session_id,
        tool_count = tools.len(),
        "MCP tools listed"
    );

    let _ = session
        .tx_event
        .send(Event::McpToolsAvailable { tools })
        .await;
}

async fn handle_run_user_shell_command(session: &Session, _sub_id: &str, command: String) {
    use std::process::Stdio;
    use tokio::process::Command;

    info!(
        session_id = %session.session_id,
        command = %command,
        "Executing user shell command"
    );

    // Get current working directory from turn settings
    let cwd = {
        let settings = session.turn_settings.lock().await;
        settings.cwd.clone()
    };

    // Execute the command
    let result = Command::new("sh")
        .arg("-c")
        .arg(&command)
        .current_dir(&cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    let (exit_code, stdout, stderr) = match result {
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            info!(
                session_id = %session.session_id,
                exit_code = exit_code,
                stdout_len = stdout.len(),
                stderr_len = stderr.len(),
                "Shell command completed"
            );

            (exit_code, stdout, stderr)
        }
        Err(e) => {
            warn!(
                session_id = %session.session_id,
                error = %e,
                "Failed to execute shell command"
            );
            (-1, String::new(), format!("Failed to execute: {}", e))
        }
    };

    let _ = session
        .tx_event
        .send(Event::ShellCommandComplete {
            command,
            exit_code,
            stdout,
            stderr,
        })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::NullStreamCallback;
    use std::path::Path;

    #[tokio::test]
    async fn test_codex_spawn() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let result = Codex::spawn(config, callback).await;
        assert!(result.is_ok());
        let spawn_ok = result.unwrap();
        assert!(!spawn_ok.session_id.is_empty());
    }

    #[tokio::test]
    async fn test_codex_submit_shutdown() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // First event should be SessionConfigured
        let event = codex.next_event().await.unwrap();
        assert!(matches!(event, Event::SessionConfigured { .. }));

        // Submit shutdown
        let id = codex.submit(Op::Shutdown).await.unwrap();
        assert!(!id.is_empty());

        // Should get ShutdownComplete
        let event = codex.next_event().await.unwrap();
        assert!(matches!(event, Event::ShutdownComplete));
    }

    #[tokio::test]
    async fn test_codex_list_models() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Skip SessionConfigured
        let _ = codex.next_event().await.unwrap();

        // List models
        codex.submit(Op::ListModels).await.unwrap();

        let event = codex.next_event().await.unwrap();
        if let Event::ModelsAvailable { models } = event {
            assert!(!models.is_empty());
            assert!(models.iter().any(|m| m.id == "gpt-4"));
        } else {
            panic!("Expected ModelsAvailable event");
        }
    }

    #[test]
    fn test_op_serialization() {
        let op = Op::UserInput {
            message: "Hello".to_string(),
            context: vec![],
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("UserInput"));
        assert!(json.contains("Hello"));

        let op2: Op = serde_json::from_str(&json).unwrap();
        if let Op::UserInput { message, .. } = op2 {
            assert_eq!(message, "Hello");
        } else {
            panic!("Deserialization failed");
        }
    }

    #[test]
    fn test_event_serialization() {
        let event = Event::TurnComplete {
            submission_id: "123".to_string(),
            turn: 1,
            response: "Done".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("TurnComplete"));
        assert!(json.contains("Done"));
    }

    #[test]
    fn test_approval_policy_default() {
        assert_eq!(ApprovalPolicy::default(), ApprovalPolicy::OnUnknown);
    }

    #[test]
    fn test_sandbox_policy_default() {
        assert_eq!(SandboxPolicy::default(), SandboxPolicy::Native);
    }

    #[test]
    fn test_approval_policy_display() {
        assert_eq!(ApprovalPolicy::Never.to_string(), "never");
        assert_eq!(ApprovalPolicy::OnUnknown.to_string(), "on-unknown");
        assert_eq!(ApprovalPolicy::Always.to_string(), "always");
    }

    #[test]
    fn test_sandbox_policy_display() {
        assert_eq!(SandboxPolicy::None.to_string(), "none");
        assert_eq!(SandboxPolicy::Native.to_string(), "native");
        assert_eq!(SandboxPolicy::Docker { image: None }.to_string(), "docker");
        assert_eq!(
            SandboxPolicy::Docker {
                image: Some("ubuntu:22.04".to_string())
            }
            .to_string(),
            "docker:ubuntu:22.04"
        );
    }

    #[test]
    fn test_approval_decision_display() {
        assert_eq!(ApprovalDecision::Approve.to_string(), "approve");
        assert_eq!(ApprovalDecision::Deny.to_string(), "deny");
        assert_eq!(
            ApprovalDecision::ApproveAndRemember.to_string(),
            "approve-remember"
        );
        assert_eq!(
            ApprovalDecision::DenyAndRemember.to_string(),
            "deny-remember"
        );
    }

    #[tokio::test]
    async fn test_list_mcp_tools() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Skip SessionConfigured
        let _ = codex.next_event().await.unwrap();

        // List MCP tools
        codex.submit(Op::ListMcpTools).await.unwrap();

        let event = codex.next_event().await.unwrap();
        // Should get McpToolsAvailable (empty since no servers connected)
        assert!(matches!(event, Event::McpToolsAvailable { .. }));
    }

    #[tokio::test]
    async fn test_run_user_shell_command() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Skip SessionConfigured
        let _ = codex.next_event().await.unwrap();

        // Run a simple shell command
        codex
            .submit(Op::RunUserShellCommand {
                command: "echo hello".to_string(),
            })
            .await
            .unwrap();

        let event = codex.next_event().await.unwrap();
        if let Event::ShellCommandComplete {
            command,
            exit_code,
            stdout,
            ..
        } = event
        {
            assert_eq!(command, "echo hello");
            assert_eq!(exit_code, 0);
            assert!(stdout.contains("hello"));
        } else {
            panic!("Expected ShellCommandComplete event, got {:?}", event);
        }
    }

    #[test]
    fn test_mcp_tool_info_serialization() {
        let tool = McpToolInfo {
            qualified_name: "mcp__test__echo".to_string(),
            name: "echo".to_string(),
            server: "test".to_string(),
            description: Some("Echo tool".to_string()),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("mcp__test__echo"));
        let parsed: McpToolInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.qualified_name, "mcp__test__echo");
    }

    #[test]
    fn test_new_event_variants_serialization() {
        // Test ExecApprovalResponse
        let event = Event::ExecApprovalResponse {
            id: "req1".to_string(),
            approved: true,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ExecApprovalResponse"));
        assert!(json.contains("approved"));

        // Test PatchApprovalResponse
        let event = Event::PatchApprovalResponse {
            id: "req2".to_string(),
            approved: false,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("PatchApprovalResponse"));

        // Test ShellCommandComplete
        let event = Event::ShellCommandComplete {
            command: "ls".to_string(),
            exit_code: 0,
            stdout: "file.txt".to_string(),
            stderr: String::new(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ShellCommandComplete"));
        assert!(json.contains("exit_code"));

        // Test McpToolsAvailable
        let event = Event::McpToolsAvailable { tools: vec![] };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("McpToolsAvailable"));
    }

    #[tokio::test]
    async fn test_exec_approval_response() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Skip SessionConfigured
        let _ = codex.next_event().await.unwrap();

        // Submit an exec approval (without a pending request, it should still respond)
        codex
            .submit(Op::ExecApproval {
                id: "test-req".to_string(),
                decision: ApprovalDecision::Approve,
            })
            .await
            .unwrap();

        let event = codex.next_event().await.unwrap();
        if let Event::ExecApprovalResponse { id, approved } = event {
            assert_eq!(id, "test-req");
            assert!(approved);
        } else {
            panic!("Expected ExecApprovalResponse event, got {:?}", event);
        }
    }

    #[tokio::test]
    async fn test_patch_approval_response() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Skip SessionConfigured
        let _ = codex.next_event().await.unwrap();

        // Submit a patch approval with deny decision
        codex
            .submit(Op::PatchApproval {
                id: "patch-req".to_string(),
                decision: ApprovalDecision::Deny,
            })
            .await
            .unwrap();

        let event = codex.next_event().await.unwrap();
        if let Event::PatchApprovalResponse { id, approved } = event {
            assert_eq!(id, "patch-req");
            assert!(!approved);
        } else {
            panic!("Expected PatchApprovalResponse event, got {:?}", event);
        }
    }

    #[test]
    fn test_context_item_file_serialization() {
        let item = ContextItem::File {
            path: PathBuf::from("/path/to/file.txt"),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("File"));
        assert!(json.contains("/path/to/file.txt"));
        let parsed: ContextItem = serde_json::from_str(&json).unwrap();
        if let ContextItem::File { path } = parsed {
            assert_eq!(path, PathBuf::from("/path/to/file.txt"));
        } else {
            panic!("Expected File variant");
        }
    }

    #[test]
    fn test_context_item_url_serialization() {
        let item = ContextItem::Url {
            url: "https://example.com".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("Url"));
        assert!(json.contains("https://example.com"));
        let parsed: ContextItem = serde_json::from_str(&json).unwrap();
        if let ContextItem::Url { url } = parsed {
            assert_eq!(url, "https://example.com");
        } else {
            panic!("Expected Url variant");
        }
    }

    #[test]
    fn test_context_item_text_serialization() {
        let item = ContextItem::Text {
            content: "Some context text".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("Text"));
        assert!(json.contains("Some context text"));
        let parsed: ContextItem = serde_json::from_str(&json).unwrap();
        if let ContextItem::Text { content } = parsed {
            assert_eq!(content, "Some context text");
        } else {
            panic!("Expected Text variant");
        }
    }

    #[test]
    fn test_review_request_serialization() {
        let request = ReviewRequest {
            review_type: ReviewType::Security,
            files: vec![PathBuf::from("src/main.rs")],
            focus: vec!["injection".to_string()],
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("security"));
        assert!(json.contains("src/main.rs"));
        let parsed: ReviewRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.review_type, ReviewType::Security);
        assert_eq!(parsed.files.len(), 1);
    }

    #[test]
    fn test_review_type_serialization() {
        // Test all variants
        let types = [
            (ReviewType::Full, "full"),
            (ReviewType::Security, "security"),
            (ReviewType::Performance, "performance"),
            (ReviewType::Style, "style"),
        ];
        for (review_type, expected_str) in types {
            let json = serde_json::to_string(&review_type).unwrap();
            assert!(json.contains(expected_str));
            let parsed: ReviewType = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, review_type);
        }
    }

    #[test]
    fn test_review_type_default() {
        assert_eq!(ReviewType::default(), ReviewType::Full);
    }

    #[test]
    fn test_risk_level_serialization() {
        let levels = [
            (RiskLevel::Safe, "safe"),
            (RiskLevel::Low, "low"),
            (RiskLevel::Medium, "medium"),
            (RiskLevel::High, "high"),
            (RiskLevel::Critical, "critical"),
        ];
        for (level, expected_str) in levels {
            let json = serde_json::to_string(&level).unwrap();
            assert!(json.contains(expected_str));
            let parsed: RiskLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, level);
        }
    }

    #[test]
    fn test_abort_reason_serialization() {
        // UserInterrupt
        let reason = AbortReason::UserInterrupt;
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("user_interrupt"));

        // TurnLimit
        let reason = AbortReason::TurnLimit;
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("turn_limit"));

        // ApprovalDenied
        let reason = AbortReason::ApprovalDenied;
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("approval_denied"));

        // Shutdown
        let reason = AbortReason::Shutdown;
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("shutdown"));

        // Error with message
        let reason = AbortReason::Error {
            message: "Test error".to_string(),
        };
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("error"));
        assert!(json.contains("Test error"));
    }

    #[test]
    fn test_command_assessment_serialization() {
        let assessment = CommandAssessment {
            risk: RiskLevel::Medium,
            reason: "Modifies system files".to_string(),
            known_safe: false,
        };
        let json = serde_json::to_string(&assessment).unwrap();
        assert!(json.contains("medium"));
        assert!(json.contains("Modifies system files"));
        assert!(json.contains("false"));
        let parsed: CommandAssessment = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.risk, RiskLevel::Medium);
        assert!(!parsed.known_safe);
    }

    #[test]
    fn test_model_info_serialization() {
        let info = ModelInfo {
            id: "gpt-4-turbo".to_string(),
            name: "GPT-4 Turbo".to_string(),
            provider: "openai".to_string(),
            supports_reasoning: true,
            max_context: Some(128000),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("gpt-4-turbo"));
        assert!(json.contains("128000"));
        let parsed: ModelInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "gpt-4-turbo");
        assert!(parsed.supports_reasoning);
        assert_eq!(parsed.max_context, Some(128000));
    }

    #[test]
    fn test_model_info_no_max_context() {
        let info = ModelInfo {
            id: "custom-model".to_string(),
            name: "Custom Model".to_string(),
            provider: "local".to_string(),
            supports_reasoning: false,
            max_context: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("custom-model"));
        let parsed: ModelInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_context, None);
        assert!(!parsed.supports_reasoning);
    }

    #[test]
    fn test_turn_settings_default() {
        let settings = TurnSettings::default();
        assert_eq!(settings.approval_policy, ApprovalPolicy::OnUnknown);
        assert_eq!(settings.sandbox_policy, SandboxPolicy::Native);
        assert!(settings.model.is_none());
        // cwd should be either current directory or "."
        assert!(settings.cwd.exists() || settings.cwd == Path::new("."));
    }

    #[test]
    fn test_op_user_turn_serialization() {
        let op = Op::UserTurn {
            message: "Hello".to_string(),
            cwd: Some(PathBuf::from("/tmp")),
            approval_policy: ApprovalPolicy::Always,
            sandbox_policy: SandboxPolicy::Docker {
                image: Some("ubuntu:22.04".to_string()),
            },
            model: Some("gpt-4".to_string()),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("UserTurn"));
        assert!(json.contains("Hello"));
        assert!(json.contains("/tmp"));
        assert!(json.contains("always"));
        assert!(json.contains("gpt-4"));
    }

    #[test]
    fn test_op_override_turn_context_serialization() {
        let op = Op::OverrideTurnContext {
            cwd: Some(PathBuf::from("/home/user")),
            approval_policy: Some(ApprovalPolicy::Never),
            sandbox_policy: None,
            model: None,
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("OverrideTurnContext"));
        assert!(json.contains("/home/user"));
        assert!(json.contains("never"));
    }

    #[test]
    fn test_op_review_serialization() {
        let op = Op::Review {
            request: ReviewRequest {
                review_type: ReviewType::Performance,
                files: vec![],
                focus: vec!["latency".to_string()],
            },
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("Review"));
        assert!(json.contains("performance"));
        assert!(json.contains("latency"));
    }

    #[test]
    fn test_event_turn_aborted_serialization() {
        let event = Event::TurnAborted {
            submission_id: "sub123".to_string(),
            reason: AbortReason::TurnLimit,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("TurnAborted"));
        assert!(json.contains("turn_limit"));
    }

    #[test]
    fn test_event_exec_approval_request_serialization() {
        let event = Event::ExecApprovalRequest {
            id: "req1".to_string(),
            command: "rm -rf /tmp/*".to_string(),
            assessment: CommandAssessment {
                risk: RiskLevel::High,
                reason: "Recursive deletion".to_string(),
                known_safe: false,
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ExecApprovalRequest"));
        assert!(json.contains("rm -rf"));
        assert!(json.contains("high"));
    }

    #[test]
    fn test_event_patch_approval_request_serialization() {
        let event = Event::PatchApprovalRequest {
            id: "patch1".to_string(),
            file: PathBuf::from("src/main.rs"),
            patch: "@@ -1,3 +1,4 @@\n+new line\n context".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("PatchApprovalRequest"));
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("new line"));
    }

    #[test]
    fn test_event_token_usage_serialization() {
        let event = Event::TokenUsage {
            input_tokens: 1500,
            output_tokens: 500,
            cost_usd: Some(0.025),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("TokenUsage"));
        assert!(json.contains("1500"));
        assert!(json.contains("500"));
        assert!(json.contains("0.025"));
    }

    #[test]
    fn test_event_compacted_serialization() {
        let event = Event::Compacted {
            original_tokens: 50000,
            new_tokens: 15000,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Compacted"));
        assert!(json.contains("50000"));
        assert!(json.contains("15000"));
    }

    // Additional tests for expanded coverage

    #[test]
    fn test_op_debug() {
        let op = Op::Interrupt;
        let debug_str = format!("{:?}", op);
        assert!(debug_str.contains("Interrupt"));

        let op = Op::UserInput {
            message: "test".to_string(),
            context: vec![],
        };
        let debug_str = format!("{:?}", op);
        assert!(debug_str.contains("UserInput"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_op_clone() {
        let op = Op::UserInput {
            message: "test".to_string(),
            context: vec![ContextItem::Text {
                content: "ctx".to_string(),
            }],
        };
        let cloned = op.clone();
        if let Op::UserInput { message, context } = cloned {
            assert_eq!(message, "test");
            assert_eq!(context.len(), 1);
        } else {
            panic!("Clone failed");
        }
    }

    #[test]
    fn test_op_interrupt_serialization() {
        let op = Op::Interrupt;
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("Interrupt"));
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::Interrupt));
    }

    #[test]
    fn test_op_add_to_history_serialization() {
        let op = Op::AddToHistory {
            text: "Some history".to_string(),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("AddToHistory"));
        assert!(json.contains("Some history"));
        let parsed: Op = serde_json::from_str(&json).unwrap();
        if let Op::AddToHistory { text } = parsed {
            assert_eq!(text, "Some history");
        } else {
            panic!("Expected AddToHistory");
        }
    }

    #[test]
    fn test_op_compact_serialization() {
        let op = Op::Compact;
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("Compact"));
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::Compact));
    }

    #[test]
    fn test_op_undo_serialization() {
        let op = Op::Undo;
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("Undo"));
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::Undo));
    }

    #[test]
    fn test_op_shutdown_serialization() {
        let op = Op::Shutdown;
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("Shutdown"));
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::Shutdown));
    }

    #[test]
    fn test_op_list_models_serialization() {
        let op = Op::ListModels;
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("ListModels"));
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::ListModels));
    }

    #[test]
    fn test_op_list_mcp_tools_serialization() {
        let op = Op::ListMcpTools;
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("ListMcpTools"));
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::ListMcpTools));
    }

    #[test]
    fn test_op_run_user_shell_command_serialization() {
        let op = Op::RunUserShellCommand {
            command: "ls -la".to_string(),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("RunUserShellCommand"));
        assert!(json.contains("ls -la"));
        let parsed: Op = serde_json::from_str(&json).unwrap();
        if let Op::RunUserShellCommand { command } = parsed {
            assert_eq!(command, "ls -la");
        } else {
            panic!("Expected RunUserShellCommand");
        }
    }

    #[test]
    fn test_op_exec_approval_serialization() {
        let op = Op::ExecApproval {
            id: "req123".to_string(),
            decision: ApprovalDecision::ApproveAndRemember,
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("ExecApproval"));
        assert!(json.contains("req123"));
        assert!(json.contains("approve_and_remember"));
        let parsed: Op = serde_json::from_str(&json).unwrap();
        if let Op::ExecApproval { id, decision } = parsed {
            assert_eq!(id, "req123");
            assert_eq!(decision, ApprovalDecision::ApproveAndRemember);
        } else {
            panic!("Expected ExecApproval");
        }
    }

    #[test]
    fn test_op_patch_approval_serialization() {
        let op = Op::PatchApproval {
            id: "patch456".to_string(),
            decision: ApprovalDecision::DenyAndRemember,
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("PatchApproval"));
        assert!(json.contains("patch456"));
        assert!(json.contains("deny_and_remember"));
        let parsed: Op = serde_json::from_str(&json).unwrap();
        if let Op::PatchApproval { id, decision } = parsed {
            assert_eq!(id, "patch456");
            assert_eq!(decision, ApprovalDecision::DenyAndRemember);
        } else {
            panic!("Expected PatchApproval");
        }
    }

    #[test]
    fn test_context_item_debug() {
        let item = ContextItem::File {
            path: PathBuf::from("/test"),
        };
        let debug_str = format!("{:?}", item);
        assert!(debug_str.contains("File"));
        assert!(debug_str.contains("/test"));
    }

    #[test]
    fn test_context_item_clone() {
        let item = ContextItem::Url {
            url: "https://test.com".to_string(),
        };
        let cloned = item.clone();
        if let ContextItem::Url { url } = cloned {
            assert_eq!(url, "https://test.com");
        } else {
            panic!("Clone failed");
        }
    }

    #[test]
    fn test_approval_policy_debug() {
        let policy = ApprovalPolicy::OnUnknown;
        let debug_str = format!("{:?}", policy);
        assert!(debug_str.contains("OnUnknown"));
    }

    #[test]
    fn test_approval_policy_clone() {
        let policy = ApprovalPolicy::Always;
        // Use Copy trait (Clone is auto-derived for Copy types)
        let cloned = policy;
        assert_eq!(cloned, ApprovalPolicy::Always);
        assert_eq!(policy, ApprovalPolicy::Always); // Original still valid
    }

    #[test]
    fn test_approval_policy_copy() {
        let policy = ApprovalPolicy::Never;
        let copied: ApprovalPolicy = policy;
        assert_eq!(copied, ApprovalPolicy::Never);
        // policy is still valid due to Copy
        assert_eq!(policy, ApprovalPolicy::Never);
    }

    #[test]
    fn test_sandbox_policy_debug() {
        let policy = SandboxPolicy::Native;
        let debug_str = format!("{:?}", policy);
        assert!(debug_str.contains("Native"));

        let policy = SandboxPolicy::Docker {
            image: Some("alpine".to_string()),
        };
        let debug_str = format!("{:?}", policy);
        assert!(debug_str.contains("Docker"));
        assert!(debug_str.contains("alpine"));
    }

    #[test]
    fn test_sandbox_policy_clone() {
        let policy = SandboxPolicy::Docker {
            image: Some("ubuntu".to_string()),
        };
        let cloned = policy.clone();
        if let SandboxPolicy::Docker { image } = cloned {
            assert_eq!(image, Some("ubuntu".to_string()));
        } else {
            panic!("Clone failed");
        }
    }

    #[test]
    fn test_sandbox_policy_serialization() {
        let policy = SandboxPolicy::None;
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("none"));
        let parsed: SandboxPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, SandboxPolicy::None);

        let policy = SandboxPolicy::Native;
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("native"));
        let parsed: SandboxPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, SandboxPolicy::Native);

        let policy = SandboxPolicy::Docker { image: None };
        let json = serde_json::to_string(&policy).unwrap();
        let parsed: SandboxPolicy = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, SandboxPolicy::Docker { image: None }));

        let policy = SandboxPolicy::Docker {
            image: Some("rust:latest".to_string()),
        };
        let json = serde_json::to_string(&policy).unwrap();
        let parsed: SandboxPolicy = serde_json::from_str(&json).unwrap();
        if let SandboxPolicy::Docker { image } = parsed {
            assert_eq!(image, Some("rust:latest".to_string()));
        } else {
            panic!("Expected Docker");
        }
    }

    #[test]
    fn test_approval_decision_debug() {
        let decision = ApprovalDecision::Approve;
        let debug_str = format!("{:?}", decision);
        assert!(debug_str.contains("Approve"));
    }

    #[test]
    fn test_approval_decision_clone() {
        let decision = ApprovalDecision::Deny;
        // Use Copy trait (Clone is auto-derived for Copy types)
        let cloned = decision;
        assert_eq!(cloned, ApprovalDecision::Deny);
        assert_eq!(decision, ApprovalDecision::Deny); // Original still valid
    }

    #[test]
    fn test_approval_decision_copy() {
        let decision = ApprovalDecision::ApproveAndRemember;
        let copied: ApprovalDecision = decision;
        assert_eq!(copied, ApprovalDecision::ApproveAndRemember);
        assert_eq!(decision, ApprovalDecision::ApproveAndRemember);
    }

    #[test]
    fn test_approval_decision_serialization() {
        let decisions = [
            (ApprovalDecision::Approve, "approve"),
            (ApprovalDecision::Deny, "deny"),
            (ApprovalDecision::ApproveAndRemember, "approve_and_remember"),
            (ApprovalDecision::DenyAndRemember, "deny_and_remember"),
        ];
        for (decision, expected) in decisions {
            let json = serde_json::to_string(&decision).unwrap();
            assert!(json.contains(expected));
            let parsed: ApprovalDecision = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, decision);
        }
    }

    #[test]
    fn test_review_request_debug() {
        let request = ReviewRequest {
            review_type: ReviewType::Full,
            files: vec![],
            focus: vec![],
        };
        let debug_str = format!("{:?}", request);
        assert!(debug_str.contains("ReviewRequest"));
        assert!(debug_str.contains("Full"));
    }

    #[test]
    fn test_review_request_clone() {
        let request = ReviewRequest {
            review_type: ReviewType::Security,
            files: vec![PathBuf::from("test.rs")],
            focus: vec!["injection".to_string()],
        };
        let cloned = request.clone();
        assert_eq!(cloned.review_type, ReviewType::Security);
        assert_eq!(cloned.files.len(), 1);
        assert_eq!(cloned.focus.len(), 1);
    }

    #[test]
    fn test_review_request_default_fields() {
        // Test default fields via serde
        let json = r#"{"review_type":"full"}"#;
        let request: ReviewRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.review_type, ReviewType::Full);
        assert!(request.files.is_empty());
        assert!(request.focus.is_empty());
    }

    #[test]
    fn test_review_type_debug() {
        for review_type in [
            ReviewType::Full,
            ReviewType::Security,
            ReviewType::Performance,
            ReviewType::Style,
        ] {
            let debug_str = format!("{:?}", review_type);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_review_type_clone() {
        let original = ReviewType::Performance;
        let cloned = original.clone();
        assert_eq!(cloned, ReviewType::Performance);
    }

    #[test]
    fn test_submission_debug() {
        let sub = Submission {
            id: "sub123".to_string(),
            op: Op::Interrupt,
        };
        let debug_str = format!("{:?}", sub);
        assert!(debug_str.contains("Submission"));
        assert!(debug_str.contains("sub123"));
        assert!(debug_str.contains("Interrupt"));
    }

    #[test]
    fn test_submission_clone() {
        let sub = Submission {
            id: "sub456".to_string(),
            op: Op::Shutdown,
        };
        let cloned = sub.clone();
        assert_eq!(cloned.id, "sub456");
        assert!(matches!(cloned.op, Op::Shutdown));
    }

    #[test]
    fn test_event_debug() {
        let event = Event::ShutdownComplete;
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("ShutdownComplete"));

        let event = Event::Error {
            submission_id: Some("sub1".to_string()),
            message: "test error".to_string(),
            recoverable: true,
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Error"));
        assert!(debug_str.contains("test error"));
    }

    #[test]
    fn test_event_clone() {
        let event = Event::TurnStarted {
            submission_id: "sub1".to_string(),
            turn: 5,
        };
        let cloned = event.clone();
        if let Event::TurnStarted {
            submission_id,
            turn,
        } = cloned
        {
            assert_eq!(submission_id, "sub1");
            assert_eq!(turn, 5);
        } else {
            panic!("Clone failed");
        }
    }

    #[test]
    fn test_event_session_configured_serialization() {
        let event = Event::SessionConfigured {
            session_id: "sess123".to_string(),
            model: "gpt-4".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("SessionConfigured"));
        assert!(json.contains("sess123"));
        assert!(json.contains("gpt-4"));
    }

    #[test]
    fn test_event_turn_started_serialization() {
        let event = Event::TurnStarted {
            submission_id: "sub1".to_string(),
            turn: 3,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("TurnStarted"));
        assert!(json.contains("sub1"));
        assert!(json.contains("3"));
    }

    #[test]
    fn test_event_reasoning_started_serialization() {
        let event = Event::ReasoningStarted { turn: 2 };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ReasoningStarted"));
        assert!(json.contains("2"));
    }

    #[test]
    fn test_event_reasoning_delta_serialization() {
        let event = Event::ReasoningDelta {
            content: "thinking...".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ReasoningDelta"));
        assert!(json.contains("thinking..."));
    }

    #[test]
    fn test_event_reasoning_complete_serialization() {
        let event = Event::ReasoningComplete {
            turn: 1,
            duration_ms: 1500,
            has_tool_calls: true,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ReasoningComplete"));
        assert!(json.contains("1500"));
        assert!(json.contains("true"));
    }

    #[test]
    fn test_event_tool_started_serialization() {
        let event = Event::ToolStarted {
            tool: "shell".to_string(),
            call_id: "call123".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ToolStarted"));
        assert!(json.contains("shell"));
        assert!(json.contains("call123"));
    }

    #[test]
    fn test_event_tool_complete_serialization() {
        let event = Event::ToolComplete {
            tool: "file_read".to_string(),
            call_id: "call456".to_string(),
            success: true,
            result: "file contents".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ToolComplete"));
        assert!(json.contains("file_read"));
        assert!(json.contains("file contents"));
    }

    #[test]
    fn test_event_session_complete_serialization() {
        let event = Event::SessionComplete {
            session_id: "sess456".to_string(),
            total_turns: 10,
            status: "completed".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("SessionComplete"));
        assert!(json.contains("sess456"));
        assert!(json.contains("10"));
        assert!(json.contains("completed"));
    }

    #[test]
    fn test_event_shutdown_complete_serialization() {
        let event = Event::ShutdownComplete;
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ShutdownComplete"));
    }

    #[test]
    fn test_event_error_serialization() {
        let event = Event::Error {
            submission_id: Some("sub789".to_string()),
            message: "Something went wrong".to_string(),
            recoverable: false,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Error"));
        assert!(json.contains("sub789"));
        assert!(json.contains("Something went wrong"));
        assert!(json.contains("false"));

        // Test without submission_id
        let event = Event::Error {
            submission_id: None,
            message: "Generic error".to_string(),
            recoverable: true,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Generic error"));
    }

    #[test]
    fn test_event_models_available_serialization() {
        let event = Event::ModelsAvailable {
            models: vec![ModelInfo {
                id: "test-model".to_string(),
                name: "Test Model".to_string(),
                provider: "test".to_string(),
                supports_reasoning: false,
                max_context: None,
            }],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ModelsAvailable"));
        assert!(json.contains("test-model"));
    }

    #[test]
    fn test_event_undo_started_serialization() {
        let event = Event::UndoStarted;
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("UndoStarted"));
    }

    #[test]
    fn test_event_undo_complete_serialization() {
        let event = Event::UndoComplete {
            description: "Restored to previous state".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("UndoComplete"));
        assert!(json.contains("Restored to previous state"));
    }

    #[test]
    fn test_command_assessment_debug() {
        let assessment = CommandAssessment {
            risk: RiskLevel::Low,
            reason: "Read-only command".to_string(),
            known_safe: true,
        };
        let debug_str = format!("{:?}", assessment);
        assert!(debug_str.contains("CommandAssessment"));
        assert!(debug_str.contains("Low"));
    }

    #[test]
    fn test_command_assessment_clone() {
        let assessment = CommandAssessment {
            risk: RiskLevel::Critical,
            reason: "Dangerous".to_string(),
            known_safe: false,
        };
        let cloned = assessment.clone();
        assert_eq!(cloned.risk, RiskLevel::Critical);
        assert_eq!(cloned.reason, "Dangerous");
        assert!(!cloned.known_safe);
    }

    #[test]
    fn test_risk_level_debug() {
        for level in [
            RiskLevel::Safe,
            RiskLevel::Low,
            RiskLevel::Medium,
            RiskLevel::High,
            RiskLevel::Critical,
        ] {
            let debug_str = format!("{:?}", level);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_risk_level_clone() {
        let level = RiskLevel::High;
        // Use Copy trait (Clone is auto-derived for Copy types)
        let cloned = level;
        assert_eq!(cloned, RiskLevel::High);
        assert_eq!(level, RiskLevel::High); // Original still valid
    }

    #[test]
    fn test_risk_level_copy() {
        let level = RiskLevel::Medium;
        let copied: RiskLevel = level;
        assert_eq!(copied, RiskLevel::Medium);
        assert_eq!(level, RiskLevel::Medium);
    }

    #[test]
    fn test_abort_reason_debug() {
        let reason = AbortReason::UserInterrupt;
        let debug_str = format!("{:?}", reason);
        assert!(debug_str.contains("UserInterrupt"));

        let reason = AbortReason::Error {
            message: "test".to_string(),
        };
        let debug_str = format!("{:?}", reason);
        assert!(debug_str.contains("Error"));
    }

    #[test]
    fn test_abort_reason_clone() {
        let reason = AbortReason::TurnLimit;
        let cloned = reason.clone();
        assert!(matches!(cloned, AbortReason::TurnLimit));

        let reason = AbortReason::Error {
            message: "err".to_string(),
        };
        let cloned = reason.clone();
        if let AbortReason::Error { message } = cloned {
            assert_eq!(message, "err");
        } else {
            panic!("Clone failed");
        }
    }

    #[test]
    fn test_model_info_debug() {
        let info = ModelInfo {
            id: "model1".to_string(),
            name: "Model One".to_string(),
            provider: "provider1".to_string(),
            supports_reasoning: true,
            max_context: Some(8192),
        };
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("ModelInfo"));
        assert!(debug_str.contains("model1"));
    }

    #[test]
    fn test_model_info_clone() {
        let info = ModelInfo {
            id: "model2".to_string(),
            name: "Model Two".to_string(),
            provider: "provider2".to_string(),
            supports_reasoning: false,
            max_context: None,
        };
        let cloned = info.clone();
        assert_eq!(cloned.id, "model2");
        assert!(!cloned.supports_reasoning);
        assert!(cloned.max_context.is_none());
    }

    #[test]
    fn test_mcp_tool_info_debug() {
        let tool = McpToolInfo {
            qualified_name: "mcp__server__tool".to_string(),
            name: "tool".to_string(),
            server: "server".to_string(),
            description: Some("A tool".to_string()),
        };
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("McpToolInfo"));
        assert!(debug_str.contains("mcp__server__tool"));
    }

    #[test]
    fn test_mcp_tool_info_clone() {
        let tool = McpToolInfo {
            qualified_name: "mcp__s__t".to_string(),
            name: "t".to_string(),
            server: "s".to_string(),
            description: None,
        };
        let cloned = tool.clone();
        assert_eq!(cloned.qualified_name, "mcp__s__t");
        assert!(cloned.description.is_none());
    }

    #[test]
    fn test_mcp_tool_info_no_description() {
        let tool = McpToolInfo {
            qualified_name: "q".to_string(),
            name: "n".to_string(),
            server: "s".to_string(),
            description: None,
        };
        let json = serde_json::to_string(&tool).unwrap();
        let parsed: McpToolInfo = serde_json::from_str(&json).unwrap();
        assert!(parsed.description.is_none());
    }

    #[test]
    fn test_turn_settings_debug() {
        let settings = TurnSettings {
            cwd: PathBuf::from("/tmp"),
            approval_policy: ApprovalPolicy::Always,
            sandbox_policy: SandboxPolicy::None,
            model: Some("test".to_string()),
        };
        let debug_str = format!("{:?}", settings);
        assert!(debug_str.contains("TurnSettings"));
        assert!(debug_str.contains("/tmp"));
    }

    #[test]
    fn test_turn_settings_clone() {
        let settings = TurnSettings {
            cwd: PathBuf::from("/home"),
            approval_policy: ApprovalPolicy::Never,
            sandbox_policy: SandboxPolicy::Native,
            model: None,
        };
        let cloned = settings.clone();
        assert_eq!(cloned.cwd, PathBuf::from("/home"));
        assert_eq!(cloned.approval_policy, ApprovalPolicy::Never);
        assert!(cloned.model.is_none());
    }

    #[test]
    fn test_codex_spawn_ok_fields() {
        // Can't easily test without async, but we can test the struct exists
        // and has the expected fields by construction
        let session_id = "test-session".to_string();
        // We need a Codex instance, which requires channels
        let (tx_sub, _rx_sub) = async_channel::bounded::<Submission>(1);
        let (_tx_event, rx_event) = async_channel::unbounded::<Event>();
        let codex = Codex {
            next_id: AtomicU64::new(0),
            tx_sub,
            rx_event,
            session_id: session_id.clone(),
        };
        let spawn_ok = CodexSpawnOk {
            codex,
            session_id: session_id.clone(),
        };
        assert_eq!(spawn_ok.session_id, "test-session");
    }

    #[tokio::test]
    async fn test_codex_session_id() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        assert!(!codex.session_id().is_empty());
        assert_eq!(codex.session_id(), spawn_ok.session_id);
    }

    #[tokio::test]
    async fn test_codex_has_pending_events() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Should have pending SessionConfigured event
        assert!(codex.has_pending_events());

        // Consume it
        let _ = codex.next_event().await.unwrap();

        // Submit something to get another event
        codex.submit(Op::ListModels).await.unwrap();

        // Give it a moment to process
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Should have pending event again
        assert!(codex.has_pending_events());
    }

    #[test]
    fn test_codex_from_channels() {
        let (tx_sub, _rx_sub) = async_channel::bounded::<Submission>(1);
        let (_tx_event, rx_event) = async_channel::unbounded::<Event>();

        let codex = Codex::from_channels(tx_sub, rx_event);

        // Session ID should be empty for delegate instances
        assert!(codex.session_id().is_empty());
    }

    #[test]
    fn test_codex_ops_sender() {
        let (tx_sub, _rx_sub) = async_channel::bounded::<Submission>(1);
        let (_tx_event, rx_event) = async_channel::unbounded::<Event>();

        let codex = Codex::from_channels(tx_sub.clone(), rx_event);
        let sender = codex.ops_sender();

        // Should be able to send through the cloned sender
        assert!(!sender.is_closed());
    }

    #[tokio::test]
    async fn test_codex_submit_with_id() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Skip SessionConfigured
        let _ = codex.next_event().await.unwrap();

        // Submit with custom ID
        let custom_sub = Submission {
            id: "custom-id-123".to_string(),
            op: Op::ListModels,
        };
        codex.submit_with_id(custom_sub).await.unwrap();

        let event = codex.next_event().await.unwrap();
        assert!(matches!(event, Event::ModelsAvailable { .. }));
    }

    #[test]
    fn test_submission_channel_capacity() {
        assert_eq!(SUBMISSION_CHANNEL_CAPACITY, 64);
    }

    #[tokio::test]
    async fn test_add_to_history() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Skip SessionConfigured
        let _ = codex.next_event().await.unwrap();

        // Add to history (no event generated, just modifies state)
        codex
            .submit(Op::AddToHistory {
                text: "Some context".to_string(),
            })
            .await
            .unwrap();

        // No event expected for AddToHistory, so submit shutdown to verify
        codex.submit(Op::Shutdown).await.unwrap();
        let event = codex.next_event().await.unwrap();
        assert!(matches!(event, Event::ShutdownComplete));
    }

    #[tokio::test]
    async fn test_interrupt() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Skip SessionConfigured
        let _ = codex.next_event().await.unwrap();

        // Send interrupt (no event generated, modifies state)
        codex.submit(Op::Interrupt).await.unwrap();

        // Shutdown to verify
        codex.submit(Op::Shutdown).await.unwrap();
        let event = codex.next_event().await.unwrap();
        assert!(matches!(event, Event::ShutdownComplete));
    }

    #[tokio::test]
    async fn test_undo_no_commits() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Skip SessionConfigured
        let _ = codex.next_event().await.unwrap();

        // Submit undo (no ghost commits available)
        codex.submit(Op::Undo).await.unwrap();

        // Should get UndoStarted then UndoComplete
        let event = codex.next_event().await.unwrap();
        assert!(matches!(event, Event::UndoStarted));

        let event = codex.next_event().await.unwrap();
        if let Event::UndoComplete { description } = event {
            assert!(description.contains("No previous state"));
        } else {
            panic!("Expected UndoComplete, got {:?}", event);
        }
    }

    #[tokio::test]
    async fn test_compact() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Skip SessionConfigured
        let _ = codex.next_event().await.unwrap();

        // Submit compact (history is small, won't actually compact)
        codex.submit(Op::Compact).await.unwrap();

        let event = codex.next_event().await.unwrap();
        if let Event::Compacted {
            original_tokens,
            new_tokens,
        } = event
        {
            // With empty history, should be unchanged
            assert_eq!(original_tokens, new_tokens);
        } else {
            panic!("Expected Compacted event, got {:?}", event);
        }
    }

    #[tokio::test]
    async fn test_override_turn_context() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Skip SessionConfigured
        let _ = codex.next_event().await.unwrap();

        // Override turn context (no event generated)
        codex
            .submit(Op::OverrideTurnContext {
                cwd: Some(PathBuf::from("/tmp")),
                approval_policy: Some(ApprovalPolicy::Always),
                sandbox_policy: Some(SandboxPolicy::None),
                model: Some("gpt-4-turbo".to_string()),
            })
            .await
            .unwrap();

        // Shutdown to verify processing
        codex.submit(Op::Shutdown).await.unwrap();
        let event = codex.next_event().await.unwrap();
        assert!(matches!(event, Event::ShutdownComplete));
    }

    #[tokio::test]
    async fn test_override_turn_context_partial() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Skip SessionConfigured
        let _ = codex.next_event().await.unwrap();

        // Partial override (only some fields)
        codex
            .submit(Op::OverrideTurnContext {
                cwd: None,
                approval_policy: Some(ApprovalPolicy::Never),
                sandbox_policy: None,
                model: None,
            })
            .await
            .unwrap();

        codex.submit(Op::Shutdown).await.unwrap();
        let event = codex.next_event().await.unwrap();
        assert!(matches!(event, Event::ShutdownComplete));
    }

    #[test]
    fn test_op_user_input_with_context() {
        let op = Op::UserInput {
            message: "Hello".to_string(),
            context: vec![
                ContextItem::File {
                    path: PathBuf::from("test.rs"),
                },
                ContextItem::Url {
                    url: "https://example.com".to_string(),
                },
                ContextItem::Text {
                    content: "context".to_string(),
                },
            ],
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("File"));
        assert!(json.contains("Url"));
        assert!(json.contains("Text"));
    }

    #[test]
    fn test_op_user_turn_defaults() {
        // Test deserialization with defaults
        let json = r#"{"type":"UserTurn","message":"test"}"#;
        let op: Op = serde_json::from_str(json).unwrap();
        if let Op::UserTurn {
            message,
            cwd,
            approval_policy,
            sandbox_policy,
            model,
        } = op
        {
            assert_eq!(message, "test");
            assert!(cwd.is_none());
            assert_eq!(approval_policy, ApprovalPolicy::OnUnknown);
            assert_eq!(sandbox_policy, SandboxPolicy::Native);
            assert!(model.is_none());
        } else {
            panic!("Expected UserTurn");
        }
    }

    #[test]
    fn test_event_review_complete_serialization() {
        use crate::review::ReviewOutputEvent;
        let event = Event::ReviewComplete {
            output: ReviewOutputEvent {
                findings: vec![],
                overall_correctness: "correct".to_string(),
                overall_explanation: "Code looks good".to_string(),
                overall_confidence_score: 0.95,
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ReviewComplete"));
        assert!(json.contains("correct"));
        assert!(json.contains("Code looks good"));
    }

    // ============================================================================
    // Additional test coverage (N=287)
    // ============================================================================

    #[test]
    fn test_op_user_input_empty_context() {
        let op = Op::UserInput {
            message: "test".to_string(),
            context: vec![],
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: Op = serde_json::from_str(&json).unwrap();
        if let Op::UserInput { message, context } = parsed {
            assert_eq!(message, "test");
            assert!(context.is_empty());
        } else {
            panic!("Expected UserInput");
        }
    }

    #[test]
    fn test_op_user_turn_all_defaults() {
        let json = r#"{"type":"UserTurn","message":"hello"}"#;
        let parsed: Op = serde_json::from_str(json).unwrap();
        if let Op::UserTurn {
            message,
            cwd,
            approval_policy,
            sandbox_policy,
            model,
        } = parsed
        {
            assert_eq!(message, "hello");
            assert!(cwd.is_none());
            assert_eq!(approval_policy, ApprovalPolicy::default());
            assert_eq!(sandbox_policy, SandboxPolicy::default());
            assert!(model.is_none());
        } else {
            panic!("Expected UserTurn");
        }
    }

    #[test]
    fn test_op_override_turn_context_all_none() {
        let op = Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            model: None,
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: Op = serde_json::from_str(&json).unwrap();
        if let Op::OverrideTurnContext {
            cwd,
            approval_policy,
            sandbox_policy,
            model,
        } = parsed
        {
            assert!(cwd.is_none());
            assert!(approval_policy.is_none());
            assert!(sandbox_policy.is_none());
            assert!(model.is_none());
        } else {
            panic!("Expected OverrideTurnContext");
        }
    }

    #[test]
    fn test_op_override_turn_context_all_some() {
        let op = Op::OverrideTurnContext {
            cwd: Some(PathBuf::from("/home")),
            approval_policy: Some(ApprovalPolicy::Always),
            sandbox_policy: Some(SandboxPolicy::None),
            model: Some("gpt-4".to_string()),
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: Op = serde_json::from_str(&json).unwrap();
        if let Op::OverrideTurnContext {
            cwd,
            approval_policy,
            sandbox_policy,
            model,
        } = parsed
        {
            assert_eq!(cwd, Some(PathBuf::from("/home")));
            assert_eq!(approval_policy, Some(ApprovalPolicy::Always));
            assert_eq!(sandbox_policy, Some(SandboxPolicy::None));
            assert_eq!(model, Some("gpt-4".to_string()));
        } else {
            panic!("Expected OverrideTurnContext");
        }
    }

    #[test]
    fn test_event_error_with_none_submission_id() {
        let event = Event::Error {
            submission_id: None,
            message: "error".to_string(),
            recoverable: true,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::Error {
            submission_id,
            message,
            recoverable,
        } = parsed
        {
            assert!(submission_id.is_none());
            assert_eq!(message, "error");
            assert!(recoverable);
        } else {
            panic!("Expected Error");
        }
    }

    #[test]
    fn test_event_token_usage_no_cost() {
        let event = Event::TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cost_usd: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::TokenUsage {
            input_tokens,
            output_tokens,
            cost_usd,
        } = parsed
        {
            assert_eq!(input_tokens, 100);
            assert_eq!(output_tokens, 50);
            assert!(cost_usd.is_none());
        } else {
            panic!("Expected TokenUsage");
        }
    }

    #[test]
    fn test_event_models_available_empty() {
        let event = Event::ModelsAvailable { models: vec![] };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::ModelsAvailable { models } = parsed {
            assert!(models.is_empty());
        } else {
            panic!("Expected ModelsAvailable");
        }
    }

    #[test]
    fn test_event_mcp_tools_available_empty() {
        let event = Event::McpToolsAvailable { tools: vec![] };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::McpToolsAvailable { tools } = parsed {
            assert!(tools.is_empty());
        } else {
            panic!("Expected McpToolsAvailable");
        }
    }

    #[test]
    fn test_context_item_file_empty_path() {
        let item = ContextItem::File {
            path: PathBuf::new(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: ContextItem = serde_json::from_str(&json).unwrap();
        if let ContextItem::File { path } = parsed {
            assert!(path.as_os_str().is_empty());
        } else {
            panic!("Expected File");
        }
    }

    #[test]
    fn test_context_item_text_empty_content() {
        let item = ContextItem::Text {
            content: String::new(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: ContextItem = serde_json::from_str(&json).unwrap();
        if let ContextItem::Text { content } = parsed {
            assert!(content.is_empty());
        } else {
            panic!("Expected Text");
        }
    }

    #[test]
    fn test_context_item_url_complex() {
        let item = ContextItem::Url {
            url: "https://user:pass@example.com:8080/path?query=1#fragment".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: ContextItem = serde_json::from_str(&json).unwrap();
        if let ContextItem::Url { url } = parsed {
            assert!(url.contains("example.com"));
            assert!(url.contains("8080"));
        } else {
            panic!("Expected Url");
        }
    }

    #[test]
    fn test_review_request_empty_focus() {
        let request = ReviewRequest {
            review_type: ReviewType::Full,
            files: vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")],
            focus: vec![],
        };
        let json = serde_json::to_string(&request).unwrap();
        let parsed: ReviewRequest = serde_json::from_str(&json).unwrap();
        assert!(parsed.focus.is_empty());
        assert_eq!(parsed.files.len(), 2);
    }

    #[test]
    fn test_review_request_many_files() {
        let files: Vec<PathBuf> = (0..10)
            .map(|i| PathBuf::from(format!("file{}.rs", i)))
            .collect();
        let request = ReviewRequest {
            review_type: ReviewType::Performance,
            files: files.clone(),
            focus: vec!["memory".to_string()],
        };
        let json = serde_json::to_string(&request).unwrap();
        let parsed: ReviewRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.files.len(), 10);
    }

    #[test]
    fn test_review_type_equality() {
        assert_eq!(ReviewType::Full, ReviewType::Full);
        assert_eq!(ReviewType::Security, ReviewType::Security);
        assert_eq!(ReviewType::Performance, ReviewType::Performance);
        assert_eq!(ReviewType::Style, ReviewType::Style);

        assert_ne!(ReviewType::Full, ReviewType::Security);
        assert_ne!(ReviewType::Security, ReviewType::Performance);
        assert_ne!(ReviewType::Performance, ReviewType::Style);
    }

    #[test]
    fn test_risk_level_equality() {
        assert_eq!(RiskLevel::Safe, RiskLevel::Safe);
        assert_eq!(RiskLevel::Low, RiskLevel::Low);
        assert_eq!(RiskLevel::Medium, RiskLevel::Medium);
        assert_eq!(RiskLevel::High, RiskLevel::High);
        assert_eq!(RiskLevel::Critical, RiskLevel::Critical);

        assert_ne!(RiskLevel::Safe, RiskLevel::Critical);
    }

    #[test]
    fn test_command_assessment_known_safe() {
        let assessment = CommandAssessment {
            risk: RiskLevel::Safe,
            reason: "Read-only operation".to_string(),
            known_safe: true,
        };
        let json = serde_json::to_string(&assessment).unwrap();
        assert!(json.contains("true"));
        assert!(json.contains("safe"));
    }

    #[test]
    fn test_command_assessment_high_risk() {
        let assessment = CommandAssessment {
            risk: RiskLevel::High,
            reason: "System modification".to_string(),
            known_safe: false,
        };
        let json = serde_json::to_string(&assessment).unwrap();
        assert!(json.contains("high"));
        assert!(json.contains("System modification"));
    }

    #[test]
    fn test_abort_reason_error_long_message() {
        let long_msg = "x".repeat(1000);
        let reason = AbortReason::Error {
            message: long_msg.clone(),
        };
        let json = serde_json::to_string(&reason).unwrap();
        let parsed: AbortReason = serde_json::from_str(&json).unwrap();
        if let AbortReason::Error { message } = parsed {
            assert_eq!(message.len(), 1000);
        } else {
            panic!("Expected Error");
        }
    }

    #[test]
    fn test_abort_reason_all_variants_deserialization() {
        let user_interrupt: AbortReason = serde_json::from_str(r#""user_interrupt""#).unwrap();
        assert!(matches!(user_interrupt, AbortReason::UserInterrupt));

        let turn_limit: AbortReason = serde_json::from_str(r#""turn_limit""#).unwrap();
        assert!(matches!(turn_limit, AbortReason::TurnLimit));

        let approval_denied: AbortReason = serde_json::from_str(r#""approval_denied""#).unwrap();
        assert!(matches!(approval_denied, AbortReason::ApprovalDenied));

        let shutdown: AbortReason = serde_json::from_str(r#""shutdown""#).unwrap();
        assert!(matches!(shutdown, AbortReason::Shutdown));
    }

    #[test]
    fn test_model_info_all_fields_populated() {
        let info = ModelInfo {
            id: "claude-3-sonnet".to_string(),
            name: "Claude 3 Sonnet".to_string(),
            provider: "anthropic".to_string(),
            supports_reasoning: true,
            max_context: Some(200000),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("claude-3-sonnet"));
        assert!(json.contains("200000"));
        assert!(json.contains("anthropic"));
    }

    #[test]
    fn test_model_info_minimal() {
        let info = ModelInfo {
            id: "local".to_string(),
            name: String::new(),
            provider: String::new(),
            supports_reasoning: false,
            max_context: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: ModelInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "local");
        assert!(parsed.name.is_empty());
        assert!(!parsed.supports_reasoning);
    }

    #[test]
    fn test_mcp_tool_info_with_description() {
        let tool = McpToolInfo {
            qualified_name: "mcp__fs__read_file".to_string(),
            name: "read_file".to_string(),
            server: "fs".to_string(),
            description: Some("Read contents of a file".to_string()),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("Read contents"));
    }

    #[test]
    fn test_mcp_tool_info_without_description() {
        let tool = McpToolInfo {
            qualified_name: "mcp__db__query".to_string(),
            name: "query".to_string(),
            server: "db".to_string(),
            description: None,
        };
        let json = serde_json::to_string(&tool).unwrap();
        let parsed: McpToolInfo = serde_json::from_str(&json).unwrap();
        assert!(parsed.description.is_none());
    }

    #[test]
    fn test_sandbox_policy_docker_none_image() {
        let policy = SandboxPolicy::Docker { image: None };
        let json = serde_json::to_string(&policy).unwrap();
        let parsed: SandboxPolicy = serde_json::from_str(&json).unwrap();
        if let SandboxPolicy::Docker { image } = parsed {
            assert!(image.is_none());
        } else {
            panic!("Expected Docker");
        }
    }

    #[test]
    fn test_sandbox_policy_docker_with_tag() {
        let policy = SandboxPolicy::Docker {
            image: Some("node:18-alpine".to_string()),
        };
        assert_eq!(policy.to_string(), "docker:node:18-alpine");
    }

    #[test]
    fn test_sandbox_policy_all_variants_equality() {
        assert_eq!(SandboxPolicy::None, SandboxPolicy::None);
        assert_eq!(SandboxPolicy::Native, SandboxPolicy::Native);

        let docker1 = SandboxPolicy::Docker { image: None };
        let docker2 = SandboxPolicy::Docker { image: None };
        assert_eq!(docker1, docker2);

        let docker3 = SandboxPolicy::Docker {
            image: Some("x".to_string()),
        };
        let docker4 = SandboxPolicy::Docker {
            image: Some("x".to_string()),
        };
        assert_eq!(docker3, docker4);

        assert_ne!(SandboxPolicy::None, SandboxPolicy::Native);
    }

    #[test]
    fn test_approval_policy_all_display() {
        assert_eq!(format!("{}", ApprovalPolicy::Never), "never");
        assert_eq!(format!("{}", ApprovalPolicy::OnUnknown), "on-unknown");
        assert_eq!(format!("{}", ApprovalPolicy::Always), "always");
    }

    #[test]
    fn test_approval_decision_all_display() {
        assert_eq!(format!("{}", ApprovalDecision::Approve), "approve");
        assert_eq!(format!("{}", ApprovalDecision::Deny), "deny");
        assert_eq!(
            format!("{}", ApprovalDecision::ApproveAndRemember),
            "approve-remember"
        );
        assert_eq!(
            format!("{}", ApprovalDecision::DenyAndRemember),
            "deny-remember"
        );
    }

    #[test]
    fn test_approval_decision_equality() {
        assert_eq!(ApprovalDecision::Approve, ApprovalDecision::Approve);
        assert_eq!(ApprovalDecision::Deny, ApprovalDecision::Deny);
        assert_eq!(
            ApprovalDecision::ApproveAndRemember,
            ApprovalDecision::ApproveAndRemember
        );
        assert_eq!(
            ApprovalDecision::DenyAndRemember,
            ApprovalDecision::DenyAndRemember
        );

        assert_ne!(ApprovalDecision::Approve, ApprovalDecision::Deny);
    }

    #[test]
    fn test_turn_settings_with_custom_values() {
        let settings = TurnSettings {
            cwd: PathBuf::from("/custom/path"),
            approval_policy: ApprovalPolicy::Never,
            sandbox_policy: SandboxPolicy::Docker {
                image: Some("rust:latest".to_string()),
            },
            model: Some("gpt-4-turbo".to_string()),
        };
        assert_eq!(settings.cwd, PathBuf::from("/custom/path"));
        assert_eq!(settings.approval_policy, ApprovalPolicy::Never);
        assert!(settings.model.is_some());
    }

    #[tokio::test]
    async fn test_codex_multiple_submissions() {
        let config = Config::default();
        let callback = Arc::new(NullStreamCallback);
        let spawn_ok = Codex::spawn(config, callback).await.unwrap();
        let codex = spawn_ok.codex;

        // Skip SessionConfigured
        let _ = codex.next_event().await.unwrap();

        // Submit multiple operations
        let id1 = codex.submit(Op::ListModels).await.unwrap();
        let id2 = codex.submit(Op::ListMcpTools).await.unwrap();

        assert_ne!(id1, id2); // IDs should be unique

        // Get both events
        let event1 = codex.next_event().await.unwrap();
        let event2 = codex.next_event().await.unwrap();

        // Should get both response types (order may vary)
        let events = [
            std::mem::discriminant(&event1),
            std::mem::discriminant(&event2),
        ];
        assert!(events
            .iter()
            .any(|e| { *e == std::mem::discriminant(&Event::ModelsAvailable { models: vec![] }) }));
        assert!(events.iter().any(|e| {
            *e == std::mem::discriminant(&Event::McpToolsAvailable { tools: vec![] })
        }));
    }

    #[test]
    fn test_codex_next_id_increments() {
        let (tx_sub, _rx_sub) = async_channel::bounded::<Submission>(1);
        let (_tx_event, rx_event) = async_channel::unbounded::<Event>();

        let codex = Codex {
            next_id: AtomicU64::new(0),
            tx_sub,
            rx_event,
            session_id: "test".to_string(),
        };

        // Check that IDs increment
        let id1 = codex.next_id.fetch_add(1, Ordering::SeqCst);
        let id2 = codex.next_id.fetch_add(1, Ordering::SeqCst);
        let id3 = codex.next_id.fetch_add(1, Ordering::SeqCst);

        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(id3, 2);
    }

    #[tokio::test]
    async fn test_codex_channel_closed_error() {
        let (tx_sub, rx_sub) = async_channel::bounded::<Submission>(1);
        let (_tx_event, rx_event) = async_channel::unbounded::<Event>();

        let codex = Codex {
            next_id: AtomicU64::new(0),
            tx_sub,
            rx_event,
            session_id: "test".to_string(),
        };

        // Close the receiving end
        drop(rx_sub);

        // Submit should fail
        let result = codex.submit(Op::Interrupt).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_codex_event_channel_closed() {
        let (tx_sub, _rx_sub) = async_channel::bounded::<Submission>(1);
        let (tx_event, rx_event) = async_channel::unbounded::<Event>();

        let codex = Codex {
            next_id: AtomicU64::new(0),
            tx_sub,
            rx_event,
            session_id: "test".to_string(),
        };

        // Close the sending end
        drop(tx_event);

        // next_event should fail
        let result = codex.next_event().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_event_reasoning_complete_no_tool_calls() {
        let event = Event::ReasoningComplete {
            turn: 5,
            duration_ms: 2500,
            has_tool_calls: false,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("2500"));
        assert!(json.contains("false"));
    }

    #[test]
    fn test_event_tool_complete_failure() {
        let event = Event::ToolComplete {
            tool: "shell".to_string(),
            call_id: "call-123".to_string(),
            success: false,
            result: "Command failed with exit code 1".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("false"));
        assert!(json.contains("exit code 1"));
    }

    #[test]
    fn test_event_shell_command_complete_with_stderr() {
        let event = Event::ShellCommandComplete {
            command: "ls /nonexistent".to_string(),
            exit_code: 2,
            stdout: String::new(),
            stderr: "ls: /nonexistent: No such file or directory".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("No such file"));
    }

    #[test]
    fn test_event_patch_approval_request_complex_patch() {
        let patch = r#"@@ -10,7 +10,8 @@
 existing line
-old line
+new line 1
+new line 2
 more context"#;
        let event = Event::PatchApprovalRequest {
            id: "patch-456".to_string(),
            file: PathBuf::from("/project/src/lib.rs"),
            patch: patch.to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("patch-456"));
        assert!(json.contains("lib.rs"));
    }

    #[test]
    fn test_submission_with_various_ops() {
        let ops = vec![
            Op::Interrupt,
            Op::Shutdown,
            Op::Compact,
            Op::Undo,
            Op::ListModels,
            Op::ListMcpTools,
        ];

        for (i, op) in ops.into_iter().enumerate() {
            let sub = Submission {
                id: format!("sub-{}", i),
                op,
            };
            let debug = format!("{:?}", sub);
            assert!(debug.contains("Submission"));
            assert!(debug.contains(&format!("sub-{}", i)));
        }
    }

    #[test]
    fn test_codex_spawn_ok_session_id_matches() {
        let session_id = "test-session-xyz".to_string();
        let (tx_sub, _rx_sub) = async_channel::bounded::<Submission>(1);
        let (_tx_event, rx_event) = async_channel::unbounded::<Event>();

        let codex = Codex {
            next_id: AtomicU64::new(0),
            tx_sub,
            rx_event,
            session_id: session_id.clone(),
        };

        let spawn_ok = CodexSpawnOk {
            codex,
            session_id: session_id.clone(),
        };

        assert_eq!(spawn_ok.session_id, "test-session-xyz");
        assert_eq!(spawn_ok.codex.session_id(), "test-session-xyz");
    }

    #[test]
    fn test_channel_capacity_is_reasonable() {
        // Channel capacity should be at least 1 and at most some reasonable number
        // Use const assertions for compile-time verification
        const _: () = assert!(SUBMISSION_CHANNEL_CAPACITY >= 1);
        const _: () = assert!(SUBMISSION_CHANNEL_CAPACITY <= 1000);
    }

    // More tests to improve coverage ratio (N=287)

    #[test]
    fn test_approval_policy_serialization_roundtrip() {
        for policy in [
            ApprovalPolicy::Never,
            ApprovalPolicy::OnUnknown,
            ApprovalPolicy::Always,
        ] {
            let json = serde_json::to_string(&policy).unwrap();
            let parsed: ApprovalPolicy = serde_json::from_str(&json).unwrap();
            assert_eq!(policy, parsed);
        }
    }

    #[test]
    fn test_review_type_all_variants_clone() {
        for rt in [
            ReviewType::Full,
            ReviewType::Security,
            ReviewType::Performance,
            ReviewType::Style,
        ] {
            let cloned = rt.clone();
            assert_eq!(rt, cloned);
        }
    }

    #[test]
    fn test_risk_level_all_variants_copy() {
        for level in [
            RiskLevel::Safe,
            RiskLevel::Low,
            RiskLevel::Medium,
            RiskLevel::High,
            RiskLevel::Critical,
        ] {
            let copied = level; // Copy
            assert_eq!(level, copied);
        }
    }

    #[test]
    fn test_event_reasoning_delta_empty() {
        let event = Event::ReasoningDelta {
            content: String::new(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ReasoningDelta"));
    }

    #[test]
    fn test_event_reasoning_delta_long_content() {
        let long_content = "x".repeat(5000);
        let event = Event::ReasoningDelta {
            content: long_content.clone(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.len() > 5000);
    }

    #[test]
    fn test_event_turn_complete_empty_response() {
        let event = Event::TurnComplete {
            submission_id: "sub1".to_string(),
            turn: 1,
            response: String::new(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::TurnComplete { response, .. } = parsed {
            assert!(response.is_empty());
        } else {
            panic!("Expected TurnComplete");
        }
    }

    #[test]
    fn test_op_review_with_all_focus_areas() {
        let op = Op::Review {
            request: ReviewRequest {
                review_type: ReviewType::Full,
                files: vec![],
                focus: vec![
                    "security".to_string(),
                    "performance".to_string(),
                    "style".to_string(),
                    "maintainability".to_string(),
                ],
            },
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("security"));
        assert!(json.contains("performance"));
        assert!(json.contains("style"));
        assert!(json.contains("maintainability"));
    }

    #[test]
    fn test_event_compacted_zero_tokens() {
        let event = Event::Compacted {
            original_tokens: 0,
            new_tokens: 0,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::Compacted {
            original_tokens,
            new_tokens,
        } = parsed
        {
            assert_eq!(original_tokens, 0);
            assert_eq!(new_tokens, 0);
        } else {
            panic!("Expected Compacted");
        }
    }

    #[test]
    fn test_event_compacted_large_savings() {
        let event = Event::Compacted {
            original_tokens: 1_000_000,
            new_tokens: 10_000,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("1000000"));
        assert!(json.contains("10000"));
    }

    #[test]
    fn test_context_item_file_deep_path() {
        let deep_path = PathBuf::from("/a/b/c/d/e/f/g/h/i/j/k/l/file.txt");
        let item = ContextItem::File {
            path: deep_path.clone(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: ContextItem = serde_json::from_str(&json).unwrap();
        if let ContextItem::File { path } = parsed {
            assert_eq!(path, deep_path);
        } else {
            panic!("Expected File");
        }
    }

    #[test]
    fn test_event_tool_started_various_tools() {
        let tools = [
            "shell",
            "file_read",
            "file_write",
            "apply_patch",
            "mcp__server__tool",
        ];
        for tool in tools {
            let event = Event::ToolStarted {
                tool: tool.to_string(),
                call_id: "call-xyz".to_string(),
            };
            let json = serde_json::to_string(&event).unwrap();
            assert!(json.contains(tool));
        }
    }

    #[test]
    fn test_model_info_large_context() {
        let info = ModelInfo {
            id: "large-model".to_string(),
            name: "Large Model".to_string(),
            provider: "custom".to_string(),
            supports_reasoning: true,
            max_context: Some(u64::MAX),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: ModelInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_context, Some(u64::MAX));
    }

    #[test]
    fn test_mcp_tool_info_long_qualified_name() {
        let long_name = format!("mcp__{}__{}", "a".repeat(100), "b".repeat(100));
        let tool = McpToolInfo {
            qualified_name: long_name.clone(),
            name: "b".repeat(100),
            server: "a".repeat(100),
            description: None,
        };
        let json = serde_json::to_string(&tool).unwrap();
        let parsed: McpToolInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.qualified_name, long_name);
    }

    #[test]
    fn test_command_assessment_empty_reason() {
        let assessment = CommandAssessment {
            risk: RiskLevel::Low,
            reason: String::new(),
            known_safe: false,
        };
        let json = serde_json::to_string(&assessment).unwrap();
        let parsed: CommandAssessment = serde_json::from_str(&json).unwrap();
        assert!(parsed.reason.is_empty());
    }

    #[test]
    fn test_abort_reason_error_empty_message() {
        let reason = AbortReason::Error {
            message: String::new(),
        };
        let json = serde_json::to_string(&reason).unwrap();
        let parsed: AbortReason = serde_json::from_str(&json).unwrap();
        if let AbortReason::Error { message } = parsed {
            assert!(message.is_empty());
        } else {
            panic!("Expected Error");
        }
    }

    #[test]
    fn test_submission_id_uniqueness() {
        use std::collections::HashSet;

        let (tx_sub, _rx_sub) = async_channel::bounded::<Submission>(100);
        let (_tx_event, rx_event) = async_channel::unbounded::<Event>();

        let codex = Codex {
            next_id: AtomicU64::new(0),
            tx_sub,
            rx_event,
            session_id: "test".to_string(),
        };

        let mut ids = HashSet::new();
        for _ in 0..100 {
            let id = codex.next_id.fetch_add(1, Ordering::SeqCst).to_string();
            assert!(ids.insert(id), "ID should be unique");
        }
        assert_eq!(ids.len(), 100);
    }

    #[test]
    fn test_event_session_complete_zero_turns() {
        let event = Event::SessionComplete {
            session_id: "sess".to_string(),
            total_turns: 0,
            status: "cancelled".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::SessionComplete { total_turns, .. } = parsed {
            assert_eq!(total_turns, 0);
        } else {
            panic!("Expected SessionComplete");
        }
    }

    #[test]
    fn test_op_user_input_unicode_message() {
        let op = Op::UserInput {
            message: "日本語 🎉 русский العربية".to_string(),
            context: vec![],
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: Op = serde_json::from_str(&json).unwrap();
        if let Op::UserInput { message, .. } = parsed {
            assert!(message.contains("日本語"));
            assert!(message.contains("🎉"));
            assert!(message.contains("русский"));
        } else {
            panic!("Expected UserInput");
        }
    }

    #[test]
    fn test_context_item_url_with_unicode() {
        let item = ContextItem::Url {
            url: "https://example.com/パス/путь".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: ContextItem = serde_json::from_str(&json).unwrap();
        if let ContextItem::Url { url } = parsed {
            assert!(url.contains("パス"));
            assert!(url.contains("путь"));
        } else {
            panic!("Expected Url");
        }
    }

    #[test]
    fn test_event_tool_complete_long_result() {
        let long_result = "x".repeat(100_000);
        let event = Event::ToolComplete {
            tool: "shell".to_string(),
            call_id: "call".to_string(),
            success: true,
            result: long_result.clone(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.len() > 100_000);
    }

    #[test]
    fn test_event_turn_aborted_all_reasons() {
        let reasons = vec![
            AbortReason::UserInterrupt,
            AbortReason::TurnLimit,
            AbortReason::ApprovalDenied,
            AbortReason::Shutdown,
            AbortReason::Error {
                message: "test".to_string(),
            },
        ];
        for reason in reasons {
            let event = Event::TurnAborted {
                submission_id: "test".to_string(),
                reason: reason.clone(),
            };
            let json = serde_json::to_string(&event).unwrap();
            let parsed: Event = serde_json::from_str(&json).unwrap();
            assert!(matches!(parsed, Event::TurnAborted { .. }));
        }
    }

    #[test]
    fn test_sandbox_policy_none_display() {
        let policy = SandboxPolicy::None;
        assert_eq!(policy.to_string(), "none");
    }

    #[test]
    fn test_sandbox_policy_native_display() {
        let policy = SandboxPolicy::Native;
        assert_eq!(policy.to_string(), "native");
    }

    #[test]
    fn test_sandbox_policy_docker_no_image_display() {
        let policy = SandboxPolicy::Docker { image: None };
        assert_eq!(policy.to_string(), "docker");
    }

    #[test]
    fn test_sandbox_policy_docker_custom_image_display() {
        let policy = SandboxPolicy::Docker {
            image: Some("python:3.11".to_string()),
        };
        assert_eq!(policy.to_string(), "docker:python:3.11");
    }

    #[test]
    fn test_approval_policy_default_value() {
        let policy = ApprovalPolicy::default();
        assert_eq!(policy, ApprovalPolicy::OnUnknown);
    }

    #[test]
    fn test_event_exec_approval_request_full_serialization() {
        let event = Event::ExecApprovalRequest {
            id: "exec-123".to_string(),
            command: "rm -rf /tmp/test".to_string(),
            assessment: CommandAssessment {
                risk: RiskLevel::Medium,
                reason: "File deletion".to_string(),
                known_safe: false,
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("exec-123"));
        assert!(json.contains("rm -rf"));
        assert!(json.contains("medium"));
    }

    #[test]
    fn test_event_session_complete_roundtrip() {
        let event = Event::SessionComplete {
            session_id: "sess-xyz".to_string(),
            total_turns: 10,
            status: "completed".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::SessionComplete {
            session_id,
            total_turns,
            status,
        } = parsed
        {
            assert_eq!(session_id, "sess-xyz");
            assert_eq!(total_turns, 10);
            assert_eq!(status, "completed");
        } else {
            panic!("Expected SessionComplete");
        }
    }

    #[test]
    fn test_event_turn_started_fields() {
        let event = Event::TurnStarted {
            submission_id: "sub-1".to_string(),
            turn: 5,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("sub-1"));
        assert!(json.contains("5"));
    }

    #[test]
    fn test_event_reasoning_started_roundtrip() {
        let event = Event::ReasoningStarted { turn: 3 };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::ReasoningStarted { turn } = parsed {
            assert_eq!(turn, 3);
        } else {
            panic!("Expected ReasoningStarted");
        }
    }

    // ============================================================================
    // Additional test coverage (N=298)
    // ============================================================================

    // --- Op serde tests ---

    #[test]
    fn test_op_interrupt_serde() {
        let op = Op::Interrupt;
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("Interrupt"));
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::Interrupt));
    }

    #[test]
    fn test_op_shutdown_serde() {
        let op = Op::Shutdown;
        let json = serde_json::to_string(&op).unwrap();
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::Shutdown));
    }

    #[test]
    fn test_op_compact_serde() {
        let op = Op::Compact;
        let json = serde_json::to_string(&op).unwrap();
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::Compact));
    }

    #[test]
    fn test_op_undo_serde() {
        let op = Op::Undo;
        let json = serde_json::to_string(&op).unwrap();
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::Undo));
    }

    #[test]
    fn test_op_list_models_serde() {
        let op = Op::ListModels;
        let json = serde_json::to_string(&op).unwrap();
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::ListModels));
    }

    #[test]
    fn test_op_list_mcp_tools_serde() {
        let op = Op::ListMcpTools;
        let json = serde_json::to_string(&op).unwrap();
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::ListMcpTools));
    }

    #[test]
    fn test_op_user_input_empty() {
        let op = Op::UserInput {
            message: String::new(),
            context: vec![],
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: Op = serde_json::from_str(&json).unwrap();
        if let Op::UserInput { message, context } = parsed {
            assert!(message.is_empty());
            assert!(context.is_empty());
        } else {
            panic!("Expected UserInput");
        }
    }

    #[test]
    fn test_op_user_input_with_file_context() {
        let op = Op::UserInput {
            message: "test".to_string(),
            context: vec![ContextItem::File {
                path: PathBuf::from("/test.txt"),
            }],
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("File"));
    }

    #[test]
    fn test_op_user_input_with_url_context() {
        let op = Op::UserInput {
            message: "test".to_string(),
            context: vec![ContextItem::Url {
                url: "https://example.com".to_string(),
            }],
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("Url"));
    }

    #[test]
    fn test_op_user_input_with_text_context() {
        let op = Op::UserInput {
            message: "test".to_string(),
            context: vec![ContextItem::Text {
                content: "some context".to_string(),
            }],
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("Text"));
    }

    #[test]
    fn test_op_user_turn_all_fields() {
        let op = Op::UserTurn {
            message: "test message".to_string(),
            cwd: Some(PathBuf::from("/home/user")),
            approval_policy: ApprovalPolicy::Always,
            sandbox_policy: SandboxPolicy::Native,
            model: Some("gpt-4".to_string()),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("test message"));
        assert!(json.contains("gpt-4"));
    }

    #[test]
    fn test_op_user_turn_minimal() {
        let op = Op::UserTurn {
            message: "minimal".to_string(),
            cwd: None,
            approval_policy: ApprovalPolicy::default(),
            sandbox_policy: SandboxPolicy::default(),
            model: None,
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::UserTurn { .. }));
    }

    #[test]
    fn test_op_override_turn_context_all_fields() {
        let op = Op::OverrideTurnContext {
            cwd: Some(PathBuf::from("/new/dir")),
            approval_policy: Some(ApprovalPolicy::Never),
            sandbox_policy: Some(SandboxPolicy::None),
            model: Some("claude".to_string()),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("OverrideTurnContext"));
    }

    #[test]
    fn test_op_override_turn_context_empty() {
        let op = Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            model: None,
        };
        let json = serde_json::to_string(&op).unwrap();
        let parsed: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Op::OverrideTurnContext { .. }));
    }

    #[test]
    fn test_op_exec_approval_approve() {
        let op = Op::ExecApproval {
            id: "exec-1".to_string(),
            decision: ApprovalDecision::Approve,
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("approve"));
    }

    #[test]
    fn test_op_exec_approval_deny() {
        let op = Op::ExecApproval {
            id: "exec-2".to_string(),
            decision: ApprovalDecision::Deny,
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("deny"));
    }

    #[test]
    fn test_op_exec_approval_approve_and_remember() {
        let op = Op::ExecApproval {
            id: "exec-3".to_string(),
            decision: ApprovalDecision::ApproveAndRemember,
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("approve_and_remember"));
    }

    #[test]
    fn test_op_exec_approval_deny_and_remember() {
        let op = Op::ExecApproval {
            id: "exec-4".to_string(),
            decision: ApprovalDecision::DenyAndRemember,
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("deny_and_remember"));
    }

    #[test]
    fn test_op_patch_approval_all_decisions() {
        for decision in [
            ApprovalDecision::Approve,
            ApprovalDecision::Deny,
            ApprovalDecision::ApproveAndRemember,
            ApprovalDecision::DenyAndRemember,
        ] {
            let op = Op::PatchApproval {
                id: "patch-1".to_string(),
                decision,
            };
            let json = serde_json::to_string(&op).unwrap();
            assert!(json.contains("PatchApproval"));
        }
    }

    #[test]
    fn test_op_add_to_history() {
        let op = Op::AddToHistory {
            text: "some history text".to_string(),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("AddToHistory"));
        assert!(json.contains("some history text"));
    }

    #[test]
    fn test_op_run_user_shell_command() {
        let op = Op::RunUserShellCommand {
            command: "ls -la".to_string(),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("RunUserShellCommand"));
        assert!(json.contains("ls -la"));
    }

    // --- ApprovalDecision tests ---

    #[test]
    fn test_approval_decision_eq_variants() {
        assert_eq!(ApprovalDecision::Approve, ApprovalDecision::Approve);
        assert_ne!(ApprovalDecision::Approve, ApprovalDecision::Deny);
    }

    #[test]
    fn test_approval_decision_copy_approve_remember() {
        let decision = ApprovalDecision::ApproveAndRemember;
        let copied: ApprovalDecision = decision; // Copy
        assert_eq!(copied, ApprovalDecision::ApproveAndRemember);
    }

    #[test]
    fn test_approval_decision_copy_deny_remember() {
        let decision = ApprovalDecision::DenyAndRemember;
        let copied = decision; // Copy
        assert_eq!(copied, ApprovalDecision::DenyAndRemember);
        assert_eq!(decision, ApprovalDecision::DenyAndRemember); // Original valid
    }

    // --- ApprovalPolicy tests ---

    #[test]
    fn test_approval_policy_display_never_n298() {
        assert_eq!(ApprovalPolicy::Never.to_string(), "never");
    }

    #[test]
    fn test_approval_policy_display_on_unknown_n298() {
        assert_eq!(ApprovalPolicy::OnUnknown.to_string(), "on-unknown");
    }

    #[test]
    fn test_approval_policy_display_always_n298() {
        assert_eq!(ApprovalPolicy::Always.to_string(), "always");
    }

    #[test]
    fn test_approval_policy_eq_variants() {
        assert_eq!(ApprovalPolicy::Never, ApprovalPolicy::Never);
        assert_ne!(ApprovalPolicy::Never, ApprovalPolicy::Always);
    }

    #[test]
    fn test_approval_policy_copy_always() {
        let policy = ApprovalPolicy::Always;
        let copied: ApprovalPolicy = policy; // Copy
        assert_eq!(copied, ApprovalPolicy::Always);
    }

    // --- SandboxPolicy tests ---

    #[test]
    fn test_sandbox_policy_eq_variants() {
        assert_eq!(SandboxPolicy::None, SandboxPolicy::None);
        assert_ne!(SandboxPolicy::None, SandboxPolicy::Native);
    }

    #[test]
    fn test_sandbox_policy_clone_docker() {
        let policy = SandboxPolicy::Docker {
            image: Some("test".to_string()),
        };
        let cloned = policy.clone();
        assert_eq!(cloned, policy);
    }

    #[test]
    fn test_sandbox_policy_serde_none_roundtrip() {
        let policy = SandboxPolicy::None;
        let json = serde_json::to_string(&policy).unwrap();
        let parsed: SandboxPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, SandboxPolicy::None);
    }

    #[test]
    fn test_sandbox_policy_serde_native_roundtrip() {
        let policy = SandboxPolicy::Native;
        let json = serde_json::to_string(&policy).unwrap();
        let parsed: SandboxPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, SandboxPolicy::Native);
    }

    #[test]
    fn test_sandbox_policy_serde_docker_image_roundtrip() {
        let policy = SandboxPolicy::Docker {
            image: Some("ubuntu:22.04".to_string()),
        };
        let json = serde_json::to_string(&policy).unwrap();
        let parsed: SandboxPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, policy);
    }

    #[test]
    fn test_sandbox_policy_default_native() {
        let policy = SandboxPolicy::default();
        assert_eq!(policy, SandboxPolicy::Native);
    }

    // --- ContextItem tests ---

    #[test]
    fn test_context_item_file_clone() {
        let item = ContextItem::File {
            path: PathBuf::from("/test.txt"),
        };
        let cloned = item.clone();
        assert!(matches!(cloned, ContextItem::File { .. }));
    }

    #[test]
    fn test_context_item_url_clone() {
        let item = ContextItem::Url {
            url: "https://example.com".to_string(),
        };
        let cloned = item.clone();
        if let ContextItem::Url { url } = cloned {
            assert_eq!(url, "https://example.com");
        } else {
            panic!("Expected Url");
        }
    }

    #[test]
    fn test_context_item_text_clone() {
        let item = ContextItem::Text {
            content: "test content".to_string(),
        };
        let cloned = item.clone();
        if let ContextItem::Text { content } = cloned {
            assert_eq!(content, "test content");
        } else {
            panic!("Expected Text");
        }
    }

    #[test]
    fn test_context_item_text_empty() {
        let item = ContextItem::Text {
            content: String::new(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: ContextItem = serde_json::from_str(&json).unwrap();
        if let ContextItem::Text { content } = parsed {
            assert!(content.is_empty());
        } else {
            panic!("Expected Text");
        }
    }

    // --- ReviewRequest tests ---

    #[test]
    fn test_review_request_empty_files_and_focus() {
        let req = ReviewRequest {
            review_type: ReviewType::Full,
            files: vec![],
            focus: vec![],
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ReviewRequest = serde_json::from_str(&json).unwrap();
        assert!(parsed.files.is_empty());
        assert!(parsed.focus.is_empty());
    }

    #[test]
    fn test_review_request_100_files() {
        let files: Vec<PathBuf> = (0..100)
            .map(|i| PathBuf::from(format!("/file{}.txt", i)))
            .collect();
        let req = ReviewRequest {
            review_type: ReviewType::Security,
            files,
            focus: vec!["injection".to_string()],
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ReviewRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.files.len(), 100);
    }

    #[test]
    fn test_review_request_clone_performance() {
        let req = ReviewRequest {
            review_type: ReviewType::Performance,
            files: vec![PathBuf::from("/test.rs")],
            focus: vec!["memory".to_string()],
        };
        let cloned = req.clone();
        assert_eq!(cloned.files.len(), 1);
        assert_eq!(cloned.focus.len(), 1);
    }

    // --- ReviewType tests ---

    #[test]
    fn test_review_type_serde_roundtrip() {
        for rt in [
            ReviewType::Full,
            ReviewType::Security,
            ReviewType::Performance,
            ReviewType::Style,
        ] {
            let json = serde_json::to_string(&rt).unwrap();
            let parsed: ReviewType = serde_json::from_str(&json).unwrap();
            assert_eq!(rt, parsed);
        }
    }

    // --- RiskLevel tests ---

    #[test]
    fn test_risk_level_serde_roundtrip() {
        for level in [
            RiskLevel::Safe,
            RiskLevel::Low,
            RiskLevel::Medium,
            RiskLevel::High,
            RiskLevel::Critical,
        ] {
            let json = serde_json::to_string(&level).unwrap();
            let parsed: RiskLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(level, parsed);
        }
    }

    #[test]
    fn test_risk_level_copy_critical() {
        let level = RiskLevel::Critical;
        let copied: RiskLevel = level; // Copy
        assert_eq!(copied, RiskLevel::Critical);
    }

    // --- CommandAssessment tests ---

    #[test]
    fn test_command_assessment_clone_high_risk() {
        let assessment = CommandAssessment {
            risk: RiskLevel::High,
            reason: "Destructive command".to_string(),
            known_safe: false,
        };
        let cloned = assessment.clone();
        assert_eq!(cloned.risk, RiskLevel::High);
        assert_eq!(cloned.reason, "Destructive command");
        assert!(!cloned.known_safe);
    }

    #[test]
    fn test_command_assessment_known_safe_true() {
        let assessment = CommandAssessment {
            risk: RiskLevel::Safe,
            reason: "Common read command".to_string(),
            known_safe: true,
        };
        let json = serde_json::to_string(&assessment).unwrap();
        let parsed: CommandAssessment = serde_json::from_str(&json).unwrap();
        assert!(parsed.known_safe);
    }

    // --- AbortReason tests ---

    #[test]
    fn test_abort_reason_serde_user_interrupt_roundtrip() {
        let reason = AbortReason::UserInterrupt;
        let json = serde_json::to_string(&reason).unwrap();
        let parsed: AbortReason = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, AbortReason::UserInterrupt));
    }

    #[test]
    fn test_abort_reason_serde_turn_limit_roundtrip() {
        let reason = AbortReason::TurnLimit;
        let json = serde_json::to_string(&reason).unwrap();
        let parsed: AbortReason = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, AbortReason::TurnLimit));
    }

    #[test]
    fn test_abort_reason_serde_approval_denied_roundtrip() {
        let reason = AbortReason::ApprovalDenied;
        let json = serde_json::to_string(&reason).unwrap();
        let parsed: AbortReason = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, AbortReason::ApprovalDenied));
    }

    #[test]
    fn test_abort_reason_serde_shutdown_roundtrip() {
        let reason = AbortReason::Shutdown;
        let json = serde_json::to_string(&reason).unwrap();
        let parsed: AbortReason = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, AbortReason::Shutdown));
    }

    #[test]
    fn test_abort_reason_clone_error() {
        let reason = AbortReason::Error {
            message: "test error".to_string(),
        };
        let cloned = reason.clone();
        if let AbortReason::Error { message } = cloned {
            assert_eq!(message, "test error");
        } else {
            panic!("Expected Error");
        }
    }

    // --- ModelInfo tests ---

    #[test]
    fn test_model_info_clone_with_reasoning() {
        let info = ModelInfo {
            id: "model-1".to_string(),
            name: "Model One".to_string(),
            provider: "provider".to_string(),
            supports_reasoning: true,
            max_context: Some(128000),
        };
        let cloned = info.clone();
        assert_eq!(cloned.id, "model-1");
        assert!(cloned.supports_reasoning);
    }

    #[test]
    fn test_model_info_no_max_context_no_reasoning() {
        let info = ModelInfo {
            id: "m".to_string(),
            name: "N".to_string(),
            provider: "p".to_string(),
            supports_reasoning: false,
            max_context: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: ModelInfo = serde_json::from_str(&json).unwrap();
        assert!(parsed.max_context.is_none());
        assert!(!parsed.supports_reasoning);
    }

    // --- McpToolInfo tests ---

    #[test]
    fn test_mcp_tool_info_clone_with_description() {
        let tool = McpToolInfo {
            qualified_name: "mcp__server__tool".to_string(),
            name: "tool".to_string(),
            server: "server".to_string(),
            description: Some("A tool".to_string()),
        };
        let cloned = tool.clone();
        assert_eq!(cloned.qualified_name, "mcp__server__tool");
    }

    #[test]
    fn test_mcp_tool_info_no_description_serde() {
        let tool = McpToolInfo {
            qualified_name: "mcp__s__t".to_string(),
            name: "t".to_string(),
            server: "s".to_string(),
            description: None,
        };
        let json = serde_json::to_string(&tool).unwrap();
        let parsed: McpToolInfo = serde_json::from_str(&json).unwrap();
        assert!(parsed.description.is_none());
    }

    // --- Event variant tests ---

    #[test]
    fn test_event_session_configured_serde() {
        let event = Event::SessionConfigured {
            session_id: "sess-1".to_string(),
            model: "gpt-4".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Event::SessionConfigured { .. }));
    }

    #[test]
    fn test_event_models_available_empty_vec() {
        let event = Event::ModelsAvailable { models: vec![] };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::ModelsAvailable { models } = parsed {
            assert!(models.is_empty());
        } else {
            panic!("Expected ModelsAvailable");
        }
    }

    #[test]
    fn test_event_models_available_multiple_models() {
        let models = vec![
            ModelInfo {
                id: "m1".to_string(),
                name: "Model 1".to_string(),
                provider: "p1".to_string(),
                supports_reasoning: false,
                max_context: None,
            },
            ModelInfo {
                id: "m2".to_string(),
                name: "Model 2".to_string(),
                provider: "p2".to_string(),
                supports_reasoning: true,
                max_context: Some(32000),
            },
        ];
        let event = Event::ModelsAvailable { models };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::ModelsAvailable { models } = parsed {
            assert_eq!(models.len(), 2);
        } else {
            panic!("Expected ModelsAvailable");
        }
    }

    #[test]
    fn test_event_mcp_tools_available_empty_vec() {
        let event = Event::McpToolsAvailable { tools: vec![] };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::McpToolsAvailable { tools } = parsed {
            assert!(tools.is_empty());
        } else {
            panic!("Expected McpToolsAvailable");
        }
    }

    #[test]
    fn test_event_tool_complete_failed_n298() {
        let event = Event::ToolComplete {
            tool: "shell".to_string(),
            call_id: "c1".to_string(),
            success: false,
            result: "exit code 1".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::ToolComplete { success, .. } = parsed {
            assert!(!success);
        } else {
            panic!("Expected ToolComplete");
        }
    }

    #[test]
    fn test_event_patch_approval_request_n298() {
        let event = Event::PatchApprovalRequest {
            id: "patch-1".to_string(),
            file: PathBuf::from("/test.rs"),
            patch: "+new line\n-old line".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("PatchApprovalRequest"));
    }

    #[test]
    fn test_event_token_usage() {
        let event = Event::TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cost_usd: Some(0.05),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::TokenUsage {
            input_tokens,
            output_tokens,
            cost_usd,
        } = parsed
        {
            assert_eq!(input_tokens, 1000);
            assert_eq!(output_tokens, 500);
            assert_eq!(cost_usd, Some(0.05));
        } else {
            panic!("Expected TokenUsage");
        }
    }

    #[test]
    fn test_event_undo_started() {
        let event = Event::UndoStarted;
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Event::UndoStarted));
    }

    #[test]
    fn test_event_undo_complete() {
        let event = Event::UndoComplete {
            description: "Reverted last change".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        if let Event::UndoComplete { description } = parsed {
            assert_eq!(description, "Reverted last change");
        } else {
            panic!("Expected UndoComplete");
        }
    }

    #[test]
    fn test_event_shutdown_complete() {
        let event = Event::ShutdownComplete;
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Event::ShutdownComplete));
    }
}
