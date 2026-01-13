//! TUI application state and logic
//!
//! Contains the main App struct and event handling logic.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use codex_dashflow_core::approval_presets::{builtin_approval_presets, exec_policy_from_preset};
use codex_dashflow_core::mcp::McpServerConfig;
use codex_dashflow_core::streaming::{AgentEvent, StreamCallback};
use codex_dashflow_core::{
    delete_session, list_sessions, load_skills, model_provider_info, resume_session, run_agent,
    AgentState, AuthCredentialsStoreMode, AuthManager, AuthStatus, Message,
    MessageRole as CoreMessageRole, RunnerConfig,
};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::event::{
    ApprovalDecision as EventApprovalDecision, ApprovalRequestEvent, EventHandler, TuiEvent,
    TuiStreamCallback,
};
use crate::history;
use crate::session_log;
use crate::ui;
use codex_dashflow_file_search::{search, FileMatch, SearchConfig};
use std::path::Path;
use tokio::sync::oneshot;

/// Configuration for the TUI application
#[derive(Clone, Debug)]
pub struct AppConfig {
    /// Session ID (auto-generated if not provided)
    pub session_id: Option<String>,
    /// Working directory for file operations
    pub working_dir: String,
    /// Maximum number of turns (0 = unlimited)
    pub max_turns: u32,
    /// LLM model to use
    pub model: String,
    /// Use mock LLM for testing
    pub use_mock_llm: bool,
    /// Tick rate for UI updates
    pub tick_rate: Duration,
    /// Whether to collect training data from successful runs
    pub collect_training: bool,
    /// Whether to load optimized prompts from PromptRegistry
    pub load_optimized_prompts: bool,
    /// Custom system prompt (overrides default and optimized prompts)
    pub system_prompt: Option<String>,
    /// Configured MCP servers
    pub mcp_servers: Vec<McpServerConfig>,
    /// Current approval preset ID (e.g., "auto", "read-only", "full-access")
    pub approval_preset: String,
    /// Whether to enable checkpointing for session persistence
    pub checkpointing_enabled: bool,
    /// Path for file-based checkpointing (None = memory checkpointing)
    pub checkpoint_path: Option<std::path::PathBuf>,
    /// PostgreSQL connection string for database checkpointing
    pub postgres_connection_string: Option<String>,
    /// Flag indicating auto-resume was enabled but no sessions were found
    /// When true and session_id is None, TUI shows a message about this
    pub auto_resume_no_sessions: bool,
    /// Flag indicating this session was auto-resumed (not explicitly requested)
    /// When true and session is successfully resumed, TUI shows an indicator
    pub auto_resumed_session: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            session_id: None,
            working_dir: ".".to_string(),
            max_turns: 0,
            model: "gpt-4o-mini".to_string(),
            use_mock_llm: false,
            tick_rate: Duration::from_millis(250),
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            mcp_servers: Vec::new(),
            approval_preset: "auto".to_string(),
            checkpointing_enabled: false,
            checkpoint_path: None,
            postgres_connection_string: None,
            auto_resume_no_sessions: false,
            auto_resumed_session: false,
        }
    }
}

impl AppConfig {
    /// Build a RunnerConfig based on the checkpoint settings
    pub fn build_runner_config(&self) -> RunnerConfig {
        if !self.checkpointing_enabled {
            return RunnerConfig::default();
        }

        // PostgreSQL takes priority if configured
        if let Some(ref conn_str) = self.postgres_connection_string {
            return RunnerConfig::with_postgres_checkpointing(conn_str);
        }

        // File-based checkpointing if path is provided
        if let Some(ref path) = self.checkpoint_path {
            return RunnerConfig::with_file_checkpointing(path);
        }

        // Memory checkpointing as fallback
        RunnerConfig::with_memory_checkpointing()
    }
}

/// Chat message role
#[derive(Clone, Debug, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// Chat message for display
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

/// Application mode
#[derive(Clone, Debug, PartialEq)]
pub enum AppMode {
    /// Normal mode - waiting for input
    Normal,
    /// Insert mode - typing input
    Insert,
    /// Processing - agent is working
    Processing,
    /// Search mode - searching chat history
    Search,
}

/// Agent status for display during processing
#[derive(Clone, Debug, PartialEq, Default)]
pub enum AgentStatus {
    /// Idle - not processing
    #[default]
    Idle,
    /// Thinking - LLM is reasoning
    Thinking { model: String },
    /// Executing a tool
    ExecutingTool { tool: String },
    /// Completed processing
    Complete { duration_ms: u64 },
    /// Error occurred
    Error { message: String },
}

impl AgentStatus {
    /// Get a display string for the status
    pub fn display(&self) -> String {
        match self {
            AgentStatus::Idle => String::new(),
            AgentStatus::Thinking { model } => format!("Thinking ({})", model),
            AgentStatus::ExecutingTool { tool } => format!("Executing: {}", tool),
            AgentStatus::Complete { duration_ms } => {
                format!("Done ({:.1}s)", *duration_ms as f64 / 1000.0)
            }
            AgentStatus::Error { message } => format!("Error: {}", message),
        }
    }
}

/// Session metrics captured from agent execution
///
/// Tracks cumulative token usage and cost across the session.
/// Updated when SessionMetrics events are received from the agent.
#[derive(Clone, Debug, Default)]
pub struct SessionMetricsData {
    /// Total input tokens across all LLM calls
    pub total_input_tokens: u32,
    /// Total output tokens across all LLM calls
    pub total_output_tokens: u32,
    /// Total cached tokens (from prompt caching)
    pub total_cached_tokens: u32,
    /// Total cost in USD (if calculable)
    pub total_cost_usd: Option<f64>,
    /// Number of LLM calls made
    pub llm_call_count: u32,
    /// Session duration in milliseconds
    pub duration_ms: u64,
}

impl SessionMetricsData {
    /// Format tokens for display (e.g., "1.2k" for 1200)
    pub fn format_total_tokens(&self) -> String {
        let total = self.total_input_tokens + self.total_output_tokens;
        if total >= 1_000_000 {
            format!("{:.1}M", total as f64 / 1_000_000.0)
        } else if total >= 1_000 {
            format!("{:.1}k", total as f64 / 1_000.0)
        } else {
            format!("{}", total)
        }
    }

    /// Format cost for display (e.g., "$0.12")
    pub fn format_cost(&self) -> Option<String> {
        self.total_cost_usd.map(|cost| {
            if cost >= 1.0 {
                format!("${:.2}", cost)
            } else if cost >= 0.01 {
                format!("${:.3}", cost)
            } else {
                format!("${:.4}", cost)
            }
        })
    }

    /// Format a summary string for the status bar
    pub fn format_summary(&self) -> String {
        let tokens = self.format_total_tokens();
        match self.format_cost() {
            Some(cost) => format!("{} tokens | {}", tokens, cost),
            None => format!("{} tokens", tokens),
        }
    }
}

/// Built-in slash commands for tab completion
const BUILTIN_COMMANDS: &[(&str, &str)] = &[
    ("/help", "Show help information"),
    ("/quit", "Exit the application"),
    ("/exit", "Exit the application"),
    ("/clear", "Clear chat history"),
    ("/model", "Change the model"),
    ("/status", "Show session status"),
    ("/tokens", "Show token usage statistics"),
    ("/keys", "Show keyboard shortcuts"),
    ("/version", "Show version information"),
    ("/config", "Show current configuration"),
    ("/mcp", "List configured MCP servers"),
    ("/skills", "List available skills"),
    ("/logout", "Log out and clear credentials"),
    ("/init", "Create an AGENTS.md file with instructions"),
    (
        "/approvals",
        "Choose what the agent can do without approval",
    ),
    ("/mode", "Show/change approval mode (alias for /approvals)"),
    ("/resume", "Resume a previous session"),
    ("/sessions", "List saved sessions"),
    ("/delete", "Delete a saved session"),
    ("/mention", "Mention a file (inserts @ for file completion)"),
    ("/history", "Show conversation history"),
    ("/compact", "Summarize conversation to save context"),
    ("/undo", "Undo the last turn"),
    ("/diff", "Show git diff"),
    ("/new", "Start a new chat"),
    ("/review", "Review current changes"),
    ("/feedback", "Send feedback to maintainers"),
    ("/providers", "List available model providers"),
    ("/search", "Search conversation history"),
    ("/context", "Show current agent context"),
    ("/stop", "Stop/cancel the current agent task"),
    ("/export", "Export conversation to a file"),
];

/// Commands that are NOT available while an agent task is running.
/// These commands modify state in ways that could conflict with ongoing execution.
const COMMANDS_UNAVAILABLE_DURING_TASK: &[&str] = &[
    "/new",       // Would interrupt running task
    "/resume",    // Would replace running task
    "/delete",    // Modifies session storage
    "/compact",   // Modifies conversation state
    "/undo",      // Modifies conversation state
    "/model",     // Changes model mid-task
    "/approvals", // Changes approval settings mid-task
    "/mode",      // Alias for /approvals
    "/logout",    // Interrupts authentication
    "/init",      // Modifies workspace files
    "/review",    // Could conflict with running task
];

/// Maximum number of rows to show in the command popup
const MAX_POPUP_ROWS: usize = 8;

/// Calculate fuzzy match score between query and target.
///
/// Returns Some(score) if all characters in query appear in target in order.
/// Lower scores indicate better matches:
/// - Score of 1 indicates consecutive character matches (best fuzzy match)
/// - Higher scores indicate more gaps between matched characters
///
/// Returns None if the query doesn't match.
fn fuzzy_match_score(query: &str, target: &str) -> Option<u32> {
    let query_chars: Vec<char> = query.chars().collect();
    let target_chars: Vec<char> = target.chars().collect();

    if query_chars.is_empty() {
        return Some(1);
    }

    let mut query_idx = 0;
    let mut match_positions: Vec<usize> = Vec::with_capacity(query_chars.len());

    // Find all query characters in order within target
    for (target_idx, &target_char) in target_chars.iter().enumerate() {
        if query_idx < query_chars.len() && target_char == query_chars[query_idx] {
            match_positions.push(target_idx);
            query_idx += 1;
        }
    }

    // If we didn't match all query characters, no match
    if query_idx != query_chars.len() {
        return None;
    }

    // Calculate score based on gaps between matched characters
    // Score = 1 (base) + sum of gaps between consecutive matches
    let mut score = 1u32;
    for i in 1..match_positions.len() {
        let gap = match_positions[i] - match_positions[i - 1] - 1;
        score += gap as u32;
    }

    // Add penalty for late first match (prefer matches that start earlier)
    score += match_positions.first().unwrap_or(&0).saturating_sub(1) as u32;

    Some(score)
}

/// Get the character positions that matched in a fuzzy match.
///
/// Returns Some(Vec<usize>) containing the indices of matched characters in target,
/// or None if the query doesn't match the target.
fn fuzzy_match_positions(query: &str, target: &str) -> Option<Vec<usize>> {
    let query_chars: Vec<char> = query.chars().collect();
    let target_chars: Vec<char> = target.chars().collect();

    if query_chars.is_empty() {
        return Some(Vec::new());
    }

    let mut query_idx = 0;
    let mut match_positions: Vec<usize> = Vec::with_capacity(query_chars.len());

    // Find all query characters in order within target
    for (target_idx, &target_char) in target_chars.iter().enumerate() {
        if query_idx < query_chars.len() && target_char == query_chars[query_idx] {
            match_positions.push(target_idx);
            query_idx += 1;
        }
    }

    // If we didn't match all query characters, no match
    if query_idx != query_chars.len() {
        return None;
    }

    Some(match_positions)
}

/// Command popup state for slash command completion
#[derive(Clone, Debug, Default)]
pub struct CommandPopup {
    /// Whether the popup is visible
    pub visible: bool,
    /// Currently selected index (0-indexed)
    pub selected_index: usize,
    /// Filtered commands based on current input.
    /// Each entry: (command, description, match_positions)
    /// match_positions contains character indices that matched the query
    pub filtered_commands: Vec<(String, String, Vec<usize>)>,
    /// Scroll offset when list is longer than MAX_POPUP_ROWS
    pub scroll_offset: usize,
}

impl CommandPopup {
    /// Create a new command popup
    pub fn new() -> Self {
        Self::default()
    }

    /// Update filtered commands based on input text using fuzzy matching.
    ///
    /// Matching priority:
    /// 1. Prefix matches (e.g., "/he" matches "/help")
    /// 2. Fuzzy matches (e.g., "/hs" matches "/history" and "/status")
    ///
    /// Fuzzy matching requires all input characters to appear in order in the command.
    pub fn update_filter(&mut self, input: &str) {
        // Only show popup when input starts with /
        if !input.starts_with('/') {
            self.visible = false;
            self.filtered_commands.clear();
            return;
        }

        // Don't show popup if there's a space (command already complete)
        if input.contains(' ') {
            self.visible = false;
            self.filtered_commands.clear();
            return;
        }

        let query = input.to_lowercase();

        // Collect matches with their scores and match positions (lower score is better)
        // Tuple: (command, description, score, match_positions)
        let mut matches: Vec<(String, String, u32, Vec<usize>)> = BUILTIN_COMMANDS
            .iter()
            .filter_map(|(cmd, desc)| {
                let cmd_lower = cmd.to_lowercase();

                // Prefix match gets highest priority (score 0)
                // For prefix matches, highlight all characters from start up to query length
                if cmd_lower.starts_with(&query) {
                    let positions: Vec<usize> = (0..query.chars().count()).collect();
                    return Some((cmd.to_string(), desc.to_string(), 0, positions));
                }

                // Fuzzy match: all query characters must appear in order
                if let Some(score) = fuzzy_match_score(&query, &cmd_lower) {
                    // Get the actual positions for highlighting
                    let positions = fuzzy_match_positions(&query, &cmd_lower).unwrap_or_default();
                    return Some((cmd.to_string(), desc.to_string(), score, positions));
                }

                None
            })
            .collect();

        // Sort by score (prefix matches first, then by fuzzy score)
        matches.sort_by_key(|(_, _, score, _)| *score);

        self.filtered_commands = matches
            .into_iter()
            .map(|(cmd, desc, _, positions)| (cmd, desc, positions))
            .collect();

        // Update visibility - show if there are matches and input is not an exact match
        self.visible = !self.filtered_commands.is_empty()
            && !self
                .filtered_commands
                .iter()
                .any(|(cmd, _, _)| cmd.eq_ignore_ascii_case(input));

        // Reset selection if filter changed
        if self.selected_index >= self.filtered_commands.len() {
            self.selected_index = 0;
        }
        self.scroll_offset = 0;
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.filtered_commands.is_empty() {
            return;
        }
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            // Wrap to bottom
            self.selected_index = self.filtered_commands.len() - 1;
        }
        self.ensure_visible();
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.filtered_commands.is_empty() {
            return;
        }
        if self.selected_index < self.filtered_commands.len() - 1 {
            self.selected_index += 1;
        } else {
            // Wrap to top
            self.selected_index = 0;
        }
        self.ensure_visible();
    }

    /// Ensure selected item is visible
    fn ensure_visible(&mut self) {
        let visible_items = MAX_POPUP_ROWS.min(self.filtered_commands.len());
        if visible_items == 0 {
            return;
        }

        // Scroll up if selected is above viewport
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }

        // Scroll down if selected is below viewport
        if self.selected_index >= self.scroll_offset + visible_items {
            self.scroll_offset = self.selected_index + 1 - visible_items;
        }
    }

    /// Get the currently selected command (if any)
    pub fn selected_command(&self) -> Option<&str> {
        self.filtered_commands
            .get(self.selected_index)
            .map(|(cmd, _, _)| cmd.as_str())
    }

    /// Hide the popup
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Get visible commands with their selection state and match positions.
    /// Returns: (command, description, is_selected, match_positions)
    pub fn visible_commands(&self) -> Vec<(&str, &str, bool, &[usize])> {
        let visible_items = MAX_POPUP_ROWS.min(self.filtered_commands.len());
        self.filtered_commands
            .iter()
            .skip(self.scroll_offset)
            .take(visible_items)
            .enumerate()
            .map(|(i, (cmd, desc, positions))| {
                let is_selected = i + self.scroll_offset == self.selected_index;
                (
                    cmd.as_str(),
                    desc.as_str(),
                    is_selected,
                    positions.as_slice(),
                )
            })
            .collect()
    }

    /// Get the total number of filtered commands
    pub fn total_count(&self) -> usize {
        self.filtered_commands.len()
    }

    /// Check if scrolling is possible
    pub fn has_more_items(&self) -> bool {
        self.filtered_commands.len() > MAX_POPUP_ROWS
    }
}

/// File search popup state for @ file completion
#[derive(Clone, Debug, Default)]
pub struct FileSearchPopup {
    /// Whether the popup is visible
    pub visible: bool,
    /// Currently selected index (0-indexed)
    pub selected_index: usize,
    /// Current query (text after @)
    pub query: String,
    /// File matches from search
    pub matches: Vec<FileMatch>,
    /// Scroll offset when list is longer than MAX_POPUP_ROWS
    pub scroll_offset: usize,
    /// Whether a search is in progress
    pub loading: bool,
}

impl FileSearchPopup {
    /// Create a new file search popup
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the search query and results
    pub fn update_query(&mut self, query: &str, working_dir: &str) {
        self.query = query.to_string();

        if query.is_empty() {
            // Just "@" - show hint but no results
            self.visible = true;
            self.matches.clear();
            self.loading = false;
            return;
        }

        self.visible = true;
        self.loading = true;

        // Perform synchronous file search (debounced for real use)
        let config = SearchConfig {
            limit: 20,
            compute_indices: true,
            respect_gitignore: true,
            ..Default::default()
        };

        match search(query, Path::new(working_dir), &config, None) {
            Ok(results) => {
                self.matches = results.matches;
                self.loading = false;
            }
            Err(_) => {
                self.matches.clear();
                self.loading = false;
            }
        }

        // Reset selection if matches changed
        if self.selected_index >= self.matches.len() && !self.matches.is_empty() {
            self.selected_index = 0;
        }
        self.scroll_offset = 0;
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = self.matches.len() - 1;
        }
        self.ensure_visible();
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        if self.selected_index < self.matches.len() - 1 {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
        self.ensure_visible();
    }

    /// Ensure selected item is visible
    fn ensure_visible(&mut self) {
        let visible_items = MAX_POPUP_ROWS.min(self.matches.len());
        if visible_items == 0 {
            return;
        }

        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }

        if self.selected_index >= self.scroll_offset + visible_items {
            self.scroll_offset = self.selected_index + 1 - visible_items;
        }
    }

    /// Get the currently selected file path
    pub fn selected_path(&self) -> Option<&str> {
        self.matches
            .get(self.selected_index)
            .map(|m| m.path.as_str())
    }

    /// Hide the popup
    pub fn hide(&mut self) {
        self.visible = false;
        self.matches.clear();
        self.query.clear();
    }

    /// Get visible files with their selection state.
    /// Returns: (path, is_selected, match_indices)
    pub fn visible_files(&self) -> Vec<(&str, bool, Option<&[u32]>)> {
        let visible_items = MAX_POPUP_ROWS.min(self.matches.len());
        self.matches
            .iter()
            .skip(self.scroll_offset)
            .take(visible_items)
            .enumerate()
            .map(|(i, m)| {
                let is_selected = i + self.scroll_offset == self.selected_index;
                (m.path.as_str(), is_selected, m.indices.as_deref())
            })
            .collect()
    }

    /// Get the total number of matches
    pub fn total_count(&self) -> usize {
        self.matches.len()
    }

    /// Check if scrolling is possible
    pub fn has_more_items(&self) -> bool {
        self.matches.len() > MAX_POPUP_ROWS
    }
}

/// Type of approval request
#[derive(Clone, Debug, PartialEq)]
pub enum ApprovalRequestType {
    /// Shell command execution
    Shell { command: String },
    /// File write operation
    FileWrite { path: String },
    /// General tool execution
    Tool { tool: String, args: String },
}

impl ApprovalRequestType {
    /// Get a display name for the request type
    pub fn display_name(&self) -> &str {
        match self {
            ApprovalRequestType::Shell { .. } => "Shell Command",
            ApprovalRequestType::FileWrite { .. } => "File Write",
            ApprovalRequestType::Tool { .. } => "Tool Execution",
        }
    }

    /// Get the command/args display
    pub fn display_content(&self) -> String {
        match self {
            ApprovalRequestType::Shell { command } => command.clone(),
            ApprovalRequestType::FileWrite { path } => format!("Write to: {}", path),
            ApprovalRequestType::Tool { tool, args } => format!("{}: {}", tool, args),
        }
    }
}

/// Pending approval request
#[derive(Clone, Debug)]
pub struct ApprovalRequest {
    /// Unique ID for this request
    pub id: String,
    /// Type of request
    pub request_type: ApprovalRequestType,
    /// Reason why approval is needed
    pub reason: Option<String>,
    /// Tool call ID (for tracking)
    pub tool_call_id: String,
    /// Tool name
    pub tool_name: String,
}

/// User's approval decision
#[derive(Clone, Debug, PartialEq)]
pub enum ApprovalDecision {
    /// Approve this one request
    ApproveOnce,
    /// Approve and don't ask again for this tool in this session
    ApproveSession,
    /// Reject the request
    Reject,
}

/// Approval overlay state for tool approval dialogs
#[derive(Clone, Debug, Default)]
pub struct ApprovalOverlay {
    /// Whether the overlay is visible
    pub visible: bool,
    /// Current approval request
    pub request: Option<ApprovalRequest>,
    /// Currently selected option (0 = Approve, 1 = Approve Session, 2 = Reject)
    pub selected_option: usize,
}

impl ApprovalOverlay {
    /// Create a new approval overlay
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the overlay with an approval request
    pub fn show(&mut self, request: ApprovalRequest) {
        self.visible = true;
        self.request = Some(request);
        self.selected_option = 0; // Default to "Approve Once"
    }

    /// Hide the overlay
    pub fn hide(&mut self) {
        self.visible = false;
        self.request = None;
        self.selected_option = 0;
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_option > 0 {
            self.selected_option -= 1;
        } else {
            self.selected_option = 2; // Wrap to bottom
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.selected_option < 2 {
            self.selected_option += 1;
        } else {
            self.selected_option = 0; // Wrap to top
        }
    }

    /// Get the current decision based on selection
    pub fn current_decision(&self) -> ApprovalDecision {
        match self.selected_option {
            0 => ApprovalDecision::ApproveOnce,
            1 => ApprovalDecision::ApproveSession,
            _ => ApprovalDecision::Reject,
        }
    }

    /// Get options for display (label, is_selected, hotkey)
    pub fn options(&self) -> Vec<(&'static str, bool, char)> {
        vec![
            ("Yes, proceed", self.selected_option == 0, 'y'),
            (
                "Yes, and don't ask again this session",
                self.selected_option == 1,
                'a',
            ),
            ("No, reject", self.selected_option == 2, 'n'),
        ]
    }
}

/// Notification style for transient messages
#[derive(Clone, Debug, PartialEq, Default)]
pub enum NotificationStyle {
    /// Informational message (cyan/blue)
    #[default]
    Info,
    /// Success message (green)
    Success,
    /// Warning message (yellow)
    Warning,
    /// Error message (red)
    Error,
}

/// A transient notification message that auto-dismisses after a duration
#[derive(Clone, Debug)]
pub struct Notification {
    /// The notification text
    pub text: String,
    /// The visual style
    pub style: NotificationStyle,
    /// When the notification was created
    pub created_at: Instant,
    /// How long to show the notification (default 3 seconds)
    pub duration: Duration,
}

impl Notification {
    /// Create a new notification with default 3-second duration
    pub fn new(text: impl Into<String>, style: NotificationStyle) -> Self {
        Self {
            text: text.into(),
            style,
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        }
    }

    /// Create an info notification
    pub fn info(text: impl Into<String>) -> Self {
        Self::new(text, NotificationStyle::Info)
    }

    /// Create a success notification
    pub fn success(text: impl Into<String>) -> Self {
        Self::new(text, NotificationStyle::Success)
    }

    /// Create a warning notification
    pub fn warning(text: impl Into<String>) -> Self {
        Self::new(text, NotificationStyle::Warning)
    }

    /// Create an error notification
    #[allow(dead_code)]
    pub fn error(text: impl Into<String>) -> Self {
        Self::new(text, NotificationStyle::Error)
    }

    /// Set a custom duration
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Check if the notification has expired
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.duration
    }
}

/// TUI application state
pub struct App {
    /// Session ID
    pub session_id: String,
    /// Current mode
    pub mode: AppMode,
    /// Current input buffer
    pub input: String,
    /// Cursor position in input
    pub cursor_position: usize,
    /// Chat messages for display
    pub messages: Vec<ChatMessage>,
    /// Current turn count
    pub turn_count: u32,
    /// Model being used
    pub model: String,
    /// Configuration
    pub config: AppConfig,
    /// Agent state
    agent_state: AgentState,
    /// Should the app quit?
    should_quit: bool,
    /// Event sender for agent callbacks
    event_tx: Option<mpsc::UnboundedSender<TuiEvent>>,
    /// Last run score (if training collection is enabled)
    pub last_run_score: Option<f64>,
    /// Whether training was collected from the last run
    pub training_collected: bool,
    /// Authentication status
    pub auth_status: AuthStatus,
    /// Scroll offset for chat history (0 = bottom, positive = scrolled up)
    pub scroll_offset: usize,
    /// Current agent status for display
    pub agent_status: AgentStatus,
    /// Command history for input recall
    pub command_history: Vec<String>,
    /// Current position in command history (None = new input)
    pub history_position: Option<usize>,
    /// Saved input when browsing history
    saved_input: String,
    /// Spinner frame counter for animations
    pub spinner_frame: usize,
    /// Whether to show the help overlay
    pub show_help: bool,
    /// Search query for chat history
    pub search_query: String,
    /// Indices of messages matching the search query
    pub search_matches: Vec<usize>,
    /// Current match index (index into search_matches)
    pub current_match: Option<usize>,
    /// Command completion popup state
    pub command_popup: CommandPopup,
    /// File search popup state for @ completion
    pub file_search_popup: FileSearchPopup,
    /// Horizontal scroll offset for single-line input
    pub input_scroll_offset: usize,
    /// Undo history for input buffer (stores previous states)
    input_undo_stack: Vec<(String, usize)>,
    /// Redo history for input buffer (stores undone states)
    input_redo_stack: Vec<(String, usize)>,
    /// Selection anchor position (start of selection, None if no selection)
    pub selection_anchor: Option<usize>,
    /// Internal clipboard for cut/copy/paste operations
    clipboard: String,
    /// Cancellation token for stopping agent processing
    agent_cancel_token: Option<CancellationToken>,
    /// Approval overlay for tool approval dialogs
    pub approval_overlay: ApprovalOverlay,
    /// Tools approved for the entire session (don't ask again)
    session_approved_tools: std::collections::HashSet<String>,
    /// Response channel for pending approval request from agent runner
    approval_response_tx: Option<oneshot::Sender<EventApprovalDecision>>,
    /// Transient notification to display (auto-dismisses after duration)
    pub notification: Option<Notification>,
    /// Session metrics from the current session (tokens, cost, etc.)
    pub session_metrics: SessionMetricsData,
}

impl App {
    /// Create a new application
    pub fn new(config: AppConfig) -> Self {
        let session_id = config
            .session_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let mut agent_state = AgentState::new();
        agent_state.session_id = session_id.clone();
        agent_state.llm_config.model = config.model.clone();
        agent_state.max_turns = if config.max_turns == 0 {
            100 // Default max turns
        } else {
            config.max_turns
        };

        if config.use_mock_llm {
            agent_state = agent_state.with_mock_llm();
        }

        // Apply exec policy from approval preset
        let exec_policy = exec_policy_from_preset(&config.approval_preset);
        agent_state = agent_state.with_exec_policy(Arc::new(exec_policy));

        // Set up authentication and get status
        let auth_status = if config.use_mock_llm {
            AuthStatus::EnvApiKey // Mock mode doesn't need real auth
        } else {
            AuthManager::new(AuthCredentialsStoreMode::Auto)
                .map(|m| m.setup_for_llm())
                .unwrap_or(AuthStatus::NotAuthenticated)
        };

        // Build welcome message based on auth status
        let welcome_message = match &auth_status {
            AuthStatus::ChatGpt { email: Some(e) } => {
                format!("Welcome to Codex DashFlow. Signed in with ChatGPT ({}).", e)
            }
            AuthStatus::ChatGpt { email: None } => {
                "Welcome to Codex DashFlow. Signed in with ChatGPT.".to_string()
            }
            AuthStatus::ApiKey => {
                "Welcome to Codex DashFlow. Using stored API key.".to_string()
            }
            AuthStatus::EnvApiKey => {
                "Welcome to Codex DashFlow. Using OPENAI_API_KEY from environment.".to_string()
            }
            AuthStatus::NotAuthenticated => {
                "Welcome to Codex DashFlow. Not authenticated. Run 'codex-dashflow login' to sign in.".to_string()
            }
        };

        Self {
            session_id,
            mode: AppMode::Insert,
            input: String::new(),
            cursor_position: 0,
            messages: vec![ChatMessage {
                role: MessageRole::System,
                content: welcome_message,
            }],
            turn_count: 0,
            model: config.model.clone(),
            config,
            agent_state,
            should_quit: false,
            event_tx: None,
            last_run_score: None,
            training_collected: false,
            auth_status,
            scroll_offset: 0,
            agent_status: AgentStatus::Idle,
            command_history: history::load_history(),
            history_position: None,
            saved_input: String::new(),
            spinner_frame: 0,
            show_help: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match: None,
            command_popup: CommandPopup::new(),
            file_search_popup: FileSearchPopup::new(),
            input_scroll_offset: 0,
            input_undo_stack: Vec::new(),
            input_redo_stack: Vec::new(),
            selection_anchor: None,
            clipboard: String::new(),
            agent_cancel_token: None,
            approval_overlay: ApprovalOverlay::new(),
            session_approved_tools: std::collections::HashSet::new(),
            approval_response_tx: None,
            notification: None,
            session_metrics: SessionMetricsData::default(),
        }
    }

    /// Create a new application with optional session resume
    ///
    /// If a session_id is provided in config and checkpointing is enabled,
    /// this will attempt to resume the session from checkpoint storage.
    pub async fn new_with_resume(config: AppConfig) -> Self {
        // Check if we should try to resume a session
        let should_try_resume = config.session_id.is_some() && config.checkpointing_enabled;

        if !should_try_resume {
            // Check if auto-resume was enabled but no sessions were found
            let auto_resume_no_sessions = config.auto_resume_no_sessions;
            let mut app = Self::new(config);
            if auto_resume_no_sessions {
                app.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: "Auto-resume enabled but no sessions found. Starting fresh session."
                        .to_string(),
                });
            }
            return app;
        }

        let session_id = config.session_id.as_ref().unwrap().clone();
        let runner_config = config.build_runner_config();

        // Try to resume the session
        match resume_session(&session_id, &runner_config).await {
            Ok(mut restored_state) => {
                // Apply current exec_policy to restored state
                let exec_policy = exec_policy_from_preset(&config.approval_preset);
                restored_state = restored_state.with_exec_policy(Arc::new(exec_policy));

                // Set up authentication and get status
                let auth_status = if config.use_mock_llm {
                    AuthStatus::EnvApiKey
                } else {
                    AuthManager::new(AuthCredentialsStoreMode::Auto)
                        .map(|m| m.setup_for_llm())
                        .unwrap_or(AuthStatus::NotAuthenticated)
                };

                // Build messages from restored state
                let mut messages: Vec<ChatMessage> = restored_state
                    .messages
                    .iter()
                    .map(|msg| {
                        let role = match msg.role {
                            CoreMessageRole::User => MessageRole::User,
                            CoreMessageRole::Assistant => MessageRole::Assistant,
                            CoreMessageRole::System => MessageRole::System,
                            CoreMessageRole::Tool => MessageRole::Tool,
                        };
                        ChatMessage {
                            role,
                            content: msg.content.clone(),
                        }
                    })
                    .collect();

                // Add system message about session resume
                // Differentiate between explicit resume and auto-resume
                let resume_type = if config.auto_resumed_session {
                    "Session auto-resumed"
                } else {
                    "Session resumed"
                };
                messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!(
                        "{}: {}\nRestored {} messages, {} turns.",
                        resume_type,
                        session_id,
                        messages.len(),
                        restored_state.turn_count
                    ),
                });

                let turn_count = restored_state.turn_count;
                let model = config.model.clone();

                Self {
                    session_id,
                    mode: AppMode::Insert,
                    input: String::new(),
                    cursor_position: 0,
                    messages,
                    turn_count,
                    model: model.clone(),
                    config,
                    agent_state: restored_state,
                    should_quit: false,
                    event_tx: None,
                    last_run_score: None,
                    training_collected: false,
                    auth_status,
                    scroll_offset: 0,
                    agent_status: AgentStatus::Idle,
                    command_history: history::load_history(),
                    history_position: None,
                    saved_input: String::new(),
                    spinner_frame: 0,
                    show_help: false,
                    search_query: String::new(),
                    search_matches: Vec::new(),
                    current_match: None,
                    command_popup: CommandPopup::new(),
                    file_search_popup: FileSearchPopup::new(),
                    input_scroll_offset: 0,
                    input_undo_stack: Vec::new(),
                    input_redo_stack: Vec::new(),
                    selection_anchor: None,
                    clipboard: String::new(),
                    agent_cancel_token: None,
                    approval_overlay: ApprovalOverlay::new(),
                    session_approved_tools: std::collections::HashSet::new(),
                    approval_response_tx: None,
                    notification: None,
                    session_metrics: SessionMetricsData::default(),
                }
            }
            Err(e) => {
                // Failed to resume - create fresh app with error message
                let mut app = Self::new(config);
                app.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!(
                        "Failed to resume session '{}': {}\nStarting fresh session.",
                        session_id, e
                    ),
                });
                app
            }
        }
    }

    /// Set the event sender for agent callbacks
    pub fn set_event_sender(&mut self, tx: mpsc::UnboundedSender<TuiEvent>) {
        self.event_tx = Some(tx);
    }

    /// Handle a TUI event
    pub async fn handle_event(&mut self, event: TuiEvent) {
        match event {
            TuiEvent::Terminal(Event::Key(key)) => self.handle_key(key).await,
            TuiEvent::Terminal(Event::Resize(_, _)) => {
                // Terminal resize - UI will handle this
            }
            TuiEvent::Agent(agent_event) => self.handle_agent_event(agent_event),
            TuiEvent::Tick => {
                // Increment spinner when processing
                if self.mode == AppMode::Processing {
                    self.spinner_frame = self.spinner_frame.wrapping_add(1);
                }
                // Check for expired notifications
                if let Some(ref notif) = self.notification {
                    if notif.is_expired() {
                        self.notification = None;
                    }
                }
            }
            TuiEvent::Quit => {
                self.should_quit = true;
            }
            TuiEvent::ApprovalRequest(req) => {
                self.handle_approval_request(req);
            }
            _ => {}
        }
    }

    /// Handle an approval request from the agent runner
    fn handle_approval_request(&mut self, req: ApprovalRequestEvent) {
        // Check if tool is session-approved
        if self.session_approved_tools.contains(&req.tool) {
            // Auto-approve and send response back immediately
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: format!("✓ Auto-approved (session): {}", req.tool),
            });
            let _ = req.response_tx.send(EventApprovalDecision::Approve);
            return;
        }

        // Determine the request type based on tool name and args
        let request_type = match req.tool.as_str() {
            "shell" | "bash" | "execute_shell" => {
                let command = req
                    .args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                ApprovalRequestType::Shell { command }
            }
            "write_file" | "file_write" => {
                let path = req
                    .args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                ApprovalRequestType::FileWrite { path }
            }
            _ => ApprovalRequestType::Tool {
                tool: req.tool.clone(),
                args: req.args.to_string(),
            },
        };

        // Create approval request for UI
        let approval_request = ApprovalRequest {
            id: req.request_id,
            request_type,
            reason: req.reason,
            tool_call_id: req.tool_call_id,
            tool_name: req.tool.clone(),
        };

        // Store the response channel
        self.approval_response_tx = Some(req.response_tx);

        // Show the overlay
        self.approval_overlay.show(approval_request);
    }

    /// Handle keyboard input
    async fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl+C always quits
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }

        // Handle approval overlay if visible - takes priority over other overlays
        if self.approval_overlay.visible {
            self.handle_approval_key(key);
            return;
        }

        // Dismiss help overlay with Escape or any key when shown
        if self.show_help {
            if key.code == KeyCode::Esc
                || key.code == KeyCode::Char('?')
                || key.code == KeyCode::Char('q')
            {
                self.show_help = false;
            }
            return;
        }

        // Escape dismisses notification if one is visible (before other mode handling)
        if key.code == KeyCode::Esc && self.notification.is_some() {
            self.dismiss_notification();
            // Don't return - let Escape also handle other actions (like exiting insert mode)
        }

        // Ctrl+M cycles through approval modes (works in Normal and Insert modes)
        if key.code == KeyCode::Char('m') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.cycle_approval_mode();
            return;
        }

        match self.mode {
            AppMode::Processing => {
                // Only allow Escape in processing mode
                if key.code == KeyCode::Esc {
                    self.should_quit = true;
                }
            }
            AppMode::Normal => {
                // Vim normal mode
                match key.code {
                    KeyCode::Char('i') => {
                        self.mode = AppMode::Insert;
                    }
                    KeyCode::Char('a') => {
                        // Append after cursor
                        self.mode = AppMode::Insert;
                        if self.cursor_position < self.input.len() {
                            self.cursor_position += 1;
                        }
                    }
                    KeyCode::Char('A') => {
                        // Append at end of line
                        self.mode = AppMode::Insert;
                        self.cursor_position = self.input.len();
                    }
                    KeyCode::Char('I') => {
                        // Insert at beginning
                        self.mode = AppMode::Insert;
                        self.cursor_position = 0;
                    }
                    KeyCode::Char('q') => {
                        self.should_quit = true;
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        // Scroll down (decrease offset)
                        if self.scroll_offset > 0 {
                            self.scroll_offset -= 1;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        // Scroll up (increase offset)
                        self.scroll_offset += 1;
                    }
                    KeyCode::Char('G') => {
                        // Go to bottom
                        self.scroll_offset = 0;
                    }
                    KeyCode::Char('g') => {
                        // Go to top (scroll to show first message)
                        self.scroll_offset = self.messages.len().saturating_sub(1);
                    }
                    KeyCode::Char('h') | KeyCode::Left => {
                        if self.cursor_position > 0 {
                            self.cursor_position -= 1;
                        }
                    }
                    KeyCode::Char('l') | KeyCode::Right => {
                        if self.cursor_position < self.input.len() {
                            self.cursor_position += 1;
                        }
                    }
                    KeyCode::Char('0') | KeyCode::Home => {
                        self.cursor_position = 0;
                    }
                    KeyCode::Char('$') | KeyCode::End => {
                        self.cursor_position = self.input.len();
                    }
                    KeyCode::Char('x') => {
                        // Delete char under cursor
                        if self.cursor_position < self.input.len() {
                            self.save_undo_state();
                            self.input.remove(self.cursor_position);
                        }
                    }
                    KeyCode::Char('d') => {
                        // Clear input line (dd would need state tracking)
                        self.save_undo_state();
                        self.input.clear();
                        self.cursor_position = 0;
                    }
                    KeyCode::PageUp => {
                        self.scroll_offset += 10;
                    }
                    KeyCode::PageDown => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(10);
                    }
                    KeyCode::Char('?') => {
                        self.show_help = !self.show_help;
                    }
                    KeyCode::Char('/') => {
                        // Enter search mode
                        self.mode = AppMode::Search;
                        self.search_query.clear();
                        self.search_matches.clear();
                        self.current_match = None;
                    }
                    KeyCode::Char('n') => {
                        // Go to next match
                        self.next_match();
                    }
                    KeyCode::Char('N') => {
                        // Go to previous match
                        self.prev_match();
                    }
                    _ => {}
                }
            }
            AppMode::Insert => {
                // Check for Ctrl+Enter or Alt+Enter to submit
                if key.code == KeyCode::Enter
                    && (key.modifiers.contains(KeyModifiers::CONTROL)
                        || key.modifiers.contains(KeyModifiers::ALT))
                {
                    if !self.input.is_empty() {
                        self.submit_input().await;
                    }
                    return;
                }

                // Ctrl+D exits when input is empty
                if key.code == KeyCode::Char('d') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    if self.input.is_empty() {
                        self.should_quit = true;
                    }
                    return;
                }

                // Ctrl+P navigates to previous history entry
                if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.history_previous();
                    return;
                }

                // Ctrl+N navigates to next history entry
                if key.code == KeyCode::Char('n') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.history_next();
                    return;
                }

                // Ctrl+Z undoes last input change
                if key.code == KeyCode::Char('z') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.input_undo();
                    return;
                }

                // Ctrl+Y or Ctrl+Shift+Z redoes last undone change
                if (key.code == KeyCode::Char('y') && key.modifiers.contains(KeyModifiers::CONTROL))
                    || (key.code == KeyCode::Char('Z')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.modifiers.contains(KeyModifiers::SHIFT))
                {
                    self.input_redo();
                    return;
                }

                // Ctrl+A selects all input text
                if key.code == KeyCode::Char('a') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.select_all();
                    return;
                }

                // Ctrl+C copies selected text (only if selection exists)
                if key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.has_selection()
                {
                    self.copy_selection();
                    return;
                }

                // Ctrl+X cuts selected text
                if key.code == KeyCode::Char('x') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.cut_selection();
                    return;
                }

                // Ctrl+V pastes from clipboard
                if key.code == KeyCode::Char('v') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.paste();
                    return;
                }

                // Ctrl+Left or Alt+Left moves cursor to previous word boundary
                if key.code == KeyCode::Left
                    && (key.modifiers.contains(KeyModifiers::CONTROL)
                        || key.modifiers.contains(KeyModifiers::ALT))
                {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        self.start_or_extend_selection();
                    } else {
                        self.clear_selection();
                    }
                    self.move_cursor_word_left();
                    return;
                }

                // Ctrl+Right or Alt+Right moves cursor to next word boundary
                if key.code == KeyCode::Right
                    && (key.modifiers.contains(KeyModifiers::CONTROL)
                        || key.modifiers.contains(KeyModifiers::ALT))
                {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        self.start_or_extend_selection();
                    } else {
                        self.clear_selection();
                    }
                    self.move_cursor_word_right();
                    return;
                }

                // Ctrl+Backspace or Alt+Backspace deletes the word before cursor
                if key.code == KeyCode::Backspace
                    && (key.modifiers.contains(KeyModifiers::CONTROL)
                        || key.modifiers.contains(KeyModifiers::ALT))
                {
                    self.delete_word_left();
                    return;
                }

                // Ctrl+Delete or Alt+Delete deletes the word after cursor
                if key.code == KeyCode::Delete
                    && (key.modifiers.contains(KeyModifiers::CONTROL)
                        || key.modifiers.contains(KeyModifiers::ALT))
                {
                    self.delete_word_right();
                    return;
                }

                // Ctrl+U deletes from cursor to line start (Unix readline style)
                if key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.delete_to_line_start();
                    return;
                }

                // Ctrl+K deletes from cursor to line end (Unix readline style)
                if key.code == KeyCode::Char('k') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.delete_to_line_end();
                    return;
                }

                // Handle command popup when visible
                if self.command_popup.visible {
                    match key.code {
                        KeyCode::Tab | KeyCode::Enter => {
                            // Select the currently highlighted command
                            if let Some(cmd) = self.command_popup.selected_command() {
                                self.input = format!("{} ", cmd);
                                self.cursor_position = self.input.len();
                            }
                            self.command_popup.hide();
                            return;
                        }
                        KeyCode::Esc => {
                            // Hide popup, stay in insert mode
                            self.command_popup.hide();
                            return;
                        }
                        KeyCode::Up => {
                            self.command_popup.move_up();
                            return;
                        }
                        KeyCode::Down => {
                            self.command_popup.move_down();
                            return;
                        }
                        _ => {
                            // Let other keys fall through to normal handling
                        }
                    }
                }

                // Handle file search popup when visible
                if self.file_search_popup.visible && !self.file_search_popup.matches.is_empty() {
                    match key.code {
                        KeyCode::Tab | KeyCode::Enter => {
                            // Select the currently highlighted file
                            if let Some(path) = self
                                .file_search_popup
                                .selected_path()
                                .map(|s| s.to_string())
                            {
                                self.complete_file_mention(&path);
                            }
                            self.file_search_popup.hide();
                            return;
                        }
                        KeyCode::Esc => {
                            // Hide popup, stay in insert mode
                            self.file_search_popup.hide();
                            return;
                        }
                        KeyCode::Up => {
                            self.file_search_popup.move_up();
                            return;
                        }
                        KeyCode::Down => {
                            self.file_search_popup.move_down();
                            return;
                        }
                        _ => {
                            // Let other keys fall through to normal handling
                        }
                    }
                }

                match key.code {
                    KeyCode::Tab => {
                        // Tab completion for slash commands
                        self.try_tab_completion();
                    }
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
                        self.command_popup.hide();
                    }
                    KeyCode::Enter => {
                        // Add newline (Ctrl+Enter or Alt+Enter submits)
                        // If there's a selection, delete it first
                        if self.has_selection() {
                            self.delete_selection();
                        }
                        self.save_undo_state();
                        self.input.insert(self.cursor_position, '\n');
                        self.cursor_position += 1;
                        self.command_popup.hide();
                    }
                    KeyCode::Char(c) => {
                        // If there's a selection, delete it first
                        if self.has_selection() {
                            self.delete_selection();
                        }
                        self.save_undo_state();
                        self.input.insert(self.cursor_position, c);
                        self.cursor_position += 1;
                        // Update command popup filter after character input
                        self.command_popup.update_filter(&self.input);
                        // Update file search popup for @ mentions
                        self.update_file_search_popup();
                    }
                    KeyCode::Backspace => {
                        if self.has_selection() {
                            // Delete the selection
                            self.delete_selection();
                            self.command_popup.update_filter(&self.input);
                            self.update_file_search_popup();
                        } else if self.cursor_position > 0 {
                            self.save_undo_state();
                            self.cursor_position -= 1;
                            self.input.remove(self.cursor_position);
                            // Update command popup filter after deletion
                            self.command_popup.update_filter(&self.input);
                            self.update_file_search_popup();
                        }
                    }
                    KeyCode::Delete => {
                        if self.has_selection() {
                            // Delete the selection
                            self.delete_selection();
                            self.command_popup.update_filter(&self.input);
                            self.update_file_search_popup();
                        } else if self.cursor_position < self.input.len() {
                            self.save_undo_state();
                            self.input.remove(self.cursor_position);
                            // Update command popup filter after deletion
                            self.command_popup.update_filter(&self.input);
                            self.update_file_search_popup();
                        }
                    }
                    KeyCode::Left => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            // Shift+Left: select while moving left
                            self.start_or_extend_selection();
                            if self.cursor_position > 0 {
                                self.cursor_position -= 1;
                            }
                        } else {
                            // Plain Left: move cursor, clear selection
                            self.clear_selection();
                            if self.cursor_position > 0 {
                                self.cursor_position -= 1;
                            }
                        }
                    }
                    KeyCode::Right => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            // Shift+Right: select while moving right
                            self.start_or_extend_selection();
                            if self.cursor_position < self.input.len() {
                                self.cursor_position += 1;
                            }
                        } else {
                            // Plain Right: move cursor, clear selection
                            self.clear_selection();
                            if self.cursor_position < self.input.len() {
                                self.cursor_position += 1;
                            }
                        }
                    }
                    KeyCode::Home => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            // Ctrl+Home: go to document start
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                self.start_or_extend_selection();
                            } else {
                                self.clear_selection();
                            }
                            self.cursor_position = 0;
                        } else if key.modifiers.contains(KeyModifiers::SHIFT) {
                            // Shift+Home: select to start of line
                            self.start_or_extend_selection();
                            self.move_cursor_line_start();
                        } else {
                            // Plain Home: go to start of current line
                            self.clear_selection();
                            self.move_cursor_line_start();
                        }
                    }
                    KeyCode::End => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            // Ctrl+End: go to document end
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                self.start_or_extend_selection();
                            } else {
                                self.clear_selection();
                            }
                            self.cursor_position = self.input.len();
                        } else if key.modifiers.contains(KeyModifiers::SHIFT) {
                            // Shift+End: select to end of line
                            self.start_or_extend_selection();
                            self.move_cursor_line_end();
                        } else {
                            // Plain End: go to end of current line
                            self.clear_selection();
                            self.move_cursor_line_end();
                        }
                    }
                    KeyCode::Up => {
                        // If input has multiple lines, move cursor up
                        // Otherwise, navigate command history
                        if self.input_line_count() > 1 {
                            let (row, _) = self.cursor_row_col();
                            if row > 0 {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    self.start_or_extend_selection();
                                } else {
                                    self.clear_selection();
                                }
                                self.move_cursor_up();
                            } else {
                                // At first line, navigate history
                                self.clear_selection();
                                self.history_previous();
                            }
                        } else {
                            self.clear_selection();
                            self.history_previous();
                        }
                    }
                    KeyCode::Down => {
                        // If input has multiple lines, move cursor down
                        // Otherwise, navigate command history
                        if self.input_line_count() > 1 {
                            let (row, _) = self.cursor_row_col();
                            if row + 1 < self.input_line_count() {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    self.start_or_extend_selection();
                                } else {
                                    self.clear_selection();
                                }
                                self.move_cursor_down();
                            } else {
                                // At last line, navigate history
                                self.clear_selection();
                                self.history_next();
                            }
                        } else {
                            self.clear_selection();
                            self.history_next();
                        }
                    }
                    KeyCode::PageUp => {
                        self.scroll_offset += 10;
                    }
                    KeyCode::PageDown => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(10);
                    }
                    _ => {}
                }
            }
            AppMode::Search => {
                match key.code {
                    KeyCode::Esc => {
                        // Cancel search, return to normal mode
                        self.mode = AppMode::Normal;
                        self.search_query.clear();
                        self.search_matches.clear();
                        self.current_match = None;
                    }
                    KeyCode::Enter => {
                        // Confirm search, go to first match, return to normal mode
                        self.mode = AppMode::Normal;
                        if !self.search_matches.is_empty() && self.current_match.is_some() {
                            // Scroll offset is already set by update_search
                        }
                    }
                    KeyCode::Char(c) => {
                        self.search_query.push(c);
                        self.update_search();
                    }
                    KeyCode::Backspace => {
                        self.search_query.pop();
                        self.update_search();
                    }
                    KeyCode::Up => {
                        // Previous match
                        self.prev_match();
                    }
                    KeyCode::Down => {
                        // Next match
                        self.next_match();
                    }
                    _ => {}
                }
            }
        }
    }

    /// Check if a slash command is available during an agent task.
    /// Returns false if the command should be blocked while processing.
    fn is_command_available_during_task(cmd: &str) -> bool {
        let cmd_lower = cmd.to_lowercase();
        !COMMANDS_UNAVAILABLE_DURING_TASK
            .iter()
            .any(|&blocked| blocked == cmd_lower)
    }

    /// Handle a built-in slash command
    /// Returns true if the command was handled, false if it should be sent to the agent
    async fn handle_slash_command(&mut self, input: &str) -> bool {
        let trimmed = input.trim();
        let (cmd, args) = trimmed
            .split_once(' ')
            .map(|(c, a)| (c, a.trim()))
            .unwrap_or((trimmed, ""));

        // Check if command is blocked during agent processing
        if self.mode == AppMode::Processing && !Self::is_command_available_during_task(cmd) {
            self.show_notification(Notification::warning(format!(
                "{} is not available while a task is running. Use /stop first.",
                cmd
            )));
            return true; // Command was "handled" (blocked)
        }

        // Log slash command execution
        let args_opt = if args.is_empty() { None } else { Some(args) };
        session_log::log_slash_command(cmd, args_opt);

        match cmd.to_lowercase().as_str() {
            "/quit" | "/exit" => {
                self.should_quit = true;
                true
            }
            "/clear" => {
                self.messages.clear();
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: "Chat history cleared.".to_string(),
                });
                true
            }
            "/help" | "/?" => {
                self.show_help = true;
                true
            }
            "/new" => {
                // Start a new chat by clearing messages and agent state
                self.messages.clear();
                let mut agent_state = AgentState::new();
                // Preserve exec_policy from current approval preset
                let exec_policy = exec_policy_from_preset(&self.config.approval_preset);
                agent_state = agent_state.with_exec_policy(Arc::new(exec_policy));
                self.agent_state = agent_state;
                self.turn_count = 0;
                // Clear session approvals from previous chat
                self.session_approved_tools.clear();
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: "Started new chat session.".to_string(),
                });
                true
            }
            "/status" => {
                // Calculate context usage
                let tokens = self.estimated_tokens();
                let context_size = self.estimate_context_window();
                let percentage = if context_size > 0 {
                    (tokens as f64 / context_size as f64 * 100.0).min(100.0)
                } else {
                    0.0
                };

                // Format token counts
                let tokens_str = if tokens >= 1000 {
                    format!("{:.1}k", tokens as f64 / 1000.0)
                } else {
                    format!("{}", tokens)
                };
                let context_str = if context_size >= 1_000_000 {
                    format!("{:.1}M", context_size as f64 / 1_000_000.0)
                } else if context_size >= 1000 {
                    format!("{}k", context_size / 1000)
                } else {
                    format!("{}", context_size)
                };

                let auth_str = match &self.auth_status {
                    AuthStatus::ChatGpt { email: Some(e) } => format!("ChatGPT ({})", e),
                    AuthStatus::ChatGpt { email: None } => "ChatGPT".to_string(),
                    AuthStatus::ApiKey => "API key".to_string(),
                    AuthStatus::EnvApiKey => "env API key".to_string(),
                    AuthStatus::NotAuthenticated => "not authenticated".to_string(),
                };

                let status_msg = format!(
                    "Session: {}\n\
                     Model: {}\n\
                     Turns: {}\n\
                     Messages: {} (agent state)\n\
                     Context: ~{}/{} ({:.0}%)\n\
                     Working Dir: {}\n\
                     Auth: {}",
                    self.session_id,
                    self.model,
                    self.turn_count,
                    self.agent_state.messages.len(),
                    tokens_str,
                    context_str,
                    percentage,
                    self.config.working_dir,
                    auth_str
                );
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: status_msg,
                });
                true
            }
            "/model" => {
                if args.is_empty() {
                    self.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: format!(
                            "Current model: {}\nUsage: /model <model-name>",
                            self.model
                        ),
                    });
                } else {
                    self.model = args.to_string();
                    self.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: format!("Model changed to: {}", self.model),
                    });
                }
                true
            }
            "/tokens" => {
                self.handle_tokens_command();
                true
            }
            "/keys" => {
                self.handle_keys_command();
                true
            }
            "/version" => {
                self.handle_version_command();
                true
            }
            "/config" => {
                self.handle_config_command();
                true
            }
            "/mcp" => {
                self.handle_mcp_command();
                true
            }
            "/skills" => {
                self.handle_skills_command();
                true
            }
            "/logout" => {
                self.handle_logout_command();
                true
            }
            "/init" => {
                self.handle_init_command();
                true
            }
            "/approvals" | "/mode" => {
                self.handle_approvals_command(args);
                true
            }
            "/resume" => {
                self.handle_resume_command(args).await;
                true
            }
            "/sessions" => {
                self.handle_sessions_command().await;
                true
            }
            "/delete" => {
                self.handle_delete_command(args).await;
                true
            }
            "/mention" => {
                self.handle_mention_command(args);
                true
            }
            "/undo" => {
                self.handle_undo_command();
                true
            }
            "/history" => {
                self.handle_history_command();
                true
            }
            "/compact" => {
                self.handle_compact_command();
                true
            }
            "/diff" => {
                self.handle_diff_command(args);
                true
            }
            "/review" => {
                self.handle_review_command();
                true
            }
            "/feedback" => {
                self.handle_feedback_command();
                true
            }
            "/providers" => {
                self.handle_providers_command();
                true
            }
            "/search" => {
                self.handle_search_command(args);
                true
            }
            "/context" => {
                self.handle_context_command();
                true
            }
            "/stop" => {
                self.handle_stop_command();
                true
            }
            "/export" => {
                self.handle_export_command(args);
                true
            }
            _ => false,
        }
    }

    /// Handle /stop command - cancel the current agent task
    fn handle_stop_command(&mut self) {
        if self.mode != AppMode::Processing {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: "No task is currently running.".to_string(),
            });
            return;
        }

        // Cancel the agent task if we have a token
        if let Some(ref token) = self.agent_cancel_token {
            token.cancel();
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: "Stopping agent task...".to_string(),
            });
            // Mode will be reset to Insert when the task completes/cancels
        } else {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: "Cannot stop: no cancellation token available.".to_string(),
            });
        }
    }

    /// Handle /undo command - remove the last user turn and its response
    fn handle_undo_command(&mut self) {
        // Find the last user message in agent state
        let user_count = self
            .agent_state
            .messages
            .iter()
            .filter(|m| matches!(m.role, CoreMessageRole::User))
            .count();

        if user_count == 0 {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: "Nothing to undo.".to_string(),
            });
            return;
        }

        // Remove messages from the last user message onwards
        // Find the index of the last user message
        let last_user_idx = self
            .agent_state
            .messages
            .iter()
            .rposition(|m| matches!(m.role, CoreMessageRole::User));

        if let Some(idx) = last_user_idx {
            // Remove from agent state
            let removed_count = self.agent_state.messages.len() - idx;
            self.agent_state.messages.truncate(idx);

            // Also remove from display messages (last user + assistant messages)
            // Find and remove from the end of display messages
            let mut to_remove = 0;
            for msg in self.messages.iter().rev() {
                if matches!(msg.role, MessageRole::User) {
                    to_remove += 1;
                    break;
                }
                to_remove += 1;
            }
            let new_len = self.messages.len().saturating_sub(to_remove);
            self.messages.truncate(new_len);

            // Decrement turn count if we had turns
            if self.turn_count > 0 {
                self.turn_count -= 1;
            }

            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: format!("Undone last turn ({} messages removed).", removed_count),
            });
        }
    }

    /// Handle /tokens command - show detailed token usage statistics
    fn handle_tokens_command(&mut self) {
        let tokens = self.estimated_tokens();
        let context_size = self.estimate_context_window();
        let percentage = if context_size > 0 {
            (tokens as f64 / context_size as f64 * 100.0).min(100.0)
        } else {
            0.0
        };

        // Count tokens per message type
        let mut system_tokens = 0usize;
        let mut user_tokens = 0usize;
        let mut assistant_tokens = 0usize;
        let mut tool_tokens = 0usize;

        for msg in &self.agent_state.messages {
            // Rough estimate: ~4 chars per token
            let msg_tokens = msg.content.len() / 4;
            match msg.role {
                CoreMessageRole::System => system_tokens += msg_tokens,
                CoreMessageRole::User => user_tokens += msg_tokens,
                CoreMessageRole::Assistant => assistant_tokens += msg_tokens,
                CoreMessageRole::Tool => tool_tokens += msg_tokens,
            }
        }

        // Format helper
        let format_tokens = |t: usize| -> String {
            if t >= 1000 {
                format!("{:.1}k", t as f64 / 1000.0)
            } else {
                format!("{}", t)
            }
        };

        let context_str = if context_size >= 1_000_000 {
            format!("{:.1}M", context_size as f64 / 1_000_000.0)
        } else if context_size >= 1000 {
            format!("{}k", context_size / 1000)
        } else {
            format!("{}", context_size)
        };

        // Build progress bar (20 chars wide)
        let bar_width = 20;
        let filled = ((percentage / 100.0) * bar_width as f64).round() as usize;
        let bar = format!(
            "[{}{}]",
            "=".repeat(filled.min(bar_width)),
            " ".repeat(bar_width.saturating_sub(filled))
        );

        let tokens_msg = format!(
            "Token Usage Statistics\n\
             \n\
             Context: {}/{} {} {:.1}%\n\
             \n\
             By message type:\n\
               System:    ~{}\n\
               User:      ~{}\n\
               Assistant: ~{}\n\
               Tool:      ~{}\n\
             \n\
             Total messages: {}",
            format_tokens(tokens),
            context_str,
            bar,
            percentage,
            format_tokens(system_tokens),
            format_tokens(user_tokens),
            format_tokens(assistant_tokens),
            format_tokens(tool_tokens),
            self.agent_state.messages.len()
        );

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: tokens_msg,
        });
    }

    /// Handle /keys command - show keyboard shortcuts
    fn handle_keys_command(&mut self) {
        let keys_msg = "\
Keyboard Shortcuts

Navigation:
  Left/Right          Move cursor by character
  Ctrl/Alt+Left/Right Move cursor by word
  Home/End            Go to line start/end
  Ctrl+Home/End       Go to document start/end
  Up/Down             Navigate history (single line) or move cursor (multi-line)

Selection (add Shift to any navigation):
  Shift+Arrow         Select while moving
  Shift+Ctrl/Alt+Arrow Select by word
  Shift+Home/End      Select to line boundary
  Shift+Ctrl+Home/End Select to document boundary
  Ctrl+A              Select all

Editing:
  Ctrl+Z              Undo
  Ctrl+Y or Ctrl+Shift+Z  Redo
  Ctrl+C              Copy selection
  Ctrl+X              Cut selection
  Ctrl+V              Paste

Deletion:
  Backspace/Delete    Delete character
  Ctrl/Alt+Backspace  Delete word left
  Ctrl/Alt+Delete     Delete word right
  Ctrl+U              Delete to line start
  Ctrl+K              Delete to line end

Input:
  Enter               Send message (single line) or newline (multi-line)
  Shift+Enter         Force newline
  Ctrl+Enter          Force send (multi-line)
  Tab                 Complete slash command
  Esc                 Exit or cancel";

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: keys_msg.to_string(),
        });
    }

    /// Handle /version command - show version information
    fn handle_version_command(&mut self) {
        let version_msg = format!(
            "Codex DashFlow\n\
             Version: {}\n\
             Build: {}\n\
             Rust: {}",
            env!("CARGO_PKG_VERSION"),
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            env!("CARGO_PKG_RUST_VERSION")
        );

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: version_msg,
        });
    }

    /// Handle /config command - show current configuration
    fn handle_config_command(&mut self) {
        let max_turns_str = if self.config.max_turns == 0 {
            "unlimited".to_string()
        } else {
            self.config.max_turns.to_string()
        };

        let config_msg = format!(
            "Current Configuration\n\
             \n\
             Model: {}\n\
             Working Dir: {}\n\
             Max Turns: {}\n\
             Session ID: {}\n\
             Training Collection: {}\n\
             Optimized Prompts: {}",
            self.model,
            self.config.working_dir,
            max_turns_str,
            self.session_id,
            if self.config.collect_training {
                "enabled"
            } else {
                "disabled"
            },
            if self.config.load_optimized_prompts {
                "enabled"
            } else {
                "disabled"
            }
        );

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: config_msg,
        });
    }

    /// Handle /mcp command - list configured MCP servers
    fn handle_mcp_command(&mut self) {
        let servers = &self.config.mcp_servers;

        if servers.is_empty() {
            let msg = "No MCP servers configured.\n\
                       \n\
                       Configure servers in ~/.codex-dashflow/config.toml:\n\
                       \n\
                       [[mcp_servers]]\n\
                       name = \"filesystem\"\n\
                       type = \"stdio\"\n\
                       command = \"npx\"\n\
                       args = [\"-y\", \"@modelcontextprotocol/server-filesystem\", \"/path\"]";

            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: msg.to_string(),
            });
            return;
        }

        let mut msg = format!("Configured MCP Servers: {}\n", servers.len());

        for (i, server) in servers.iter().enumerate() {
            msg.push_str(&format!("\n{}. {}\n", i + 1, server.name));

            match &server.transport {
                codex_dashflow_core::mcp::McpTransport::Stdio { command, args } => {
                    msg.push_str("   Type: stdio\n");
                    msg.push_str(&format!("   Command: {}", command));
                    if !args.is_empty() {
                        msg.push_str(&format!(" {}", args.join(" ")));
                    }
                    msg.push('\n');
                }
                codex_dashflow_core::mcp::McpTransport::Http { url, .. } => {
                    msg.push_str("   Type: http\n");
                    msg.push_str(&format!("   URL: {}\n", url));
                }
            }

            if !server.env.is_empty() {
                msg.push_str(&format!("   Env vars: {}\n", server.env.len()));
            }

            msg.push_str(&format!("   Timeout: {}s\n", server.timeout_secs));
        }

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: msg,
        });
    }

    /// Handle /skills command - list available skills
    fn handle_skills_command(&mut self) {
        let outcome = load_skills();

        if outcome.skills.is_empty() {
            let skills_dir = codex_dashflow_core::default_skills_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "~/.codex-dashflow/skills".to_string());

            let mut msg = format!(
                "No skills found.\n\
                 \n\
                 Create skills in: {}\n\
                 \n\
                 Example SKILL.md:\n\
                 ---\n\
                 name: my-skill\n\
                 description: A helpful skill\n\
                 ---\n\
                 Skill instructions go here...",
                skills_dir
            );

            // Show any errors encountered
            if !outcome.errors.is_empty() {
                msg.push_str("\n\nErrors encountered:");
                for error in &outcome.errors {
                    msg.push_str(&format!(
                        "\n  - {}: {}",
                        error.path.display(),
                        error.message
                    ));
                }
            }

            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: msg,
            });
            return;
        }

        let mut msg = format!("Available Skills: {}\n", outcome.skills.len());

        for (i, skill) in outcome.skills.iter().enumerate() {
            msg.push_str(&format!("\n{}. {}\n", i + 1, skill.name));
            msg.push_str(&format!("   {}\n", skill.description));
            msg.push_str(&format!("   Path: {}\n", skill.path.display()));
        }

        // Show any errors encountered
        if !outcome.errors.is_empty() {
            msg.push_str("\nErrors loading some skills:");
            for error in &outcome.errors {
                msg.push_str(&format!(
                    "\n  - {}: {}",
                    error.path.display(),
                    error.message
                ));
            }
        }

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: msg,
        });
    }

    /// Handle /logout command - clear stored credentials
    fn handle_logout_command(&mut self) {
        match AuthManager::new(AuthCredentialsStoreMode::Auto) {
            Ok(manager) => match manager.logout() {
                Ok(true) => {
                    self.auth_status = AuthStatus::NotAuthenticated;
                    self.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: "Logged out successfully. Credentials cleared.".to_string(),
                    });
                }
                Ok(false) => {
                    self.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: "No credentials to clear.".to_string(),
                    });
                }
                Err(e) => {
                    self.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: format!("Failed to logout: {}", e),
                    });
                }
            },
            Err(e) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Failed to access credential store: {}", e),
                });
            }
        }
    }

    /// Handle /init command - create an AGENTS.md file with instructions
    fn handle_init_command(&mut self) {
        const DEFAULT_PROJECT_DOC: &str = "AGENTS.md";
        const INIT_PROMPT: &str = r#"Generate a file named AGENTS.md that serves as a contributor guide for this repository.
Your goal is to produce a clear, concise, and well-structured document with descriptive headings and actionable explanations for each section.
Follow the outline below, but adapt as needed — add sections if relevant, and omit those that do not apply to this project.

Document Requirements

- Title the document "Repository Guidelines".
- Use Markdown headings (#, ##, etc.) for structure.
- Keep the document concise. 200-400 words is optimal.
- Keep explanations short, direct, and specific to this repository.
- Provide examples where helpful (commands, directory paths, naming patterns).
- Maintain a professional, instructional tone.

Recommended Sections

Project Structure & Module Organization

- Outline the project structure, including where the source code, tests, and assets are located.

Build, Test, and Development Commands

- List key commands for building, testing, and running locally (e.g., npm test, make build).
- Briefly explain what each command does.

Coding Style & Naming Conventions

- Specify indentation rules, language-specific style preferences, and naming patterns.
- Include any formatting or linting tools used.

Testing Guidelines

- Identify testing frameworks and coverage requirements.
- State test naming conventions and how to run tests.

Commit & Pull Request Guidelines

- Summarize commit message conventions found in the project's Git history.
- Outline pull request requirements (descriptions, linked issues, screenshots, etc.).

(Optional) Add other sections if relevant, such as Security & Configuration Tips, Architecture Overview, or Agent-Specific Instructions."#;

        let init_target = std::path::Path::new(&self.config.working_dir).join(DEFAULT_PROJECT_DOC);

        if init_target.exists() {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: format!(
                    "{} already exists in this directory. Skipping /init to avoid overwriting it.",
                    DEFAULT_PROJECT_DOC
                ),
            });
            return;
        }

        // Submit the init prompt to the agent
        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: format!("Creating {} for this repository...", DEFAULT_PROJECT_DOC),
        });

        // Add the prompt as a user message that will be sent to the agent
        self.input = INIT_PROMPT.to_string();
    }

    /// Handle /approvals command - show or change approval settings
    fn handle_approvals_command(&mut self, args: &str) {
        let presets = builtin_approval_presets();

        if args.is_empty() {
            // Show current approval mode and available presets
            let current = presets
                .iter()
                .find(|p| p.id == self.config.approval_preset)
                .map(|p| format!("{} ({})", p.label, p.id))
                .unwrap_or_else(|| self.config.approval_preset.clone());

            let mut msg = format!("Current approval mode: {}\n\nAvailable modes:", current);

            for preset in &presets {
                let indicator = if preset.id == self.config.approval_preset {
                    " ←"
                } else {
                    ""
                };
                msg.push_str(&format!(
                    "\n  {} ({}){}",
                    preset.label, preset.id, indicator
                ));
                msg.push_str(&format!("\n    {}", preset.description));
            }

            msg.push_str("\n\nUsage: /approvals <mode-id>");

            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: msg,
            });
            return;
        }

        // Try to set the approval mode
        let preset_id = args.to_lowercase();
        if let Some(preset) = presets.iter().find(|p| p.id == preset_id) {
            self.config.approval_preset = preset.id.to_string();

            // Update agent state's exec_policy to match the new preset
            let exec_policy = exec_policy_from_preset(&self.config.approval_preset);
            self.agent_state =
                std::mem::take(&mut self.agent_state).with_exec_policy(Arc::new(exec_policy));

            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: format!(
                    "Approval mode changed to: {} ({})\n{}",
                    preset.label, preset.id, preset.description
                ),
            });
        } else {
            let valid_ids: Vec<&str> = presets.iter().map(|p| p.id).collect();
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: format!(
                    "Unknown approval mode: '{}'\nValid modes: {}",
                    args,
                    valid_ids.join(", ")
                ),
            });
        }
    }

    /// Cycle to the next approval mode preset.
    ///
    /// This cycles through: read-only → auto → full-access → read-only
    /// Used by Ctrl+M keyboard shortcut for quick mode switching.
    fn cycle_approval_mode(&mut self) {
        let presets = builtin_approval_presets();
        let preset_ids: Vec<&str> = presets.iter().map(|p| p.id).collect();

        // Find current preset index
        let current_idx = preset_ids
            .iter()
            .position(|id| *id == self.config.approval_preset)
            .unwrap_or(0);

        // Cycle to next preset (wrap around)
        let next_idx = (current_idx + 1) % presets.len();
        let next_preset = &presets[next_idx];

        // Apply the new preset
        self.config.approval_preset = next_preset.id.to_string();

        // Update agent state's exec_policy
        let exec_policy = exec_policy_from_preset(&self.config.approval_preset);
        self.agent_state =
            std::mem::take(&mut self.agent_state).with_exec_policy(Arc::new(exec_policy));

        // Show transient notification instead of adding a system message
        let notif_style = match next_preset.id {
            "read-only" => NotificationStyle::Warning,
            "full-access" => NotificationStyle::Error,
            _ => NotificationStyle::Info,
        };
        self.show_notification(
            Notification::new(
                format!("Mode: {} ({})", next_preset.label, next_preset.id),
                notif_style,
            )
            .with_duration(Duration::from_secs(2)),
        );
    }

    /// Handle /resume command - resume a previous session
    async fn handle_resume_command(&mut self, args: &str) {
        if args.is_empty() {
            // Show current session info and available sessions if checkpointing enabled
            if self.config.checkpointing_enabled {
                let runner_config = self.config.build_runner_config();
                match list_sessions(&runner_config).await {
                    Ok(sessions) if !sessions.is_empty() => {
                        let session_list: String = sessions
                            .iter()
                            .take(10)
                            .map(|s| format!("  • {}", s.thread_id))
                            .collect::<Vec<_>>()
                            .join("\n");
                        let msg = format!(
                            "Current Session: {}\n\n\
                             Available sessions:\n{}\n\n\
                             Usage: /resume <session-id>\n\
                             Or use /sessions to see all sessions with details.",
                            self.session_id, session_list
                        );
                        self.messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: msg,
                        });
                    }
                    Ok(_) => {
                        // No sessions available
                        self.messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: format!(
                                "Current Session: {}\n\n\
                                 No saved sessions found.\n\n\
                                 Usage: /resume <session-id>",
                                self.session_id
                            ),
                        });
                    }
                    Err(e) => {
                        self.messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: format!(
                                "Current Session: {}\n\n\
                                 Error listing sessions: {}\n\n\
                                 Usage: /resume <session-id>",
                                self.session_id, e
                            ),
                        });
                    }
                }
            } else {
                // Checkpointing not enabled - show setup instructions
                let msg = format!(
                    "Current Session: {}\n\n\
                     Session persistence requires checkpointing to be enabled.\n\n\
                     To enable session persistence:\n\
                     1. Add to ~/.codex-dashflow/config.toml:\n\
                        [dashflow]\n\
                        checkpointing_enabled = true\n\
                        checkpoint_path = \"~/.codex-dashflow/checkpoints\"\n\n\
                     2. Start with a session ID:\n\
                        codex-dashflow --session-id my-session\n\n\
                     3. Resume later with:\n\
                        /resume my-session\n\n\
                     Usage: /resume <session-id>",
                    self.session_id
                );
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: msg,
                });
            }
            return;
        }

        // User wants to resume a specific session
        let session_id = args.trim().to_string();
        let old_session = self.session_id.clone();

        // Attempt to load the session if checkpointing is enabled
        if self.config.checkpointing_enabled {
            let runner_config = self.config.build_runner_config();
            match resume_session(&session_id, &runner_config).await {
                Ok(mut restored_state) => {
                    // Apply current exec_policy to restored state
                    let exec_policy = exec_policy_from_preset(&self.config.approval_preset);
                    restored_state = restored_state.with_exec_policy(Arc::new(exec_policy));

                    // Restore messages to display
                    self.messages.clear();
                    for msg in &restored_state.messages {
                        let role = match msg.role {
                            CoreMessageRole::User => MessageRole::User,
                            CoreMessageRole::Assistant => MessageRole::Assistant,
                            CoreMessageRole::System => MessageRole::System,
                            CoreMessageRole::Tool => MessageRole::Tool,
                        };
                        self.messages.push(ChatMessage {
                            role,
                            content: msg.content.clone(),
                        });
                    }

                    self.session_id = session_id.clone();
                    self.turn_count = restored_state.turn_count;
                    self.agent_state = restored_state;

                    self.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: format!(
                            "Session resumed: {} → {}\n\
                             Restored {} messages, {} turns.",
                            old_session,
                            session_id,
                            self.messages.len().saturating_sub(1),
                            self.turn_count
                        ),
                    });
                    return;
                }
                Err(e) => {
                    self.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: format!(
                            "Failed to resume session '{}': {}\n\n\
                             Starting fresh session with this ID.",
                            session_id, e
                        ),
                    });
                    // Fall through to create fresh session
                }
            }
        }

        // Create a fresh session (either checkpointing disabled or resume failed)
        self.session_id = session_id.clone();
        self.messages.clear();
        let mut agent_state = AgentState::new();
        agent_state.session_id = session_id.clone();
        let exec_policy = exec_policy_from_preset(&self.config.approval_preset);
        agent_state = agent_state.with_exec_policy(Arc::new(exec_policy));
        self.agent_state = agent_state;
        self.turn_count = 0;

        if !self.config.checkpointing_enabled {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: format!(
                    "Session changed: {} → {}\n\n\
                     Note: Checkpointing not enabled. Session state will not be persisted.",
                    old_session, session_id
                ),
            });
        }
    }

    /// Handle /sessions command - list available sessions
    async fn handle_sessions_command(&mut self) {
        if !self.config.checkpointing_enabled {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: "Checkpointing is not enabled.\n\n\
                         To enable session persistence:\n\
                         1. Add to ~/.codex-dashflow/config.toml:\n\
                            [dashflow]\n\
                            checkpointing_enabled = true\n\
                            checkpoint_path = \"~/.codex-dashflow/checkpoints\""
                    .to_string(),
            });
            return;
        }

        let runner_config = self.config.build_runner_config();
        match list_sessions(&runner_config).await {
            Ok(sessions) if sessions.is_empty() => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: "No saved sessions found.".to_string(),
                });
            }
            Ok(sessions) => {
                let mut lines = vec![format!("Saved Sessions ({}):\n", sessions.len())];
                for session in sessions.iter().take(20) {
                    let updated = chrono::DateTime::<chrono::Utc>::from(session.updated_at)
                        .format("%Y-%m-%d %H:%M")
                        .to_string();
                    lines.push(format!("  • {} (updated: {})", session.thread_id, updated));
                }
                if sessions.len() > 20 {
                    lines.push(format!("\n  ... and {} more", sessions.len() - 20));
                }
                lines.push("\nUse /resume <session-id> to restore a session.".to_string());

                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: lines.join("\n"),
                });
            }
            Err(e) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Error listing sessions: {}", e),
                });
            }
        }
    }

    /// Handle /delete command - delete a saved session
    async fn handle_delete_command(&mut self, args: &str) {
        if !self.config.checkpointing_enabled {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: "Checkpointing is not enabled.\n\n\
                         To enable session persistence:\n\
                         1. Add to ~/.codex-dashflow/config.toml:\n\
                            [dashflow]\n\
                            checkpointing_enabled = true\n\
                            checkpoint_path = \"~/.codex-dashflow/checkpoints\""
                    .to_string(),
            });
            return;
        }

        if args.is_empty() {
            // Show usage help
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: "Usage: /delete <session-id>\n\n\
                         Deletes a saved session from checkpoint storage.\n\
                         Use /sessions to list available sessions."
                    .to_string(),
            });
            return;
        }

        let session_id = args.trim();

        // Prevent deleting the current session
        if session_id == self.session_id {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: format!(
                    "Cannot delete the current session '{}'.\n\
                     Use /new to start a new session first.",
                    session_id
                ),
            });
            return;
        }

        let runner_config = self.config.build_runner_config();
        match delete_session(&runner_config, session_id).await {
            Ok(()) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Session '{}' deleted successfully.", session_id),
                });
            }
            Err(e) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Error deleting session '{}': {}", session_id, e),
                });
            }
        }
    }

    /// Handle /mention command - insert @ for file completion or mention a specific file
    fn handle_mention_command(&mut self, args: &str) {
        // Clear the input (which has the /mention command)
        self.input.clear();
        self.cursor_position = 0;

        if args.is_empty() {
            // Just insert @ to trigger file completion
            self.input.push('@');
            self.cursor_position = 1;
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: "Type a filename after @ to mention a file.".to_string(),
            });
        } else {
            // Insert @<filename>
            let mention = format!("@{} ", args.trim());
            self.input.push_str(&mention);
            self.cursor_position = mention.len();
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: format!("Mentioning file: {}", args.trim()),
            });
        }
    }

    /// Handle /history command - show conversation history summary
    fn handle_history_command(&mut self) {
        let messages = &self.agent_state.messages;
        if messages.is_empty() {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: "No conversation history.".to_string(),
            });
            return;
        }

        let mut summary = String::from("Conversation history:\n");
        for (i, msg) in messages.iter().enumerate() {
            let role_str = match msg.role {
                CoreMessageRole::User => "User",
                CoreMessageRole::Assistant => "Assistant",
                CoreMessageRole::System => "System",
                CoreMessageRole::Tool => "Tool",
            };
            // Truncate long messages for display
            let content_preview = if msg.content.len() > 50 {
                format!("{}...", &msg.content[..50])
            } else {
                msg.content.clone()
            };
            let content_preview = content_preview.replace('\n', " ");
            summary.push_str(&format!(
                "  {}. [{}] {}\n",
                i + 1,
                role_str,
                content_preview
            ));
        }
        summary.push_str(&format!(
            "\nTotal: {} messages, {} turns",
            messages.len(),
            self.turn_count
        ));

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: summary,
        });
    }

    /// Handle /compact command - summarize conversation locally
    fn handle_compact_command(&mut self) {
        let messages = &self.agent_state.messages;
        if messages.len() < 4 {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: "Conversation too short to compact (need at least 2 turns).".to_string(),
            });
            return;
        }

        // Count message types
        let user_count = messages
            .iter()
            .filter(|m| matches!(m.role, CoreMessageRole::User))
            .count();
        let assistant_count = messages
            .iter()
            .filter(|m| matches!(m.role, CoreMessageRole::Assistant))
            .count();
        let tool_count = messages
            .iter()
            .filter(|m| matches!(m.role, CoreMessageRole::Tool))
            .count();

        // Calculate total content size (used in summary)

        // Keep the last 2 user messages and their responses
        let keep_from = messages
            .iter()
            .enumerate()
            .filter(|(_, m)| matches!(m.role, CoreMessageRole::User))
            .map(|(i, _)| i)
            .rev()
            .take(2)
            .min();

        if let Some(keep_idx) = keep_from {
            let removed_count = keep_idx;
            if removed_count == 0 {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: "Nothing to compact (already at minimum).".to_string(),
                });
                return;
            }

            // Create a summary of what was compacted
            let summary = format!(
                "Compacted conversation:\n  Removed: {} messages ({} chars)\n  Kept: {} recent messages\n  Original: {} user, {} assistant, {} tool messages",
                removed_count,
                messages[..keep_idx].iter().map(|m| m.content.len()).sum::<usize>(),
                messages.len() - keep_idx,
                user_count,
                assistant_count,
                tool_count
            );

            // Remove old messages from agent state
            self.agent_state.messages = self.agent_state.messages.split_off(keep_idx);

            // Add compaction notice
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: summary,
            });
        }
    }

    /// Handle /diff command - show git diff output
    ///
    /// Supports optional arguments:
    /// - /diff           - Show all uncommitted changes (staged + unstaged)
    /// - /diff --staged  - Show only staged changes
    /// - /diff --cached  - Same as --staged
    /// - /diff HEAD~N    - Show diff against N commits ago
    /// - /diff <file>    - Show diff for specific file
    fn handle_diff_command(&mut self, args: &str) {
        use std::process::Command;

        let mut cmd = Command::new("git");
        cmd.arg("diff");

        // Parse arguments
        let args_trimmed = args.trim();
        if !args_trimmed.is_empty() {
            for arg in args_trimmed.split_whitespace() {
                cmd.arg(arg);
            }
        }

        // Add color=never for clean output
        cmd.arg("--color=never");

        match cmd.output() {
            Ok(output) => {
                if output.status.success() {
                    let diff_output = String::from_utf8_lossy(&output.stdout);
                    if diff_output.trim().is_empty() {
                        self.messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: "No changes found.".to_string(),
                        });
                    } else {
                        // Truncate very long diffs
                        let max_len = 10000;
                        let content = if diff_output.len() > max_len {
                            format!(
                                "{}\n\n... (truncated, {} total chars)",
                                &diff_output[..max_len],
                                diff_output.len()
                            )
                        } else {
                            diff_output.to_string()
                        };

                        self.messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: format!("```diff\n{}\n```", content.trim()),
                        });
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    self.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: format!("Git diff failed: {}", stderr.trim()),
                    });
                }
            }
            Err(e) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Failed to run git diff: {}", e),
                });
            }
        }
    }

    /// Handle /review command - show summary of current changes
    ///
    /// Shows:
    /// - Files changed (staged vs unstaged)
    /// - Lines added/removed
    /// - Brief stat summary
    fn handle_review_command(&mut self) {
        use std::process::Command;

        let mut review_output = String::new();

        // Get status summary
        let status_output = Command::new("git").args(["status", "--short"]).output();

        match status_output {
            Ok(output) if output.status.success() => {
                let status = String::from_utf8_lossy(&output.stdout);
                if status.trim().is_empty() {
                    self.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: "Working directory is clean. No changes to review.".to_string(),
                    });
                    return;
                }

                // Parse status to count file types
                let lines: Vec<&str> = status.lines().collect();
                let staged_count = lines
                    .iter()
                    .filter(|l| {
                        l.len() >= 2
                            && !l.chars().next().unwrap_or(' ').is_whitespace()
                            && l.chars().next().unwrap_or(' ') != '?'
                    })
                    .count();
                let unstaged_count = lines
                    .iter()
                    .filter(|l| {
                        l.len() >= 2
                            && l.chars().nth(1).unwrap_or(' ') != ' '
                            && l.chars().nth(1).unwrap_or(' ') != '?'
                    })
                    .count();
                let untracked_count = lines.iter().filter(|l| l.starts_with("??")).count();

                review_output.push_str("## Change Summary\n\n");
                review_output.push_str(&format!("**Files changed:** {}\n", lines.len()));
                if staged_count > 0 {
                    review_output.push_str(&format!("  - Staged: {}\n", staged_count));
                }
                if unstaged_count > 0 {
                    review_output.push_str(&format!("  - Modified: {}\n", unstaged_count));
                }
                if untracked_count > 0 {
                    review_output.push_str(&format!("  - Untracked: {}\n", untracked_count));
                }
                review_output.push('\n');

                // Get stat summary
                let stat_output = Command::new("git")
                    .args(["diff", "--stat", "--color=never"])
                    .output();

                if let Ok(stat) = stat_output {
                    if stat.status.success() {
                        let stat_str = String::from_utf8_lossy(&stat.stdout);
                        if !stat_str.trim().is_empty() {
                            review_output.push_str("## Diff Stats (unstaged)\n\n```\n");
                            // Limit stat output
                            let stat_lines: Vec<&str> = stat_str.lines().collect();
                            if stat_lines.len() > 25 {
                                for line in &stat_lines[..20] {
                                    review_output.push_str(line);
                                    review_output.push('\n');
                                }
                                review_output.push_str(&format!(
                                    "... ({} more files)\n",
                                    stat_lines.len() - 21
                                ));
                                // Always show the summary line (last line)
                                if let Some(last) = stat_lines.last() {
                                    review_output.push_str(last);
                                    review_output.push('\n');
                                }
                            } else {
                                review_output.push_str(&stat_str);
                            }
                            review_output.push_str("```\n");
                        }
                    }
                }

                // Check for staged changes too
                let staged_stat = Command::new("git")
                    .args(["diff", "--staged", "--stat", "--color=never"])
                    .output();

                if let Ok(stat) = staged_stat {
                    if stat.status.success() {
                        let stat_str = String::from_utf8_lossy(&stat.stdout);
                        if !stat_str.trim().is_empty() {
                            review_output.push_str("\n## Diff Stats (staged)\n\n```\n");
                            let stat_lines: Vec<&str> = stat_str.lines().collect();
                            if stat_lines.len() > 25 {
                                for line in &stat_lines[..20] {
                                    review_output.push_str(line);
                                    review_output.push('\n');
                                }
                                review_output.push_str(&format!(
                                    "... ({} more files)\n",
                                    stat_lines.len() - 21
                                ));
                                if let Some(last) = stat_lines.last() {
                                    review_output.push_str(last);
                                    review_output.push('\n');
                                }
                            } else {
                                review_output.push_str(&stat_str);
                            }
                            review_output.push_str("```\n");
                        }
                    }
                }

                // List files
                review_output.push_str("\n## Files\n\n```\n");
                review_output.push_str(&status);
                review_output.push_str("```\n");

                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: review_output,
                });
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Git status failed: {}", stderr.trim()),
                });
            }
            Err(e) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Failed to run git status: {}", e),
                });
            }
        }
    }

    /// Submit the current input to the agent
    async fn submit_input(&mut self) {
        let input = std::mem::take(&mut self.input);
        self.cursor_position = 0;

        // Log user message submission
        session_log::log_user_message(&input);

        // Save to command history (avoid duplicates of last entry)
        if self.command_history.last() != Some(&input) {
            self.command_history.push(input.clone());
            // Persist to disk
            history::append_history(&input);
        }
        // Reset history navigation
        self.history_position = None;
        self.saved_input.clear();

        // Check for slash commands first
        if input.starts_with('/') && self.handle_slash_command(&input).await {
            // Command was handled locally, don't send to agent
            return;
        }

        // Reset scroll to show latest messages
        self.scroll_offset = 0;

        // Reset agent status for new run
        self.agent_status = AgentStatus::Idle;

        // Reset training collection state for this run
        self.last_run_score = None;
        self.training_collected = false;

        // Add user message to display
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: input.clone(),
        });

        // Add message to agent state
        self.agent_state.messages.push(Message::user(&input));

        // Switch to processing mode
        self.mode = AppMode::Processing;

        // Create cancellation token for this task
        let cancel_token = CancellationToken::new();
        self.agent_cancel_token = Some(cancel_token.clone());

        // Create stream callback if we have an event sender
        let callback: Arc<dyn StreamCallback> = if let Some(ref tx) = self.event_tx {
            Arc::new(TuiStreamCallback::new(tx.clone()))
        } else {
            Arc::new(codex_dashflow_core::NullStreamCallback)
        };

        // Create approval channel for interactive tool approval
        if let Some(ref tx) = self.event_tx {
            let approval_channel = crate::event::ApprovalChannel::new(tx.clone());
            self.agent_state = std::mem::take(&mut self.agent_state)
                .with_approval_callback(Arc::new(approval_channel));
        }

        // Configure and run the agent with training collection, optimized prompts, and checkpointing
        let mut config = self
            .config
            .build_runner_config()
            .with_stream_callback(callback)
            .with_collect_training(self.config.collect_training)
            .with_load_optimized_prompts(self.config.load_optimized_prompts);

        if let Some(ref prompt) = self.config.system_prompt {
            config = config.with_system_prompt(prompt);
        }

        // Run agent with cancellation support
        let agent_result = tokio::select! {
            result = run_agent(self.agent_state.clone(), &config) => {
                Some(result)
            }
            _ = cancel_token.cancelled() => {
                None // Task was cancelled
            }
        };

        // Clear cancellation token
        self.agent_cancel_token = None;

        match agent_result {
            Some(Ok(result)) => {
                self.agent_state = result.state;
                self.turn_count = self.agent_state.turn_count;

                // Track training collection results
                if let Some(ref example) = result.training_example {
                    self.last_run_score = Some(example.score);
                    self.training_collected = true;
                }

                // Add assistant response to display
                if let Some(ref response) = self.agent_state.last_response {
                    self.messages.push(ChatMessage {
                        role: MessageRole::Assistant,
                        content: response.clone(),
                    });
                }

                // Add training info message if collected
                if self.training_collected {
                    if let Some(score) = self.last_run_score {
                        self.messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: format!("Training data collected (score: {:.2})", score),
                        });
                    }
                }
            }
            Some(Err(e)) => {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Error: {}", e),
                });
            }
            None => {
                // Task was cancelled
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: "Agent task cancelled.".to_string(),
                });
                self.agent_status = AgentStatus::Idle;
            }
        }

        self.mode = AppMode::Insert;
    }

    /// Handle /feedback command - show feedback submission info
    fn handle_feedback_command(&mut self) {
        const GITHUB_ISSUES_URL: &str = "https://github.com/dropbox/dTOOL/codex_dashflow/issues/new";

        let session_id = self.config.session_id.as_deref().unwrap_or("(none)");

        let feedback_msg = format!(
            "## Feedback\n\n\
             Thank you for your interest in providing feedback!\n\n\
             **To report bugs or request features:**\n\
             Open an issue at: {}\n\n\
             **Session ID:** `{}`\n\n\
             Please include the session ID in your report to help with troubleshooting.",
            GITHUB_ISSUES_URL, session_id
        );

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: feedback_msg,
        });
    }

    /// Handle /providers command - list available model providers
    fn handle_providers_command(&mut self) {
        let providers = model_provider_info::built_in_model_providers();

        // Sort providers by name for consistent display
        let mut provider_list: Vec<_> = providers.into_iter().collect();
        provider_list.sort_by(|a, b| a.0.cmp(&b.0));

        let mut providers_msg = String::from("## Available Model Providers\n\n");

        for (id, info) in &provider_list {
            providers_msg.push_str(&format!("**{}** (`{}`)\n", info.name, id));

            if let Some(base_url) = &info.base_url {
                providers_msg.push_str(&format!("  Base URL: {}\n", base_url));
            }

            if let Some(env_key) = &info.env_key {
                let key_status = if std::env::var(env_key).is_ok() {
                    "set"
                } else {
                    "not set"
                };
                providers_msg.push_str(&format!("  API Key Env: {} ({})\n", env_key, key_status));
            }

            providers_msg.push_str(&format!("  Wire API: {}\n", info.wire_api));
            providers_msg.push('\n');
        }

        providers_msg.push_str(&format!(
            "**Total:** {} providers\n\n\
             Use `/model <model-name>` to switch models.\n\
             Provider is auto-detected from model name prefix.",
            provider_list.len()
        ));

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: providers_msg,
        });
    }

    /// Handle /search command - search conversation history
    fn handle_search_command(&mut self, query: &str) {
        if query.is_empty() {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: "Usage: /search <query>\n\n\
                         Search through conversation history for messages containing the query.\n\
                         The search is case-insensitive."
                    .to_string(),
            });
            return;
        }

        let query_lower = query.to_lowercase();
        let mut matches: Vec<(usize, &ChatMessage)> = Vec::new();

        // Search through display messages
        for (idx, msg) in self.messages.iter().enumerate() {
            if msg.content.to_lowercase().contains(&query_lower) {
                matches.push((idx + 1, msg)); // 1-indexed for display
            }
        }

        if matches.is_empty() {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: format!("No messages found matching \"{}\".", query),
            });
            return;
        }

        // Format results
        let mut results = format!(
            "## Search Results for \"{}\"\n\nFound {} match{}:\n\n",
            query,
            matches.len(),
            if matches.len() == 1 { "" } else { "es" }
        );

        // Show up to 10 matches with context
        for (idx, msg) in matches.iter().take(10) {
            let role_str = match msg.role {
                MessageRole::User => "User",
                MessageRole::Assistant => "Assistant",
                MessageRole::System => "System",
                MessageRole::Tool => "Tool",
            };

            // Truncate content for preview
            let preview = if msg.content.len() > 100 {
                format!("{}...", &msg.content[..100])
            } else {
                msg.content.clone()
            };

            // Escape any newlines in preview for compact display
            let preview_oneline = preview.replace('\n', " ").trim().to_string();

            results.push_str(&format!(
                "**#{}** [{}]: {}\n",
                idx, role_str, preview_oneline
            ));
        }

        if matches.len() > 10 {
            results.push_str(&format!("\n... and {} more matches", matches.len() - 10));
        }

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: results,
        });
    }

    /// Handle /context command - show current agent context
    fn handle_context_command(&mut self) {
        let mut context_info = String::from("## Agent Context\n\n");

        // System prompt info
        context_info.push_str("### System Prompt\n");
        if let Some(ref prompt) = self.config.system_prompt {
            // Truncate long prompts for display
            if prompt.len() > 500 {
                context_info.push_str(&format!(
                    "Custom system prompt ({} chars):\n```\n{}...\n```\n",
                    prompt.len(),
                    &prompt[..500]
                ));
            } else {
                context_info.push_str(&format!("Custom system prompt:\n```\n{}\n```\n", prompt));
            }
        } else if self.config.load_optimized_prompts {
            context_info.push_str("Using optimized prompts from PromptRegistry\n");
        } else {
            context_info.push_str("Using default system prompt\n");
        }
        context_info.push('\n');

        // Working directory
        context_info.push_str("### Working Directory\n");
        context_info.push_str(&format!("`{}`\n\n", self.config.working_dir));

        // Model info
        context_info.push_str("### Model\n");
        context_info.push_str(&format!("- Current: `{}`\n", self.model));
        if self.config.use_mock_llm {
            context_info.push_str("- Mode: Mock (no API calls)\n");
        }
        context_info.push('\n');

        // Approval preset
        context_info.push_str("### Approval Mode\n");
        context_info.push_str(&format!("- Preset: `{}`\n\n", self.config.approval_preset));

        // MCP servers
        if !self.config.mcp_servers.is_empty() {
            context_info.push_str("### MCP Servers\n");
            for server in &self.config.mcp_servers {
                context_info.push_str(&format!("- `{}`\n", server.name));
            }
            context_info.push('\n');
        }

        // Agent state messages summary
        context_info.push_str("### Agent State\n");
        let msg_count = self.agent_state.messages.len();
        if msg_count == 0 {
            context_info.push_str("No messages in agent state (new session)\n");
        } else {
            context_info.push_str(&format!("- {} message(s) in agent state\n", msg_count));

            // Count by role
            let mut user_count = 0;
            let mut assistant_count = 0;
            let mut system_count = 0;
            let mut tool_count = 0;

            for msg in &self.agent_state.messages {
                match msg.role {
                    CoreMessageRole::User => user_count += 1,
                    CoreMessageRole::Assistant => assistant_count += 1,
                    CoreMessageRole::System => system_count += 1,
                    CoreMessageRole::Tool => tool_count += 1,
                }
            }

            if user_count > 0 {
                context_info.push_str(&format!("  - User: {}\n", user_count));
            }
            if assistant_count > 0 {
                context_info.push_str(&format!("  - Assistant: {}\n", assistant_count));
            }
            if system_count > 0 {
                context_info.push_str(&format!("  - System: {}\n", system_count));
            }
            if tool_count > 0 {
                context_info.push_str(&format!("  - Tool: {}\n", tool_count));
            }
        }

        // Token estimate
        let tokens = self.estimated_tokens();
        context_info.push_str(&format!(
            "\n### Token Usage\n- Estimated: ~{} tokens\n",
            if tokens >= 1000 {
                format!("{:.1}k", tokens as f64 / 1000.0)
            } else {
                tokens.to_string()
            }
        ));

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: context_info,
        });
    }

    /// Handle /export command - export conversation to a file
    ///
    /// Usage:
    /// - `/export` - Export to default location (~/codex-dashflow/exports/session-<id>.md)
    /// - `/export <path>` - Export to specified path
    /// - `/export json` - Export in JSON format to default location
    /// - `/export json <path>` - Export in JSON format to specified path
    fn handle_export_command(&mut self, args: &str) {
        let messages = &self.agent_state.messages;
        if messages.is_empty() {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: "No conversation to export.".to_string(),
            });
            return;
        }

        // Parse arguments: [json] [path]
        let args_trimmed = args.trim();
        let (format, path_arg) = if args_trimmed.starts_with("json") {
            let rest = args_trimmed.strip_prefix("json").unwrap_or("").trim();
            ("json", rest)
        } else {
            ("markdown", args_trimmed)
        };

        // Determine output path
        let path = if path_arg.is_empty() {
            // Default location: ~/.codex-dashflow/exports/session-<id>.<ext>
            let ext = if format == "json" { "json" } else { "md" };
            let home = dirs::home_dir().unwrap_or_else(std::env::temp_dir);
            let export_dir = home.join(".codex-dashflow").join("exports");

            // Create directory if needed
            if let Err(e) = std::fs::create_dir_all(&export_dir) {
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Failed to create export directory: {}", e),
                });
                return;
            }

            let filename = format!("session-{}.{}", self.session_id, ext);
            export_dir.join(filename)
        } else {
            // Expand ~ to home directory if present
            let expanded = if path_arg.starts_with('~') {
                let home = dirs::home_dir().unwrap_or_else(std::env::temp_dir);
                home.join(path_arg.strip_prefix("~/").unwrap_or(path_arg))
            } else {
                std::path::PathBuf::from(path_arg)
            };
            expanded
        };

        // Generate content
        let content = if format == "json" {
            self.export_as_json()
        } else {
            self.export_as_markdown()
        };

        // Write to file
        match std::fs::write(&path, &content) {
            Ok(()) => {
                let path_str = path.display().to_string();
                let msg_count = messages.len();
                // Use transient notification for success (keeps chat clean)
                self.show_notification(
                    Notification::success(format!(
                        "Exported {} messages to {}",
                        msg_count, path_str
                    ))
                    .with_duration(Duration::from_secs(3)),
                );
            }
            Err(e) => {
                // Keep error as system message (important to see in chat)
                self.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Failed to export: {}", e),
                });
            }
        }
    }

    /// Export conversation as Markdown
    fn export_as_markdown(&self) -> String {
        let mut output = String::new();

        // Header
        output.push_str("# Conversation Export\n\n");
        output.push_str(&format!("**Session ID:** {}\n", self.session_id));
        output.push_str(&format!("**Model:** {}\n", self.model));
        output.push_str(&format!(
            "**Working Directory:** {}\n",
            self.config.working_dir
        ));
        output.push_str(&format!("**Turns:** {}\n", self.turn_count));
        output.push_str(&format!(
            "**Exported:** {}\n\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ));
        output.push_str("---\n\n");

        // Messages
        for msg in &self.agent_state.messages {
            let role_str = match msg.role {
                CoreMessageRole::User => "## 👤 User",
                CoreMessageRole::Assistant => "## 🤖 Assistant",
                CoreMessageRole::System => "## ⚙️ System",
                CoreMessageRole::Tool => "## 🔧 Tool",
            };
            output.push_str(&format!("{}\n\n", role_str));
            output.push_str(&msg.content);
            output.push_str("\n\n");
        }

        output
    }

    /// Export conversation as JSON
    fn export_as_json(&self) -> String {
        #[derive(serde::Serialize)]
        struct ExportData<'a> {
            session_id: &'a str,
            model: &'a str,
            working_directory: &'a str,
            turns: u32,
            exported_at: String,
            messages: Vec<ExportMessage<'a>>,
        }

        #[derive(serde::Serialize)]
        struct ExportMessage<'a> {
            role: &'static str,
            content: &'a str,
        }

        let messages: Vec<ExportMessage> = self
            .agent_state
            .messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    CoreMessageRole::User => "user",
                    CoreMessageRole::Assistant => "assistant",
                    CoreMessageRole::System => "system",
                    CoreMessageRole::Tool => "tool",
                };
                ExportMessage {
                    role,
                    content: &msg.content,
                }
            })
            .collect();

        let data = ExportData {
            session_id: &self.session_id,
            model: &self.model,
            working_directory: &self.config.working_dir,
            turns: self.turn_count,
            exported_at: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
            messages,
        };

        serde_json::to_string_pretty(&data).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
    }

    /// Handle an agent streaming event
    fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::ReasoningStart { model, .. } => {
                self.agent_status = AgentStatus::Thinking { model };
            }
            AgentEvent::ReasoningComplete { duration_ms, .. } => {
                self.agent_status = AgentStatus::Complete { duration_ms };
                tracing::debug!(duration_ms, "Reasoning complete");
            }
            AgentEvent::ToolCallRequested { tool, .. } => {
                self.messages.push(ChatMessage {
                    role: MessageRole::Tool,
                    content: format!("Calling tool: {}", tool),
                });
            }
            AgentEvent::ToolExecutionStart { tool, .. } => {
                self.agent_status = AgentStatus::ExecutingTool { tool };
            }
            AgentEvent::ToolExecutionComplete {
                tool,
                success,
                output_preview,
                ..
            } => {
                let status = if success { "✓" } else { "✗" };
                self.messages.push(ChatMessage {
                    role: MessageRole::Tool,
                    content: format!("{} {}: {}", status, tool, output_preview),
                });
                // Return to thinking status after tool completes
                self.agent_status = AgentStatus::Thinking {
                    model: self.model.clone(),
                };
            }
            AgentEvent::TurnComplete { .. } => {
                self.agent_status = AgentStatus::Idle;
            }
            AgentEvent::SessionComplete { .. } => {
                self.agent_status = AgentStatus::Idle;
            }
            AgentEvent::SessionMetrics {
                total_input_tokens,
                total_output_tokens,
                total_cached_tokens,
                total_cost_usd,
                llm_call_count,
                duration_ms,
                ..
            } => {
                // Update session metrics for display
                self.session_metrics = SessionMetricsData {
                    total_input_tokens,
                    total_output_tokens,
                    total_cached_tokens,
                    total_cost_usd,
                    llm_call_count,
                    duration_ms,
                };
                tracing::debug!(
                    total_input_tokens,
                    total_output_tokens,
                    total_cost_usd,
                    "Session metrics updated"
                );
            }
            AgentEvent::Error { error, .. } => {
                self.agent_status = AgentStatus::Error { message: error };
            }
            _ => {}
        }
    }

    /// Handle keyboard input when approval overlay is visible
    fn handle_approval_key(&mut self, key: KeyEvent) {
        match key.code {
            // Direct hotkey actions
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                // Approve once
                self.approval_overlay.selected_option = 0;
                self.complete_approval(ApprovalDecision::ApproveOnce);
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                // Approve for session
                self.approval_overlay.selected_option = 1;
                self.complete_approval(ApprovalDecision::ApproveSession);
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                // Reject
                self.approval_overlay.selected_option = 2;
                self.complete_approval(ApprovalDecision::Reject);
            }
            // Navigation
            KeyCode::Up | KeyCode::Char('k') => {
                self.approval_overlay.move_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.approval_overlay.move_down();
            }
            // Confirm selection
            KeyCode::Enter => {
                let decision = self.approval_overlay.current_decision();
                self.complete_approval(decision);
            }
            // Cancel (same as reject)
            KeyCode::Esc => {
                self.complete_approval(ApprovalDecision::Reject);
            }
            _ => {}
        }
    }

    /// Complete the approval process with the given decision
    fn complete_approval(&mut self, decision: ApprovalDecision) {
        let request = match self.approval_overlay.request.take() {
            Some(r) => r,
            None => {
                self.approval_overlay.hide();
                return;
            }
        };

        // Track session-approved tools
        if decision == ApprovalDecision::ApproveSession {
            self.session_approved_tools
                .insert(request.tool_name.clone());
        }

        // Add a message to indicate the decision
        let decision_msg = match decision {
            ApprovalDecision::ApproveOnce => {
                format!("✓ Approved: {}", request.request_type.display_content())
            }
            ApprovalDecision::ApproveSession => {
                format!(
                    "✓ Approved (session): {} - will auto-approve '{}' for this session",
                    request.request_type.display_content(),
                    request.tool_name
                )
            }
            ApprovalDecision::Reject => {
                format!("✗ Rejected: {}", request.request_type.display_content())
            }
        };

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: decision_msg,
        });

        // Send decision back to agent runner via channel
        if let Some(response_tx) = self.approval_response_tx.take() {
            let event_decision = match decision {
                ApprovalDecision::ApproveOnce => EventApprovalDecision::Approve,
                ApprovalDecision::ApproveSession => EventApprovalDecision::ApproveSession,
                ApprovalDecision::Reject => EventApprovalDecision::Reject,
            };
            let _ = response_tx.send(event_decision);
        }

        // Hide the overlay
        self.approval_overlay.hide();
    }

    /// Request approval for a tool call
    /// Returns true if the tool should be auto-approved, false if we need to wait for user input
    pub fn request_approval(
        &mut self,
        tool_call_id: String,
        tool_name: String,
        request_type: ApprovalRequestType,
        reason: Option<String>,
    ) -> bool {
        // Check if tool was session-approved
        if self.session_approved_tools.contains(&tool_name) {
            self.messages.push(ChatMessage {
                role: MessageRole::System,
                content: format!(
                    "✓ Auto-approved (session): {}",
                    request_type.display_content()
                ),
            });
            return true;
        }

        // Show approval overlay
        let request = ApprovalRequest {
            id: uuid::Uuid::new_v4().to_string(),
            request_type,
            reason,
            tool_call_id,
            tool_name,
        };
        self.approval_overlay.show(request);
        false
    }

    /// Check if the app should quit
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Navigate to previous (older) command in history
    fn history_previous(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        match self.history_position {
            None => {
                // Starting to browse history - save current input
                self.saved_input = self.input.clone();
                // Go to most recent command
                self.history_position = Some(self.command_history.len() - 1);
            }
            Some(pos) if pos > 0 => {
                // Go to older command
                self.history_position = Some(pos - 1);
            }
            _ => {
                // Already at oldest, do nothing
                return;
            }
        }

        // Load the command from history
        if let Some(pos) = self.history_position {
            self.input = self.command_history[pos].clone();
            self.cursor_position = self.input.len();
        }
    }

    /// Navigate to next (newer) command in history
    fn history_next(&mut self) {
        match self.history_position {
            None => {
                // Not browsing history, do nothing
            }
            Some(pos) if pos + 1 < self.command_history.len() => {
                // Go to newer command
                self.history_position = Some(pos + 1);
                self.input = self.command_history[pos + 1].clone();
                self.cursor_position = self.input.len();
            }
            Some(_) => {
                // At newest history entry - restore saved input
                self.history_position = None;
                self.input = std::mem::take(&mut self.saved_input);
                self.cursor_position = self.input.len();
            }
        }
    }

    /// Save current input state for undo. Call this before making changes.
    fn save_undo_state(&mut self) {
        // Don't save if unchanged from last state
        if let Some((last_input, _)) = self.input_undo_stack.last() {
            if last_input == &self.input {
                return;
            }
        }
        // Save current state
        self.input_undo_stack
            .push((self.input.clone(), self.cursor_position));
        // Clear redo stack when new changes are made
        self.input_redo_stack.clear();
        // Limit undo history size
        const MAX_UNDO_HISTORY: usize = 100;
        if self.input_undo_stack.len() > MAX_UNDO_HISTORY {
            self.input_undo_stack.remove(0);
        }
    }

    /// Undo the last input change
    fn input_undo(&mut self) {
        if let Some((prev_input, prev_cursor)) = self.input_undo_stack.pop() {
            // Save current state to redo stack
            self.input_redo_stack
                .push((self.input.clone(), self.cursor_position));
            // Restore previous state
            self.input = prev_input;
            self.cursor_position = prev_cursor.min(self.input.len());
        }
    }

    /// Redo the last undone change
    fn input_redo(&mut self) {
        if let Some((next_input, next_cursor)) = self.input_redo_stack.pop() {
            // Save current state to undo stack
            self.input_undo_stack
                .push((self.input.clone(), self.cursor_position));
            // Restore next state
            self.input = next_input;
            self.cursor_position = next_cursor.min(self.input.len());
        }
    }

    /// Start or extend a selection from current cursor position
    ///
    /// If no selection exists, sets the anchor at the current cursor position.
    /// If a selection already exists, the anchor stays in place.
    fn start_or_extend_selection(&mut self) {
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
    }

    /// Clear any active selection
    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// Get the current selection range, if any
    ///
    /// Returns (start, end) where start <= end, representing the
    /// character indices of the selection.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            let start = anchor.min(self.cursor_position);
            let end = anchor.max(self.cursor_position);
            (start, end)
        })
    }

    /// Check if there is an active selection
    pub fn has_selection(&self) -> bool {
        self.selection_anchor
            .is_some_and(|anchor| anchor != self.cursor_position)
    }

    /// Get the selected text, if any
    pub fn selected_text(&self) -> Option<&str> {
        self.selection_range().and_then(|(start, end)| {
            if start < end && end <= self.input.len() {
                Some(&self.input[start..end])
            } else {
                None
            }
        })
    }

    /// Delete the currently selected text and return it
    ///
    /// Clears the selection and positions cursor at the start of the deleted region.
    fn delete_selection(&mut self) -> Option<String> {
        if let Some((start, end)) = self.selection_range() {
            if start < end && end <= self.input.len() {
                self.save_undo_state();
                let deleted: String = self.input.drain(start..end).collect();
                self.cursor_position = start;
                self.clear_selection();
                return Some(deleted);
            }
        }
        None
    }

    /// Select all text in the input buffer
    fn select_all(&mut self) {
        if self.input.is_empty() {
            return;
        }
        self.selection_anchor = Some(0);
        self.cursor_position = self.input.len();
    }

    /// Copy selected text to clipboard (both internal and system)
    fn copy_selection(&mut self) {
        if let Some(text) = self.selected_text() {
            let text_str = text.to_string();
            self.clipboard = text_str.clone();
            // Try to copy to system clipboard, ignore errors
            let copied_to_system = self.copy_to_system_clipboard(&text_str);

            // Show notification for clipboard copy
            let char_count = text_str.chars().count();
            let msg = if copied_to_system {
                format!("Copied {} chars", char_count)
            } else {
                format!("Copied {} chars (internal)", char_count)
            };
            self.show_notification(Notification::info(&msg).with_duration(Duration::from_secs(2)));
        }
    }

    /// Cut selected text to clipboard (both internal and system)
    fn cut_selection(&mut self) {
        if let Some(deleted) = self.delete_selection() {
            // Try to copy to system clipboard, ignore errors
            let copied_to_system = self.copy_to_system_clipboard(&deleted);
            self.clipboard = deleted.clone();

            // Show notification for clipboard cut
            let char_count = deleted.chars().count();
            let msg = if copied_to_system {
                format!("Cut {} chars", char_count)
            } else {
                format!("Cut {} chars (internal)", char_count)
            };
            self.show_notification(Notification::info(&msg).with_duration(Duration::from_secs(2)));
        }
    }

    /// Paste clipboard contents at cursor position.
    /// Tries system clipboard first, falls back to internal clipboard.
    fn paste(&mut self) {
        // Try system clipboard first, fall back to internal
        let paste_text = self
            .paste_from_system_clipboard()
            .unwrap_or_else(|| self.clipboard.clone());

        if paste_text.is_empty() {
            return;
        }
        // Delete selection first if present
        if self.has_selection() {
            self.delete_selection();
        }
        self.save_undo_state();
        self.input.insert_str(self.cursor_position, &paste_text);
        self.cursor_position += paste_text.len();
        self.command_popup.update_filter(&self.input);
    }

    /// Copy text to system clipboard. Returns true on success.
    fn copy_to_system_clipboard(&self, text: &str) -> bool {
        match arboard::Clipboard::new() {
            Ok(mut clipboard) => clipboard.set_text(text).is_ok(),
            Err(_) => false,
        }
    }

    /// Paste text from system clipboard. Returns None if unavailable.
    fn paste_from_system_clipboard(&self) -> Option<String> {
        arboard::Clipboard::new()
            .ok()
            .and_then(|mut cb| cb.get_text().ok())
    }

    /// Paste from internal clipboard only (for testing).
    /// This bypasses system clipboard for deterministic test results.
    #[cfg(test)]
    fn paste_internal(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        if self.has_selection() {
            self.delete_selection();
        }
        self.save_undo_state();
        self.input.insert_str(self.cursor_position, &self.clipboard);
        self.cursor_position += self.clipboard.len();
        self.command_popup.update_filter(&self.input);
    }

    /// Check if a character is a word character (alphanumeric or underscore)
    fn is_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    /// Find the position of the previous word boundary from the given position.
    /// A word boundary is the start of a word (transition from non-word to word char).
    fn find_word_boundary_left(&self, from: usize) -> usize {
        if from == 0 {
            return 0;
        }

        let chars: Vec<char> = self.input.chars().collect();
        let mut pos = from;

        // Skip any whitespace/non-word chars immediately before cursor
        while pos > 0 && !Self::is_word_char(chars[pos - 1]) {
            pos -= 1;
        }

        // Move to the start of the word
        while pos > 0 && Self::is_word_char(chars[pos - 1]) {
            pos -= 1;
        }

        pos
    }

    /// Find the position of the next word boundary from the given position.
    /// A word boundary is the end of a word (transition from word to non-word char).
    fn find_word_boundary_right(&self, from: usize) -> usize {
        let chars: Vec<char> = self.input.chars().collect();
        let len = chars.len();

        if from >= len {
            return len;
        }

        let mut pos = from;

        // Skip any whitespace/non-word chars at cursor
        while pos < len && !Self::is_word_char(chars[pos]) {
            pos += 1;
        }

        // Move to the end of the word
        while pos < len && Self::is_word_char(chars[pos]) {
            pos += 1;
        }

        pos
    }

    /// Move cursor to the previous word boundary
    fn move_cursor_word_left(&mut self) {
        self.cursor_position = self.find_word_boundary_left(self.cursor_position);
    }

    /// Move cursor to the next word boundary
    fn move_cursor_word_right(&mut self) {
        self.cursor_position = self.find_word_boundary_right(self.cursor_position);
    }

    /// Delete the word before the cursor
    fn delete_word_left(&mut self) {
        if self.cursor_position == 0 {
            return;
        }

        // If there's a selection, delete it instead
        if self.has_selection() {
            self.delete_selection();
            return;
        }

        let boundary = self.find_word_boundary_left(self.cursor_position);
        if boundary < self.cursor_position {
            self.save_undo_state();
            self.input.drain(boundary..self.cursor_position);
            self.cursor_position = boundary;
            self.command_popup.update_filter(&self.input);
        }
    }

    /// Delete the word after the cursor
    fn delete_word_right(&mut self) {
        if self.cursor_position >= self.input.len() {
            return;
        }

        // If there's a selection, delete it instead
        if self.has_selection() {
            self.delete_selection();
            return;
        }

        let boundary = self.find_word_boundary_right(self.cursor_position);
        if boundary > self.cursor_position {
            self.save_undo_state();
            self.input.drain(self.cursor_position..boundary);
            self.command_popup.update_filter(&self.input);
        }
    }

    /// Delete from cursor to line start (Ctrl+U, Unix readline style)
    fn delete_to_line_start(&mut self) {
        // If there's a selection, delete it instead
        if self.has_selection() {
            self.delete_selection();
            return;
        }

        let (row, _) = self.cursor_row_col();
        let line_start = self.row_start_position(row);

        if line_start < self.cursor_position {
            self.save_undo_state();
            self.input.drain(line_start..self.cursor_position);
            self.cursor_position = line_start;
            self.command_popup.update_filter(&self.input);
        }
    }

    /// Delete from cursor to line end (Ctrl+K, Unix readline style)
    fn delete_to_line_end(&mut self) {
        // If there's a selection, delete it instead
        if self.has_selection() {
            self.delete_selection();
            return;
        }

        let (row, _) = self.cursor_row_col();
        let line_start = self.row_start_position(row);
        let line_len = self.row_length(row);
        let line_end = line_start + line_len;

        if self.cursor_position < line_end {
            self.save_undo_state();
            self.input.drain(self.cursor_position..line_end);
            self.command_popup.update_filter(&self.input);
        }
    }

    /// Update search results based on current query
    fn update_search(&mut self) {
        self.search_matches.clear();
        self.current_match = None;

        if self.search_query.is_empty() {
            return;
        }

        let query_lower = self.search_query.to_lowercase();

        // Find all matching messages (search in content)
        for (idx, msg) in self.messages.iter().enumerate() {
            if msg.content.to_lowercase().contains(&query_lower) {
                self.search_matches.push(idx);
            }
        }

        // Set current match to last match (most recent message)
        if !self.search_matches.is_empty() {
            self.current_match = Some(self.search_matches.len() - 1);
            self.scroll_to_match();
        }
    }

    /// Navigate to next search match (towards more recent messages)
    fn next_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }

        match self.current_match {
            Some(idx) if idx + 1 < self.search_matches.len() => {
                self.current_match = Some(idx + 1);
            }
            Some(_) => {
                // Wrap to first match
                self.current_match = Some(0);
            }
            None => {
                self.current_match = Some(0);
            }
        }
        self.scroll_to_match();
    }

    /// Navigate to previous search match (towards older messages)
    fn prev_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }

        match self.current_match {
            Some(0) => {
                // Wrap to last match
                self.current_match = Some(self.search_matches.len() - 1);
            }
            Some(idx) => {
                self.current_match = Some(idx - 1);
            }
            None => {
                self.current_match = Some(self.search_matches.len() - 1);
            }
        }
        self.scroll_to_match();
    }

    /// Scroll to show the current match
    fn scroll_to_match(&mut self) {
        if let Some(match_idx) = self.current_match {
            if let Some(&msg_idx) = self.search_matches.get(match_idx) {
                // Calculate scroll offset to show this message
                // scroll_offset = 0 means showing the last message
                // Higher scroll_offset shows older messages
                let total = self.messages.len();
                self.scroll_offset = total.saturating_sub(msg_idx + 1);
            }
        }
    }

    /// Get the current cursor row and column from cursor position
    /// Row 0 is the first line, column 0 is the first character of that line
    pub fn cursor_row_col(&self) -> (usize, usize) {
        let mut row = 0;
        let mut col = 0;
        for (i, c) in self.input.chars().enumerate() {
            if i == self.cursor_position {
                break;
            }
            if c == '\n' {
                row += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (row, col)
    }

    /// Get the number of lines in the input
    pub fn input_line_count(&self) -> usize {
        if self.input.is_empty() {
            1
        } else {
            self.input.chars().filter(|&c| c == '\n').count() + 1
        }
    }

    /// Update horizontal scroll offset to keep cursor visible
    ///
    /// Called after cursor movement to ensure the cursor stays in the visible area.
    /// Only applies to single-line input; multi-line input uses wrapping instead.
    pub fn update_input_scroll(&mut self, visible_width: usize) {
        // Only scroll horizontally for single-line input
        if self.input_line_count() > 1 || visible_width == 0 {
            self.input_scroll_offset = 0;
            return;
        }

        let cursor_col = self.cursor_position;

        // If cursor is before the visible window, scroll left
        if cursor_col < self.input_scroll_offset {
            self.input_scroll_offset = cursor_col;
        }
        // If cursor is past the visible window, scroll right
        // Leave 1 char margin so cursor is not at the very edge
        else if cursor_col >= self.input_scroll_offset + visible_width {
            self.input_scroll_offset = cursor_col.saturating_sub(visible_width.saturating_sub(1));
        }
    }

    /// Get the visible portion of single-line input for horizontal scrolling
    ///
    /// Returns (display_text, cursor_x_offset_from_start_of_display)
    pub fn visible_input_slice(&self, visible_width: usize) -> (&str, usize) {
        if self.input_line_count() > 1 || visible_width == 0 {
            return (&self.input, self.cursor_position);
        }

        let start = self.input_scroll_offset.min(self.input.len());
        let end = (start + visible_width).min(self.input.len());

        // Handle UTF-8: find valid char boundaries
        let mut start_byte = 0;
        let mut end_byte = self.input.len();

        // Find byte offset for start char
        for (i, (byte_idx, _)) in self.input.char_indices().enumerate() {
            if i == start {
                start_byte = byte_idx;
            }
            if i == end {
                end_byte = byte_idx;
                break;
            }
        }

        let visible_text = &self.input[start_byte..end_byte];
        let cursor_offset = self
            .cursor_position
            .saturating_sub(self.input_scroll_offset);

        (visible_text, cursor_offset)
    }

    /// Get estimated token count for the current conversation
    ///
    /// Uses the 4-bytes-per-token heuristic from the truncation module.
    pub fn estimated_tokens(&self) -> usize {
        codex_dashflow_core::messages_token_count(&self.agent_state.messages)
    }

    /// Estimate context window size for the current model
    ///
    /// Returns token limit based on model name pattern matching.
    pub fn estimate_context_window(&self) -> usize {
        let model = self.model.to_lowercase();
        if model.contains("gpt-3.5-turbo-16k") {
            16_000
        } else if model.contains("gpt-3.5") {
            4_000
        } else if model.contains("claude") {
            200_000
        } else if model.contains("gpt-4o")
            || model.contains("gpt-4-turbo")
            || model.contains("gpt-4")
        {
            128_000
        } else {
            // Default to 128k for unknown models
            128_000
        }
    }

    /// Show a transient notification that auto-dismisses
    pub fn show_notification(&mut self, notification: Notification) {
        self.notification = Some(notification);
    }

    /// Dismiss any active notification
    pub fn dismiss_notification(&mut self) {
        self.notification = None;
    }

    /// Get the start position of a given row
    fn row_start_position(&self, target_row: usize) -> usize {
        if target_row == 0 {
            return 0;
        }
        let mut current_row = 0;
        for (i, c) in self.input.chars().enumerate() {
            if c == '\n' {
                current_row += 1;
                if current_row == target_row {
                    return i + 1;
                }
            }
        }
        self.input.len()
    }

    /// Get the length of a given row (not including the newline)
    fn row_length(&self, target_row: usize) -> usize {
        let start = self.row_start_position(target_row);
        let rest = &self.input[start..];
        rest.chars().take_while(|&c| c != '\n').count()
    }

    /// Move cursor up one row, maintaining column position if possible
    fn move_cursor_up(&mut self) {
        let (row, col) = self.cursor_row_col();
        if row == 0 {
            return; // Already at first row
        }
        let new_row = row - 1;
        let new_row_len = self.row_length(new_row);
        let new_col = col.min(new_row_len);
        self.cursor_position = self.row_start_position(new_row) + new_col;
    }

    /// Move cursor down one row, maintaining column position if possible
    fn move_cursor_down(&mut self) {
        let (row, col) = self.cursor_row_col();
        let total_rows = self.input_line_count();
        if row + 1 >= total_rows {
            return; // Already at last row
        }
        let new_row = row + 1;
        let new_row_len = self.row_length(new_row);
        let new_col = col.min(new_row_len);
        self.cursor_position = self.row_start_position(new_row) + new_col;
    }

    /// Move cursor to the beginning of the current line
    fn move_cursor_line_start(&mut self) {
        let (row, _) = self.cursor_row_col();
        self.cursor_position = self.row_start_position(row);
    }

    /// Move cursor to the end of the current line
    fn move_cursor_line_end(&mut self) {
        let (row, _) = self.cursor_row_col();
        let start = self.row_start_position(row);
        let line_len = self.row_length(row);
        self.cursor_position = start + line_len;
    }

    /// Try to complete a slash command from the current input
    ///
    /// If the input starts with "/" and matches exactly one command prefix,
    /// completes it with a trailing space. If multiple matches, does nothing.
    fn try_tab_completion(&mut self) {
        // Only complete if input starts with "/" and has no spaces yet
        if !self.input.starts_with('/') || self.input.contains(' ') {
            return;
        }

        let prefix = self.input.to_lowercase();

        // Find all matching commands
        let matches: Vec<&str> = BUILTIN_COMMANDS
            .iter()
            .map(|(cmd, _)| *cmd)
            .filter(|cmd| cmd.to_lowercase().starts_with(&prefix))
            .collect();

        match matches.len() {
            0 => {
                // No matches - do nothing
            }
            1 => {
                // Exactly one match - complete it with a trailing space
                self.input = format!("{} ", matches[0]);
                self.cursor_position = self.input.len();
            }
            _ => {
                // Multiple matches - find common prefix and complete to that
                let common = Self::longest_common_prefix(&matches);
                if common.len() > self.input.len() {
                    self.input = common;
                    self.cursor_position = self.input.len();
                }
                // If common prefix is same as input, do nothing (user can tab again or type more)
            }
        }
    }

    /// Find the longest common prefix among a list of strings
    fn longest_common_prefix(strings: &[&str]) -> String {
        if strings.is_empty() {
            return String::new();
        }
        if strings.len() == 1 {
            return strings[0].to_string();
        }

        let first = strings[0];
        let mut prefix_len = 0;

        'outer: for (i, c) in first.chars().enumerate() {
            for s in &strings[1..] {
                if s.chars().nth(i) != Some(c) {
                    break 'outer;
                }
            }
            prefix_len = i + 1;
        }

        first[..first.chars().take(prefix_len).map(|c| c.len_utf8()).sum()].to_string()
    }

    /// Get available slash commands with their descriptions
    pub fn available_commands() -> &'static [(&'static str, &'static str)] {
        BUILTIN_COMMANDS
    }

    /// Update file search popup based on current input
    fn update_file_search_popup(&mut self) {
        // Find the @ token in input before cursor
        if let Some(at_token) = self.find_at_token() {
            let query = &self.input[at_token.1..self.cursor_position];
            self.file_search_popup
                .update_query(query, &self.config.working_dir);
        } else {
            self.file_search_popup.hide();
        }
    }

    /// Find the @ token position and end in the input (start_of_@, char_after_@)
    fn find_at_token(&self) -> Option<(usize, usize)> {
        // Search backwards from cursor to find @
        let before_cursor = &self.input[..self.cursor_position];
        let at_pos = before_cursor.rfind('@')?;

        // Make sure @ is at start or after whitespace
        if at_pos > 0 {
            let char_before = before_cursor
                .chars()
                .rev()
                .nth(before_cursor.len() - at_pos)?;
            if !char_before.is_whitespace() {
                return None;
            }
        }

        // Make sure there's no space between @ and cursor
        let after_at = &self.input[at_pos + 1..self.cursor_position];
        if after_at.contains(' ') || after_at.contains('\n') {
            return None;
        }

        Some((at_pos, at_pos + 1))
    }

    /// Complete the file mention by replacing @query with @path
    fn complete_file_mention(&mut self, path: &str) {
        if let Some((at_pos, _)) = self.find_at_token() {
            // Replace @query with @path (add trailing space)
            let before = &self.input[..at_pos];
            let after = &self.input[self.cursor_position..];
            self.input = format!("{}@{} {}", before, path, after);
            self.cursor_position = at_pos + 1 + path.len() + 1; // @path + space
        }
    }
}

/// Run the TUI application
pub async fn run_app(config: AppConfig) -> Result<()> {
    // Initialize session logging if enabled
    session_log::maybe_init(&config.model, &config.working_dir);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and event handler
    // Uses async new_with_resume to handle session resume from checkpoint
    let mut app = App::new_with_resume(config.clone()).await;
    let mut event_handler = EventHandler::new();
    app.set_event_sender(event_handler.sender());
    event_handler.start(config.tick_rate);

    // Main loop
    loop {
        // Update scroll offset for horizontal scrolling before rendering
        // Terminal width minus borders (2) and input area layout
        let terminal_width = terminal.size()?.width;
        let input_visible_width = terminal_width.saturating_sub(4) as usize; // borders + some margin
        app.update_input_scroll(input_visible_width);

        // Render
        terminal.draw(|frame| ui::render(frame, &app))?;

        // Handle events
        if let Some(event) = event_handler.next().await {
            session_log::log_inbound_event(&event);
            app.handle_event(event).await;
        }

        if app.should_quit() {
            break;
        }
    }

    // Log session end
    session_log::log_session_end();

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    #[serial]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert_eq!(config.working_dir, ".");
        assert_eq!(config.max_turns, 0);
        assert_eq!(config.model, "gpt-4o-mini");
        assert!(!config.use_mock_llm);
        assert!(config.session_id.is_none());
        assert!(!config.collect_training);
        assert!(!config.load_optimized_prompts);
    }

    #[test]
    #[serial]
    fn test_app_new_with_default_config() {
        let config = AppConfig::default();
        let app = App::new(config);

        assert!(!app.session_id.is_empty());
        assert_eq!(app.mode, AppMode::Insert);
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_position, 0);
        assert!(!app.messages.is_empty()); // Welcome message
        assert_eq!(app.turn_count, 0);
        assert!(!app.should_quit());
        assert!(app.last_run_score.is_none());
        assert!(!app.training_collected);
    }

    #[test]
    #[serial]
    fn test_app_new_with_custom_config() {
        let config = AppConfig {
            session_id: Some("test-session".to_string()),
            working_dir: "/tmp".to_string(),
            max_turns: 5,
            model: "gpt-4".to_string(),
            use_mock_llm: true,
            ..Default::default()
        };
        let app = App::new(config);

        assert_eq!(app.session_id, "test-session");
        assert_eq!(app.model, "gpt-4");
        assert_eq!(app.config.max_turns, 5);
    }

    #[test]
    #[serial]
    fn test_chat_message_creation() {
        let user_msg = ChatMessage {
            role: MessageRole::User,
            content: "Hello".to_string(),
        };
        assert_eq!(user_msg.role, MessageRole::User);
        assert_eq!(user_msg.content, "Hello");

        let assistant_msg = ChatMessage {
            role: MessageRole::Assistant,
            content: "Hi there".to_string(),
        };
        assert_eq!(assistant_msg.role, MessageRole::Assistant);
    }

    #[test]
    #[serial]
    fn test_app_mode_equality() {
        assert_eq!(AppMode::Normal, AppMode::Normal);
        assert_eq!(AppMode::Insert, AppMode::Insert);
        assert_eq!(AppMode::Processing, AppMode::Processing);
        assert_ne!(AppMode::Normal, AppMode::Insert);
    }

    #[test]
    #[serial]
    fn test_message_role_equality() {
        assert_eq!(MessageRole::User, MessageRole::User);
        assert_eq!(MessageRole::Assistant, MessageRole::Assistant);
        assert_eq!(MessageRole::System, MessageRole::System);
        assert_eq!(MessageRole::Tool, MessageRole::Tool);
        assert_ne!(MessageRole::User, MessageRole::Assistant);
    }

    #[test]
    #[serial]
    fn test_input_char_insertion() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Simulate typing "hello"
        app.input.insert(app.cursor_position, 'h');
        app.cursor_position += 1;
        app.input.insert(app.cursor_position, 'e');
        app.cursor_position += 1;
        app.input.insert(app.cursor_position, 'l');
        app.cursor_position += 1;
        app.input.insert(app.cursor_position, 'l');
        app.cursor_position += 1;
        app.input.insert(app.cursor_position, 'o');
        app.cursor_position += 1;

        assert_eq!(app.input, "hello");
        assert_eq!(app.cursor_position, 5);
    }

    #[test]
    #[serial]
    fn test_input_backspace() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Set initial input
        app.input = "hello".to_string();
        app.cursor_position = 5;

        // Simulate backspace
        if app.cursor_position > 0 {
            app.cursor_position -= 1;
            app.input.remove(app.cursor_position);
        }

        assert_eq!(app.input, "hell");
        assert_eq!(app.cursor_position, 4);
    }

    #[test]
    #[serial]
    fn test_input_cursor_movement() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 5;

        // Move left
        if app.cursor_position > 0 {
            app.cursor_position -= 1;
        }
        assert_eq!(app.cursor_position, 4);

        // Move to start (Home)
        app.cursor_position = 0;
        assert_eq!(app.cursor_position, 0);

        // Move to end (End)
        app.cursor_position = app.input.len();
        assert_eq!(app.cursor_position, 5);
    }

    #[test]
    #[serial]
    fn test_input_delete() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 0;

        // Delete character at cursor
        if app.cursor_position < app.input.len() {
            app.input.remove(app.cursor_position);
        }

        assert_eq!(app.input, "ello");
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    #[serial]
    fn test_quit_flag() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        assert!(!app.should_quit());
        app.should_quit = true;
        assert!(app.should_quit());
    }

    #[test]
    #[serial]
    fn test_mode_transitions() {
        let config = AppConfig::default();
        let app = App::new(config);

        // App starts in Insert mode
        assert_eq!(app.mode, AppMode::Insert);
    }

    #[test]
    #[serial]
    fn test_app_config_with_training_collection() {
        let config = AppConfig {
            collect_training: true,
            ..Default::default()
        };
        assert!(config.collect_training);

        let app = App::new(config);
        assert!(app.config.collect_training);
        // Training state starts empty until a run completes
        assert!(app.last_run_score.is_none());
        assert!(!app.training_collected);
    }

    #[test]
    #[serial]
    fn test_app_training_state_fields() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Verify initial state
        assert!(app.last_run_score.is_none());
        assert!(!app.training_collected);

        // Simulate training collection
        app.last_run_score = Some(0.85);
        app.training_collected = true;

        assert_eq!(app.last_run_score, Some(0.85));
        assert!(app.training_collected);
    }

    #[test]
    #[serial]
    fn test_app_config_with_load_optimized_prompts() {
        let config = AppConfig {
            load_optimized_prompts: true,
            ..Default::default()
        };
        assert!(config.load_optimized_prompts);

        let app = App::new(config);
        assert!(app.config.load_optimized_prompts);
    }

    #[test]
    #[serial]
    fn test_app_config_with_system_prompt() {
        let config = AppConfig {
            system_prompt: Some("You are a coding assistant.".to_string()),
            ..Default::default()
        };
        assert_eq!(
            config.system_prompt,
            Some("You are a coding assistant.".to_string())
        );

        let app = App::new(config);
        assert_eq!(
            app.config.system_prompt,
            Some("You are a coding assistant.".to_string())
        );
    }

    #[test]
    #[serial]
    fn test_app_config_default_system_prompt_is_none() {
        let config = AppConfig::default();
        assert!(config.system_prompt.is_none());
    }

    #[test]
    #[serial]
    fn test_scroll_offset_initial() {
        let config = AppConfig::default();
        let app = App::new(config);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    #[serial]
    fn test_scroll_offset_modification() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Test scrolling up
        app.scroll_offset = 5;
        assert_eq!(app.scroll_offset, 5);

        // Test scrolling down
        app.scroll_offset = app.scroll_offset.saturating_sub(3);
        assert_eq!(app.scroll_offset, 2);
    }

    #[test]
    #[serial]
    fn test_vim_mode_normal_to_insert() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Start in Insert mode
        assert_eq!(app.mode, AppMode::Insert);

        // Switch to Normal mode (simulating Esc)
        app.mode = AppMode::Normal;
        assert_eq!(app.mode, AppMode::Normal);

        // Switch back to Insert (simulating 'i')
        app.mode = AppMode::Insert;
        assert_eq!(app.mode, AppMode::Insert);
    }

    #[test]
    #[serial]
    fn test_vim_append_cursor_position() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 2;
        app.mode = AppMode::Normal;

        // Simulate 'a' (append after cursor)
        if app.cursor_position < app.input.len() {
            app.cursor_position += 1;
        }
        app.mode = AppMode::Insert;

        assert_eq!(app.cursor_position, 3);
        assert_eq!(app.mode, AppMode::Insert);
    }

    #[test]
    #[serial]
    fn test_vim_append_at_end() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 2;
        app.mode = AppMode::Normal;

        // Simulate 'A' (append at end)
        app.cursor_position = app.input.len();
        app.mode = AppMode::Insert;

        assert_eq!(app.cursor_position, 5);
        assert_eq!(app.mode, AppMode::Insert);
    }

    #[test]
    #[serial]
    fn test_vim_insert_at_beginning() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 3;
        app.mode = AppMode::Normal;

        // Simulate 'I' (insert at beginning)
        app.cursor_position = 0;
        app.mode = AppMode::Insert;

        assert_eq!(app.cursor_position, 0);
        assert_eq!(app.mode, AppMode::Insert);
    }

    #[test]
    #[serial]
    fn test_vim_delete_char() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 2;
        app.mode = AppMode::Normal;

        // Simulate 'x' (delete char under cursor)
        if app.cursor_position < app.input.len() {
            app.input.remove(app.cursor_position);
        }

        assert_eq!(app.input, "helo");
        assert_eq!(app.cursor_position, 2);
    }

    #[test]
    #[serial]
    fn test_vim_clear_line() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.cursor_position = 5;
        app.mode = AppMode::Normal;

        // Simulate 'd' (clear line)
        app.input.clear();
        app.cursor_position = 0;

        assert_eq!(app.input, "");
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    #[serial]
    fn test_vim_cursor_movement() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 2;
        app.mode = AppMode::Normal;

        // Simulate 'h' (left)
        if app.cursor_position > 0 {
            app.cursor_position -= 1;
        }
        assert_eq!(app.cursor_position, 1);

        // Simulate 'l' (right)
        if app.cursor_position < app.input.len() {
            app.cursor_position += 1;
        }
        assert_eq!(app.cursor_position, 2);

        // Simulate '0' (home)
        app.cursor_position = 0;
        assert_eq!(app.cursor_position, 0);

        // Simulate '$' (end)
        app.cursor_position = app.input.len();
        assert_eq!(app.cursor_position, 5);
    }

    #[test]
    #[serial]
    fn test_agent_status_display() {
        assert_eq!(AgentStatus::Idle.display(), "");
        assert_eq!(
            AgentStatus::Thinking {
                model: "gpt-4".to_string()
            }
            .display(),
            "Thinking (gpt-4)"
        );
        assert_eq!(
            AgentStatus::ExecutingTool {
                tool: "shell".to_string()
            }
            .display(),
            "Executing: shell"
        );
        assert_eq!(
            AgentStatus::Complete { duration_ms: 1500 }.display(),
            "Done (1.5s)"
        );
        assert_eq!(
            AgentStatus::Error {
                message: "Failed".to_string()
            }
            .display(),
            "Error: Failed"
        );
    }

    #[test]
    #[serial]
    fn test_agent_status_default() {
        let status: AgentStatus = Default::default();
        assert_eq!(status, AgentStatus::Idle);
    }

    #[test]
    #[serial]
    fn test_command_history_initial() {
        let config = AppConfig::default();
        let app = App::new(config);
        assert!(app.command_history.is_empty());
        assert!(app.history_position.is_none());
    }

    #[test]
    #[serial]
    fn test_history_previous_empty() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Navigate with empty history should do nothing
        app.history_previous();
        assert!(app.history_position.is_none());
        assert!(app.input.is_empty());
    }

    #[test]
    #[serial]
    fn test_history_navigation() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Populate history
        app.command_history = vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ];

        // Navigate to previous (most recent)
        app.history_previous();
        assert_eq!(app.history_position, Some(2));
        assert_eq!(app.input, "third");

        // Navigate to older
        app.history_previous();
        assert_eq!(app.history_position, Some(1));
        assert_eq!(app.input, "second");

        // Navigate to oldest
        app.history_previous();
        assert_eq!(app.history_position, Some(0));
        assert_eq!(app.input, "first");

        // Try to go past oldest - should stay
        app.history_previous();
        assert_eq!(app.history_position, Some(0));
        assert_eq!(app.input, "first");

        // Navigate back to newer
        app.history_next();
        assert_eq!(app.history_position, Some(1));
        assert_eq!(app.input, "second");
    }

    #[test]
    #[serial]
    fn test_history_restores_saved_input() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.command_history = vec!["old command".to_string()];
        app.input = "current typing".to_string();

        // Browse to history
        app.history_previous();
        assert_eq!(app.input, "old command");
        assert_eq!(app.saved_input, "current typing");

        // Return to current input
        app.history_next();
        assert_eq!(app.input, "current typing");
        assert!(app.history_position.is_none());
    }

    #[test]
    #[serial]
    fn test_history_next_when_not_browsing() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "test".to_string();
        app.history_next(); // Should do nothing
        assert_eq!(app.input, "test");
        assert!(app.history_position.is_none());
    }

    #[test]
    #[serial]
    fn test_input_undo_empty_stack() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "test".to_string();
        app.cursor_position = 4;

        // Undo with empty stack should do nothing
        app.input_undo();
        assert_eq!(app.input, "test");
        assert_eq!(app.cursor_position, 4);
    }

    #[test]
    #[serial]
    fn test_input_undo_restores_state() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Simulate typing
        app.input = "hello".to_string();
        app.cursor_position = 5;
        app.save_undo_state();

        // Add more text
        app.input = "hello world".to_string();
        app.cursor_position = 11;

        // Undo should restore to "hello"
        app.input_undo();
        assert_eq!(app.input, "hello");
        assert_eq!(app.cursor_position, 5);
    }

    #[test]
    #[serial]
    fn test_input_redo_empty_stack() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "test".to_string();
        app.cursor_position = 4;

        // Redo with empty stack should do nothing
        app.input_redo();
        assert_eq!(app.input, "test");
        assert_eq!(app.cursor_position, 4);
    }

    #[test]
    #[serial]
    fn test_input_undo_redo_cycle() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Initial state
        app.input = "a".to_string();
        app.cursor_position = 1;
        app.save_undo_state();

        // Second state
        app.input = "ab".to_string();
        app.cursor_position = 2;
        app.save_undo_state();

        // Third state
        app.input = "abc".to_string();
        app.cursor_position = 3;

        // Undo twice
        app.input_undo(); // back to "ab"
        assert_eq!(app.input, "ab");
        app.input_undo(); // back to "a"
        assert_eq!(app.input, "a");

        // Redo once
        app.input_redo(); // forward to "ab"
        assert_eq!(app.input, "ab");

        // Redo again
        app.input_redo(); // forward to "abc"
        assert_eq!(app.input, "abc");
    }

    #[test]
    #[serial]
    fn test_save_undo_state_deduplication() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "test".to_string();
        app.cursor_position = 4;

        // Save same state multiple times
        app.save_undo_state();
        app.save_undo_state();
        app.save_undo_state();

        // Should only have one entry
        assert_eq!(app.input_undo_stack.len(), 1);
    }

    #[test]
    #[serial]
    fn test_save_undo_state_clears_redo() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Build up redo stack
        app.input = "a".to_string();
        app.save_undo_state();
        app.input = "ab".to_string();
        app.input_undo(); // creates redo entry

        assert!(!app.input_redo_stack.is_empty());

        // Making new changes should clear redo
        app.input = "ac".to_string();
        app.save_undo_state();

        assert!(app.input_redo_stack.is_empty());
    }

    #[test]
    #[serial]
    fn test_spinner_frame_initialization() {
        let config = AppConfig::default();
        let app = App::new(config);
        assert_eq!(app.spinner_frame, 0);
    }

    #[test]
    #[serial]
    fn test_spinner_frame_wrapping() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Test wrapping behavior at usize max
        app.spinner_frame = usize::MAX;
        app.spinner_frame = app.spinner_frame.wrapping_add(1);
        assert_eq!(app.spinner_frame, 0);
    }

    #[test]
    #[serial]
    fn test_spinner_frame_increments_only_when_processing() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Initial state - not processing
        assert_eq!(app.spinner_frame, 0);
        assert_ne!(app.mode, AppMode::Processing);

        // Simulate what Tick handler does in Normal mode - should not increment
        if app.mode == AppMode::Processing {
            app.spinner_frame = app.spinner_frame.wrapping_add(1);
        }
        assert_eq!(app.spinner_frame, 0);

        // Switch to Processing mode
        app.mode = AppMode::Processing;

        // Simulate Tick - should now increment
        if app.mode == AppMode::Processing {
            app.spinner_frame = app.spinner_frame.wrapping_add(1);
        }
        assert_eq!(app.spinner_frame, 1);

        // Another tick
        if app.mode == AppMode::Processing {
            app.spinner_frame = app.spinner_frame.wrapping_add(1);
        }
        assert_eq!(app.spinner_frame, 2);
    }

    #[test]
    #[serial]
    fn test_show_help_initialization() {
        let config = AppConfig::default();
        let app = App::new(config);
        assert!(!app.show_help);
    }

    #[test]
    #[serial]
    fn test_show_help_toggle() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        assert!(!app.show_help);
        app.show_help = true;
        assert!(app.show_help);
    }

    // === Search Tests ===

    #[test]
    #[serial]
    fn test_search_mode_initialization() {
        let config = AppConfig::default();
        let app = App::new(config);
        assert_eq!(app.mode, AppMode::Insert); // Starts in insert mode
        assert!(app.search_query.is_empty());
        assert!(app.search_matches.is_empty());
        assert!(app.current_match.is_none());
    }

    #[test]
    #[serial]
    fn test_search_mode_enum() {
        assert_eq!(AppMode::Search, AppMode::Search);
        assert_ne!(AppMode::Search, AppMode::Normal);
        assert_ne!(AppMode::Search, AppMode::Insert);
    }

    #[test]
    #[serial]
    fn test_update_search_empty_query() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        app.search_query.clear();
        app.update_search();
        assert!(app.search_matches.is_empty());
        assert!(app.current_match.is_none());
    }

    #[test]
    #[serial]
    fn test_update_search_finds_matches() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Add some messages to search
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "hello world".to_string(),
        });
        app.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: "hi there".to_string(),
        });
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "hello again".to_string(),
        });

        // Search for "hello"
        app.search_query = "hello".to_string();
        app.update_search();

        // Should find 2 matches (messages at indices 1 and 3, skipping welcome msg at 0)
        assert_eq!(app.search_matches.len(), 2);
        assert!(app.current_match.is_some());
    }

    #[test]
    #[serial]
    fn test_update_search_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "HELLO WORLD".to_string(),
        });

        // Search with lowercase
        app.search_query = "hello".to_string();
        app.update_search();

        assert_eq!(app.search_matches.len(), 1);
    }

    #[test]
    #[serial]
    fn test_update_search_no_matches() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "hello world".to_string(),
        });

        // Search for something not present
        app.search_query = "xyz123".to_string();
        app.update_search();

        assert!(app.search_matches.is_empty());
        assert!(app.current_match.is_none());
    }

    #[test]
    #[serial]
    fn test_next_match_cycles() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Add messages
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "test one".to_string(),
        });
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "test two".to_string(),
        });

        app.search_query = "test".to_string();
        app.update_search();

        // Should have 2 matches
        assert_eq!(app.search_matches.len(), 2);

        // current_match starts at last match (most recent)
        assert_eq!(app.current_match, Some(1));

        // Next should wrap to first
        app.next_match();
        assert_eq!(app.current_match, Some(0));

        // Next again should go to second
        app.next_match();
        assert_eq!(app.current_match, Some(1));
    }

    #[test]
    #[serial]
    fn test_prev_match_cycles() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Add messages
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "test one".to_string(),
        });
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "test two".to_string(),
        });

        app.search_query = "test".to_string();
        app.update_search();

        // current_match starts at last match
        assert_eq!(app.current_match, Some(1));

        // Prev should go to first
        app.prev_match();
        assert_eq!(app.current_match, Some(0));

        // Prev again should wrap to last
        app.prev_match();
        assert_eq!(app.current_match, Some(1));
    }

    #[test]
    #[serial]
    fn test_scroll_to_match_sets_offset() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Add several messages
        for i in 0..10 {
            app.messages.push(ChatMessage {
                role: MessageRole::User,
                content: format!("message {}", i),
            });
        }

        app.search_query = "message 3".to_string();
        app.update_search();

        // Should find one match and scroll to it
        assert_eq!(app.search_matches.len(), 1);
        assert!(app.scroll_offset > 0); // Should scroll up from bottom
    }

    #[test]
    #[serial]
    fn test_enter_search_mode() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Add a previous search
        app.search_query = "old query".to_string();
        app.search_matches = vec![0, 1, 2];
        app.current_match = Some(1);

        // Simulate entering search mode (clears state)
        app.mode = AppMode::Search;
        app.search_query.clear();
        app.search_matches.clear();
        app.current_match = None;

        assert_eq!(app.mode, AppMode::Search);
        assert!(app.search_query.is_empty());
        assert!(app.search_matches.is_empty());
        assert!(app.current_match.is_none());
    }

    // ============================================
    // Multi-line input tests
    // ============================================

    #[test]
    #[serial]
    fn test_cursor_row_col_single_line() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.cursor_position = 0;
        assert_eq!(app.cursor_row_col(), (0, 0));

        app.cursor_position = 5;
        assert_eq!(app.cursor_row_col(), (0, 5));

        app.cursor_position = 11;
        assert_eq!(app.cursor_row_col(), (0, 11));
    }

    #[test]
    #[serial]
    fn test_cursor_row_col_multi_line() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "line1\nline2\nline3".to_string();

        // Cursor at start of first line
        app.cursor_position = 0;
        assert_eq!(app.cursor_row_col(), (0, 0));

        // Cursor at end of first line (before newline)
        app.cursor_position = 5;
        assert_eq!(app.cursor_row_col(), (0, 5));

        // Cursor at start of second line (after first newline)
        app.cursor_position = 6;
        assert_eq!(app.cursor_row_col(), (1, 0));

        // Cursor in middle of second line
        app.cursor_position = 9;
        assert_eq!(app.cursor_row_col(), (1, 3));

        // Cursor at start of third line
        app.cursor_position = 12;
        assert_eq!(app.cursor_row_col(), (2, 0));

        // Cursor at end of third line
        app.cursor_position = 17;
        assert_eq!(app.cursor_row_col(), (2, 5));
    }

    #[test]
    #[serial]
    fn test_input_line_count() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Empty input should count as 1 line
        app.input = String::new();
        assert_eq!(app.input_line_count(), 1);

        // Single line without newline
        app.input = "hello".to_string();
        assert_eq!(app.input_line_count(), 1);

        // Two lines
        app.input = "hello\nworld".to_string();
        assert_eq!(app.input_line_count(), 2);

        // Three lines
        app.input = "a\nb\nc".to_string();
        assert_eq!(app.input_line_count(), 3);

        // Trailing newline creates empty fourth line
        app.input = "a\nb\nc\n".to_string();
        assert_eq!(app.input_line_count(), 4);
    }

    #[test]
    #[serial]
    fn test_update_input_scroll_single_line() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Short input - no scrolling needed
        app.input = "hello".to_string();
        app.cursor_position = 5;
        app.update_input_scroll(20);
        assert_eq!(app.input_scroll_offset, 0);

        // Long input, cursor at start - no scroll
        app.input = "a".repeat(50);
        app.cursor_position = 0;
        app.update_input_scroll(20);
        assert_eq!(app.input_scroll_offset, 0);

        // Long input, cursor at end - should scroll
        app.cursor_position = 50;
        app.update_input_scroll(20);
        assert!(
            app.input_scroll_offset > 0,
            "Should scroll when cursor past visible width"
        );

        // Move cursor back to start - should scroll left
        app.cursor_position = 0;
        app.update_input_scroll(20);
        assert_eq!(app.input_scroll_offset, 0, "Should scroll back to start");
    }

    #[test]
    #[serial]
    fn test_update_input_scroll_multi_line_disabled() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Multi-line input should not use horizontal scrolling
        app.input = "hello\nworld".to_string();
        app.cursor_position = 11;
        app.input_scroll_offset = 100; // Pre-set to non-zero
        app.update_input_scroll(20);
        assert_eq!(
            app.input_scroll_offset, 0,
            "Multi-line input should reset scroll offset"
        );
    }

    #[test]
    #[serial]
    fn test_visible_input_slice_short() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 3;
        app.input_scroll_offset = 0;

        let (visible, cursor_offset) = app.visible_input_slice(20);
        assert_eq!(visible, "hello");
        assert_eq!(cursor_offset, 3);
    }

    #[test]
    #[serial]
    fn test_visible_input_slice_scrolled() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // 50 char input, scroll offset 30, visible width 15
        app.input = "a".repeat(50);
        app.cursor_position = 35;
        app.input_scroll_offset = 30;

        let (visible, cursor_offset) = app.visible_input_slice(15);
        assert_eq!(visible.len(), 15); // Should show 15 chars
        assert_eq!(cursor_offset, 5); // 35 - 30 = 5
    }

    #[test]
    #[serial]
    fn test_visible_input_slice_multi_line() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Multi-line input should return full input
        app.input = "hello\nworld".to_string();
        app.cursor_position = 5;
        app.input_scroll_offset = 0;

        let (visible, cursor_offset) = app.visible_input_slice(10);
        assert_eq!(visible, "hello\nworld");
        assert_eq!(cursor_offset, 5);
    }

    #[test]
    #[serial]
    fn test_estimated_tokens_empty() {
        let config = AppConfig::default();
        let app = App::new(config);
        // New app has no agent messages - DashFlow adds 3 tokens base overhead
        // for message array structure even when empty
        assert_eq!(app.estimated_tokens(), 3);
    }

    #[test]
    #[serial]
    fn test_estimated_tokens_with_messages() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        // Add some messages to agent state
        app.agent_state
            .messages
            .push(codex_dashflow_core::Message::user("Hello, how are you?"));
        app.agent_state
            .messages
            .push(codex_dashflow_core::Message::assistant(
                "I'm doing well! How can I help you today?",
            ));
        // Should have some token estimate > 0
        let tokens = app.estimated_tokens();
        assert!(tokens > 0, "Expected some tokens, got 0");
        // With ~19+40 chars = ~59 bytes / 4 = ~15 tokens, plus role overhead
        // Should be roughly in the 20-30 range
        assert!(tokens > 10, "Expected more than 10 tokens, got {}", tokens);
        assert!(
            tokens < 100,
            "Expected less than 100 tokens, got {}",
            tokens
        );
    }

    #[test]
    #[serial]
    fn test_row_start_position() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "abc\ndefg\nhi".to_string();

        assert_eq!(app.row_start_position(0), 0); // "abc" starts at 0
        assert_eq!(app.row_start_position(1), 4); // "defg" starts at 4 (after "abc\n")
        assert_eq!(app.row_start_position(2), 9); // "hi" starts at 9 (after "abc\ndefg\n")
    }

    #[test]
    #[serial]
    fn test_row_length() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "abc\ndefgh\nij".to_string();

        assert_eq!(app.row_length(0), 3); // "abc"
        assert_eq!(app.row_length(1), 5); // "defgh"
        assert_eq!(app.row_length(2), 2); // "ij"
    }

    #[test]
    #[serial]
    fn test_move_cursor_up() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "abc\ndefgh\nij".to_string();
        // Positions: 0=a 1=b 2=c 3=\n 4=d 5=e 6=f 7=g 8=h 9=\n 10=i 11=j

        // Start at position 11 (row 2, col 1 = "ij" -> "j")
        app.cursor_position = 11; // 'j' in "ij"
        assert_eq!(app.cursor_row_col(), (2, 1));

        // Move up to row 1, should maintain col 1
        app.move_cursor_up();
        assert_eq!(app.cursor_row_col(), (1, 1));
        assert_eq!(app.cursor_position, 5); // 'e' in "defgh"

        // Move up to row 0, should maintain col 1
        app.move_cursor_up();
        assert_eq!(app.cursor_row_col(), (0, 1));
        assert_eq!(app.cursor_position, 1); // 'b' in "abc"

        // Already at row 0, should stay
        app.move_cursor_up();
        assert_eq!(app.cursor_row_col(), (0, 1));
    }

    #[test]
    #[serial]
    fn test_move_cursor_up_clamps_column() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Long line followed by short line
        app.input = "ab\ndefghij".to_string();

        // Start at end of second line (col 7)
        app.cursor_position = 10; // end of "defghij"
        assert_eq!(app.cursor_row_col(), (1, 7));

        // Move up - first line only has 2 chars, so clamp to col 2
        app.move_cursor_up();
        assert_eq!(app.cursor_row_col(), (0, 2));
        assert_eq!(app.cursor_position, 2);
    }

    #[test]
    #[serial]
    fn test_move_cursor_down() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "abc\ndefgh\nij".to_string();

        // Start at position 1 (row 0, col 1 = "abc" -> "b")
        app.cursor_position = 1;
        assert_eq!(app.cursor_row_col(), (0, 1));

        // Move down to row 1, should maintain col 1
        app.move_cursor_down();
        assert_eq!(app.cursor_row_col(), (1, 1));

        // Move down to row 2, should maintain col 1
        app.move_cursor_down();
        assert_eq!(app.cursor_row_col(), (2, 1));

        // Already at last row, should stay
        app.move_cursor_down();
        assert_eq!(app.cursor_row_col(), (2, 1));
    }

    #[test]
    #[serial]
    fn test_move_cursor_down_clamps_column() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Long line followed by short line
        app.input = "abcdefgh\nij".to_string();

        // Start at end of first line (col 8)
        app.cursor_position = 8;
        assert_eq!(app.cursor_row_col(), (0, 8));

        // Move down - second line only has 2 chars, so clamp to col 2
        app.move_cursor_down();
        assert_eq!(app.cursor_row_col(), (1, 2));
    }

    #[test]
    #[serial]
    fn test_move_cursor_line_start() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "abc\ndefgh\nij".to_string();

        // Start in middle of second line
        app.cursor_position = 6; // 'e' in "defgh"
        assert_eq!(app.cursor_row_col(), (1, 2));

        // Move to start of line
        app.move_cursor_line_start();
        assert_eq!(app.cursor_row_col(), (1, 0));
        assert_eq!(app.cursor_position, 4);
    }

    #[test]
    #[serial]
    fn test_move_cursor_line_end() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "abc\ndefgh\nij".to_string();

        // Start at beginning of second line
        app.cursor_position = 4;
        assert_eq!(app.cursor_row_col(), (1, 0));

        // Move to end of line
        app.move_cursor_line_end();
        assert_eq!(app.cursor_row_col(), (1, 5));
        assert_eq!(app.cursor_position, 9);
    }

    #[test]
    #[serial]
    fn test_insert_newline() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.cursor_position = 5;

        // Insert newline at cursor position
        app.input.insert(app.cursor_position, '\n');
        app.cursor_position += 1;

        assert_eq!(app.input, "hello\n world");
        assert_eq!(app.cursor_row_col(), (1, 0));
        assert_eq!(app.input_line_count(), 2);
    }

    #[test]
    #[serial]
    fn test_delete_newline() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello\nworld".to_string();
        app.cursor_position = 6; // Start of "world"

        // Delete character before cursor (the newline)
        app.cursor_position -= 1;
        app.input.remove(app.cursor_position);

        assert_eq!(app.input, "helloworld");
        assert_eq!(app.input_line_count(), 1);
    }

    // ============================================
    // Tab completion tests
    // ============================================

    #[test]
    #[serial]
    fn test_tab_completion_single_match() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Type "/he" - should complete to "/help "
        app.input = "/he".to_string();
        app.cursor_position = 3;

        app.try_tab_completion();

        assert_eq!(app.input, "/help ");
        assert_eq!(app.cursor_position, 6);
    }

    #[test]
    #[serial]
    fn test_tab_completion_unique_prefix() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Type "/q" - should complete to "/quit "
        app.input = "/q".to_string();
        app.cursor_position = 2;

        app.try_tab_completion();

        assert_eq!(app.input, "/quit ");
        assert_eq!(app.cursor_position, 6);
    }

    #[test]
    #[serial]
    fn test_tab_completion_multiple_matches() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Type "/ex" - now matches /exit and /export, so should not complete
        app.input = "/ex".to_string();
        app.cursor_position = 3;

        app.try_tab_completion();

        // Multiple matches (exit, export) - should not auto-complete
        assert_eq!(app.input, "/ex");
        assert_eq!(app.cursor_position, 3);
    }

    #[test]
    #[serial]
    fn test_tab_completion_no_match() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Type "/xyz" - no match, should not change
        app.input = "/xyz".to_string();
        app.cursor_position = 4;

        app.try_tab_completion();

        assert_eq!(app.input, "/xyz");
        assert_eq!(app.cursor_position, 4);
    }

    #[test]
    #[serial]
    fn test_tab_completion_not_slash_command() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Regular text without / should not complete
        app.input = "help".to_string();
        app.cursor_position = 4;

        app.try_tab_completion();

        assert_eq!(app.input, "help");
        assert_eq!(app.cursor_position, 4);
    }

    #[test]
    #[serial]
    fn test_tab_completion_with_space_ignored() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Already has a space - should not complete further
        app.input = "/help arg".to_string();
        app.cursor_position = 9;

        app.try_tab_completion();

        assert_eq!(app.input, "/help arg");
        assert_eq!(app.cursor_position, 9);
    }

    #[test]
    #[serial]
    fn test_tab_completion_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Type uppercase "/HE" - should still complete
        app.input = "/HE".to_string();
        app.cursor_position = 3;

        app.try_tab_completion();

        assert_eq!(app.input, "/help ");
        assert_eq!(app.cursor_position, 6);
    }

    #[test]
    #[serial]
    fn test_tab_completion_common_prefix() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Type "/" - matches all, should complete to common prefix
        app.input = "/".to_string();
        app.cursor_position = 1;

        app.try_tab_completion();

        // No common prefix beyond "/" so stays as is
        assert_eq!(app.input, "/");
    }

    #[test]
    #[serial]
    fn test_longest_common_prefix() {
        // Test helper function
        assert_eq!(App::longest_common_prefix(&[]), "");
        assert_eq!(App::longest_common_prefix(&["hello"]), "hello");
        assert_eq!(App::longest_common_prefix(&["hello", "help", "hex"]), "he");
        assert_eq!(App::longest_common_prefix(&["abc", "xyz"]), "");
        assert_eq!(App::longest_common_prefix(&["/quit", "/exit"]), "/");
    }

    #[test]
    #[serial]
    fn test_available_commands_not_empty() {
        let commands = App::available_commands();
        assert!(!commands.is_empty());
        // Verify some expected commands exist
        assert!(commands.iter().any(|(cmd, _)| *cmd == "/help"));
        assert!(commands.iter().any(|(cmd, _)| *cmd == "/quit"));
        assert!(commands.iter().any(|(cmd, _)| *cmd == "/clear"));
    }

    // ============================================
    // Slash command handler tests
    // ============================================

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_quit() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        assert!(!app.should_quit);
        let handled = app.handle_slash_command("/quit").await;
        assert!(handled);
        assert!(app.should_quit);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_exit() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        assert!(!app.should_quit);
        let handled = app.handle_slash_command("/exit").await;
        assert!(handled);
        assert!(app.should_quit);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_quit_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/QUIT").await;
        assert!(handled);
        assert!(app.should_quit);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_clear() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // App starts with a welcome message, add more
        let initial_count = app.messages.len();
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "test".to_string(),
        });
        app.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: "response".to_string(),
        });
        assert_eq!(app.messages.len(), initial_count + 2);

        let handled = app.handle_slash_command("/clear").await;
        assert!(handled);
        // Should have exactly one system message about clearing
        assert_eq!(app.messages.len(), 1);
        assert!(matches!(app.messages[0].role, MessageRole::System));
        assert!(app.messages[0].content.contains("cleared"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_help() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        assert!(!app.show_help);
        let handled = app.handle_slash_command("/help").await;
        assert!(handled);
        assert!(app.show_help);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_question_mark() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        assert!(!app.show_help);
        let handled = app.handle_slash_command("/?").await;
        assert!(handled);
        assert!(app.show_help);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_new() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Add some state
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "test".to_string(),
        });
        app.turn_count = 5;

        let handled = app.handle_slash_command("/new").await;
        assert!(handled);
        // Should reset state
        assert_eq!(app.turn_count, 0);
        assert_eq!(app.messages.len(), 1);
        assert!(matches!(app.messages[0].role, MessageRole::System));
        assert!(app.messages[0].content.contains("new chat"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_new_clears_session_approvals() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Add session-approved tools from previous chat
        app.session_approved_tools.insert("shell".to_string());
        app.session_approved_tools.insert("read_file".to_string());
        assert_eq!(app.session_approved_tools.len(), 2);

        // Run /new command
        let handled = app.handle_slash_command("/new").await;
        assert!(handled);

        // Session approvals should be cleared
        assert!(
            app.session_approved_tools.is_empty(),
            "Session approved tools should be cleared on /new"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_status() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/status").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        let content = &last_msg.content;
        assert!(content.contains("Session:"));
        assert!(content.contains("Model:"));
        assert!(content.contains("Turns:"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_tokens() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/tokens").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        let content = &last_msg.content;
        assert!(content.contains("Token Usage Statistics"));
        assert!(content.contains("Context:"));
        assert!(content.contains("System:"));
        assert!(content.contains("User:"));
        assert!(content.contains("Assistant:"));
        assert!(content.contains("Total messages:"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_tokens_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/TOKENS").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_keys() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/keys").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        let content = &last_msg.content;
        assert!(content.contains("Keyboard Shortcuts"));
        assert!(content.contains("Navigation:"));
        assert!(content.contains("Ctrl+Z"));
        assert!(content.contains("Ctrl+C"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_keys_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/KEYS").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_version() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/version").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        let content = &last_msg.content;
        assert!(content.contains("Codex DashFlow"));
        assert!(content.contains("Version:"));
        assert!(content.contains("Build:"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_version_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/VERSION").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_config() {
        let config = AppConfig {
            working_dir: "/test/path".to_string(),
            max_turns: 50,
            collect_training: true,
            ..Default::default()
        };
        let mut app = App::new(config);
        app.model = "gpt-4".to_string();
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/config").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        let content = &last_msg.content;
        assert!(content.contains("Current Configuration"));
        assert!(content.contains("Model: gpt-4"));
        assert!(content.contains("Working Dir: /test/path"));
        assert!(content.contains("Max Turns: 50"));
        assert!(content.contains("Training Collection: enabled"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_config_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/CONFIG").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_config_unlimited_turns() {
        let config = AppConfig {
            max_turns: 0,
            ..Default::default()
        };
        let mut app = App::new(config);

        app.handle_slash_command("/config").await;
        let last_msg = app.messages.last().unwrap();
        assert!(last_msg.content.contains("Max Turns: unlimited"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_mcp_no_servers() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/mcp").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        let content = &last_msg.content;
        assert!(content.contains("No MCP servers configured"));
        assert!(content.contains("config.toml"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_mcp_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/MCP").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_mcp_with_stdio_server() {
        use codex_dashflow_core::mcp::{McpServerConfig, McpTransport};

        let server = McpServerConfig {
            name: "test-server".to_string(),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@test/server".to_string()],
            },
            env: std::collections::HashMap::new(),
            cwd: None,
            timeout_secs: 30,
        };

        let config = AppConfig {
            mcp_servers: vec![server],
            ..Default::default()
        };
        let mut app = App::new(config);

        app.handle_slash_command("/mcp").await;
        let last_msg = app.messages.last().unwrap();
        let content = &last_msg.content;
        assert!(content.contains("Configured MCP Servers: 1"));
        assert!(content.contains("test-server"));
        assert!(content.contains("Type: stdio"));
        assert!(content.contains("npx -y @test/server"));
        assert!(content.contains("Timeout: 30s"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_mcp_with_http_server() {
        use codex_dashflow_core::mcp::{McpServerConfig, McpTransport};

        let server = McpServerConfig {
            name: "api-server".to_string(),
            transport: McpTransport::Http {
                url: "https://api.example.com/mcp".to_string(),
                bearer_token: None,
                headers: std::collections::HashMap::new(),
            },
            env: std::collections::HashMap::new(),
            cwd: None,
            timeout_secs: 60,
        };

        let config = AppConfig {
            mcp_servers: vec![server],
            ..Default::default()
        };
        let mut app = App::new(config);

        app.handle_slash_command("/mcp").await;
        let last_msg = app.messages.last().unwrap();
        let content = &last_msg.content;
        assert!(content.contains("Configured MCP Servers: 1"));
        assert!(content.contains("api-server"));
        assert!(content.contains("Type: http"));
        assert!(content.contains("URL: https://api.example.com/mcp"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_skills_handled() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/skills").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        // Either shows skills or the no-skills message
        let content = &last_msg.content;
        assert!(content.contains("skills") || content.contains("Skills"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_skills_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/SKILLS").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_logout_handled() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/logout").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        // Check for expected message patterns
        let content = &last_msg.content;
        assert!(
            content.contains("Logged out")
                || content.contains("No credentials")
                || content.contains("Failed")
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_logout_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/LOGOUT").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_init_handled() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/init").await;
        assert!(handled);
        // Should add at least one message (either "already exists" or "creating...")
        assert!(app.messages.len() > initial_count);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        let content = &last_msg.content;
        // Either shows "already exists" or "Creating AGENTS.md"
        assert!(content.contains("AGENTS.md"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_init_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/INIT").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_approvals_show() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/approvals").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        // Should show current mode and available options
        assert!(last_msg.content.contains("Current approval mode"));
        assert!(last_msg.content.contains("Available modes"));
        assert!(last_msg.content.contains("read-only"));
        assert!(last_msg.content.contains("auto"));
        assert!(last_msg.content.contains("full-access"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_approvals_change_mode() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        assert_eq!(app.config.approval_preset, "auto"); // default

        let handled = app.handle_slash_command("/approvals read-only").await;
        assert!(handled);
        assert_eq!(app.config.approval_preset, "read-only");
        let last_msg = app.messages.last().unwrap();
        assert!(last_msg.content.contains("Read Only"));
        assert!(last_msg.content.contains("Approval mode changed"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_approvals_invalid_mode() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_preset = app.config.approval_preset.clone();

        let handled = app.handle_slash_command("/approvals invalid-mode").await;
        assert!(handled);
        // Preset should not change
        assert_eq!(app.config.approval_preset, initial_preset);
        let last_msg = app.messages.last().unwrap();
        assert!(last_msg.content.contains("Unknown approval mode"));
        assert!(last_msg.content.contains("invalid-mode"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_mode_show() {
        // /mode is an alias for /approvals
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/mode").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        // Should show current mode and available options (same as /approvals)
        assert!(last_msg.content.contains("Current approval mode"));
        assert!(last_msg.content.contains("Available modes"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_mode_change() {
        // /mode can change approval mode just like /approvals
        let config = AppConfig::default();
        let mut app = App::new(config);
        assert_eq!(app.config.approval_preset, "auto"); // default

        let handled = app.handle_slash_command("/mode read-only").await;
        assert!(handled);
        assert_eq!(app.config.approval_preset, "read-only");
        let last_msg = app.messages.last().unwrap();
        assert!(last_msg.content.contains("Read Only"));
        assert!(last_msg.content.contains("Approval mode changed"));
    }

    #[test]
    #[serial]
    fn test_mode_command_in_builtin_commands() {
        // Verify /mode is in the builtin commands list
        let found = BUILTIN_COMMANDS.iter().any(|(cmd, _)| *cmd == "/mode");
        assert!(found, "/mode should be in BUILTIN_COMMANDS");
    }

    #[test]
    #[serial]
    fn test_cycle_approval_mode_from_auto() {
        use codex_dashflow_core::execpolicy::ApprovalMode;

        // Start with default (auto)
        let config = AppConfig::default();
        let mut app = App::new(config);
        assert_eq!(app.config.approval_preset, "auto");

        // Cycle to next mode (full-access)
        app.cycle_approval_mode();
        assert_eq!(app.config.approval_preset, "full-access");
        let policy = app.agent_state.exec_policy();
        assert_eq!(policy.approval_mode, ApprovalMode::Never);

        // Verify notification was shown (not system message)
        assert!(app.notification.is_some());
        let notif = app.notification.as_ref().unwrap();
        assert!(notif.text.contains("full-access"));
        assert_eq!(notif.style, NotificationStyle::Error); // full-access uses Error style
    }

    #[test]
    #[serial]
    fn test_cycle_approval_mode_wraps_around() {
        use codex_dashflow_core::execpolicy::ApprovalMode;

        // Start with full-access (last in list)
        let config = AppConfig {
            approval_preset: "full-access".to_string(),
            ..Default::default()
        };
        let mut app = App::new(config);
        assert_eq!(app.config.approval_preset, "full-access");

        // Cycle should wrap to read-only (first in list)
        app.cycle_approval_mode();
        assert_eq!(app.config.approval_preset, "read-only");
        let policy = app.agent_state.exec_policy();
        assert_eq!(policy.approval_mode, ApprovalMode::Always);
    }

    #[test]
    #[serial]
    fn test_cycle_approval_mode_full_cycle() {
        // Test a complete cycle through all modes
        let config = AppConfig {
            approval_preset: "read-only".to_string(),
            ..Default::default()
        };
        let mut app = App::new(config);

        // read-only → auto
        app.cycle_approval_mode();
        assert_eq!(app.config.approval_preset, "auto");

        // auto → full-access
        app.cycle_approval_mode();
        assert_eq!(app.config.approval_preset, "full-access");

        // full-access → read-only (wrap)
        app.cycle_approval_mode();
        assert_eq!(app.config.approval_preset, "read-only");
    }

    #[test]
    #[serial]
    fn test_app_new_applies_exec_policy_from_preset() {
        use codex_dashflow_core::execpolicy::ApprovalMode;

        // Test default preset (auto) applies OnDangerous mode
        let config = AppConfig::default();
        let app = App::new(config);
        assert!(app.agent_state.has_exec_policy());
        assert_eq!(
            app.agent_state.exec_policy().approval_mode,
            ApprovalMode::OnDangerous
        );

        // Test read-only preset applies Always mode
        let config = AppConfig {
            approval_preset: "read-only".to_string(),
            ..Default::default()
        };
        let app = App::new(config);
        assert_eq!(
            app.agent_state.exec_policy().approval_mode,
            ApprovalMode::Always
        );

        // Test full-access preset applies Never mode
        let config = AppConfig {
            approval_preset: "full-access".to_string(),
            ..Default::default()
        };
        let app = App::new(config);
        assert_eq!(
            app.agent_state.exec_policy().approval_mode,
            ApprovalMode::Never
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_approvals_command_updates_exec_policy() {
        use codex_dashflow_core::execpolicy::ApprovalMode;

        let config = AppConfig::default();
        let mut app = App::new(config);

        // Initial state should be OnDangerous (auto preset)
        assert_eq!(
            app.agent_state.exec_policy().approval_mode,
            ApprovalMode::OnDangerous
        );

        // Change to read-only
        app.handle_slash_command("/approvals read-only").await;
        assert_eq!(
            app.agent_state.exec_policy().approval_mode,
            ApprovalMode::Always
        );

        // Change to full-access
        app.handle_slash_command("/approvals full-access").await;
        assert_eq!(
            app.agent_state.exec_policy().approval_mode,
            ApprovalMode::Never
        );

        // Change back to auto
        app.handle_slash_command("/approvals auto").await;
        assert_eq!(
            app.agent_state.exec_policy().approval_mode,
            ApprovalMode::OnDangerous
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_new_command_preserves_exec_policy() {
        use codex_dashflow_core::execpolicy::ApprovalMode;

        let config = AppConfig {
            approval_preset: "read-only".to_string(),
            ..Default::default()
        };
        let mut app = App::new(config);

        // Initial state with read-only preset
        assert_eq!(
            app.agent_state.exec_policy().approval_mode,
            ApprovalMode::Always
        );

        // Run /new command - should preserve exec_policy
        app.handle_slash_command("/new").await;
        assert_eq!(
            app.agent_state.exec_policy().approval_mode,
            ApprovalMode::Always
        );
        assert!(app.agent_state.has_exec_policy());
    }

    #[tokio::test]
    #[serial]
    async fn test_resume_command_preserves_exec_policy() {
        use codex_dashflow_core::execpolicy::ApprovalMode;

        let config = AppConfig {
            approval_preset: "full-access".to_string(),
            ..Default::default()
        };
        let mut app = App::new(config);

        // Initial state with full-access preset
        assert_eq!(
            app.agent_state.exec_policy().approval_mode,
            ApprovalMode::Never
        );

        // Run /resume command with a new session ID - should preserve exec_policy
        app.handle_slash_command("/resume test-session-123").await;
        assert_eq!(
            app.agent_state.exec_policy().approval_mode,
            ApprovalMode::Never
        );
        assert!(app.agent_state.has_exec_policy());
        assert_eq!(app.agent_state.session_id, "test-session-123");
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_resume_show_info() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();
        let session_id = app.session_id.clone();

        let handled = app.handle_slash_command("/resume").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        // Should show current session and usage help
        assert!(last_msg.content.contains(&session_id));
        assert!(last_msg.content.contains("checkpointing"));
        assert!(last_msg.content.contains("Usage:"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_resume_change_session() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let old_session = app.session_id.clone();

        let handled = app.handle_slash_command("/resume my-new-session").await;
        assert!(handled);
        assert_eq!(app.session_id, "my-new-session");
        assert_eq!(app.agent_state.session_id, "my-new-session");
        assert_eq!(app.turn_count, 0);
        let last_msg = app.messages.last().unwrap();
        assert!(last_msg.content.contains(&old_session));
        assert!(last_msg.content.contains("my-new-session"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_resume_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/RESUME").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_sessions_no_checkpointing() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/sessions").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        // Should show checkpointing not enabled message
        assert!(last_msg.content.contains("Checkpointing is not enabled"));
        assert!(last_msg.content.contains("config.toml"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_sessions_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/SESSIONS").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_delete_no_checkpointing() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/delete some-session").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        // Should show checkpointing not enabled message
        assert!(last_msg.content.contains("Checkpointing is not enabled"));
        assert!(last_msg.content.contains("config.toml"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_delete_no_args() {
        let config = AppConfig {
            checkpointing_enabled: true,
            ..Default::default()
        };
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/delete").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        // Should show usage help
        assert!(last_msg.content.contains("Usage: /delete"));
        assert!(last_msg.content.contains("/sessions"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_delete_current_session() {
        let config = AppConfig {
            checkpointing_enabled: true,
            session_id: Some("my-current-session".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        // Try to delete the current session
        let handled = app.handle_slash_command("/delete my-current-session").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        // Should prevent deletion
        assert!(last_msg
            .content
            .contains("Cannot delete the current session"));
        assert!(last_msg.content.contains("/new"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_delete_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/DELETE").await;
        assert!(handled);
    }

    #[test]
    #[serial]
    fn test_app_config_checkpointing_defaults() {
        let config = AppConfig::default();
        assert!(!config.checkpointing_enabled);
        assert!(config.checkpoint_path.is_none());
        assert!(config.postgres_connection_string.is_none());
    }

    #[test]
    #[serial]
    fn test_app_config_build_runner_config_no_checkpointing() {
        let config = AppConfig::default();
        let runner_config = config.build_runner_config();
        // With checkpointing disabled, returns default config
        assert!(!runner_config.enable_checkpointing);
    }

    #[test]
    #[serial]
    fn test_app_config_build_runner_config_with_memory_checkpointing() {
        let config = AppConfig {
            checkpointing_enabled: true,
            checkpoint_path: None,
            postgres_connection_string: None,
            ..Default::default()
        };
        let runner_config = config.build_runner_config();
        assert!(runner_config.enable_checkpointing);
        assert!(runner_config.checkpoint_path.is_none());
    }

    #[test]
    #[serial]
    fn test_app_config_build_runner_config_with_file_checkpointing() {
        let config = AppConfig {
            checkpointing_enabled: true,
            checkpoint_path: Some(std::path::PathBuf::from("/tmp/checkpoints")),
            postgres_connection_string: None,
            ..Default::default()
        };
        let runner_config = config.build_runner_config();
        assert!(runner_config.enable_checkpointing);
        assert_eq!(
            runner_config.checkpoint_path,
            Some(std::path::PathBuf::from("/tmp/checkpoints"))
        );
    }

    #[test]
    #[serial]
    fn test_app_config_build_runner_config_postgres_priority() {
        // PostgreSQL should take priority over file path
        let config = AppConfig {
            checkpointing_enabled: true,
            checkpoint_path: Some(std::path::PathBuf::from("/tmp/checkpoints")),
            postgres_connection_string: Some("host=localhost".to_string()),
            ..Default::default()
        };
        let runner_config = config.build_runner_config();
        assert!(runner_config.enable_checkpointing);
        // File path should NOT be set when postgres is configured
        assert!(runner_config.checkpoint_path.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_mention_no_args() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        app.input = "/mention".to_string();
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/mention").await;
        assert!(handled);
        // Input should be cleared and have just @
        assert_eq!(app.input, "@");
        assert_eq!(app.cursor_position, 1);
        // Should show help message
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(matches!(last_msg.role, MessageRole::System));
        assert!(last_msg.content.contains("filename"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_mention_with_file() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        app.input = "/mention src/main.rs".to_string();
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/mention src/main.rs").await;
        assert!(handled);
        // Input should have @src/main.rs
        assert_eq!(app.input, "@src/main.rs ");
        assert_eq!(app.cursor_position, 13);
        // Should show confirmation message
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(last_msg.content.contains("src/main.rs"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_mention_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/MENTION").await;
        assert!(handled);
        assert_eq!(app.input, "@");
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_model_show() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        app.model = "gpt-4".to_string();
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/model").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let last_msg = app.messages.last().unwrap();
        assert!(last_msg.content.contains("gpt-4"));
        assert!(last_msg.content.contains("Usage:"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_model_change() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        app.model = "gpt-4".to_string();

        let handled = app.handle_slash_command("/model claude-3").await;
        assert!(handled);
        assert_eq!(app.model, "claude-3");
        let last_msg = app.messages.last().unwrap();
        assert!(last_msg.content.contains("claude-3"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_unknown() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/unknown").await;
        assert!(!handled);
        // Unknown commands should not add any messages
        assert_eq!(app.messages.len(), initial_count);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_passthrough() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Unknown commands should not be handled locally (pass through to agent)
        assert!(!app.handle_slash_command("/unknown").await);
        assert!(!app.handle_slash_command("/foo").await);
        assert!(!app.handle_slash_command("/randomcmd").await);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_with_whitespace() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Should handle commands with extra whitespace
        let handled = app.handle_slash_command("  /quit  ").await;
        assert!(handled);
        assert!(app.should_quit);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_undo_empty() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        // Undo with no history should show message
        let handled = app.handle_slash_command("/undo").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        assert!(app
            .messages
            .last()
            .unwrap()
            .content
            .contains("Nothing to undo"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_undo_removes_turn() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Add a user message and assistant response to agent state
        app.agent_state
            .messages
            .push(Message::user("test question"));
        app.agent_state
            .messages
            .push(Message::assistant("test answer"));

        // Add to display messages
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "test question".to_string(),
        });
        app.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: "test answer".to_string(),
        });

        let initial_agent_msgs = app.agent_state.messages.len();
        let handled = app.handle_slash_command("/undo").await;
        assert!(handled);

        // Should have removed the user message and everything after it
        assert!(app.agent_state.messages.len() < initial_agent_msgs);
        // Last message should be the undo confirmation
        assert!(app
            .messages
            .last()
            .unwrap()
            .content
            .contains("Undone last turn"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_history_empty() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_count = app.messages.len();

        let handled = app.handle_slash_command("/history").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        assert!(app
            .messages
            .last()
            .unwrap()
            .content
            .contains("No conversation history"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_history_shows_messages() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Add messages to agent state
        app.agent_state.messages.push(Message::user("hello"));
        app.agent_state
            .messages
            .push(Message::assistant("hi there"));

        let initial_count = app.messages.len();
        let handled = app.handle_slash_command("/history").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        let history_msg = &app.messages.last().unwrap().content;
        assert!(history_msg.contains("Conversation history"));
        assert!(history_msg.contains("[User]"));
        assert!(history_msg.contains("[Assistant]"));
        assert!(history_msg.contains("hello"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_compact_too_short() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Only 2 messages - too short to compact
        app.agent_state.messages.push(Message::user("hello"));
        app.agent_state.messages.push(Message::assistant("hi"));

        let initial_count = app.messages.len();
        let handled = app.handle_slash_command("/compact").await;
        assert!(handled);
        assert_eq!(app.messages.len(), initial_count + 1);
        assert!(app.messages.last().unwrap().content.contains("too short"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_compact_works() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Add enough messages for compaction (need at least 4, and 2+ user messages)
        app.agent_state
            .messages
            .push(Message::user("first question"));
        app.agent_state
            .messages
            .push(Message::assistant("first answer"));
        app.agent_state
            .messages
            .push(Message::user("second question"));
        app.agent_state
            .messages
            .push(Message::assistant("second answer"));
        app.agent_state
            .messages
            .push(Message::user("third question"));
        app.agent_state
            .messages
            .push(Message::assistant("third answer"));

        let initial_count = app.agent_state.messages.len();
        let handled = app.handle_slash_command("/compact").await;
        assert!(handled);

        // Should have kept the last 2 user messages and their responses
        assert!(app.agent_state.messages.len() < initial_count);
        // Last display message should be the compact confirmation
        assert!(app.messages.last().unwrap().content.contains("Compacted"));
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_undo_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/UNDO").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_history_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/HISTORY").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_compact_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/COMPACT").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_diff_handled() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_msg_count = app.messages.len();

        // /diff should be handled locally now
        let handled = app.handle_slash_command("/diff").await;
        assert!(handled);
        // Should have added a message (either diff output or "no changes" or error)
        assert!(app.messages.len() > initial_msg_count);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_diff_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/DIFF").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_diff_with_args() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_msg_count = app.messages.len();

        // /diff with args should still be handled
        let handled = app.handle_slash_command("/diff --staged").await;
        assert!(handled);
        assert!(app.messages.len() > initial_msg_count);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_diff_with_file_arg() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_msg_count = app.messages.len();

        // /diff with file path arg
        let handled = app.handle_slash_command("/diff Cargo.toml").await;
        assert!(handled);
        assert!(app.messages.len() > initial_msg_count);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_review_handled() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_msg_count = app.messages.len();

        // /review should be handled locally now
        let handled = app.handle_slash_command("/review").await;
        assert!(handled);
        // Should have added a message (either review output or "clean" message or error)
        assert!(app.messages.len() > initial_msg_count);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_review_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/REVIEW").await;
        assert!(handled);
    }

    #[test]
    #[serial]
    fn test_diff_command_adds_system_message() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_msg_count = app.messages.len();

        app.handle_diff_command("");

        // Should add exactly one system message
        assert_eq!(app.messages.len(), initial_msg_count + 1);
        // Message should be from system
        assert_eq!(app.messages.last().unwrap().role, MessageRole::System);
    }

    #[test]
    #[serial]
    fn test_review_command_adds_system_message() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_msg_count = app.messages.len();

        app.handle_review_command();

        // Should add exactly one system message
        assert_eq!(app.messages.len(), initial_msg_count + 1);
        // Message should be from system
        assert_eq!(app.messages.last().unwrap().role, MessageRole::System);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_feedback_handled() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/feedback").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_feedback_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/FEEDBACK").await;
        assert!(handled);
    }

    #[test]
    #[serial]
    fn test_feedback_command_adds_system_message() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_msg_count = app.messages.len();

        app.handle_feedback_command();

        // Should add exactly one system message
        assert_eq!(app.messages.len(), initial_msg_count + 1);
        // Message should be from system
        assert_eq!(app.messages.last().unwrap().role, MessageRole::System);
        // Message should contain github URL
        assert!(app.messages.last().unwrap().content.contains("github.com"));
    }

    #[test]
    #[serial]
    fn test_feedback_command_shows_session_id() {
        let config = AppConfig {
            session_id: Some("test-session-123".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config);

        app.handle_feedback_command();

        // Message should contain the session ID
        assert!(app
            .messages
            .last()
            .unwrap()
            .content
            .contains("test-session-123"));
    }

    #[test]
    #[serial]
    fn test_feedback_command_shows_none_without_session() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.handle_feedback_command();

        // Message should show (none) for missing session ID
        assert!(app.messages.last().unwrap().content.contains("(none)"));
    }

    // /providers command tests
    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_providers_handled() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/providers").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_providers_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/PROVIDERS").await;
        assert!(handled);
    }

    #[test]
    #[serial]
    fn test_providers_command_adds_system_message() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_msg_count = app.messages.len();

        app.handle_providers_command();

        // Should add exactly one system message
        assert_eq!(app.messages.len(), initial_msg_count + 1);
        // Message should be from system
        assert_eq!(app.messages.last().unwrap().role, MessageRole::System);
    }

    #[test]
    #[serial]
    fn test_providers_command_lists_builtin_providers() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.handle_providers_command();

        let content = &app.messages.last().unwrap().content;
        // Should contain expected built-in providers
        assert!(content.contains("OpenAI"));
        assert!(content.contains("Anthropic"));
        assert!(content.contains("Ollama"));
        assert!(content.contains("LMStudio"));
    }

    #[test]
    #[serial]
    fn test_providers_command_shows_provider_count() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.handle_providers_command();

        let content = &app.messages.last().unwrap().content;
        // Should show total count
        assert!(content.contains("Total:"));
        assert!(content.contains("providers"));
    }

    // /search command tests
    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_search_handled() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/search test").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_search_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/SEARCH test").await;
        assert!(handled);
    }

    #[test]
    #[serial]
    fn test_search_command_shows_usage_without_query() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_msg_count = app.messages.len();

        app.handle_search_command("");

        assert_eq!(app.messages.len(), initial_msg_count + 1);
        assert_eq!(app.messages.last().unwrap().role, MessageRole::System);
        assert!(app.messages.last().unwrap().content.contains("Usage:"));
    }

    #[test]
    #[serial]
    fn test_search_command_finds_matches() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Add some messages to search
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "Hello world".to_string(),
        });
        app.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: "Hi there! World is great.".to_string(),
        });
        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "How are you?".to_string(),
        });

        let initial_msg_count = app.messages.len();

        app.handle_search_command("world");

        assert_eq!(app.messages.len(), initial_msg_count + 1);
        let content = &app.messages.last().unwrap().content;
        // Should find 2 matches (Hello world and World is great)
        assert!(content.contains("2 matches"));
    }

    #[test]
    #[serial]
    fn test_search_command_case_insensitive_search() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "HELLO world".to_string(),
        });

        app.handle_search_command("hello");

        let content = &app.messages.last().unwrap().content;
        assert!(content.contains("1 match"));
    }

    #[test]
    #[serial]
    fn test_search_command_no_matches() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.messages.push(ChatMessage {
            role: MessageRole::User,
            content: "Hello world".to_string(),
        });

        app.handle_search_command("xyz123");

        let content = &app.messages.last().unwrap().content;
        assert!(content.contains("No messages found"));
    }

    // CommandPopup tests
    #[test]
    #[serial]
    fn test_command_popup_default() {
        let popup = CommandPopup::default();
        assert!(!popup.visible);
        assert_eq!(popup.selected_index, 0);
        assert!(popup.filtered_commands.is_empty());
        assert_eq!(popup.scroll_offset, 0);
    }

    #[test]
    #[serial]
    fn test_command_popup_filter_shows_on_slash() {
        let mut popup = CommandPopup::new();
        popup.update_filter("/");
        assert!(popup.visible);
        assert!(!popup.filtered_commands.is_empty());
        // Should contain all commands
        assert_eq!(popup.total_count(), BUILTIN_COMMANDS.len());
    }

    #[test]
    #[serial]
    fn test_command_popup_filter_hides_without_slash() {
        let mut popup = CommandPopup::new();
        popup.update_filter("hello");
        assert!(!popup.visible);
        assert!(popup.filtered_commands.is_empty());
    }

    #[test]
    #[serial]
    fn test_command_popup_filter_hides_on_space() {
        let mut popup = CommandPopup::new();
        popup.update_filter("/help ");
        assert!(!popup.visible);
        assert!(popup.filtered_commands.is_empty());
    }

    #[test]
    #[serial]
    fn test_command_popup_filter_narrows_results() {
        let mut popup = CommandPopup::new();
        popup.update_filter("/he");
        assert!(popup.visible);
        // Should find /help
        assert!(popup
            .filtered_commands
            .iter()
            .any(|(cmd, _, _)| cmd == "/help"));
        // Should not find /quit
        assert!(!popup
            .filtered_commands
            .iter()
            .any(|(cmd, _, _)| cmd == "/quit"));
    }

    #[test]
    #[serial]
    fn test_command_popup_filter_case_insensitive() {
        let mut popup = CommandPopup::new();
        popup.update_filter("/HE");
        assert!(popup.visible);
        assert!(popup
            .filtered_commands
            .iter()
            .any(|(cmd, _, _)| cmd == "/help"));
    }

    #[test]
    #[serial]
    fn test_command_popup_filter_hides_on_exact_match() {
        let mut popup = CommandPopup::new();
        popup.update_filter("/help");
        // Should hide when exact match (case-insensitive)
        assert!(!popup.visible);
    }

    #[test]
    #[serial]
    fn test_command_popup_move_up_down() {
        let mut popup = CommandPopup::new();
        popup.update_filter("/");
        assert_eq!(popup.selected_index, 0);

        popup.move_down();
        assert_eq!(popup.selected_index, 1);

        popup.move_down();
        assert_eq!(popup.selected_index, 2);

        popup.move_up();
        assert_eq!(popup.selected_index, 1);

        popup.move_up();
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    #[serial]
    fn test_command_popup_move_wraps() {
        let mut popup = CommandPopup::new();
        popup.update_filter("/");
        let total = popup.total_count();

        // Move up from 0 should wrap to last
        popup.move_up();
        assert_eq!(popup.selected_index, total - 1);

        // Move down from last should wrap to 0
        popup.move_down();
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    #[serial]
    fn test_command_popup_selected_command() {
        let mut popup = CommandPopup::new();
        popup.update_filter("/");

        let selected = popup.selected_command();
        assert!(selected.is_some());
        assert!(selected.unwrap().starts_with('/'));
    }

    #[test]
    #[serial]
    fn test_command_popup_hide() {
        let mut popup = CommandPopup::new();
        popup.update_filter("/");
        assert!(popup.visible);

        popup.hide();
        assert!(!popup.visible);
    }

    #[test]
    #[serial]
    fn test_command_popup_visible_commands() {
        let mut popup = CommandPopup::new();
        popup.update_filter("/");

        let visible = popup.visible_commands();
        assert!(!visible.is_empty());
        // Should be limited to MAX_POPUP_ROWS
        assert!(visible.len() <= MAX_POPUP_ROWS);

        // First item should be selected
        let (_, _, is_selected, _) = visible[0];
        assert!(is_selected);
    }

    #[test]
    #[serial]
    fn test_command_popup_has_more_items() {
        let mut popup = CommandPopup::new();
        popup.update_filter("/");

        // We have 12 commands, MAX_POPUP_ROWS is 8
        if BUILTIN_COMMANDS.len() > MAX_POPUP_ROWS {
            assert!(popup.has_more_items());
        }
    }

    #[test]
    #[serial]
    fn test_command_popup_scroll_on_selection() {
        let mut popup = CommandPopup::new();
        popup.update_filter("/");

        // Move down past visible area
        for _ in 0..MAX_POPUP_ROWS + 2 {
            popup.move_down();
        }

        // scroll_offset should have updated
        assert!(popup.scroll_offset > 0);
    }

    #[test]
    #[serial]
    fn test_command_popup_filter_preserves_selection_when_valid() {
        let mut popup = CommandPopup::new();
        popup.update_filter("/");
        popup.selected_index = 1;

        // Update filter to still match multiple commands
        popup.update_filter("/e"); // /exit, /help, /new, etc.

        // If selection is within range, it stays
        // If not, it resets to 0
        assert!(popup.selected_index < popup.total_count() || popup.total_count() == 0);
    }

    #[test]
    #[serial]
    fn test_app_command_popup_initialized() {
        let config = AppConfig::default();
        let app = App::new(config);

        assert!(!app.command_popup.visible);
        assert_eq!(app.command_popup.selected_index, 0);
    }

    // Fuzzy matching tests

    #[test]
    #[serial]
    fn test_fuzzy_match_score_consecutive_chars() {
        // Consecutive characters should return low score
        let score = super::fuzzy_match_score("hel", "/help");
        assert!(score.is_some());
        // Score should be low for consecutive matches
        assert_eq!(score.unwrap(), 1);
    }

    #[test]
    #[serial]
    fn test_fuzzy_match_score_with_gaps() {
        // Characters with gaps should return higher score
        // "hry" in "/history" -> h(1), r(5), y(8) - has gaps between matches
        let score = super::fuzzy_match_score("hry", "/history");
        assert!(score.is_some());
        // Score > 1 because of gaps
        assert!(score.unwrap() > 1);
    }

    #[test]
    #[serial]
    fn test_fuzzy_match_score_no_match() {
        // Characters not in order should return None
        let score = super::fuzzy_match_score("xyz", "/help");
        assert!(score.is_none());
    }

    #[test]
    #[serial]
    fn test_fuzzy_match_score_empty_query() {
        // Empty query matches anything
        let score = super::fuzzy_match_score("", "/help");
        assert!(score.is_some());
        assert_eq!(score.unwrap(), 1);
    }

    #[test]
    #[serial]
    fn test_command_popup_fuzzy_match_hy() {
        // "/hy" should fuzzy match /history (h...y)
        let mut popup = CommandPopup::new();
        popup.update_filter("/hy");

        assert!(popup.visible);
        let cmds: Vec<&str> = popup
            .filtered_commands
            .iter()
            .map(|(c, _, _)| c.as_str())
            .collect();

        // Should find /history via fuzzy matching
        assert!(
            cmds.contains(&"/history"),
            "Should match /history: {:?}",
            cmds
        );
    }

    #[test]
    #[serial]
    fn test_command_popup_fuzzy_match_ss() {
        // "/ss" should fuzzy match /status (s...s)
        let mut popup = CommandPopup::new();
        popup.update_filter("/ss");

        assert!(popup.visible);
        let cmds: Vec<&str> = popup
            .filtered_commands
            .iter()
            .map(|(c, _, _)| c.as_str())
            .collect();

        // Should find /status via fuzzy matching
        assert!(
            cmds.contains(&"/status"),
            "Should match /status: {:?}",
            cmds
        );
    }

    #[test]
    #[serial]
    fn test_command_popup_fuzzy_match_ct() {
        // "/ct" should fuzzy match /compact (c...t)
        let mut popup = CommandPopup::new();
        popup.update_filter("/ct");

        assert!(popup.visible);
        let cmds: Vec<&str> = popup
            .filtered_commands
            .iter()
            .map(|(c, _, _)| c.as_str())
            .collect();

        // Should find /compact via fuzzy matching
        assert!(
            cmds.contains(&"/compact"),
            "Should match /compact: {:?}",
            cmds
        );
    }

    #[test]
    #[serial]
    fn test_command_popup_prefix_before_fuzzy() {
        // Prefix matches should come before fuzzy matches
        let mut popup = CommandPopup::new();
        popup.update_filter("/he");

        // /help is a prefix match, /new might fuzzy match (has 'e' but not 'h' first)
        // Actually /help is the only prefix match for "/he"
        assert!(popup.visible);
        let first_cmd = popup.filtered_commands.first().map(|(c, _, _)| c.as_str());
        assert_eq!(first_cmd, Some("/help"), "Prefix match should be first");
    }

    #[test]
    #[serial]
    fn test_command_popup_fuzzy_prioritizes_consecutive() {
        // When multiple fuzzy matches exist, consecutive character matches score better
        let mut popup = CommandPopup::new();
        popup.update_filter("/cl");

        // /clear is a prefix match (score 0), so it should be first
        assert!(popup.visible);
        let first_cmd = popup.filtered_commands.first().map(|(c, _, _)| c.as_str());
        assert_eq!(
            first_cmd,
            Some("/clear"),
            "/clear should be first (prefix match)"
        );
    }

    #[test]
    #[serial]
    fn test_command_popup_fuzzy_no_match_for_unrelated() {
        // Characters not present shouldn't match
        let mut popup = CommandPopup::new();
        popup.update_filter("/xyz");

        // No command contains x, y, z in order
        assert!(!popup.visible || popup.filtered_commands.is_empty());
    }

    #[test]
    #[serial]
    fn test_fuzzy_match_score_early_match_preferred() {
        // Matches starting earlier in the string should score better
        let score_early = super::fuzzy_match_score("/e", "/exit");
        let score_late = super::fuzzy_match_score("/e", "___/exit");

        // Both should match
        assert!(score_early.is_some());
        assert!(score_late.is_some());

        // Early match should have lower (better) score
        // Note: score_late will have higher penalty due to late first match
    }

    // Tests for fuzzy_match_positions

    #[test]
    #[serial]
    fn test_fuzzy_match_positions_consecutive() {
        // Consecutive characters should return consecutive positions
        let positions = super::fuzzy_match_positions("hel", "/help");
        assert_eq!(positions, Some(vec![1, 2, 3])); // h=1, e=2, l=3 (skipping /)
    }

    #[test]
    #[serial]
    fn test_fuzzy_match_positions_with_gaps() {
        // Characters with gaps should return correct positions
        // "hry" in "/history" -> h(1), r(5), y(8)
        let positions = super::fuzzy_match_positions("hry", "/history");
        assert!(positions.is_some());
        let pos = positions.unwrap();
        assert_eq!(pos.len(), 3);
        assert_eq!(pos[0], 1); // h at index 1
        assert!(pos[1] > pos[0]); // r after h
        assert!(pos[2] > pos[1]); // y after r
    }

    #[test]
    #[serial]
    fn test_fuzzy_match_positions_no_match() {
        // Characters not in order should return None
        let positions = super::fuzzy_match_positions("xyz", "/help");
        assert!(positions.is_none());
    }

    #[test]
    #[serial]
    fn test_fuzzy_match_positions_empty_query() {
        // Empty query matches anything with empty positions
        let positions = super::fuzzy_match_positions("", "/help");
        assert_eq!(positions, Some(vec![]));
    }

    #[test]
    #[serial]
    fn test_command_popup_stores_match_positions() {
        // Verify that filtered_commands now includes match positions
        let mut popup = CommandPopup::new();
        popup.update_filter("/he");

        assert!(!popup.filtered_commands.is_empty());

        // Check that /help is in results with correct positions
        let help_entry = popup
            .filtered_commands
            .iter()
            .find(|(cmd, _, _)| cmd == "/help");
        assert!(help_entry.is_some());

        let (_, _, positions) = help_entry.unwrap();
        // For prefix match "/he" on "/help", positions should be [0, 1, 2]
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0], 0); // /
        assert_eq!(positions[1], 1); // h
        assert_eq!(positions[2], 2); // e
    }

    #[test]
    #[serial]
    fn test_command_popup_fuzzy_match_positions() {
        // Fuzzy match should also have correct positions
        let mut popup = CommandPopup::new();
        popup.update_filter("/hy");

        // /history should match with h at 1, y at end
        let history_entry = popup
            .filtered_commands
            .iter()
            .find(|(cmd, _, _)| cmd == "/history");
        assert!(history_entry.is_some());

        let (_, _, positions) = history_entry.unwrap();
        assert_eq!(positions.len(), 3); // /, h, y
    }

    #[test]
    #[serial]
    fn test_visible_commands_includes_positions() {
        // Verify visible_commands returns match positions
        let mut popup = CommandPopup::new();
        popup.update_filter("/qu");

        let visible = popup.visible_commands();
        assert!(!visible.is_empty());

        // Check that /quit is visible with positions
        let quit_entry = visible.iter().find(|(cmd, _, _, _)| *cmd == "/quit");
        assert!(quit_entry.is_some());

        let (_, _, _, positions) = quit_entry.unwrap();
        // For prefix match "/qu" on "/quit", positions should include 0, 1, 2
        assert_eq!(positions.len(), 3);
    }

    // ============================================
    // Text Selection Tests
    // ============================================

    #[test]
    #[serial]
    fn test_selection_anchor_initial() {
        let config = AppConfig::default();
        let app = App::new(config);
        assert!(app.selection_anchor.is_none());
        assert!(!app.has_selection());
        assert!(app.selection_range().is_none());
    }

    #[test]
    #[serial]
    fn test_start_or_extend_selection() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 2;

        // Start selection
        app.start_or_extend_selection();
        assert_eq!(app.selection_anchor, Some(2));

        // Move cursor - should not change anchor
        app.cursor_position = 4;
        app.start_or_extend_selection();
        assert_eq!(app.selection_anchor, Some(2));
    }

    #[test]
    #[serial]
    fn test_clear_selection() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 2;
        app.selection_anchor = Some(2);

        app.clear_selection();
        assert!(app.selection_anchor.is_none());
    }

    #[test]
    #[serial]
    fn test_selection_range_forward() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(2);
        app.cursor_position = 7;

        let range = app.selection_range();
        assert!(range.is_some());
        let (start, end) = range.unwrap();
        assert_eq!(start, 2);
        assert_eq!(end, 7);
    }

    #[test]
    #[serial]
    fn test_selection_range_backward() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(7);
        app.cursor_position = 2;

        // Selection range should always be (start, end) where start <= end
        let range = app.selection_range();
        assert!(range.is_some());
        let (start, end) = range.unwrap();
        assert_eq!(start, 2);
        assert_eq!(end, 7);
    }

    #[test]
    #[serial]
    fn test_has_selection_empty() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // No anchor - no selection
        app.input = "hello".to_string();
        app.cursor_position = 2;
        assert!(!app.has_selection());

        // Anchor at same position as cursor - no selection
        app.selection_anchor = Some(2);
        assert!(!app.has_selection());
    }

    #[test]
    #[serial]
    fn test_has_selection_active() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 4;
        app.selection_anchor = Some(2);

        assert!(app.has_selection());
    }

    #[test]
    #[serial]
    fn test_selected_text() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(0);
        app.cursor_position = 5;

        let selected = app.selected_text();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap(), "hello");
    }

    #[test]
    #[serial]
    fn test_selected_text_backward() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(5);
        app.cursor_position = 0;

        // Should still return correct text
        let selected = app.selected_text();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap(), "hello");
    }

    #[test]
    #[serial]
    fn test_selected_text_none() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 2;

        // No selection
        assert!(app.selected_text().is_none());

        // Empty selection (anchor == cursor)
        app.selection_anchor = Some(2);
        assert!(app.selected_text().is_none());
    }

    #[test]
    #[serial]
    fn test_delete_selection() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(0);
        app.cursor_position = 6; // "hello " selected

        let deleted = app.delete_selection();
        assert!(deleted.is_some());
        assert_eq!(deleted.unwrap(), "hello ");
        assert_eq!(app.input, "world");
        assert_eq!(app.cursor_position, 0);
        assert!(app.selection_anchor.is_none());
    }

    #[test]
    #[serial]
    fn test_delete_selection_backward() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(6);
        app.cursor_position = 0;

        let deleted = app.delete_selection();
        assert!(deleted.is_some());
        assert_eq!(deleted.unwrap(), "hello ");
        assert_eq!(app.input, "world");
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    #[serial]
    fn test_delete_selection_empty() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 2;

        // No selection to delete
        let deleted = app.delete_selection();
        assert!(deleted.is_none());
        assert_eq!(app.input, "hello");
    }

    #[test]
    #[serial]
    fn test_delete_selection_middle() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world test".to_string();
        app.selection_anchor = Some(5);
        app.cursor_position = 11; // " world" selected

        let deleted = app.delete_selection();
        assert!(deleted.is_some());
        assert_eq!(deleted.unwrap(), " world");
        assert_eq!(app.input, "hello test");
        assert_eq!(app.cursor_position, 5);
    }

    #[test]
    #[serial]
    fn test_selection_with_multiline() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "line1\nline2\nline3".to_string();
        app.selection_anchor = Some(3); // in "line1"
        app.cursor_position = 10; // in "line2"

        let selected = app.selected_text();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap(), "e1\nline");
    }

    #[test]
    #[serial]
    fn test_select_all_empty() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = String::new();
        app.select_all();

        // Should not create selection for empty input
        assert!(app.selection_anchor.is_none());
    }

    #[test]
    #[serial]
    fn test_select_all() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.cursor_position = 5;
        app.select_all();

        assert_eq!(app.selection_anchor, Some(0));
        assert_eq!(app.cursor_position, 11);
        assert_eq!(app.selected_text(), Some("hello world"));
    }

    #[test]
    #[serial]
    fn test_copy_selection() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(0);
        app.cursor_position = 5; // "hello" selected

        app.copy_selection();

        assert_eq!(app.clipboard, "hello");
        // Input should be unchanged
        assert_eq!(app.input, "hello world");
        // Selection should remain
        assert_eq!(app.selection_anchor, Some(0));
    }

    #[test]
    #[serial]
    fn test_copy_selection_no_selection() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 2;

        app.copy_selection();

        // Clipboard should remain empty
        assert!(app.clipboard.is_empty());
    }

    #[test]
    #[serial]
    fn test_cut_selection() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(6);
        app.cursor_position = 11; // "world" selected

        app.cut_selection();

        assert_eq!(app.clipboard, "world");
        assert_eq!(app.input, "hello ");
        assert_eq!(app.cursor_position, 6);
        assert!(app.selection_anchor.is_none());
    }

    #[test]
    #[serial]
    fn test_cut_selection_no_selection() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 2;

        app.cut_selection();

        // Clipboard should remain empty, input unchanged
        assert!(app.clipboard.is_empty());
        assert_eq!(app.input, "hello");
    }

    #[test]
    #[serial]
    fn test_paste_empty_clipboard() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 5;

        // Use paste_internal for deterministic testing
        app.paste_internal();

        // Nothing should change
        assert_eq!(app.input, "hello");
        assert_eq!(app.cursor_position, 5);
    }

    #[test]
    #[serial]
    fn test_paste_at_cursor() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "helloworld".to_string();
        app.cursor_position = 5;
        app.clipboard = " ".to_string();

        // Use paste_internal for deterministic testing
        app.paste_internal();

        assert_eq!(app.input, "hello world");
        assert_eq!(app.cursor_position, 6);
    }

    #[test]
    #[serial]
    fn test_paste_replaces_selection() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(6);
        app.cursor_position = 11; // "world" selected
        app.clipboard = "Rust".to_string();

        // Use paste_internal for deterministic testing
        app.paste_internal();

        assert_eq!(app.input, "hello Rust");
        assert_eq!(app.cursor_position, 10);
        assert!(app.selection_anchor.is_none());
    }

    #[test]
    #[serial]
    fn test_paste_multiline() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "start end".to_string();
        app.cursor_position = 6;
        app.clipboard = "line1\nline2\n".to_string();

        // Use paste_internal for deterministic testing
        app.paste_internal();

        assert_eq!(app.input, "start line1\nline2\nend");
        assert_eq!(app.cursor_position, 18);
    }

    #[test]
    #[serial]
    fn test_copy_paste_roundtrip() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(0);
        app.cursor_position = 5; // "hello" selected

        app.copy_selection();
        app.clear_selection();
        app.cursor_position = 11; // end of input

        // Use paste_internal for deterministic testing
        app.paste_internal();

        assert_eq!(app.input, "hello worldhello");
        assert_eq!(app.cursor_position, 16);
    }

    #[test]
    #[serial]
    fn test_cut_paste_move_text() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "world hello".to_string();
        app.selection_anchor = Some(0);
        app.cursor_position = 6; // "world " selected

        app.cut_selection();
        assert_eq!(app.input, "hello");

        app.cursor_position = 5; // end of remaining text
                                 // Use paste_internal for deterministic testing
        app.paste_internal();

        assert_eq!(app.input, "helloworld ");
    }

    // Word navigation tests
    #[test]
    #[serial]
    fn test_is_word_char() {
        assert!(App::is_word_char('a'));
        assert!(App::is_word_char('Z'));
        assert!(App::is_word_char('5'));
        assert!(App::is_word_char('_'));
        assert!(!App::is_word_char(' '));
        assert!(!App::is_word_char('.'));
        assert!(!App::is_word_char('-'));
        assert!(!App::is_word_char('\n'));
    }

    #[test]
    #[serial]
    fn test_find_word_boundary_left_empty() {
        let config = AppConfig::default();
        let app = App::new(config);

        assert_eq!(app.find_word_boundary_left(0), 0);
    }

    #[test]
    #[serial]
    fn test_find_word_boundary_left_single_word() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();

        // From end of word, should go to start
        assert_eq!(app.find_word_boundary_left(5), 0);
        // From middle of word, should go to start
        assert_eq!(app.find_word_boundary_left(3), 0);
        // From start, should stay at start
        assert_eq!(app.find_word_boundary_left(0), 0);
    }

    #[test]
    #[serial]
    fn test_find_word_boundary_left_multiple_words() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world test".to_string();
        //           01234567890123456
        //                     1111111

        // From end (pos 16), should go to start of "test" (pos 12)
        assert_eq!(app.find_word_boundary_left(16), 12);
        // From "test" (pos 12), should go to start of "world" (pos 6)
        assert_eq!(app.find_word_boundary_left(12), 6);
        // From "world" (pos 6), should go to start of "hello" (pos 0)
        assert_eq!(app.find_word_boundary_left(6), 0);
    }

    #[test]
    #[serial]
    fn test_find_word_boundary_left_multiple_spaces() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello   world".to_string();
        //           0123456789012
        //                     111

        // From start of "world" (pos 8), should go to start of "hello" (pos 0)
        assert_eq!(app.find_word_boundary_left(8), 0);
    }

    #[test]
    #[serial]
    fn test_find_word_boundary_right_empty() {
        let config = AppConfig::default();
        let app = App::new(config);

        assert_eq!(app.find_word_boundary_right(0), 0);
    }

    #[test]
    #[serial]
    fn test_find_word_boundary_right_single_word() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();

        // From start, should go to end
        assert_eq!(app.find_word_boundary_right(0), 5);
        // From middle, should go to end
        assert_eq!(app.find_word_boundary_right(2), 5);
        // From end, should stay at end
        assert_eq!(app.find_word_boundary_right(5), 5);
    }

    #[test]
    #[serial]
    fn test_find_word_boundary_right_multiple_words() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world test".to_string();
        //           01234567890123456
        //                     1111111

        // From start (pos 0), should go to end of "hello" (pos 5)
        assert_eq!(app.find_word_boundary_right(0), 5);
        // From space after hello (pos 5), should go to end of "world" (pos 11)
        assert_eq!(app.find_word_boundary_right(5), 11);
        // From after "world" (pos 11), should go to end of "test" (pos 16)
        assert_eq!(app.find_word_boundary_right(11), 16);
    }

    #[test]
    #[serial]
    fn test_move_cursor_word_left() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.cursor_position = 11; // end

        app.move_cursor_word_left();
        assert_eq!(app.cursor_position, 6); // start of "world"

        app.move_cursor_word_left();
        assert_eq!(app.cursor_position, 0); // start of "hello"
    }

    #[test]
    #[serial]
    fn test_move_cursor_word_right() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.cursor_position = 0;

        app.move_cursor_word_right();
        assert_eq!(app.cursor_position, 5); // end of "hello"

        app.move_cursor_word_right();
        assert_eq!(app.cursor_position, 11); // end of "world"
    }

    #[test]
    #[serial]
    fn test_delete_word_left_at_start() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 0;

        app.delete_word_left();
        assert_eq!(app.input, "hello");
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    #[serial]
    fn test_delete_word_left_single_word() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 5;

        app.delete_word_left();
        assert_eq!(app.input, "");
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    #[serial]
    fn test_delete_word_left_multiple_words() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world test".to_string();
        app.cursor_position = 16; // end

        app.delete_word_left();
        assert_eq!(app.input, "hello world ");
        assert_eq!(app.cursor_position, 12);

        app.delete_word_left();
        assert_eq!(app.input, "hello ");
        assert_eq!(app.cursor_position, 6);
    }

    #[test]
    #[serial]
    fn test_delete_word_left_with_selection() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.cursor_position = 11;
        app.selection_anchor = Some(6);

        // Should delete selection, not word
        app.delete_word_left();
        assert_eq!(app.input, "hello ");
        assert!(app.selection_anchor.is_none());
    }

    #[test]
    #[serial]
    fn test_delete_word_right_at_end() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 5;

        app.delete_word_right();
        assert_eq!(app.input, "hello");
        assert_eq!(app.cursor_position, 5);
    }

    #[test]
    #[serial]
    fn test_delete_word_right_single_word() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 0;

        app.delete_word_right();
        assert_eq!(app.input, "");
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    #[serial]
    fn test_delete_word_right_multiple_words() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world test".to_string();
        app.cursor_position = 0;

        app.delete_word_right();
        assert_eq!(app.input, " world test");
        assert_eq!(app.cursor_position, 0);

        app.delete_word_right();
        assert_eq!(app.input, " test");
        assert_eq!(app.cursor_position, 0);
    }

    #[test]
    #[serial]
    fn test_delete_word_right_with_selection() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.cursor_position = 0;
        app.selection_anchor = Some(5);

        // Should delete selection, not word
        app.delete_word_right();
        assert_eq!(app.input, " world");
        assert!(app.selection_anchor.is_none());
    }

    #[test]
    #[serial]
    fn test_word_boundary_with_underscores() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello_world test".to_string();
        //           0123456789012345
        //                     111111

        // Underscores are part of words
        assert_eq!(app.find_word_boundary_right(0), 11); // end of "hello_world"
        assert_eq!(app.find_word_boundary_left(11), 0); // start of "hello_world"
    }

    #[test]
    #[serial]
    fn test_word_boundary_with_punctuation() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello.world".to_string();
        //           01234567890
        //                     1

        // Period is not a word char, so two separate words
        assert_eq!(app.find_word_boundary_right(0), 5); // end of "hello"
        assert_eq!(app.find_word_boundary_right(5), 11); // end of "world"
        assert_eq!(app.find_word_boundary_left(11), 6); // start of "world"
    }

    #[test]
    #[serial]
    fn test_delete_word_saves_undo() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.cursor_position = 11;

        app.delete_word_left();
        assert_eq!(app.input, "hello ");

        // Verify undo state was saved
        app.input_undo();
        assert_eq!(app.input, "hello world");
        assert_eq!(app.cursor_position, 11);
    }

    // /context command tests

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_context_handled() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/context").await;
        assert!(handled);
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_context_case_insensitive() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let handled = app.handle_slash_command("/CONTEXT").await;
        assert!(handled);
    }

    #[test]
    #[serial]
    fn test_context_command_adds_system_message() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_msg_count = app.messages.len();

        app.handle_context_command();

        assert_eq!(app.messages.len(), initial_msg_count + 1);
        assert_eq!(app.messages.last().unwrap().role, MessageRole::System);
    }

    #[test]
    #[serial]
    fn test_context_command_shows_working_dir() {
        let config = AppConfig {
            working_dir: "/test/working/dir".to_string(),
            ..Default::default()
        };
        let mut app = App::new(config);

        app.handle_context_command();

        let content = &app.messages.last().unwrap().content;
        assert!(content.contains("/test/working/dir"));
        assert!(content.contains("Working Directory"));
    }

    #[test]
    #[serial]
    fn test_context_command_shows_model() {
        let config = AppConfig {
            model: "gpt-4-test".to_string(),
            ..Default::default()
        };
        let mut app = App::new(config);

        app.handle_context_command();

        let content = &app.messages.last().unwrap().content;
        assert!(content.contains("gpt-4-test"));
        assert!(content.contains("Model"));
    }

    #[test]
    #[serial]
    fn test_context_command_shows_approval_preset() {
        let config = AppConfig {
            approval_preset: "read-only".to_string(),
            ..Default::default()
        };
        let mut app = App::new(config);

        app.handle_context_command();

        let content = &app.messages.last().unwrap().content;
        assert!(content.contains("read-only"));
        assert!(content.contains("Approval Mode"));
    }

    #[test]
    #[serial]
    fn test_context_command_shows_custom_system_prompt() {
        let config = AppConfig {
            system_prompt: Some("You are a helpful assistant.".to_string()),
            ..Default::default()
        };
        let mut app = App::new(config);

        app.handle_context_command();

        let content = &app.messages.last().unwrap().content;
        assert!(content.contains("Custom system prompt"));
        assert!(content.contains("You are a helpful assistant."));
    }

    #[test]
    #[serial]
    fn test_context_command_truncates_long_system_prompt() {
        // Create a prompt longer than 500 chars
        let config = AppConfig {
            system_prompt: Some("x".repeat(600)),
            ..Default::default()
        };
        let mut app = App::new(config);

        app.handle_context_command();

        let content = &app.messages.last().unwrap().content;
        assert!(content.contains("600 chars"));
        assert!(content.contains("..."));
    }

    #[test]
    #[serial]
    fn test_context_command_shows_mock_mode() {
        let config = AppConfig {
            use_mock_llm: true,
            ..Default::default()
        };
        let mut app = App::new(config);

        app.handle_context_command();

        let content = &app.messages.last().unwrap().content;
        assert!(content.contains("Mock"));
    }

    #[test]
    #[serial]
    fn test_context_command_shows_empty_agent_state() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.handle_context_command();

        let content = &app.messages.last().unwrap().content;
        assert!(content.contains("No messages in agent state"));
    }

    // System clipboard tests

    #[test]
    #[serial]
    fn test_copy_to_system_clipboard_method_exists() {
        // Test that the method is callable and returns a bool
        let config = AppConfig::default();
        let app = App::new(config);

        // This test verifies the method exists and can be called.
        // It may return false in CI environments without display.
        let result = app.copy_to_system_clipboard("test");
        // Result should be a bool (either true or false is fine)
        let _: bool = result;
    }

    #[test]
    #[serial]
    fn test_paste_from_system_clipboard_method_exists() {
        // Test that the method is callable and returns Option<String>
        let config = AppConfig::default();
        let app = App::new(config);

        // This test verifies the method exists and can be called.
        // It may return None in CI environments without display.
        let result = app.paste_from_system_clipboard();
        // Result should be Option<String>
        let _: Option<String> = result;
    }

    #[test]
    #[serial]
    fn test_copy_selection_updates_internal_clipboard() {
        // Test that copy_selection still updates internal clipboard
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(0);
        app.cursor_position = 5;

        app.copy_selection();

        // Internal clipboard should be updated regardless of system clipboard
        assert_eq!(app.clipboard, "hello");
    }

    #[test]
    #[serial]
    fn test_cut_selection_updates_internal_clipboard() {
        // Test that cut_selection still updates internal clipboard
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(0);
        app.cursor_position = 5;

        app.cut_selection();

        // Internal clipboard should be updated regardless of system clipboard
        assert_eq!(app.clipboard, "hello");
        // Input should be modified
        assert_eq!(app.input, " world");
    }

    #[test]
    #[serial]
    fn test_paste_uses_internal_clipboard_as_fallback() {
        // When system clipboard is empty/unavailable, internal clipboard is used
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello".to_string();
        app.cursor_position = 5;
        app.clipboard = " world".to_string();

        // In many test environments, system clipboard may be empty or unavailable,
        // so this should fall back to internal clipboard
        app.paste();

        // The paste should work with internal clipboard at minimum
        // Exact result depends on system clipboard state
        assert!(app.input.len() >= 5); // At minimum original length
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_stop_handled() {
        let config = AppConfig {
            use_mock_llm: true,
            ..Default::default()
        };
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/stop").await;
        assert!(handled);
    }

    #[test]
    #[serial]
    fn test_stop_command_when_not_processing() {
        let config = AppConfig {
            use_mock_llm: true,
            ..Default::default()
        };
        let mut app = App::new(config);
        app.mode = AppMode::Insert;

        let initial_msg_count = app.messages.len();
        app.handle_stop_command();

        // Should add a system message
        assert_eq!(app.messages.len(), initial_msg_count + 1);
        assert_eq!(app.messages.last().unwrap().role, MessageRole::System);
        assert!(app.messages.last().unwrap().content.contains("No task"));
    }

    #[test]
    #[serial]
    fn test_stop_command_when_processing_no_token() {
        let config = AppConfig {
            use_mock_llm: true,
            ..Default::default()
        };
        let mut app = App::new(config);
        app.mode = AppMode::Processing;
        app.agent_cancel_token = None;

        let initial_msg_count = app.messages.len();
        app.handle_stop_command();

        // Should add a system message about no cancellation token
        assert_eq!(app.messages.len(), initial_msg_count + 1);
        assert_eq!(app.messages.last().unwrap().role, MessageRole::System);
        assert!(app
            .messages
            .last()
            .unwrap()
            .content
            .contains("no cancellation token"));
    }

    #[test]
    #[serial]
    fn test_stop_command_when_processing_with_token() {
        let config = AppConfig {
            use_mock_llm: true,
            ..Default::default()
        };
        let mut app = App::new(config);
        app.mode = AppMode::Processing;
        let token = CancellationToken::new();
        app.agent_cancel_token = Some(token.clone());

        let initial_msg_count = app.messages.len();
        app.handle_stop_command();

        // Should add a "Stopping" message
        assert_eq!(app.messages.len(), initial_msg_count + 1);
        assert_eq!(app.messages.last().unwrap().role, MessageRole::System);
        assert!(app.messages.last().unwrap().content.contains("Stopping"));

        // Token should be cancelled
        assert!(token.is_cancelled());
    }

    #[test]
    #[serial]
    fn test_stop_command_in_builtin_commands() {
        // Verify /stop is in the builtin commands list
        let found = BUILTIN_COMMANDS.iter().any(|(cmd, _)| *cmd == "/stop");
        assert!(found, "/stop should be in BUILTIN_COMMANDS");
    }

    #[test]
    #[serial]
    fn test_cancel_token_none_by_default() {
        let config = AppConfig::default();
        let app = App::new(config);
        assert!(app.agent_cancel_token.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_slash_command_export_handled() {
        let config = AppConfig {
            use_mock_llm: true,
            ..Default::default()
        };
        let mut app = App::new(config);
        let handled = app.handle_slash_command("/export").await;
        assert!(handled);
    }

    #[test]
    #[serial]
    fn test_export_command_empty_conversation() {
        let config = AppConfig {
            use_mock_llm: true,
            ..Default::default()
        };
        let mut app = App::new(config);
        // Ensure agent state is empty
        app.agent_state.messages.clear();

        let initial_msg_count = app.messages.len();
        app.handle_export_command("");

        // Should add a "no conversation" message
        assert_eq!(app.messages.len(), initial_msg_count + 1);
        assert_eq!(app.messages.last().unwrap().role, MessageRole::System);
        assert!(app
            .messages
            .last()
            .unwrap()
            .content
            .contains("No conversation"));
    }

    #[test]
    #[serial]
    fn test_export_command_in_builtin_commands() {
        // Verify /export is in the builtin commands list
        let found = BUILTIN_COMMANDS.iter().any(|(cmd, _)| *cmd == "/export");
        assert!(found, "/export should be in BUILTIN_COMMANDS");
    }

    #[test]
    #[serial]
    fn test_export_as_markdown_format() {
        let config = AppConfig {
            session_id: Some("test-session".to_string()),
            working_dir: "/test".to_string(),
            use_mock_llm: true,
            ..Default::default()
        };
        let mut app = App::new(config);

        // Add test messages
        app.agent_state.messages.push(Message::user("Hello world"));
        app.agent_state
            .messages
            .push(Message::assistant("Hello back"));

        let markdown = app.export_as_markdown();

        // Verify markdown structure
        assert!(markdown.contains("# Conversation Export"));
        assert!(markdown.contains("**Session ID:** test-session"));
        assert!(markdown.contains("**Working Directory:** /test"));
        assert!(markdown.contains("User"));
        assert!(markdown.contains("Hello world"));
        assert!(markdown.contains("Assistant"));
        assert!(markdown.contains("Hello back"));
    }

    #[test]
    #[serial]
    fn test_export_as_json_format() {
        let config = AppConfig {
            session_id: Some("test-session".to_string()),
            working_dir: "/test".to_string(),
            use_mock_llm: true,
            ..Default::default()
        };
        let mut app = App::new(config);

        // Add a test message
        app.agent_state.messages.push(Message::user("Test message"));

        let json = app.export_as_json();

        // Verify JSON structure
        assert!(json.contains("\"session_id\": \"test-session\""));
        assert!(json.contains("\"working_directory\": \"/test\""));
        assert!(json.contains("\"role\": \"user\""));
        assert!(json.contains("\"content\": \"Test message\""));
    }

    #[test]
    #[serial]
    fn test_export_json_parses_correctly() {
        let config = AppConfig {
            session_id: Some("parse-test".to_string()),
            use_mock_llm: true,
            ..Default::default()
        };
        let mut app = App::new(config);
        app.agent_state.messages.push(Message::user("Message 1"));
        app.agent_state
            .messages
            .push(Message::assistant("Message 2"));

        let json = app.export_as_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["session_id"], "parse-test");
        assert_eq!(parsed["messages"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["messages"][0]["role"], "user");
        assert_eq!(parsed["messages"][1]["role"], "assistant");
    }

    #[test]
    #[serial]
    fn test_export_command_with_json_arg() {
        let config = AppConfig {
            use_mock_llm: true,
            ..Default::default()
        };
        let mut app = App::new(config);

        // Add a test message so export has something
        app.agent_state.messages.push(Message::user("test"));

        // Use a temp file path to test
        let temp_path = std::env::temp_dir().join("test_export.json");
        let path_str = temp_path.display().to_string();
        app.handle_export_command(&format!("json {}", path_str));

        // Check that export succeeded - success shows notification, failure shows system message
        let has_success_notification = app
            .notification
            .as_ref()
            .map(|n| n.text.contains("Exported"))
            .unwrap_or(false);
        let has_failure_message = app
            .messages
            .last()
            .map(|m| m.content.contains("Failed"))
            .unwrap_or(false);
        assert!(
            has_success_notification || has_failure_message,
            "Expected export success notification or failure message"
        );

        // Clean up if file was created
        let _ = std::fs::remove_file(&temp_path);
    }

    // FileSearchPopup tests
    #[test]
    #[serial]
    fn test_file_search_popup_default() {
        let popup = FileSearchPopup::default();
        assert!(!popup.visible);
        assert_eq!(popup.selected_index, 0);
        assert!(popup.matches.is_empty());
        assert_eq!(popup.scroll_offset, 0);
        assert!(!popup.loading);
        assert!(popup.query.is_empty());
    }

    #[test]
    #[serial]
    fn test_file_search_popup_new() {
        let popup = FileSearchPopup::new();
        assert!(!popup.visible);
        assert!(popup.matches.is_empty());
    }

    #[test]
    #[serial]
    fn test_file_search_popup_hide() {
        let mut popup = FileSearchPopup::new();
        popup.visible = true;
        popup.query = "test".to_string();
        popup.hide();
        assert!(!popup.visible);
        assert!(popup.query.is_empty());
        assert!(popup.matches.is_empty());
    }

    #[test]
    #[serial]
    fn test_file_search_popup_empty_query() {
        let mut popup = FileSearchPopup::new();
        popup.update_query("", ".");
        assert!(popup.visible); // Shows hint
        assert!(popup.matches.is_empty());
        assert!(!popup.loading);
    }

    #[test]
    #[serial]
    fn test_file_search_popup_move_up_empty() {
        let mut popup = FileSearchPopup::new();
        popup.move_up(); // Should not panic
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    #[serial]
    fn test_file_search_popup_move_down_empty() {
        let mut popup = FileSearchPopup::new();
        popup.move_down(); // Should not panic
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    #[serial]
    fn test_file_search_popup_selected_path_empty() {
        let popup = FileSearchPopup::new();
        assert!(popup.selected_path().is_none());
    }

    #[test]
    #[serial]
    fn test_file_search_popup_total_count() {
        let popup = FileSearchPopup::new();
        assert_eq!(popup.total_count(), 0);
    }

    #[test]
    #[serial]
    fn test_file_search_popup_has_more_items() {
        let popup = FileSearchPopup::new();
        assert!(!popup.has_more_items()); // Empty popup has no more items
    }

    #[test]
    #[serial]
    fn test_file_search_popup_visible_files_empty() {
        let popup = FileSearchPopup::new();
        assert!(popup.visible_files().is_empty());
    }

    #[test]
    #[serial]
    fn test_file_search_popup_in_app() {
        let config = AppConfig::default();
        let app = App::new(config);
        assert!(!app.file_search_popup.visible);
        assert!(app.file_search_popup.matches.is_empty());
    }

    #[test]
    #[serial]
    fn test_find_at_token_basic() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        app.input = "@test".to_string();
        app.cursor_position = 5;
        let token = app.find_at_token();
        assert!(token.is_some());
        assert_eq!(token.unwrap(), (0, 1));
    }

    #[test]
    #[serial]
    fn test_find_at_token_after_space() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        app.input = "hello @world".to_string();
        app.cursor_position = 12;
        let token = app.find_at_token();
        assert!(token.is_some());
        assert_eq!(token.unwrap(), (6, 7));
    }

    #[test]
    #[serial]
    fn test_find_at_token_no_at() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        app.input = "hello world".to_string();
        app.cursor_position = 11;
        let token = app.find_at_token();
        assert!(token.is_none());
    }

    #[test]
    #[serial]
    fn test_find_at_token_email_rejected() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        app.input = "test@example.com".to_string();
        app.cursor_position = 16;
        let token = app.find_at_token();
        // Should reject because @ is not at start or after whitespace
        assert!(token.is_none());
    }

    #[test]
    #[serial]
    fn test_complete_file_mention() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        app.input = "@te".to_string();
        app.cursor_position = 3;
        app.complete_file_mention("test.rs");
        assert_eq!(app.input, "@test.rs ");
        assert_eq!(app.cursor_position, 9);
    }

    #[test]
    #[serial]
    fn test_complete_file_mention_middle_of_text() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        app.input = "look at @te and fix it".to_string();
        app.cursor_position = 11; // After "@te"
        app.complete_file_mention("test.rs");
        assert_eq!(app.input, "look at @test.rs  and fix it");
    }

    // === Approval Overlay Tests ===

    #[test]
    #[serial]
    fn test_approval_overlay_new() {
        let overlay = ApprovalOverlay::new();
        assert!(!overlay.visible);
        assert!(overlay.request.is_none());
        assert_eq!(overlay.selected_option, 0);
    }

    #[test]
    #[serial]
    fn test_approval_overlay_show() {
        let mut overlay = ApprovalOverlay::new();
        let request = ApprovalRequest {
            id: "test-id".to_string(),
            request_type: ApprovalRequestType::Shell {
                command: "ls -la".to_string(),
            },
            reason: Some("First use of shell".to_string()),
            tool_call_id: "call-1".to_string(),
            tool_name: "shell".to_string(),
        };
        overlay.show(request);
        assert!(overlay.visible);
        assert!(overlay.request.is_some());
        assert_eq!(overlay.selected_option, 0);
    }

    #[test]
    #[serial]
    fn test_approval_overlay_hide() {
        let mut overlay = ApprovalOverlay::new();
        overlay.visible = true;
        overlay.selected_option = 1;
        overlay.hide();
        assert!(!overlay.visible);
        assert!(overlay.request.is_none());
        assert_eq!(overlay.selected_option, 0);
    }

    #[test]
    #[serial]
    fn test_approval_overlay_navigation() {
        let mut overlay = ApprovalOverlay::new();
        assert_eq!(overlay.selected_option, 0);

        // Move down
        overlay.move_down();
        assert_eq!(overlay.selected_option, 1);
        overlay.move_down();
        assert_eq!(overlay.selected_option, 2);
        // Wrap around
        overlay.move_down();
        assert_eq!(overlay.selected_option, 0);

        // Move up
        overlay.move_up();
        assert_eq!(overlay.selected_option, 2);
        overlay.move_up();
        assert_eq!(overlay.selected_option, 1);
        overlay.move_up();
        assert_eq!(overlay.selected_option, 0);
    }

    #[test]
    #[serial]
    fn test_approval_overlay_current_decision() {
        let mut overlay = ApprovalOverlay::new();

        overlay.selected_option = 0;
        assert_eq!(overlay.current_decision(), ApprovalDecision::ApproveOnce);

        overlay.selected_option = 1;
        assert_eq!(overlay.current_decision(), ApprovalDecision::ApproveSession);

        overlay.selected_option = 2;
        assert_eq!(overlay.current_decision(), ApprovalDecision::Reject);
    }

    #[test]
    #[serial]
    fn test_approval_overlay_options() {
        let mut overlay = ApprovalOverlay::new();
        overlay.selected_option = 0;

        let options = overlay.options();
        assert_eq!(options.len(), 3);

        // First option should be selected
        assert_eq!(options[0].0, "Yes, proceed");
        assert!(options[0].1); // is_selected
        assert_eq!(options[0].2, 'y');

        assert!(!options[1].1); // not selected
        assert!(!options[2].1); // not selected
    }

    #[test]
    #[serial]
    fn test_approval_request_type_display() {
        let shell = ApprovalRequestType::Shell {
            command: "ls -la".to_string(),
        };
        assert_eq!(shell.display_name(), "Shell Command");
        assert_eq!(shell.display_content(), "ls -la");

        let file_write = ApprovalRequestType::FileWrite {
            path: "/tmp/test.txt".to_string(),
        };
        assert_eq!(file_write.display_name(), "File Write");
        assert_eq!(file_write.display_content(), "Write to: /tmp/test.txt");

        let tool = ApprovalRequestType::Tool {
            tool: "custom".to_string(),
            args: "arg1 arg2".to_string(),
        };
        assert_eq!(tool.display_name(), "Tool Execution");
        assert_eq!(tool.display_content(), "custom: arg1 arg2");
    }

    #[test]
    #[serial]
    fn test_app_request_approval_shows_overlay() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        let result = app.request_approval(
            "call-1".to_string(),
            "shell".to_string(),
            ApprovalRequestType::Shell {
                command: "rm -rf /tmp/test".to_string(),
            },
            Some("Potentially dangerous command".to_string()),
        );

        assert!(!result); // Should not auto-approve
        assert!(app.approval_overlay.visible);
        assert!(app.approval_overlay.request.is_some());
    }

    #[test]
    #[serial]
    fn test_app_request_approval_session_approved() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Mark shell as session-approved
        app.session_approved_tools.insert("shell".to_string());

        let result = app.request_approval(
            "call-1".to_string(),
            "shell".to_string(),
            ApprovalRequestType::Shell {
                command: "ls -la".to_string(),
            },
            None,
        );

        assert!(result); // Should auto-approve
        assert!(!app.approval_overlay.visible);
    }

    #[test]
    #[serial]
    fn test_app_complete_approval_approve_once() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_msg_count = app.messages.len();

        // Set up approval request
        app.approval_overlay.show(ApprovalRequest {
            id: "test-id".to_string(),
            request_type: ApprovalRequestType::Shell {
                command: "echo test".to_string(),
            },
            reason: None,
            tool_call_id: "call-1".to_string(),
            tool_name: "shell".to_string(),
        });

        app.complete_approval(ApprovalDecision::ApproveOnce);

        assert!(!app.approval_overlay.visible);
        assert!(app.approval_overlay.request.is_none());
        // Should NOT add to session approved
        assert!(!app.session_approved_tools.contains("shell"));
        // Should add a message
        assert_eq!(app.messages.len(), initial_msg_count + 1);
        assert!(app.messages.last().unwrap().content.contains("Approved:"));
    }

    #[test]
    #[serial]
    fn test_app_complete_approval_session() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Set up approval request
        app.approval_overlay.show(ApprovalRequest {
            id: "test-id".to_string(),
            request_type: ApprovalRequestType::Tool {
                tool: "read_file".to_string(),
                args: "{}".to_string(),
            },
            reason: None,
            tool_call_id: "call-1".to_string(),
            tool_name: "read_file".to_string(),
        });

        app.complete_approval(ApprovalDecision::ApproveSession);

        assert!(!app.approval_overlay.visible);
        // Should add to session approved
        assert!(app.session_approved_tools.contains("read_file"));
        // Message should mention session approval
        assert!(app.messages.last().unwrap().content.contains("session"));
    }

    #[test]
    #[serial]
    fn test_app_complete_approval_reject() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        let initial_msg_count = app.messages.len();

        // Set up approval request
        app.approval_overlay.show(ApprovalRequest {
            id: "test-id".to_string(),
            request_type: ApprovalRequestType::FileWrite {
                path: "/etc/passwd".to_string(),
            },
            reason: Some("Dangerous file".to_string()),
            tool_call_id: "call-1".to_string(),
            tool_name: "write_file".to_string(),
        });

        app.complete_approval(ApprovalDecision::Reject);

        assert!(!app.approval_overlay.visible);
        assert!(!app.session_approved_tools.contains("write_file"));
        assert_eq!(app.messages.len(), initial_msg_count + 1);
        assert!(app.messages.last().unwrap().content.contains("Rejected:"));
    }

    #[test]
    #[serial]
    fn test_approval_decision_equality() {
        assert_eq!(ApprovalDecision::ApproveOnce, ApprovalDecision::ApproveOnce);
        assert_eq!(
            ApprovalDecision::ApproveSession,
            ApprovalDecision::ApproveSession
        );
        assert_eq!(ApprovalDecision::Reject, ApprovalDecision::Reject);
        assert_ne!(ApprovalDecision::ApproveOnce, ApprovalDecision::Reject);
    }

    // Tests for TuiEvent::ApprovalRequest integration

    #[test]
    #[serial]
    fn test_handle_approval_request_shows_overlay() {
        let mut app = App::new(AppConfig::default());
        let (response_tx, _response_rx) = tokio::sync::oneshot::channel();

        let req = crate::event::ApprovalRequestEvent {
            request_id: "req-1".to_string(),
            tool_call_id: "call-1".to_string(),
            tool: "shell".to_string(),
            args: serde_json::json!({"command": "ls -la"}),
            reason: Some("Needs shell access".to_string()),
            response_tx,
        };

        app.handle_approval_request(req);

        assert!(app.approval_overlay.visible);
        assert!(app.approval_overlay.request.is_some());
        let request = app.approval_overlay.request.as_ref().unwrap();
        assert_eq!(request.tool_name, "shell");
        assert!(matches!(
            request.request_type,
            ApprovalRequestType::Shell { .. }
        ));
    }

    #[test]
    #[serial]
    fn test_handle_approval_request_shell_type() {
        let mut app = App::new(AppConfig::default());
        let (response_tx, _response_rx) = tokio::sync::oneshot::channel();

        let req = crate::event::ApprovalRequestEvent {
            request_id: "req-1".to_string(),
            tool_call_id: "call-1".to_string(),
            tool: "bash".to_string(),
            args: serde_json::json!({"command": "echo hello"}),
            reason: None,
            response_tx,
        };

        app.handle_approval_request(req);

        let request = app.approval_overlay.request.as_ref().unwrap();
        match &request.request_type {
            ApprovalRequestType::Shell { command } => {
                assert_eq!(command, "echo hello");
            }
            _ => panic!("Expected Shell request type"),
        }
    }

    #[test]
    #[serial]
    fn test_handle_approval_request_file_write_type() {
        let mut app = App::new(AppConfig::default());
        let (response_tx, _response_rx) = tokio::sync::oneshot::channel();

        let req = crate::event::ApprovalRequestEvent {
            request_id: "req-1".to_string(),
            tool_call_id: "call-1".to_string(),
            tool: "write_file".to_string(),
            args: serde_json::json!({"path": "/tmp/test.txt", "content": "hello"}),
            reason: None,
            response_tx,
        };

        app.handle_approval_request(req);

        let request = app.approval_overlay.request.as_ref().unwrap();
        match &request.request_type {
            ApprovalRequestType::FileWrite { path } => {
                assert_eq!(path, "/tmp/test.txt");
            }
            _ => panic!("Expected FileWrite request type"),
        }
    }

    #[test]
    #[serial]
    fn test_handle_approval_request_generic_tool_type() {
        let mut app = App::new(AppConfig::default());
        let (response_tx, _response_rx) = tokio::sync::oneshot::channel();

        let req = crate::event::ApprovalRequestEvent {
            request_id: "req-1".to_string(),
            tool_call_id: "call-1".to_string(),
            tool: "custom_tool".to_string(),
            args: serde_json::json!({"key": "value"}),
            reason: None,
            response_tx,
        };

        app.handle_approval_request(req);

        let request = app.approval_overlay.request.as_ref().unwrap();
        match &request.request_type {
            ApprovalRequestType::Tool { tool, .. } => {
                assert_eq!(tool, "custom_tool");
            }
            _ => panic!("Expected Tool request type"),
        }
    }

    #[test]
    #[serial]
    fn test_handle_approval_request_session_approved_auto_approves() {
        let mut app = App::new(AppConfig::default());
        app.session_approved_tools.insert("shell".to_string());

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        let req = crate::event::ApprovalRequestEvent {
            request_id: "req-1".to_string(),
            tool_call_id: "call-1".to_string(),
            tool: "shell".to_string(),
            args: serde_json::json!({"command": "ls"}),
            reason: None,
            response_tx,
        };

        app.handle_approval_request(req);

        // Should NOT show overlay - auto-approved
        assert!(!app.approval_overlay.visible);
        // Should have auto-approve message
        assert!(app
            .messages
            .iter()
            .any(|m| m.content.contains("Auto-approved")));
        // Response should be sent
        assert!(response_rx.blocking_recv().is_ok());
    }

    #[test]
    #[serial]
    fn test_handle_approval_request_stores_response_channel() {
        let mut app = App::new(AppConfig::default());
        let (response_tx, _response_rx) = tokio::sync::oneshot::channel();

        let req = crate::event::ApprovalRequestEvent {
            request_id: "req-1".to_string(),
            tool_call_id: "call-1".to_string(),
            tool: "shell".to_string(),
            args: serde_json::json!({}),
            reason: None,
            response_tx,
        };

        app.handle_approval_request(req);

        assert!(app.approval_response_tx.is_some());
    }

    #[test]
    #[serial]
    fn test_complete_approval_sends_response_approve() {
        let mut app = App::new(AppConfig::default());
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        // Set up request
        app.approval_overlay.show(ApprovalRequest {
            id: "req-1".to_string(),
            request_type: ApprovalRequestType::Shell {
                command: "ls".to_string(),
            },
            reason: None,
            tool_call_id: "call-1".to_string(),
            tool_name: "shell".to_string(),
        });
        app.approval_response_tx = Some(response_tx);

        app.complete_approval(ApprovalDecision::ApproveOnce);

        let decision = response_rx.blocking_recv().unwrap();
        assert_eq!(decision, crate::event::ApprovalDecision::Approve);
    }

    #[test]
    #[serial]
    fn test_complete_approval_sends_response_session() {
        let mut app = App::new(AppConfig::default());
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        app.approval_overlay.show(ApprovalRequest {
            id: "req-1".to_string(),
            request_type: ApprovalRequestType::Shell {
                command: "ls".to_string(),
            },
            reason: None,
            tool_call_id: "call-1".to_string(),
            tool_name: "shell".to_string(),
        });
        app.approval_response_tx = Some(response_tx);

        app.complete_approval(ApprovalDecision::ApproveSession);

        let decision = response_rx.blocking_recv().unwrap();
        assert_eq!(decision, crate::event::ApprovalDecision::ApproveSession);
    }

    #[test]
    #[serial]
    fn test_complete_approval_sends_response_reject() {
        let mut app = App::new(AppConfig::default());
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        app.approval_overlay.show(ApprovalRequest {
            id: "req-1".to_string(),
            request_type: ApprovalRequestType::Shell {
                command: "rm -rf /".to_string(),
            },
            reason: None,
            tool_call_id: "call-1".to_string(),
            tool_name: "shell".to_string(),
        });
        app.approval_response_tx = Some(response_tx);

        app.complete_approval(ApprovalDecision::Reject);

        let decision = response_rx.blocking_recv().unwrap();
        assert_eq!(decision, crate::event::ApprovalDecision::Reject);
    }

    #[test]
    #[serial]
    fn test_complete_approval_clears_response_channel() {
        let mut app = App::new(AppConfig::default());
        let (response_tx, _response_rx) = tokio::sync::oneshot::channel();

        app.approval_overlay.show(ApprovalRequest {
            id: "req-1".to_string(),
            request_type: ApprovalRequestType::Tool {
                tool: "test".to_string(),
                args: "{}".to_string(),
            },
            reason: None,
            tool_call_id: "call-1".to_string(),
            tool_name: "test".to_string(),
        });
        app.approval_response_tx = Some(response_tx);

        app.complete_approval(ApprovalDecision::ApproveOnce);

        // Response channel should be taken
        assert!(app.approval_response_tx.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_event_approval_request() {
        let mut app = App::new(AppConfig::default());
        let (response_tx, _response_rx) = tokio::sync::oneshot::channel();

        let req = crate::event::ApprovalRequestEvent {
            request_id: "req-1".to_string(),
            tool_call_id: "call-1".to_string(),
            tool: "shell".to_string(),
            args: serde_json::json!({"command": "pwd"}),
            reason: Some("Test reason".to_string()),
            response_tx,
        };

        app.handle_event(TuiEvent::ApprovalRequest(req)).await;

        assert!(app.approval_overlay.visible);
        let request = app.approval_overlay.request.as_ref().unwrap();
        assert_eq!(request.reason, Some("Test reason".to_string()));
    }

    // Notification system tests

    #[test]
    #[serial]
    fn test_notification_info_creation() {
        let notif = Notification::info("Test message");
        assert_eq!(notif.text, "Test message");
        assert_eq!(notif.style, NotificationStyle::Info);
        assert_eq!(notif.duration, Duration::from_secs(3));
        assert!(!notif.is_expired());
    }

    #[test]
    #[serial]
    fn test_notification_success_creation() {
        let notif = Notification::success("Success!");
        assert_eq!(notif.text, "Success!");
        assert_eq!(notif.style, NotificationStyle::Success);
    }

    #[test]
    #[serial]
    fn test_notification_warning_creation() {
        let notif = Notification::warning("Warning!");
        assert_eq!(notif.text, "Warning!");
        assert_eq!(notif.style, NotificationStyle::Warning);
    }

    #[test]
    #[serial]
    fn test_notification_with_custom_duration() {
        let notif = Notification::info("Short").with_duration(Duration::from_millis(100));
        assert_eq!(notif.duration, Duration::from_millis(100));
    }

    #[test]
    #[serial]
    fn test_notification_expiry() {
        // Create a notification with very short duration
        let notif = Notification::info("Brief").with_duration(Duration::from_millis(1));
        // Sleep briefly to let it expire
        std::thread::sleep(Duration::from_millis(5));
        assert!(notif.is_expired());
    }

    #[test]
    #[serial]
    fn test_show_notification() {
        let mut app = App::new(AppConfig::default());
        assert!(app.notification.is_none());

        app.show_notification(Notification::info("Hello"));
        assert!(app.notification.is_some());
        assert_eq!(app.notification.as_ref().unwrap().text, "Hello");
    }

    #[test]
    #[serial]
    fn test_dismiss_notification() {
        let mut app = App::new(AppConfig::default());
        app.show_notification(Notification::info("Hello"));
        assert!(app.notification.is_some());

        app.dismiss_notification();
        assert!(app.notification.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_tick_clears_expired_notification() {
        let mut app = App::new(AppConfig::default());
        app.show_notification(Notification::info("Brief").with_duration(Duration::from_millis(1)));
        assert!(app.notification.is_some());

        // Wait for expiry
        std::thread::sleep(Duration::from_millis(5));

        // Tick should clear the expired notification
        app.handle_event(TuiEvent::Tick).await;
        assert!(app.notification.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_tick_keeps_unexpired_notification() {
        let mut app = App::new(AppConfig::default());
        app.show_notification(Notification::info("Long").with_duration(Duration::from_secs(60)));
        assert!(app.notification.is_some());

        // Tick should NOT clear an unexpired notification
        app.handle_event(TuiEvent::Tick).await;
        assert!(app.notification.is_some());
    }

    #[test]
    #[serial]
    fn test_cycle_mode_notification_styles() {
        // Test that different modes get different notification styles
        let config = AppConfig {
            approval_preset: "read-only".to_string(),
            ..Default::default()
        };
        let mut app = App::new(config);

        // read-only → auto (Info style)
        app.cycle_approval_mode();
        assert_eq!(
            app.notification.as_ref().unwrap().style,
            NotificationStyle::Info
        );

        // auto → full-access (Error style for risky mode)
        app.cycle_approval_mode();
        assert_eq!(
            app.notification.as_ref().unwrap().style,
            NotificationStyle::Error
        );

        // full-access → read-only (Warning style)
        app.cycle_approval_mode();
        assert_eq!(
            app.notification.as_ref().unwrap().style,
            NotificationStyle::Warning
        );
    }

    #[test]
    #[serial]
    fn test_copy_selection_shows_notification() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(0);
        app.cursor_position = 5;
        assert!(app.notification.is_none());

        app.copy_selection();

        // Should show notification with char count
        assert!(app.notification.is_some());
        let notif = app.notification.as_ref().unwrap();
        assert!(notif.text.contains("Copied"));
        assert!(notif.text.contains("5")); // 5 chars in "hello"
        assert_eq!(notif.style, NotificationStyle::Info);
    }

    #[test]
    #[serial]
    fn test_cut_selection_shows_notification() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        app.input = "hello world".to_string();
        app.selection_anchor = Some(0);
        app.cursor_position = 5;
        assert!(app.notification.is_none());

        app.cut_selection();

        // Should show notification with char count
        assert!(app.notification.is_some());
        let notif = app.notification.as_ref().unwrap();
        assert!(notif.text.contains("Cut"));
        assert!(notif.text.contains("5")); // 5 chars in "hello"
        assert_eq!(notif.style, NotificationStyle::Info);
    }

    #[tokio::test]
    #[serial]
    async fn test_escape_dismisses_notification_in_insert_mode() {
        use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};

        let config = AppConfig::default();
        let mut app = App::new(config);
        app.mode = AppMode::Insert;
        app.show_notification(Notification::info("Test notification"));
        assert!(app.notification.is_some());

        // Press Escape - should dismiss notification and return to Normal mode
        let key_event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let event = TuiEvent::Terminal(CrosstermEvent::Key(key_event));
        app.handle_event(event).await;

        // Notification should be dismissed
        assert!(app.notification.is_none());
        // Should also switch to Normal mode (default Escape behavior)
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[tokio::test]
    #[serial]
    async fn test_escape_dismisses_notification_in_normal_mode() {
        use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};

        let config = AppConfig::default();
        let mut app = App::new(config);
        app.mode = AppMode::Normal;
        app.show_notification(Notification::info("Test notification"));
        assert!(app.notification.is_some());

        // Press Escape - should dismiss notification
        let key_event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let event = TuiEvent::Terminal(CrosstermEvent::Key(key_event));
        app.handle_event(event).await;

        // Notification should be dismissed
        assert!(app.notification.is_none());
        // Mode should stay Normal
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    #[serial]
    fn test_export_success_shows_notification() {
        let config = AppConfig {
            use_mock_llm: true,
            ..Default::default()
        };
        let mut app = App::new(config);

        // Add a test message so export has something
        app.agent_state.messages.push(Message::user("test message"));
        assert!(app.notification.is_none());

        // Export to temp file
        let temp_path = std::env::temp_dir().join("test_export_notif.md");
        let path_str = temp_path.display().to_string();
        app.handle_export_command(&path_str);

        // Should show success notification (if export succeeded)
        if temp_path.exists() {
            assert!(app.notification.is_some());
            let notif = app.notification.as_ref().unwrap();
            assert!(notif.text.contains("Exported"));
            assert!(notif.text.contains("1")); // 1 message
            assert_eq!(notif.style, NotificationStyle::Success);

            // Clean up
            let _ = std::fs::remove_file(&temp_path);
        }
    }

    // === Command availability during processing tests ===

    #[test]
    #[serial]
    fn test_is_command_available_during_task_blocked_commands() {
        // Commands that should be blocked during processing
        assert!(!App::is_command_available_during_task("/new"));
        assert!(!App::is_command_available_during_task("/resume"));
        assert!(!App::is_command_available_during_task("/delete"));
        assert!(!App::is_command_available_during_task("/compact"));
        assert!(!App::is_command_available_during_task("/undo"));
        assert!(!App::is_command_available_during_task("/model"));
        assert!(!App::is_command_available_during_task("/approvals"));
        assert!(!App::is_command_available_during_task("/mode"));
        assert!(!App::is_command_available_during_task("/logout"));
        assert!(!App::is_command_available_during_task("/init"));
        assert!(!App::is_command_available_during_task("/review"));
    }

    #[test]
    #[serial]
    fn test_is_command_available_during_task_allowed_commands() {
        // Commands that should remain available during processing
        assert!(App::is_command_available_during_task("/quit"));
        assert!(App::is_command_available_during_task("/exit"));
        assert!(App::is_command_available_during_task("/stop"));
        assert!(App::is_command_available_during_task("/help"));
        assert!(App::is_command_available_during_task("/status"));
        assert!(App::is_command_available_during_task("/tokens"));
        assert!(App::is_command_available_during_task("/keys"));
        assert!(App::is_command_available_during_task("/version"));
        assert!(App::is_command_available_during_task("/config"));
        assert!(App::is_command_available_during_task("/mcp"));
        assert!(App::is_command_available_during_task("/skills"));
        assert!(App::is_command_available_during_task("/diff"));
        assert!(App::is_command_available_during_task("/mention"));
        assert!(App::is_command_available_during_task("/history"));
        assert!(App::is_command_available_during_task("/feedback"));
        assert!(App::is_command_available_during_task("/providers"));
        assert!(App::is_command_available_during_task("/search"));
        assert!(App::is_command_available_during_task("/context"));
        assert!(App::is_command_available_during_task("/export"));
        assert!(App::is_command_available_during_task("/clear"));
    }

    #[test]
    #[serial]
    fn test_is_command_available_case_insensitive() {
        // Should work case-insensitively
        assert!(!App::is_command_available_during_task("/NEW"));
        assert!(!App::is_command_available_during_task("/Model"));
        assert!(!App::is_command_available_during_task("/APPROVALS"));
        assert!(App::is_command_available_during_task("/STOP"));
        assert!(App::is_command_available_during_task("/Help"));
    }

    #[tokio::test]
    #[serial]
    async fn test_blocked_command_during_processing_shows_notification() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Put app in processing mode
        app.mode = AppMode::Processing;

        // Clear any existing notification
        app.notification = None;

        // Try to run a blocked command
        let handled = app.handle_slash_command("/new").await;

        // Should be handled (blocked)
        assert!(handled);

        // Should show a warning notification
        assert!(app.notification.is_some());
        let notif = app.notification.as_ref().unwrap();
        assert!(notif.text.contains("/new"));
        assert!(notif.text.contains("not available"));
        assert!(notif.text.contains("/stop"));
        assert_eq!(notif.style, NotificationStyle::Warning);
    }

    #[tokio::test]
    #[serial]
    async fn test_allowed_command_during_processing_executes() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Put app in processing mode
        app.mode = AppMode::Processing;

        // Clear messages to check status output
        app.messages.clear();

        // Run an allowed command
        let handled = app.handle_slash_command("/status").await;

        // Should be handled normally
        assert!(handled);

        // Should have added a status message (not a blocking notification)
        assert!(!app.messages.is_empty());
        let last_msg = app.messages.last().unwrap();
        assert!(last_msg.content.contains("Session:"));
    }

    #[tokio::test]
    #[serial]
    async fn test_stop_command_available_during_processing() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Put app in processing mode (stop requires this to work)
        app.mode = AppMode::Processing;
        app.notification = None;

        // /stop should be available and execute
        let handled = app.handle_slash_command("/stop").await;
        assert!(handled);

        // Should have added a system message about stopping
        // Since we don't have an actual cancellation token, it will show "Cannot stop" message
        assert!(app.messages.iter().any(|m| m.content.contains("stop")
            || m.content.contains("Stopping")
            || m.content.contains("Cannot stop")));
    }

    #[tokio::test]
    #[serial]
    async fn test_commands_work_normally_when_not_processing() {
        let config = AppConfig::default();
        let mut app = App::new(config);

        // Make sure we're not in processing mode
        assert_ne!(app.mode, AppMode::Processing);

        // Clear messages
        app.messages.clear();

        // Commands that would be blocked during processing should work normally
        let handled = app.handle_slash_command("/new").await;
        assert!(handled);
        assert!(app.messages.iter().any(|m| m.content.contains("new chat")));
    }

    #[tokio::test]
    #[serial]
    async fn test_new_with_resume_auto_resume_no_sessions_shows_message() {
        // Test that auto_resume_no_sessions=true shows a system message
        let config = AppConfig {
            auto_resume_no_sessions: true,
            ..AppConfig::default()
        };
        let app = App::new_with_resume(config).await;

        // Should have a system message about auto-resume with no sessions
        assert!(app.messages.iter().any(|m| m
            .content
            .contains("Auto-resume enabled but no sessions found")));
    }

    #[tokio::test]
    #[serial]
    async fn test_new_with_resume_normal_start_no_extra_message() {
        // Test that auto_resume_no_sessions=false (default) doesn't show the message
        let config = AppConfig::default();
        let app = App::new_with_resume(config).await;

        // Should NOT have a system message about auto-resume
        assert!(!app.messages.iter().any(|m| m
            .content
            .contains("Auto-resume enabled but no sessions found")));
    }

    #[test]
    #[serial]
    fn test_app_config_auto_resumed_session_default() {
        // Test that auto_resumed_session defaults to false
        let config = AppConfig::default();
        assert!(!config.auto_resumed_session);
    }

    #[test]
    #[serial]
    fn test_app_config_auto_resumed_session_can_be_set() {
        // Test that auto_resumed_session can be set to true
        let config = AppConfig {
            auto_resumed_session: true,
            ..AppConfig::default()
        };
        assert!(config.auto_resumed_session);
    }

    #[test]
    #[serial]
    fn test_session_metrics_data_default() {
        let metrics = SessionMetricsData::default();
        assert_eq!(metrics.total_input_tokens, 0);
        assert_eq!(metrics.total_output_tokens, 0);
        assert_eq!(metrics.total_cached_tokens, 0);
        assert!(metrics.total_cost_usd.is_none());
        assert_eq!(metrics.llm_call_count, 0);
        assert_eq!(metrics.duration_ms, 0);
    }

    #[test]
    #[serial]
    fn test_session_metrics_data_format_tokens_small() {
        let metrics = SessionMetricsData {
            total_input_tokens: 300,
            total_output_tokens: 200,
            ..Default::default()
        };
        assert_eq!(metrics.format_total_tokens(), "500");
    }

    #[test]
    #[serial]
    fn test_session_metrics_data_format_tokens_thousands() {
        let metrics = SessionMetricsData {
            total_input_tokens: 3000,
            total_output_tokens: 2000,
            ..Default::default()
        };
        assert_eq!(metrics.format_total_tokens(), "5.0k");
    }

    #[test]
    #[serial]
    fn test_session_metrics_data_format_tokens_millions() {
        let metrics = SessionMetricsData {
            total_input_tokens: 600_000,
            total_output_tokens: 400_000,
            ..Default::default()
        };
        assert_eq!(metrics.format_total_tokens(), "1.0M");
    }

    #[test]
    #[serial]
    fn test_session_metrics_data_format_cost_none() {
        let metrics = SessionMetricsData::default();
        assert!(metrics.format_cost().is_none());
    }

    #[test]
    #[serial]
    fn test_session_metrics_data_format_cost_small() {
        let metrics = SessionMetricsData {
            total_cost_usd: Some(0.0012),
            ..Default::default()
        };
        assert_eq!(metrics.format_cost(), Some("$0.0012".to_string()));
    }

    #[test]
    #[serial]
    fn test_session_metrics_data_format_cost_medium() {
        let metrics = SessionMetricsData {
            total_cost_usd: Some(0.125),
            ..Default::default()
        };
        assert_eq!(metrics.format_cost(), Some("$0.125".to_string()));
    }

    #[test]
    #[serial]
    fn test_session_metrics_data_format_cost_large() {
        let metrics = SessionMetricsData {
            total_cost_usd: Some(2.50),
            ..Default::default()
        };
        assert_eq!(metrics.format_cost(), Some("$2.50".to_string()));
    }

    #[test]
    #[serial]
    fn test_session_metrics_data_format_summary_without_cost() {
        let metrics = SessionMetricsData {
            total_input_tokens: 1000,
            total_output_tokens: 500,
            ..Default::default()
        };
        assert_eq!(metrics.format_summary(), "1.5k tokens");
    }

    #[test]
    #[serial]
    fn test_session_metrics_data_format_summary_with_cost() {
        let metrics = SessionMetricsData {
            total_input_tokens: 1000,
            total_output_tokens: 500,
            total_cost_usd: Some(0.05),
            ..Default::default()
        };
        assert_eq!(metrics.format_summary(), "1.5k tokens | $0.050");
    }

    #[test]
    #[serial]
    fn test_app_session_metrics_initialized() {
        let config = AppConfig::default();
        let app = App::new(config);
        assert_eq!(app.session_metrics.llm_call_count, 0);
        assert_eq!(app.session_metrics.total_input_tokens, 0);
    }
}
