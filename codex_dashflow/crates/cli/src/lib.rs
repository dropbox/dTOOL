//! Codex DashFlow CLI Library
//!
//! This module provides the core CLI functionality that can be unit tested
//! separately from the binary entry point.

use std::path::PathBuf;
use std::sync::Arc;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum, ValueHint};
use clap_complete::{generate, Shell};
use codex_dashflow_core::{
    optimize::{optimize_prompts, OptimizeConfig, PromptRegistry, TrainingData, TrainingExample},
    sandbox::{SandboxExecutor, SandboxMode},
    streaming::StreamCallback,
    AgentState, ApprovalMode, Config, ConfigValidationResult, PolicyConfig, RunnerConfig,
};
use colored::Colorize;

/// Codex DashFlow - AI coding assistant powered by DashFlow orchestration
#[derive(Parser, Debug, Clone)]
#[command(name = "codex-dashflow")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Run in non-interactive mode with the given prompt
    #[arg(short, long)]
    pub exec: Option<String>,

    /// Read prompt from stdin (for multiline prompts or piping)
    /// Use with --exec to specify exec mode without inline prompt
    #[arg(long)]
    pub stdin: bool,

    /// Read prompt from a file (for multiline prompts or stored prompts)
    /// Takes precedence over --exec but not --stdin
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub prompt_file: Option<PathBuf>,

    /// Working directory for file operations
    #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
    pub working_dir: Option<String>,

    /// Maximum number of agent turns (0 = unlimited)
    #[arg(short = 't', long)]
    pub max_turns: Option<u32>,

    /// Session ID to resume. Use without argument to resume the most recent session.
    /// Example: --session abc123 (specific), or --session (most recent)
    #[arg(short = 's', long, num_args = 0..=1, default_missing_value = "latest")]
    pub session: Option<String>,

    /// Use mock LLM for testing (no API key required)
    #[arg(long)]
    pub mock: bool,

    /// LLM model to use
    #[arg(short = 'm', long)]
    pub model: Option<String>,

    /// Output tool calls and their results (verbose mode)
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Suppress all non-essential output (only print final result)
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Show resolved configuration and exit without running the agent
    #[arg(long)]
    pub dry_run: bool,

    /// Validate configuration and exit with status (0=valid, 1=warnings, 2=errors)
    /// Unlike --dry-run, this performs comprehensive validation and reports issues
    #[arg(long)]
    pub check: bool,

    /// Output in JSON format (for use with --check or --dry-run)
    #[arg(long)]
    pub json: bool,

    /// Path to config file (default: ~/.codex-dashflow/config.toml)
    #[arg(short = 'c', long, value_hint = ValueHint::FilePath)]
    pub config: Option<PathBuf>,

    /// Enable DashFlow Streaming telemetry (requires --dashstream-bootstrap)
    #[arg(long)]
    pub dashstream: bool,

    /// Kafka bootstrap servers for DashFlow Streaming (e.g., "localhost:9092")
    #[arg(long, value_hint = ValueHint::Hostname)]
    pub dashstream_bootstrap: Option<String>,

    /// Kafka topic for DashFlow Streaming events
    #[arg(long, default_value = "codex-events")]
    pub dashstream_topic: String,

    /// Tool approval mode: never, on-first-use, on-dangerous, always
    #[arg(long, value_enum, default_value = "on-dangerous")]
    pub approval_mode: CliApprovalMode,

    /// Collect training data from successful runs for prompt optimization
    #[arg(long)]
    pub collect_training: bool,

    /// Load optimized prompts from PromptRegistry for agent execution
    #[arg(long)]
    pub load_optimized_prompts: bool,

    /// Custom system prompt for the agent (overrides default and optimized prompts)
    #[arg(long)]
    pub system_prompt: Option<String>,

    /// Path to file containing system prompt (overrides default, but --system-prompt takes precedence)
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub system_prompt_file: Option<PathBuf>,

    /// Sandbox mode for command execution
    /// Values: read-only, workspace-write, danger-full-access
    /// (Config file aliases: readonly/read_only, workspacewrite/workspace_write, full-access/full_access/fullaccess)
    #[arg(short = 'S', long, value_enum)]
    pub sandbox: Option<CliSandboxMode>,

    /// PostgreSQL connection string for session checkpointing (enables persistent sessions)
    /// Format: host=localhost user=postgres password=secret dbname=codex
    /// Note: Requires the `postgres` feature to be enabled
    #[arg(long, value_hint = ValueHint::Other)]
    pub postgres: Option<String>,

    /// Path for file-based session checkpointing
    /// Enables state persistence to disk for resumable sessions
    /// Overrides config file's dashflow.checkpoint_path
    #[arg(long, value_hint = ValueHint::DirPath)]
    pub checkpoint_path: Option<PathBuf>,

    /// Enable AI introspection (graph manifest in system prompt)
    /// Overrides config file's dashflow.introspection_enabled
    /// Use --no-introspection to disable
    #[arg(long, overrides_with = "no_introspection")]
    pub introspection: bool,

    /// Disable AI introspection (no graph manifest in system prompt)
    #[arg(long, overrides_with = "introspection")]
    pub no_introspection: bool,

    /// Enable auto-resume of most recent session on startup
    /// Overrides config file's dashflow.auto_resume
    /// Use --no-auto-resume to disable
    #[arg(long, overrides_with = "no_auto_resume")]
    pub auto_resume: bool,

    /// Disable auto-resume of most recent session on startup
    #[arg(long, overrides_with = "auto_resume")]
    pub no_auto_resume: bool,

    /// Maximum age in seconds for auto-resume sessions
    /// Sessions older than this are skipped during auto-resume.
    /// Overrides config file's dashflow.auto_resume_max_age_secs
    /// Example: 86400 = 24 hours, 604800 = 7 days
    #[arg(long, value_name = "SECONDS")]
    pub auto_resume_max_age: Option<u64>,

    /// Subcommand to run
    #[command(subcommand)]
    pub command: Option<Command>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            exec: None,
            stdin: false,
            prompt_file: None,
            working_dir: None,
            max_turns: None,
            session: None,
            mock: false,
            model: None,
            verbose: false,
            quiet: false,
            dry_run: false,
            check: false,
            json: false,
            config: None,
            dashstream: false,
            dashstream_bootstrap: None,
            dashstream_topic: "codex-events".to_string(),
            approval_mode: CliApprovalMode::OnDangerous,
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox: None,
            postgres: None,
            checkpoint_path: None,
            introspection: false,
            no_introspection: false,
            auto_resume: false,
            no_auto_resume: false,
            auto_resume_max_age: None,
            command: None,
        }
    }
}

/// CLI subcommands
#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Optimize prompts using collected training data
    Optimize(OptimizeArgs),
    /// Run as an MCP server (exposes codex as a tool for other MCP clients)
    McpServer(McpServerArgs),
    /// Generate shell completions for the CLI
    Completions(CompletionsArgs),
    /// Show detailed version information
    Version(VersionArgs),
    /// Check system setup and configuration
    Doctor(DoctorArgs),
    /// Initialize configuration file with defaults
    Init(InitArgs),
    /// Sign in with your ChatGPT account or API key
    Login(LoginArgs),
    /// Sign out and clear stored credentials
    Logout(LogoutArgs),
    /// Display agent introspection data (graph structure, capabilities)
    #[command(visible_alias = "architecture")]
    Introspect(IntrospectArgs),
    /// List saved sessions (checkpoints) that can be resumed
    Sessions(SessionsArgs),
    /// Display DashFlow platform capabilities
    Capabilities(CapabilitiesArgs),
    /// List compiled feature flags and their status
    Features(FeaturesArgs),
}

/// Arguments for the introspect subcommand
#[derive(Parser, Debug, Clone, Default)]
pub struct IntrospectArgs {
    /// Output format: json (default), mermaid, or text
    #[arg(short, long, default_value = "json")]
    pub format: IntrospectFormat,

    /// Include platform registry information
    #[arg(long)]
    pub platform: bool,

    /// Show only specific section: graph, nodes, edges, tools
    #[arg(long)]
    pub section: Option<String>,

    /// Show brief summary (node flow, entry/exit, capabilities)
    #[arg(short, long)]
    pub brief: bool,
}

/// Output format for introspection
#[derive(Debug, Clone, Default, clap::ValueEnum)]
pub enum IntrospectFormat {
    /// JSON format (default)
    #[default]
    Json,
    /// Mermaid diagram format
    Mermaid,
    /// Human-readable text format
    Text,
}

/// Arguments for the capabilities subcommand
#[derive(Parser, Debug, Clone, Default)]
pub struct CapabilitiesArgs {
    /// Output format: text (default) or json
    #[arg(short, long, default_value = "text")]
    pub format: CapabilitiesFormat,
}

/// Output format for capabilities command
#[derive(Debug, Clone, Default, clap::ValueEnum)]
pub enum CapabilitiesFormat {
    /// Human-readable text format (default)
    #[default]
    Text,
    /// JSON format
    Json,
}

/// Arguments for the features subcommand
#[derive(Parser, Debug, Clone, Default)]
pub struct FeaturesArgs {
    /// Output format: text (default) or json
    #[arg(short, long, default_value = "text")]
    pub format: FeaturesFormat,
}

/// Output format for features command
#[derive(Debug, Clone, Default, clap::ValueEnum)]
pub enum FeaturesFormat {
    /// Human-readable text format (default)
    #[default]
    Text,
    /// JSON format
    Json,
}

/// Arguments for the version subcommand
#[derive(Parser, Debug, Clone, Default)]
pub struct VersionArgs {
    /// Show agent graph version and registration info
    #[arg(short, long)]
    pub agent: bool,

    /// Output format: text (default) or json
    #[arg(short, long, default_value = "text")]
    pub format: VersionFormat,
}

/// Output format for version command
#[derive(Debug, Clone, Default, clap::ValueEnum)]
pub enum VersionFormat {
    /// Human-readable text format (default)
    #[default]
    Text,
    /// JSON format
    Json,
}

/// Arguments for the sessions subcommand
#[derive(Parser, Debug, Clone, Default)]
pub struct SessionsArgs {
    /// Output format: table (default) or json
    #[arg(short, long, default_value = "table")]
    pub format: SessionsFormat,

    /// Path for file-based checkpointing (overrides config)
    #[arg(long, value_hint = ValueHint::DirPath)]
    pub checkpoint_path: Option<PathBuf>,

    /// Show detailed information for a specific session
    #[arg(long)]
    pub show: Option<String>,

    /// Delete a session by ID (deletes all checkpoints for the session)
    #[arg(long, conflicts_with = "delete_all")]
    pub delete: Option<String>,

    /// Delete ALL sessions (removes all checkpoints). Requires --force.
    #[arg(long, conflicts_with = "delete", requires = "force")]
    pub delete_all: bool,

    /// Force deletion without confirmation (required for --delete and --delete-all)
    #[arg(long)]
    pub force: bool,
}

/// Output format for sessions listing
#[derive(Debug, Clone, Default, clap::ValueEnum)]
pub enum SessionsFormat {
    /// Human-readable table format (default)
    #[default]
    Table,
    /// JSON format for machine parsing
    Json,
}

/// Arguments for the doctor subcommand
#[derive(Parser, Debug, Clone, Default)]
pub struct DoctorArgs {
    /// Show detailed output including timing for each check
    #[arg(short, long)]
    pub verbose: bool,

    /// Output results in JSON format for machine parsing
    #[arg(long)]
    pub json: bool,

    /// Suppress all output (exit code only: 0=ok, 1=warnings, 2=errors)
    /// Useful for scripts that only need to check if the system is healthy
    #[arg(short, long)]
    pub quiet: bool,

    /// Threshold in milliseconds for flagging slow checks (default from config or 100)
    /// Checks exceeding this threshold will be highlighted in verbose mode
    #[arg(long)]
    pub slow_threshold: Option<u64>,
}

impl DoctorArgs {
    /// Get the effective slow threshold, using config value if CLI not provided
    pub fn effective_slow_threshold(&self, config: &Config) -> u64 {
        self.slow_threshold
            .unwrap_or(config.doctor.slow_threshold_ms)
    }
}

/// Arguments for the init subcommand
#[derive(Parser, Debug, Clone, Default)]
pub struct InitArgs {
    /// Force overwrite of existing config file
    #[arg(short, long)]
    pub force: bool,

    /// Output config to stdout instead of writing to file
    #[arg(long)]
    pub stdout: bool,

    /// Output results in JSON format for machine parsing
    #[arg(long)]
    pub json: bool,

    /// Config template to use (minimal, standard, full, development)
    #[arg(short = 't', long, value_enum, default_value = "standard")]
    pub template: ConfigTemplate,

    /// Custom path for config file (default: ~/.codex-dashflow/config.toml)
    #[arg(short = 'o', long, value_hint = ValueHint::FilePath)]
    pub output: Option<PathBuf>,

    /// List available configuration templates and exit
    #[arg(long)]
    pub list_templates: bool,
}

/// Configuration template types for the init command
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum ConfigTemplate {
    /// Minimal configuration with only essential settings
    Minimal,
    /// Standard configuration with common settings (default)
    #[default]
    Standard,
    /// Full configuration with all options documented
    Full,
    /// Development configuration with debug settings enabled
    Development,
}

impl ConfigTemplate {
    /// Get all available templates
    pub fn all() -> &'static [ConfigTemplate] {
        &[
            ConfigTemplate::Minimal,
            ConfigTemplate::Standard,
            ConfigTemplate::Full,
            ConfigTemplate::Development,
        ]
    }

    /// Get the name of the template
    pub fn name(&self) -> &'static str {
        match self {
            ConfigTemplate::Minimal => "minimal",
            ConfigTemplate::Standard => "standard",
            ConfigTemplate::Full => "full",
            ConfigTemplate::Development => "development",
        }
    }

    /// Get a short description of the template
    pub fn description(&self) -> &'static str {
        match self {
            ConfigTemplate::Minimal => "Essential settings only - model, max_turns, sandbox_mode",
            ConfigTemplate::Standard => {
                "Common settings with serialized defaults and commented examples"
            }
            ConfigTemplate::Full => "Complete configuration with all options fully documented",
            ConfigTemplate::Development => {
                "Debug-friendly settings with permissive security for local dev"
            }
        }
    }

    /// Check if this is the default template
    pub fn is_default(&self) -> bool {
        matches!(self, ConfigTemplate::Standard)
    }
}

/// Arguments for the completions subcommand
#[derive(Parser, Debug, Clone)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: CliShell,
}

/// CLI-friendly enum for shell types
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum CliShell {
    /// Bash shell
    Bash,
    /// Zsh shell
    Zsh,
    /// Fish shell
    Fish,
    /// PowerShell
    #[value(name = "powershell")]
    PowerShell,
    /// Elvish shell
    Elvish,
}

impl From<CliShell> for Shell {
    fn from(cli: CliShell) -> Self {
        match cli {
            CliShell::Bash => Shell::Bash,
            CliShell::Zsh => Shell::Zsh,
            CliShell::Fish => Shell::Fish,
            CliShell::PowerShell => Shell::PowerShell,
            CliShell::Elvish => Shell::Elvish,
        }
    }
}

/// Arguments for the mcp-server subcommand
#[derive(Parser, Debug, Clone)]
pub struct McpServerArgs {
    /// Working directory for file operations
    #[arg(short = 'd', long, value_hint = ValueHint::DirPath)]
    pub working_dir: Option<String>,

    /// Sandbox mode for command execution
    /// (Aliases in config: readonly, read_only, workspacewrite, workspace_write, full-access, full_access, fullaccess)
    #[arg(short = 'S', long, value_enum, default_value = "workspace-write")]
    pub sandbox: CliSandboxMode,

    /// Use mock LLM for testing (no API key required)
    #[arg(long)]
    pub mock: bool,
}

/// Arguments for the login subcommand
#[derive(Parser, Debug, Clone, Default)]
pub struct LoginArgs {
    /// Use API key authentication instead of OAuth
    /// Prompts for API key or reads from stdin
    #[arg(long)]
    pub with_api_key: bool,

    /// Credential storage mode: file, keyring, or auto
    #[arg(long, value_enum, default_value = "auto")]
    pub store_mode: CliAuthStoreMode,

    /// Output results in JSON format
    #[arg(long)]
    pub json: bool,
}

/// Arguments for the logout subcommand
#[derive(Parser, Debug, Clone, Default)]
pub struct LogoutArgs {
    /// Output results in JSON format
    #[arg(long)]
    pub json: bool,
}

/// CLI-friendly enum for auth storage modes
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum CliAuthStoreMode {
    /// Use OS keychain when available, fall back to file
    #[default]
    Auto,
    /// Store credentials in ~/.codex-dashflow/auth.json
    File,
    /// Store credentials in OS keychain only
    Keyring,
}

impl From<CliAuthStoreMode> for codex_dashflow_core::AuthCredentialsStoreMode {
    fn from(cli: CliAuthStoreMode) -> Self {
        match cli {
            CliAuthStoreMode::Auto => codex_dashflow_core::AuthCredentialsStoreMode::Auto,
            CliAuthStoreMode::File => codex_dashflow_core::AuthCredentialsStoreMode::File,
            CliAuthStoreMode::Keyring => codex_dashflow_core::AuthCredentialsStoreMode::Keyring,
        }
    }
}

/// Arguments for the optimize subcommand
#[derive(Parser, Debug, Clone)]
pub struct OptimizeArgs {
    /// Subcommand for optimize operations
    #[command(subcommand)]
    pub action: OptimizeAction,
}

/// Optimize subcommand actions
#[derive(Subcommand, Debug, Clone)]
pub enum OptimizeAction {
    /// Run prompt optimization using training data
    Run {
        /// Maximum number of few-shot examples to generate
        #[arg(short = 'n', long, default_value = "3")]
        few_shot_count: usize,

        /// Minimum score threshold for training examples (0.0-1.0)
        #[arg(long, default_value = "0.7")]
        min_score: f64,

        /// Path to training data file (default: ~/.codex-dashflow/training.toml)
        #[arg(long, value_hint = ValueHint::FilePath)]
        training_file: Option<PathBuf>,

        /// Path to prompts file (default: ~/.codex-dashflow/prompts.toml)
        #[arg(long, value_hint = ValueHint::FilePath)]
        prompts_file: Option<PathBuf>,
    },

    /// Show statistics about collected training data
    Stats {
        /// Path to training data file (default: ~/.codex-dashflow/training.toml)
        #[arg(long, value_hint = ValueHint::FilePath)]
        training_file: Option<PathBuf>,
    },

    /// Add a training example manually
    Add {
        /// User input for the example
        #[arg(short = 'i', long)]
        input: String,

        /// Agent output for the example
        #[arg(short = 'o', long)]
        output: String,

        /// Quality score for the example (0.0-1.0)
        #[arg(short = 's', long, default_value = "0.8")]
        score: f64,

        /// Tool calls used (comma-separated)
        #[arg(long)]
        tools: Option<String>,

        /// Path to training data file (default: ~/.codex-dashflow/training.toml)
        #[arg(long, value_hint = ValueHint::FilePath)]
        training_file: Option<PathBuf>,
    },

    /// Show current optimized prompts
    Show {
        /// Path to prompts file (default: ~/.codex-dashflow/prompts.toml)
        #[arg(long, value_hint = ValueHint::FilePath)]
        prompts_file: Option<PathBuf>,
    },
}

/// CLI-friendly enum for approval mode
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum CliApprovalMode {
    /// Never ask for approval - auto-approve all non-forbidden tools
    Never,
    /// Ask for approval on first use of each tool type
    OnFirstUse,
    /// Ask for approval for dangerous commands (default)
    #[default]
    OnDangerous,
    /// Always ask for approval
    Always,
}

impl From<CliApprovalMode> for ApprovalMode {
    fn from(cli: CliApprovalMode) -> Self {
        match cli {
            CliApprovalMode::Never => ApprovalMode::Never,
            CliApprovalMode::OnFirstUse => ApprovalMode::OnFirstUse,
            CliApprovalMode::OnDangerous => ApprovalMode::OnDangerous,
            CliApprovalMode::Always => ApprovalMode::Always,
        }
    }
}

/// CLI-friendly enum for sandbox mode
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum CliSandboxMode {
    /// Read-only: read filesystem, no writes, no network
    ReadOnly,
    /// Workspace write: write within workspace, no network
    WorkspaceWrite,
    /// Full access: no restrictions (only use in isolated environments)
    #[value(name = "danger-full-access")]
    DangerFullAccess,
}

impl From<CliSandboxMode> for SandboxMode {
    fn from(cli: CliSandboxMode) -> Self {
        match cli {
            CliSandboxMode::ReadOnly => SandboxMode::ReadOnly,
            CliSandboxMode::WorkspaceWrite => SandboxMode::WorkspaceWrite,
            CliSandboxMode::DangerFullAccess => SandboxMode::DangerFullAccess,
        }
    }
}

/// Streaming callback that prints events to stdout for exec mode
pub struct ExecStreamCallback {
    verbose: bool,
}

impl ExecStreamCallback {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

#[async_trait::async_trait]
impl StreamCallback for ExecStreamCallback {
    async fn on_event(&self, event: codex_dashflow_core::streaming::AgentEvent) {
        use codex_dashflow_core::streaming::AgentEvent;

        // Audit #69: Always log ApprovalRequired events even in non-verbose mode
        // These indicate that a tool was requested but rejected (in exec mode with
        // approval modes other than Never, tools requiring approval are auto-rejected).
        // This visibility is important for debugging and audit trails.
        if let AgentEvent::ApprovalRequired { tool, reason, .. } = &event {
            let reason_str = reason
                .as_ref()
                .map(|r| format!(": {}", r))
                .unwrap_or_default();
            eprintln!(
                "[Approval] Tool '{}' requires approval{} (rejected in exec mode)",
                tool, reason_str
            );
            return;
        }

        // Audit #69: Always log errors even in non-verbose mode
        if let AgentEvent::Error { error, context, .. } = &event {
            eprintln!("[Error] {} in {}", error, context);
            return;
        }

        if !self.verbose {
            return;
        }

        match event {
            AgentEvent::ReasoningStart { .. } => {
                eprintln!("[Agent] Thinking...");
            }
            AgentEvent::ReasoningComplete { duration_ms, .. } => {
                eprintln!("[Agent] Reasoning complete ({} ms)", duration_ms);
            }
            AgentEvent::ToolCallRequested { tool, args, .. } => {
                eprintln!("[Tool] Calling: {} with args: {}", tool, args);
            }
            AgentEvent::ToolExecutionStart { tool, .. } => {
                eprintln!("[Tool] Executing: {}", tool);
            }
            AgentEvent::ToolExecutionComplete {
                tool,
                success,
                output_preview,
                ..
            } => {
                let status = if success { "✓" } else { "✗" };
                eprintln!("[Tool] {} {}: {}", status, tool, output_preview);
            }
            AgentEvent::TurnComplete { turn, .. } => {
                eprintln!("[Agent] Turn {} complete", turn);
            }
            AgentEvent::SessionComplete {
                total_turns,
                status,
                ..
            } => {
                eprintln!(
                    "[Agent] Session complete: {} turns, status: {}",
                    total_turns, status
                );
            }
            _ => {}
        }
    }
}

// ============================================================================
// Completions Subcommand Implementation
// ============================================================================

/// Execute the completions subcommand
///
/// Generates shell completion scripts that can be sourced or installed
/// in the appropriate shell configuration.
///
/// # Usage Examples
///
/// For bash (add to ~/.bashrc):
/// ```bash
/// codex-dashflow completions bash >> ~/.bashrc
/// ```
///
/// For zsh (add to ~/.zshrc):
/// ```bash
/// codex-dashflow completions zsh >> ~/.zshrc
/// ```
///
/// For fish (add to ~/.config/fish/completions/codex-dashflow.fish):
/// ```bash
/// codex-dashflow completions fish > ~/.config/fish/completions/codex-dashflow.fish
/// ```
///
/// For PowerShell (add to $PROFILE):
/// ```powershell
/// codex-dashflow completions powershell >> $PROFILE
/// ```
pub fn run_completions_command(args: &CompletionsArgs) {
    let mut cmd = Args::command();
    let shell: Shell = args.shell.into();
    generate(shell, &mut cmd, "codex-dashflow", &mut std::io::stdout());
}

// ============================================================================
// Version Subcommand Implementation
// ============================================================================

/// Build-time version information captured by build.rs
pub struct VersionInfo {
    /// Package version from Cargo.toml
    pub version: &'static str,
    /// Git commit hash (short) with optional -dirty suffix
    pub git_hash: &'static str,
    /// Git commit date (YYYY-MM-DD)
    pub git_date: &'static str,
    /// Build timestamp (UTC)
    pub build_timestamp: &'static str,
    /// Target platform (e.g., x86_64-apple-darwin)
    pub build_target: &'static str,
}

impl VersionInfo {
    /// Get the current build's version info
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            git_hash: env!("GIT_HASH"),
            git_date: env!("GIT_DATE"),
            build_timestamp: env!("BUILD_TIMESTAMP"),
            build_target: env!("BUILD_TARGET"),
        }
    }
}

impl std::fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "codex-dashflow {}", self.version)?;
        writeln!(f)?;
        writeln!(f, "Git commit:    {} ({})", self.git_hash, self.git_date)?;
        writeln!(f, "Build time:    {}", self.build_timestamp)?;
        writeln!(f, "Target:        {}", self.build_target)?;
        writeln!(f)?;
        writeln!(f, "Powered by DashFlow orchestration framework")?;
        writeln!(
            f,
            "Repository: https://github.com/dropbox/dTOOL/codex_dashflow"
        )?;
        Ok(())
    }
}

/// Execute the version subcommand, printing detailed version info
///
/// # Arguments
/// * `args` - Version command arguments
///
/// # Returns
/// Exit code (0 for success)
///
/// # Example
///
/// ```bash
/// # Show basic version info
/// codex version
///
/// # Show agent graph version info
/// codex version --agent
///
/// # JSON output with agent info
/// codex version --agent --format json
/// ```
pub fn run_version_command(args: &VersionArgs) -> i32 {
    use codex_dashflow_core::{get_graph_registry, AGENT_GRAPH_NAME, AGENT_GRAPH_VERSION};

    if args.agent {
        // Show agent graph version information
        let registry = get_graph_registry();
        let registry_guard = registry.read().expect("Failed to read graph registry");

        match args.format {
            VersionFormat::Json => {
                let agent_entry = registry_guard.get(AGENT_GRAPH_NAME);
                let output = serde_json::json!({
                    "agent": {
                        "name": AGENT_GRAPH_NAME,
                        "version": AGENT_GRAPH_VERSION,
                        "registered": agent_entry.is_some(),
                        "entry": agent_entry.as_ref().map(|e| {
                            serde_json::json!({
                                "graph_id": e.graph_id,
                                "execution_count": e.execution_count,
                                "active": e.active,
                                "metadata": {
                                    "name": e.metadata.name,
                                    "version": e.metadata.version,
                                    "description": e.metadata.description,
                                    "tags": e.metadata.tags,
                                    "author": e.metadata.author,
                                },
                                "manifest": {
                                    "node_count": e.manifest.node_count(),
                                    "edge_count": e.manifest.edge_count(),
                                    "entry_point": e.manifest.entry_point,
                                    "nodes": e.manifest.nodes.keys().collect::<Vec<_>>(),
                                }
                            })
                        })
                    },
                    "binary": {
                        "version": env!("CARGO_PKG_VERSION"),
                        "git_hash": env!("GIT_HASH"),
                        "git_date": env!("GIT_DATE"),
                        "build_timestamp": env!("BUILD_TIMESTAMP"),
                        "build_target": env!("BUILD_TARGET"),
                    }
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            }
            VersionFormat::Text => {
                println!("Codex DashFlow Agent");
                println!("====================");
                println!("Graph: {} v{}", AGENT_GRAPH_NAME, AGENT_GRAPH_VERSION);
                println!();

                if let Some(entry) = registry_guard.get(AGENT_GRAPH_NAME) {
                    println!("Registry Status: Registered");
                    println!("Active: {}", if entry.active { "yes" } else { "no" });
                    println!("Execution Count: {}", entry.execution_count);
                    println!();

                    println!("Graph Structure:");
                    println!(
                        "  Nodes: {} ({})",
                        entry.manifest.node_count(),
                        entry
                            .manifest
                            .nodes
                            .keys()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    println!("  Edges: {}", entry.manifest.edge_count());
                    println!("  Entry Point: {}", entry.manifest.entry_point);
                    println!();

                    println!("Metadata:");
                    println!("  Description: {}", entry.metadata.description);
                    println!("  Tags: {}", entry.metadata.tags.join(", "));
                    if let Some(author) = &entry.metadata.author {
                        println!("  Author: {}", author);
                    }
                } else {
                    println!("Registry Status: Not registered (build_agent_graph() not called)");
                    println!();
                    println!(
                        "Note: The agent graph is registered when build_agent_graph() is called."
                    );
                    println!("      Run 'codex introspect' to build and inspect the graph.");
                }
            }
        }
    } else {
        // Show basic version info
        match args.format {
            VersionFormat::Json => {
                let output = serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "git_hash": env!("GIT_HASH"),
                    "git_date": env!("GIT_DATE"),
                    "build_timestamp": env!("BUILD_TIMESTAMP"),
                    "build_target": env!("BUILD_TARGET"),
                    "framework": "DashFlow",
                    "repository": "https://github.com/dropbox/dTOOL/codex_dashflow"
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            }
            VersionFormat::Text => {
                print!("{}", VersionInfo::current());
            }
        }
    }

    0
}

// ============================================================================
// Init Subcommand Implementation
// ============================================================================

/// Exit codes for the init command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitExitCode {
    /// Configuration file created successfully
    Success = 0,
    /// File already exists (use --force to overwrite)
    FileExists = 1,
    /// Failed to create directory or write file
    WriteError = 2,
}

impl InitExitCode {
    /// Convert to process exit code
    pub fn code(self) -> i32 {
        self as i32
    }
}

/// Result of the init command for JSON output
#[derive(Debug, Clone, serde::Serialize)]
pub struct InitResult {
    /// Whether the operation succeeded
    pub success: bool,
    /// Path where config was written (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Human-readable message
    pub message: String,
}

/// Information about a configuration template for JSON output
#[derive(Debug, Clone, serde::Serialize)]
pub struct TemplateInfo {
    /// Template name (used with --template flag)
    pub name: String,
    /// Description of what the template provides
    pub description: String,
    /// Whether this is the default template
    pub is_default: bool,
}

/// Exit codes for the login command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginExitCode {
    /// Login succeeded
    Success = 0,
    /// Login was cancelled by user
    Cancelled = 1,
    /// Login failed due to error
    Failed = 2,
}

impl LoginExitCode {
    /// Convert to process exit code
    pub fn code(self) -> i32 {
        self as i32
    }
}

/// Exit codes for the logout command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoutExitCode {
    /// Logout succeeded
    Success = 0,
    /// Already logged out (no credentials found)
    NotLoggedIn = 1,
    /// Logout failed due to error
    Failed = 2,
}

impl LogoutExitCode {
    /// Convert to process exit code
    pub fn code(self) -> i32 {
        self as i32
    }
}

/// Result of the login command for JSON output
#[derive(Debug, Clone, serde::Serialize)]
pub struct LoginResult {
    /// Whether the login succeeded
    pub success: bool,
    /// Authentication method used (oauth or api_key)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Email (if OAuth login)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Human-readable message
    pub message: String,
}

/// Result of the logout command for JSON output
#[derive(Debug, Clone, serde::Serialize)]
pub struct LogoutResult {
    /// Whether the logout succeeded
    pub success: bool,
    /// Human-readable message
    pub message: String,
}

/// Generate a sample configuration file with explanatory comments
fn generate_sample_config(template: ConfigTemplate) -> String {
    match template {
        ConfigTemplate::Minimal => generate_minimal_config(),
        ConfigTemplate::Standard => generate_standard_config(),
        ConfigTemplate::Full => generate_full_config(),
        ConfigTemplate::Development => generate_development_config(),
    }
}

/// Generate minimal configuration with only essential settings
fn generate_minimal_config() -> String {
    let mut output = String::new();
    output.push_str("# Codex DashFlow Configuration (Minimal)\n");
    output.push_str("# ======================================\n");
    output.push_str("#\n");
    output.push_str("# Minimal configuration with essential settings only.\n");
    output.push_str("# For more options, run: codex-dashflow init --template full\n");
    output.push_str("#\n\n");

    output.push_str("# LLM model to use (default: gpt-4o)\n");
    output.push_str("model = \"gpt-4o\"\n\n");

    output.push_str("# Maximum agent turns per session (0 = unlimited)\n");
    output.push_str("max_turns = 10\n\n");

    output.push_str("# Sandbox mode: read-only, workspace-write, danger-full-access\n");
    output.push_str("sandbox_mode = \"read-only\"\n");

    output
}

/// Generate standard configuration with common settings
fn generate_standard_config() -> String {
    let config = Config::default();
    let toml_content = toml::to_string_pretty(&config)
        .unwrap_or_else(|_| "# Failed to serialize default config\n".to_string());

    // Add a header with documentation
    let mut output = String::new();
    output.push_str("# Codex DashFlow Configuration\n");
    output.push_str("# =============================\n");
    output.push_str("#\n");
    output.push_str("# This file configures the codex-dashflow CLI.\n");
    output.push_str("# See https://github.com/dropbox/dTOOL/codex_dashflow for documentation.\n");
    output.push_str("#\n");
    output.push_str("# Environment variables:\n");
    output.push_str("#   OPENAI_API_KEY  - Required for API access\n");
    output.push_str("#   OPENAI_BASE_URL - Custom API endpoint (optional)\n");
    output.push_str("#\n\n");
    output.push_str(&toml_content);

    // Add documented examples as comments
    output.push_str(
        "\n# =============================================================================\n",
    );
    output.push_str("# Additional Configuration Examples (uncomment to use)\n");
    output.push_str(
        "# =============================================================================\n\n",
    );

    output.push_str("# MCP Server Configuration\n");
    output.push_str("# [[mcp_servers]]\n");
    output.push_str("# name = \"my-mcp-server\"\n");
    output.push_str("# [mcp_servers.transport.stdio]\n");
    output.push_str("# command = \"/path/to/mcp-server\"\n");
    output.push_str("# args = [\"--config\", \"server.json\"]\n\n");

    output.push_str("# Custom Policy Rules\n");
    output.push_str("# [[policy.rules]]\n");
    output.push_str("# pattern = \"rm -rf\"\n");
    output.push_str("# decision = \"forbidden\"\n");
    output.push_str("# reason = \"Dangerous recursive delete\"\n\n");

    output.push_str("# [[policy.rules]]\n");
    output.push_str("# pattern = \"git commit\"\n");
    output.push_str("# decision = \"allow\"\n");
    output.push_str("# reason = \"Safe git operation\"\n\n");

    output
}

/// Generate full configuration with all options documented
fn generate_full_config() -> String {
    let mut output = String::new();
    output.push_str("# Codex DashFlow Configuration (Full)\n");
    output.push_str("# ====================================\n");
    output.push_str("#\n");
    output.push_str("# Complete configuration with all available options.\n");
    output.push_str("# See https://github.com/dropbox/dTOOL/codex_dashflow for documentation.\n");
    output.push_str("#\n");
    output.push_str("# Environment variables:\n");
    output.push_str("#   OPENAI_API_KEY       - Required for API access\n");
    output.push_str("#   OPENAI_BASE_URL      - Custom API endpoint (optional)\n");
    output.push_str("#   OPENAI_ORG_ID        - Organization ID (optional)\n");
    output.push_str("#   CODEX_DASHFLOW_CONFIG - Override config file path\n");
    output.push_str("#   RUST_LOG             - Logging level (e.g., debug, info)\n");
    output.push_str("#   NO_COLOR             - Disable colored output\n");
    output.push_str("#\n\n");

    // Core settings
    output.push_str(
        "# =============================================================================\n",
    );
    output.push_str("# Core Settings\n");
    output.push_str(
        "# =============================================================================\n\n",
    );

    output.push_str("# LLM model to use\n");
    output.push_str("# Common options: gpt-4o, gpt-4o-mini, gpt-4-turbo, claude-3-opus, etc.\n");
    output.push_str("model = \"gpt-4o\"\n\n");

    output.push_str("# Custom API endpoint (leave empty for default OpenAI endpoint)\n");
    output.push_str("# Use this for Azure OpenAI, local LLM servers, or API proxies\n");
    output.push_str("api_base = \"\"\n\n");

    output.push_str("# Maximum number of agent turns per session\n");
    output.push_str("# 0 = unlimited (agent continues until task complete or error)\n");
    output.push_str("max_turns = 10\n\n");

    output.push_str("# Default working directory (relative paths resolved from here)\n");
    output.push_str("# working_dir = \"/path/to/project\"\n\n");

    output.push_str("# Collect training data for prompt optimization\n");
    output.push_str("# Data stored in ~/.codex-dashflow/training.toml\n");
    output.push_str("collect_training = false\n\n");

    // Security settings
    output.push_str(
        "# =============================================================================\n",
    );
    output.push_str("# Security Settings\n");
    output.push_str(
        "# =============================================================================\n\n",
    );

    output.push_str("# Sandbox mode for command execution\n");
    output.push_str("# - read-only: Read filesystem, no writes, no network (safest)\n");
    output.push_str("# - workspace-write: Write within workspace and /tmp, no network\n");
    output.push_str("# - danger-full-access: No restrictions (use in containers only)\n");
    output.push_str("sandbox_mode = \"read-only\"\n\n");

    // Policy configuration
    output.push_str("[policy]\n");
    output.push_str("# Tool approval mode:\n");
    output.push_str("# - never: Auto-approve all non-forbidden tools\n");
    output.push_str("# - on-first-use: Ask once per tool type\n");
    output.push_str("# - on-dangerous: Ask for dangerous commands (default)\n");
    output.push_str("# - always: Ask for every tool invocation\n");
    output.push_str("approval_mode = \"on-dangerous\"\n\n");

    output.push_str("# Include built-in dangerous command patterns\n");
    output.push_str("# Disable only if you have comprehensive custom rules\n");
    output.push_str("include_dangerous_patterns = true\n\n");

    // DashFlow settings
    output.push_str(
        "# =============================================================================\n",
    );
    output.push_str("# DashFlow Orchestration Settings\n");
    output.push_str(
        "# =============================================================================\n\n",
    );

    output.push_str("[dashflow]\n");
    output.push_str("# Enable DashFlow Streaming telemetry\n");
    output.push_str("streaming_enabled = false\n\n");

    output.push_str("# Checkpointer type: memory, file, postgres\n");
    output.push_str("checkpointer = \"memory\"\n\n");

    // Doctor settings
    output.push_str(
        "# =============================================================================\n",
    );
    output.push_str("# Doctor Command Settings\n");
    output.push_str(
        "# =============================================================================\n\n",
    );

    output.push_str("[doctor]\n");
    output.push_str("# Threshold in milliseconds for flagging slow checks\n");
    output.push_str("slow_threshold_ms = 100\n\n");

    // Example MCP servers
    output.push_str(
        "# =============================================================================\n",
    );
    output.push_str("# MCP (Model Context Protocol) Servers\n");
    output.push_str(
        "# =============================================================================\n\n",
    );

    output.push_str("# Uncomment to enable MCP servers for extended capabilities\n\n");

    output.push_str("# Example: Filesystem access server\n");
    output.push_str("# [[mcp_servers]]\n");
    output.push_str("# name = \"filesystem\"\n");
    output.push_str("# [mcp_servers.transport.stdio]\n");
    output.push_str("# command = \"mcp-server-filesystem\"\n");
    output.push_str("# args = [\"/home/user/projects\"]\n\n");

    output.push_str("# Example: GitHub integration server\n");
    output.push_str("# [[mcp_servers]]\n");
    output.push_str("# name = \"github\"\n");
    output.push_str("# [mcp_servers.transport.stdio]\n");
    output.push_str("# command = \"mcp-server-github\"\n");
    output.push_str("# args = []\n");
    output.push_str("# [mcp_servers.transport.stdio.env]\n");
    output.push_str("# GITHUB_TOKEN = \"${GITHUB_TOKEN}\"\n\n");

    // Example policy rules
    output.push_str(
        "# =============================================================================\n",
    );
    output.push_str("# Custom Policy Rules\n");
    output.push_str(
        "# =============================================================================\n\n",
    );

    output.push_str("# Uncomment to add custom approval rules\n\n");

    output.push_str("# Forbid dangerous commands\n");
    output.push_str("# [[policy.rules]]\n");
    output.push_str("# pattern = \"rm -rf /\"\n");
    output.push_str("# decision = \"forbidden\"\n");
    output.push_str("# reason = \"System-wide recursive delete not allowed\"\n\n");

    output.push_str("# Auto-approve safe operations\n");
    output.push_str("# [[policy.rules]]\n");
    output.push_str("# pattern = \"git status\"\n");
    output.push_str("# decision = \"allow\"\n");
    output.push_str("# reason = \"Read-only git command\"\n\n");

    output.push_str("# [[policy.rules]]\n");
    output.push_str("# pattern = \"cargo test\"\n");
    output.push_str("# decision = \"allow\"\n");
    output.push_str("# reason = \"Running tests is safe\"\n\n");

    output.push_str("# Require approval for deployments\n");
    output.push_str("# [[policy.rules]]\n");
    output.push_str("# pattern = \"kubectl apply\"\n");
    output.push_str("# decision = \"needs_approval\"\n");
    output.push_str("# reason = \"Deployment commands require explicit approval\"\n\n");

    output
}

/// Generate development configuration with debug settings
fn generate_development_config() -> String {
    let mut output = String::new();
    output.push_str("# Codex DashFlow Configuration (Development)\n");
    output.push_str("# ==========================================\n");
    output.push_str("#\n");
    output.push_str("# Development configuration with debug settings enabled.\n");
    output.push_str("# NOT recommended for production use.\n");
    output.push_str("#\n\n");

    // Core settings with development defaults
    output.push_str("# Use mock LLM for testing (no API key required)\n");
    output.push_str("# Note: Set via --mock CLI flag; config only shows intent\n\n");

    output.push_str("# Model for development (faster, cheaper)\n");
    output.push_str("model = \"gpt-4o-mini\"\n\n");

    output.push_str("# Allow unlimited turns for debugging\n");
    output.push_str("max_turns = 0\n\n");

    output.push_str("# Collect training data for analysis\n");
    output.push_str("collect_training = true\n\n");

    // Permissive security for development
    output.push_str(
        "# =============================================================================\n",
    );
    output.push_str("# Development Security Settings (PERMISSIVE)\n");
    output.push_str(
        "# =============================================================================\n\n",
    );

    output.push_str("# Allow workspace writes for testing\n");
    output.push_str("sandbox_mode = \"workspace-write\"\n\n");

    output.push_str("[policy]\n");
    output.push_str("# Auto-approve all tools in development (faster iteration)\n");
    output.push_str("# WARNING: Do not use this setting in production!\n");
    output.push_str("approval_mode = \"never\"\n\n");

    output.push_str("# Still include dangerous patterns as a safety net\n");
    output.push_str("include_dangerous_patterns = true\n\n");

    // DashFlow with streaming enabled for debugging
    output.push_str("[dashflow]\n");
    output.push_str("# Enable streaming for debugging agent behavior\n");
    output.push_str("streaming_enabled = true\n\n");

    output.push_str("# Use memory checkpointer (no persistence between restarts)\n");
    output.push_str("checkpointer = \"memory\"\n\n");

    // Doctor settings with lower threshold for performance testing
    output.push_str("[doctor]\n");
    output.push_str("# Lower threshold to catch performance issues early\n");
    output.push_str("slow_threshold_ms = 50\n\n");

    // Example test policy rules
    output.push_str(
        "# =============================================================================\n",
    );
    output.push_str("# Development Policy Rules\n");
    output.push_str(
        "# =============================================================================\n\n",
    );

    output.push_str("# Allow all cargo commands for Rust development\n");
    output.push_str("[[policy.rules]]\n");
    output.push_str("pattern = \"cargo\"\n");
    output.push_str("decision = \"allow\"\n");
    output.push_str("reason = \"Rust development tools\"\n\n");

    output.push_str("# Allow git operations\n");
    output.push_str("[[policy.rules]]\n");
    output.push_str("pattern = \"git\"\n");
    output.push_str("decision = \"allow\"\n");
    output.push_str("reason = \"Git version control\"\n\n");

    output.push_str("# Allow npm/pnpm for JS development\n");
    output.push_str("[[policy.rules]]\n");
    output.push_str("pattern = \"npm\"\n");
    output.push_str("decision = \"allow\"\n");
    output.push_str("reason = \"Node.js package manager\"\n\n");

    output.push_str("[[policy.rules]]\n");
    output.push_str("pattern = \"pnpm\"\n");
    output.push_str("decision = \"allow\"\n");
    output.push_str("reason = \"Node.js package manager\"\n\n");

    output
}

/// Execute the init subcommand, creating a default configuration file.
///
/// Creates `~/.codex-dashflow/config.toml` with sensible defaults and
/// explanatory comments to help users configure the CLI.
///
/// Returns an exit code indicating the result:
/// - 0: Success (file created or output to stdout)
/// - 1: File already exists (use --force to overwrite)
/// - 2: Failed to create directory or write file
pub fn run_init_command(args: &InitArgs) -> InitExitCode {
    // Handle --list-templates flag first
    if args.list_templates {
        let templates: Vec<TemplateInfo> = ConfigTemplate::all()
            .iter()
            .map(|t| TemplateInfo {
                name: t.name().to_string(),
                description: t.description().to_string(),
                is_default: t.is_default(),
            })
            .collect();

        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&templates).unwrap_or_else(|e| {
                    format!(r#"{{"error": "Failed to serialize: {}"}}"#, e)
                })
            );
        } else {
            println!("Available configuration templates:\n");
            for template in &templates {
                let default_marker = if template.is_default {
                    " (default)".green().to_string()
                } else {
                    String::new()
                };
                println!("  {}{}", template.name.cyan(), default_marker);
                println!("    {}\n", template.description);
            }
            println!("Use {} to specify a template.", "--template <name>".cyan());
        }
        return InitExitCode::Success;
    }

    let config_content = generate_sample_config(args.template);

    // If --stdout, just print config content and return
    // Note: --stdout is incompatible with --json since --stdout outputs the config itself
    if args.stdout {
        println!("{}", config_content);
        return InitExitCode::Success;
    }

    // Determine output path
    let output_path = args.output.clone().unwrap_or_else(|| {
        dirs::home_dir()
            .map(|h| h.join(".codex-dashflow").join("config.toml"))
            .unwrap_or_else(|| PathBuf::from("config.toml"))
    });

    // Check if file exists
    if output_path.exists() && !args.force {
        let result = InitResult {
            success: false,
            path: Some(output_path.display().to_string()),
            message: format!(
                "Configuration file already exists: {}. Use --force to overwrite.",
                output_path.display()
            ),
        };

        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|e| format!(r#"{{"error": "Failed to serialize: {}"}}"#, e))
            );
        } else {
            eprintln!(
                "{}",
                format!(
                    "Configuration file already exists: {}",
                    output_path.display()
                )
                .yellow()
            );
            eprintln!("Use {} to overwrite.", "--force".cyan());
        }
        return InitExitCode::FileExists;
    }

    // Create parent directories if needed
    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                let result = InitResult {
                    success: false,
                    path: Some(output_path.display().to_string()),
                    message: format!("Failed to create directory {}: {}", parent.display(), e),
                };

                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&result).unwrap_or_else(|e| {
                            format!(r#"{{"error": "Failed to serialize: {}"}}"#, e)
                        })
                    );
                } else {
                    eprintln!(
                        "{}",
                        format!("Failed to create directory {}: {}", parent.display(), e).red()
                    );
                }
                return InitExitCode::WriteError;
            }
        }
    }

    // Write the config file
    match std::fs::write(&output_path, config_content) {
        Ok(()) => {
            let result = InitResult {
                success: true,
                path: Some(output_path.display().to_string()),
                message: format!("Created configuration file: {}", output_path.display()),
            };

            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_else(|e| format!(
                        r#"{{"error": "Failed to serialize: {}"}}"#,
                        e
                    ))
                );
            } else {
                println!(
                    "{}",
                    format!("Created configuration file: {}", output_path.display()).green()
                );
                println!();
                println!("Next steps:");
                println!(
                    "  1. Set your API key: {}",
                    "export OPENAI_API_KEY=sk-...".cyan()
                );
                println!(
                    "  2. Edit the config file to customize settings: {}",
                    output_path.display().to_string().cyan()
                );
                println!("  3. Verify your setup: {}", "codex-dashflow doctor".cyan());
            }
            InitExitCode::Success
        }
        Err(e) => {
            let result = InitResult {
                success: false,
                path: Some(output_path.display().to_string()),
                message: format!("Failed to write config file: {}", e),
            };

            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_else(|e| format!(
                        r#"{{"error": "Failed to serialize: {}"}}"#,
                        e
                    ))
                );
            } else {
                eprintln!("{}", format!("Failed to write config file: {}", e).red());
            }
            InitExitCode::WriteError
        }
    }
}

// ============================================================================
// Login/Logout Subcommand Implementation
// ============================================================================

/// Execute the login subcommand for user authentication.
///
/// Supports two authentication methods:
/// - API key: Direct entry of an OpenAI API key
/// - OAuth: (Future) Browser-based authentication with ChatGPT account
///
/// Returns an exit code indicating the result:
/// - 0: Login succeeded
/// - 1: Login cancelled
/// - 2: Login failed
pub fn run_login_command(args: &LoginArgs) -> LoginExitCode {
    use codex_dashflow_core::{AuthCredentialsStoreMode, AuthManager};
    use std::io::{self, Write};

    let store_mode: AuthCredentialsStoreMode = args.store_mode.into();

    let auth = match AuthManager::new(store_mode) {
        Ok(auth) => auth,
        Err(e) => {
            let result = LoginResult {
                success: false,
                method: None,
                email: None,
                message: format!("Failed to initialize auth: {}", e),
            };
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                eprintln!("{}", format!("Failed to initialize auth: {}", e).red());
            }
            return LoginExitCode::Failed;
        }
    };

    // Check if already authenticated
    match auth.is_authenticated() {
        Ok(true) => {
            if !args.json {
                println!(
                    "{}",
                    "Already authenticated. Use 'codex-dashflow logout' to sign out first."
                        .yellow()
                );
            }
            // Still treat as success - user is authenticated
            let (method, email) = match auth.get_account_info() {
                Ok(Some((_, email))) => ("oauth".to_string(), email),
                _ => match auth.get_api_key() {
                    Ok(Some(_)) => ("api_key".to_string(), None),
                    _ => ("unknown".to_string(), None),
                },
            };
            let result = LoginResult {
                success: true,
                method: Some(method),
                email,
                message: "Already authenticated".to_string(),
            };
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            }
            return LoginExitCode::Success;
        }
        Ok(false) => {}
        Err(e) => {
            let result = LoginResult {
                success: false,
                method: None,
                email: None,
                message: format!("Failed to check auth status: {}", e),
            };
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                eprintln!("{}", format!("Failed to check auth status: {}", e).red());
            }
            return LoginExitCode::Failed;
        }
    }

    if args.with_api_key {
        // API key authentication
        if !args.json {
            println!("Enter your OpenAI API key:");
            print!("> ");
            io::stdout().flush().ok();
        }

        let mut api_key = String::new();
        match io::stdin().read_line(&mut api_key) {
            Ok(_) => {
                let api_key = api_key.trim();
                if api_key.is_empty() {
                    let result = LoginResult {
                        success: false,
                        method: None,
                        email: None,
                        message: "No API key provided".to_string(),
                    };
                    if args.json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&result).unwrap_or_default()
                        );
                    } else {
                        eprintln!("{}", "No API key provided".red());
                    }
                    return LoginExitCode::Cancelled;
                }

                // Validate API key format
                if !api_key.starts_with("sk-") {
                    let result = LoginResult {
                        success: false,
                        method: None,
                        email: None,
                        message: "Invalid API key format (should start with 'sk-')".to_string(),
                    };
                    if args.json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&result).unwrap_or_default()
                        );
                    } else {
                        eprintln!(
                            "{}",
                            "Invalid API key format (should start with 'sk-')".red()
                        );
                    }
                    return LoginExitCode::Failed;
                }

                match auth.store_api_key(api_key) {
                    Ok(()) => {
                        let result = LoginResult {
                            success: true,
                            method: Some("api_key".to_string()),
                            email: None,
                            message: "Successfully stored API key".to_string(),
                        };
                        if args.json {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&result).unwrap_or_default()
                            );
                        } else {
                            println!("{}", "Successfully stored API key".green());
                        }
                        LoginExitCode::Success
                    }
                    Err(e) => {
                        let result = LoginResult {
                            success: false,
                            method: None,
                            email: None,
                            message: format!("Failed to store API key: {}", e),
                        };
                        if args.json {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&result).unwrap_or_default()
                            );
                        } else {
                            eprintln!("{}", format!("Failed to store API key: {}", e).red());
                        }
                        LoginExitCode::Failed
                    }
                }
            }
            Err(e) => {
                let result = LoginResult {
                    success: false,
                    method: None,
                    email: None,
                    message: format!("Failed to read input: {}", e),
                };
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&result).unwrap_or_default()
                    );
                } else {
                    eprintln!("{}", format!("Failed to read input: {}", e).red());
                }
                LoginExitCode::Failed
            }
        }
    } else {
        // OAuth browser flow authentication
        use codex_dashflow_core::auth::get_codex_home;
        use codex_dashflow_core::auth::oauth::{run_login_server, ServerOptions};

        let codex_home = match get_codex_home() {
            Ok(home) => home,
            Err(e) => {
                let result = LoginResult {
                    success: false,
                    method: None,
                    email: None,
                    message: format!("Failed to get codex home: {}", e),
                };
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&result).unwrap_or_default()
                    );
                } else {
                    eprintln!("{}", format!("Failed to get codex home: {}", e).red());
                }
                return LoginExitCode::Failed;
            }
        };

        let opts = ServerOptions::new(codex_home, None, store_mode);

        if !args.json {
            println!("{}", "Opening browser for authentication...".cyan());
            println!("{}", "If the browser doesn't open, visit:".dimmed());
            println!("  {}", opts.issuer.clone());
        }

        // Run the login server
        let login_server = match run_login_server(opts) {
            Ok(server) => {
                if !args.json {
                    println!(
                        "{}",
                        format!("Listening on http://localhost:{}", server.actual_port).dimmed()
                    );
                    println!("{}", "Waiting for authentication...".yellow());
                }
                server
            }
            Err(e) => {
                let result = LoginResult {
                    success: false,
                    method: None,
                    email: None,
                    message: format!("Failed to start login server: {}", e),
                };
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&result).unwrap_or_default()
                    );
                } else {
                    eprintln!("{}", format!("Failed to start login server: {}", e).red());
                }
                return LoginExitCode::Failed;
            }
        };

        // Block until login completes
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        match rt.block_on(login_server.block_until_done()) {
            Ok(()) => {
                // Reload auth to get account info
                let auth = match AuthManager::new(store_mode) {
                    Ok(auth) => auth,
                    Err(_) => {
                        let result = LoginResult {
                            success: true,
                            method: Some("oauth".to_string()),
                            email: None,
                            message: "Successfully authenticated".to_string(),
                        };
                        if args.json {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&result).unwrap_or_default()
                            );
                        } else {
                            println!("{}", "Successfully authenticated!".green());
                        }
                        return LoginExitCode::Success;
                    }
                };

                let email = match auth.get_account_info() {
                    Ok(Some((_, email))) => email,
                    _ => None,
                };

                let result = LoginResult {
                    success: true,
                    method: Some("oauth".to_string()),
                    email: email.clone(),
                    message: "Successfully authenticated with ChatGPT account".to_string(),
                };
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&result).unwrap_or_default()
                    );
                } else if let Some(email) = email {
                    println!("{}", format!("Successfully signed in as {}", email).green());
                } else {
                    println!("{}", "Successfully signed in with ChatGPT account!".green());
                }
                LoginExitCode::Success
            }
            Err(e) => {
                let message = e.to_string();
                let is_cancelled = e.kind() == std::io::ErrorKind::Interrupted;
                let result = LoginResult {
                    success: false,
                    method: None,
                    email: None,
                    message: message.clone(),
                };
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&result).unwrap_or_default()
                    );
                } else if is_cancelled {
                    println!("{}", "Login cancelled".yellow());
                } else {
                    eprintln!("{}", format!("Login failed: {}", message).red());
                }
                if is_cancelled {
                    LoginExitCode::Cancelled
                } else {
                    LoginExitCode::Failed
                }
            }
        }
    }
}

/// Execute the logout subcommand to clear stored credentials.
///
/// Returns an exit code indicating the result:
/// - 0: Logout succeeded
/// - 1: Not logged in (no credentials found)
/// - 2: Logout failed
pub fn run_logout_command(args: &LogoutArgs) -> LogoutExitCode {
    use codex_dashflow_core::{AuthCredentialsStoreMode, AuthManager};

    let auth = match AuthManager::new(AuthCredentialsStoreMode::Auto) {
        Ok(auth) => auth,
        Err(e) => {
            let result = LogoutResult {
                success: false,
                message: format!("Failed to initialize auth: {}", e),
            };
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                eprintln!("{}", format!("Failed to initialize auth: {}", e).red());
            }
            return LogoutExitCode::Failed;
        }
    };

    // Check if authenticated
    match auth.is_authenticated() {
        Ok(false) => {
            let result = LogoutResult {
                success: true, // Technically successful - no credentials to clear
                message: "Not logged in".to_string(),
            };
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                println!("{}", "Not logged in".yellow());
            }
            return LogoutExitCode::NotLoggedIn;
        }
        Ok(true) => {}
        Err(e) => {
            let result = LogoutResult {
                success: false,
                message: format!("Failed to check auth status: {}", e),
            };
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                eprintln!("{}", format!("Failed to check auth status: {}", e).red());
            }
            return LogoutExitCode::Failed;
        }
    }

    match auth.logout() {
        Ok(true) => {
            let result = LogoutResult {
                success: true,
                message: "Successfully logged out".to_string(),
            };
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                println!("{}", "Successfully logged out".green());
            }
            LogoutExitCode::Success
        }
        Ok(false) => {
            let result = LogoutResult {
                success: true,
                message: "No credentials found".to_string(),
            };
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                println!("{}", "No credentials found".yellow());
            }
            LogoutExitCode::NotLoggedIn
        }
        Err(e) => {
            let result = LogoutResult {
                success: false,
                message: format!("Failed to clear credentials: {}", e),
            };
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                eprintln!("{}", format!("Failed to clear credentials: {}", e).red());
            }
            LogoutExitCode::Failed
        }
    }
}

// ============================================================================
// Doctor Subcommand Implementation
// ============================================================================

/// Result of a single doctor check
#[derive(Debug, Clone, serde::Serialize)]
pub struct DoctorCheckResult {
    pub name: &'static str,
    pub status: DoctorCheckStatus,
    pub message: String,
}

/// Status of a doctor check
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorCheckStatus {
    /// Check passed
    Ok,
    /// Check passed with a warning
    Warn,
    /// Check failed
    Error,
}

impl DoctorCheckResult {
    fn ok(name: &'static str, message: impl Into<String>) -> Self {
        Self {
            name,
            status: DoctorCheckStatus::Ok,
            message: message.into(),
        }
    }

    fn warn(name: &'static str, message: impl Into<String>) -> Self {
        Self {
            name,
            status: DoctorCheckStatus::Warn,
            message: message.into(),
        }
    }

    fn error(name: &'static str, message: impl Into<String>) -> Self {
        Self {
            name,
            status: DoctorCheckStatus::Error,
            message: message.into(),
        }
    }
}

/// Result of a single doctor check with optional timing
#[derive(Debug, Clone, serde::Serialize)]
pub struct TimedDoctorCheckResult {
    #[serde(flatten)]
    pub result: DoctorCheckResult,
    pub duration_us: u64,
    /// Whether this check exceeded the slow threshold
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slow: Option<bool>,
}

impl TimedDoctorCheckResult {
    fn new(result: DoctorCheckResult, duration: std::time::Duration) -> Self {
        Self {
            result,
            duration_us: duration.as_micros() as u64,
            slow: None,
        }
    }

    /// Mark check as slow or not slow based on threshold
    fn with_slow_threshold(mut self, threshold_ms: u64) -> Self {
        let duration_ms = self.duration_us / 1000;
        self.slow = Some(duration_ms >= threshold_ms);
        self
    }
}

/// JSON output format for doctor command
#[derive(Debug, Clone, serde::Serialize)]
pub struct DoctorJsonOutput {
    /// Version of codex-dashflow
    pub version: String,
    /// Total number of checks
    pub total_checks: usize,
    /// Number of errors
    pub errors: usize,
    /// Number of warnings
    pub warnings: usize,
    /// Number of slow checks (exceeded threshold)
    pub slow_checks: usize,
    /// Slow check threshold in milliseconds
    pub slow_threshold_ms: u64,
    /// Total time in microseconds
    pub total_duration_us: u64,
    /// Individual check results
    pub checks: Vec<TimedDoctorCheckResult>,
    /// Overall status: "ok", "warnings", or "errors"
    pub overall_status: String,
    /// Configuration summary (Audit #24: collect_training flag visibility)
    pub config_summary: DoctorConfigSummary,
}

/// Configuration summary for doctor output (Audit #24)
#[derive(Debug, Clone, serde::Serialize)]
pub struct DoctorConfigSummary {
    /// Whether training data collection is enabled
    pub collect_training: bool,
    /// Configured model
    pub model: String,
    /// Configured sandbox mode (if set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_mode: Option<String>,
    /// Whether DashStream streaming is configured
    pub streaming_enabled: bool,
    /// Whether checkpointing is enabled
    pub checkpointing_enabled: bool,
    /// Whether AI introspection is enabled (graph manifest in system prompt)
    pub introspection_enabled: bool,
    /// Whether auto-resume of most recent session is enabled
    pub auto_resume_enabled: bool,
    /// Maximum age in seconds for auto-resume sessions (None = no limit)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_resume_max_age_secs: Option<u64>,
}

/// Exit codes for the doctor command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorExitCode {
    /// All checks passed
    Ok = 0,
    /// Some checks had warnings but no errors
    Warnings = 1,
    /// Some checks had errors
    Errors = 2,
}

impl DoctorExitCode {
    /// Convert to process exit code
    pub fn code(self) -> i32 {
        self as i32
    }
}

// ============================================================================
// Introspect Command Implementation
// ============================================================================

/// Execute the introspect subcommand, displaying agent graph structure.
///
/// This enables AI self-awareness by exposing the agent's:
/// - Graph structure (nodes, edges, routing)
/// - Available tools and capabilities
/// - Platform information (optional)
pub fn run_introspect_command(args: &IntrospectArgs) -> i32 {
    use codex_dashflow_core::{build_agent_graph_manifest, get_agent_graph_mermaid};

    let manifest = build_agent_graph_manifest();

    // Handle specific section requests
    if let Some(ref section) = args.section {
        match section.as_str() {
            "nodes" => {
                let nodes_json = serde_json::to_string_pretty(&manifest.nodes).unwrap();
                println!("{}", nodes_json);
                return 0;
            }
            "edges" => {
                let edges_json = serde_json::to_string_pretty(&manifest.edges).unwrap();
                println!("{}", edges_json);
                return 0;
            }
            "tools" => {
                // Extract tools from reasoning node
                if let Some(reasoning) = manifest.nodes.get("reasoning") {
                    let tools_json =
                        serde_json::to_string_pretty(&reasoning.tools_available).unwrap();
                    println!("{}", tools_json);
                } else {
                    println!("[]");
                }
                return 0;
            }
            "graph" => {
                // Just graph metadata
                let graph_info = serde_json::json!({
                    "graph_id": manifest.graph_id,
                    "graph_name": manifest.graph_name,
                    "entry_point": manifest.entry_point,
                    "node_count": manifest.nodes.len(),
                    "edge_count": manifest.edges.values().map(|v| v.len()).sum::<usize>(),
                });
                println!("{}", serde_json::to_string_pretty(&graph_info).unwrap());
                return 0;
            }
            "registry" => {
                // Graph registry information for AI self-awareness
                use codex_dashflow_core::{
                    build_agent_graph, get_graph_registry, AGENT_GRAPH_NAME, AGENT_GRAPH_VERSION,
                };
                // Ensure graph is registered by building it
                let _ = build_agent_graph();
                let registry = get_graph_registry();
                let guard = registry.read().expect("Registry lock");
                let graphs: Vec<serde_json::Value> = guard
                    .list_graphs()
                    .iter()
                    .map(|entry| {
                        serde_json::json!({
                            "graph_id": entry.graph_id,
                            "name": entry.metadata.name,
                            "version": entry.metadata.version,
                            "description": entry.metadata.description,
                            "tags": entry.metadata.tags,
                            "author": entry.metadata.author,
                            "active": entry.active,
                            "execution_count": entry.execution_count,
                        })
                    })
                    .collect();
                let registry_info = serde_json::json!({
                    "graph_count": graphs.len(),
                    "current_version": AGENT_GRAPH_VERSION,
                    "current_graph": AGENT_GRAPH_NAME,
                    "graphs": graphs,
                });
                println!("{}", serde_json::to_string_pretty(&registry_info).unwrap());
                return 0;
            }
            "metrics" => {
                // Metrics configuration and capabilities information
                // DashFlow collects metrics by default on all graph executions
                let metrics_info = serde_json::json!({
                    "enabled_by_default": true,
                    "description": "DashFlow automatically collects execution metrics for all graph runs",
                    "collected_metrics": {
                        "node_durations": "Duration spent in each graph node",
                        "node_execution_counts": "Number of times each node was executed",
                        "total_duration": "Total wall-clock execution time",
                        "checkpoint_count": "Number of checkpoints saved during execution",
                        "checkpoint_loads": "Number of checkpoints loaded",
                        "edges_traversed": "Number of graph edges traversed",
                        "conditional_branches": "Number of conditional branch evaluations",
                        "parallel_executions": "Number of parallel node executions",
                        "peak_concurrency": "Maximum concurrent nodes during execution",
                        "state_size_bytes": "Size of serialized state (if available)"
                    },
                    "access": {
                        "via_agent_result": "AgentResult.execution_metrics contains metrics after run_agent()",
                        "pretty_print": "Use ExecutionMetrics::to_string_pretty() for human-readable output"
                    },
                    "notes": [
                        "Metrics are collected automatically - no configuration required",
                        "Use compile_without_metrics() to disable if needed (not recommended)",
                        "Metrics help identify performance bottlenecks in agent workflows"
                    ]
                });
                println!("{}", serde_json::to_string_pretty(&metrics_info).unwrap());
                return 0;
            }
            "quality" => {
                // Quality assurance and validation capabilities
                // Documents DashFlow's QualityGate and related types
                let quality_info = serde_json::json!({
                    "description": "DashFlow provides quality gates for validating LLM outputs",
                    "types": {
                        "QualityGate": {
                            "description": "Main quality gate for validating and retrying LLM responses",
                            "fields": {
                                "config": "QualityGateConfig with threshold and retry settings"
                            },
                            "methods": {
                                "new": "Create from QualityGateConfig",
                                "try_new": "Fallible construction",
                                "check": "Validate a response against quality criteria",
                                "check_with_retry": "Validate with automatic retries"
                            }
                        },
                        "QualityGateConfig": {
                            "description": "Configuration for quality gate behavior",
                            "fields": {
                                "threshold": "Minimum quality score (0.0-1.0)",
                                "max_retries": "Maximum retry attempts",
                                "retry_strategy": "Strategy for retrying (Fixed, Exponential)",
                                "emit_telemetry": "Whether to emit streaming events"
                            }
                        },
                        "QualityScore": {
                            "description": "Represents a quality score with value and confidence",
                            "fields": {
                                "value": "Quality score (0.0-1.0)",
                                "confidence": "Confidence in the score (0.0-1.0)",
                                "feedback": "Optional feedback message"
                            }
                        },
                        "ResponseValidator": {
                            "description": "Trait for validating LLM responses",
                            "methods": {
                                "validate": "Validate a response and return ValidationResult"
                            }
                        },
                        "ToolResultValidator": {
                            "description": "Validate tool execution results",
                            "fields": {
                                "config": "ToolValidatorConfig with validation rules"
                            }
                        }
                    },
                    "llm_as_judge": {
                        "description": "Optional LLM-as-judge quality scoring (requires llm-judge feature)",
                        "feature_flag": "llm-judge",
                        "types": {
                            "MultiDimensionalJudge": {
                                "description": "LLM-based quality judge from dashflow-evals",
                                "dimensions": [
                                    "accuracy (25% weight) - Factual correctness",
                                    "relevance (25% weight) - How well response addresses query",
                                    "completeness (20% weight) - Coverage of necessary aspects",
                                    "safety (15% weight) - Absence of harmful content",
                                    "coherence (10% weight) - Logical flow and readability",
                                    "conciseness (5% weight) - Efficiency without verbosity"
                                ]
                            }
                        },
                        "configuration": {
                            "with_llm_judge": "Enable LLM-as-judge with specific model",
                            "with_default_llm_judge": "Enable LLM-as-judge with gpt-4o-mini",
                            "llm_judge_model": "Configure judge model (default: gpt-4o-mini)"
                        },
                        "usage": "state.with_quality_gate(config).with_llm_judge(\"gpt-4o\")"
                    },
                    "usage": {
                        "heuristic_scoring": "let state = AgentState::new().with_quality_gate(QualityGateConfig { threshold: 0.8, max_retries: 3, ..Default::default() })",
                        "llm_judge_scoring": "let state = AgentState::new().with_quality_gate(config).with_llm_judge(\"gpt-4o\")",
                        "default_llm_judge": "let state = AgentState::new().with_quality_gate(config).with_default_llm_judge()"
                    },
                    "exports": [
                        "QualityGate",
                        "QualityGateConfig",
                        "QualityGateResult",
                        "QualityScore",
                        "ResponseValidator",
                        "RetryStrategy",
                        "ToolResultValidator",
                        "ToolValidationAction",
                        "ToolValidationResult",
                        "ToolValidatorConfig",
                        "ValidationAction",
                        "ValidationResult"
                    ],
                    "notes": [
                        "Use QualityGate to ensure LLM outputs meet quality standards",
                        "RetryStrategy supports Fixed and Exponential backoff",
                        "ToolResultValidator helps validate tool execution results",
                        "LLM-as-judge provides more accurate quality scoring at cost of additional API calls",
                        "Enable llm-judge feature: cargo build --features llm-judge",
                        "All types are re-exported from codex_dashflow_core"
                    ]
                });
                println!("{}", serde_json::to_string_pretty(&quality_info).unwrap());
                return 0;
            }
            "templates" => {
                // Graph template capabilities
                // Documents DashFlow's GraphTemplate and builder patterns
                let templates_info = serde_json::json!({
                    "description": "DashFlow provides pre-built graph templates for common agent patterns",
                    "types": {
                        "GraphTemplate": {
                            "description": "Base template type for building standardized agent graphs",
                            "methods": {
                                "build": "Build a CompiledGraph from the template"
                            }
                        },
                        "MapReduceBuilder": {
                            "description": "Template for map-reduce parallel processing patterns",
                            "usage": "Split a task across multiple workers, then combine results",
                            "methods": {
                                "new": "Create a new MapReduce builder",
                                "with_mapper": "Set the map function",
                                "with_reducer": "Set the reduce function",
                                "with_workers": "Set the number of parallel workers",
                                "build": "Build the graph"
                            }
                        },
                        "SupervisorBuilder": {
                            "description": "Template for supervisor-worker patterns",
                            "usage": "Create a supervisor that delegates to specialized worker agents",
                            "methods": {
                                "new": "Create a new Supervisor builder",
                                "with_supervisor": "Set the supervisor node",
                                "add_worker": "Add a worker agent",
                                "with_routing": "Configure task routing logic",
                                "build": "Build the graph"
                            }
                        }
                    },
                    "patterns": {
                        "map_reduce": {
                            "description": "Parallel processing with aggregation",
                            "flow": "Input → Map (parallel) → Reduce → Output",
                            "use_cases": ["Batch document processing", "Parallel API calls", "Data transformation"]
                        },
                        "supervisor": {
                            "description": "Hierarchical task delegation",
                            "flow": "Task → Supervisor → Worker Selection → Worker Execution → Result",
                            "use_cases": ["Multi-agent coordination", "Task routing", "Specialized agents"]
                        }
                    },
                    "exports": [
                        "GraphTemplate",
                        "MapReduceBuilder",
                        "SupervisorBuilder"
                    ],
                    "notes": [
                        "Templates provide proven patterns for common agent architectures",
                        "MapReduceBuilder is ideal for embarrassingly parallel tasks",
                        "SupervisorBuilder enables hierarchical multi-agent systems",
                        "All templates are re-exported from codex_dashflow_core"
                    ]
                });
                println!("{}", serde_json::to_string_pretty(&templates_info).unwrap());
                return 0;
            }
            "performance" => {
                // Performance monitoring and analysis capabilities
                // Documents DashFlow's PerformanceMetrics and related types
                let performance_info = serde_json::json!({
                    "description": "DashFlow provides rich performance monitoring for AI agent workflows",
                    "types": {
                        "PerformanceMetrics": {
                            "description": "Snapshot of current performance metrics",
                            "fields": {
                                "current_latency_ms": "Current operation latency in milliseconds",
                                "average_latency_ms": "Average latency over recent operations",
                                "p95_latency_ms": "95th percentile latency",
                                "p99_latency_ms": "99th percentile latency",
                                "tokens_per_second": "Token processing throughput",
                                "error_rate": "Current error rate (0.0 to 1.0)",
                                "memory_usage_mb": "Memory usage in megabytes",
                                "cpu_usage_percent": "CPU usage percentage (0.0 to 100.0)",
                                "sample_count": "Number of operations in sample window",
                                "sample_window_secs": "Sample window duration in seconds"
                            }
                        },
                        "ResourceUsage": {
                            "description": "Track resource consumption during execution",
                            "fields": {
                                "tokens_used": "Total tokens consumed",
                                "api_calls": "Number of API calls made",
                                "cost_usd": "Estimated cost in USD",
                                "memory_peak_mb": "Peak memory usage",
                                "execution_time_ms": "Total execution time"
                            }
                        },
                        "BottleneckAnalysis": {
                            "description": "Identify performance bottlenecks in agent workflows",
                            "fields": {
                                "bottlenecks": "List of detected bottlenecks with severity",
                                "recommendations": "Suggested optimizations",
                                "affected_nodes": "Nodes impacted by bottlenecks"
                            }
                        }
                    },
                    "usage": {
                        "builder_pattern": "PerformanceMetrics::builder().current_latency_ms(123.0).build()",
                        "from_execution_metrics": "Compute from ExecutionMetrics after run_agent()",
                        "resource_tracking": "ResourceUsage::builder().tokens_used(1000).cost_usd(0.01).build()"
                    },
                    "exports": [
                        "PerformanceMetrics",
                        "PerformanceMetricsBuilder",
                        "PerformanceThresholds",
                        "ResourceUsage",
                        "ResourceUsageBuilder",
                        "ResourceUsageHistory",
                        "Bottleneck",
                        "BottleneckAnalysis",
                        "BottleneckBuilder",
                        "BottleneckMetric",
                        "BottleneckSeverity",
                        "BottleneckThresholds"
                    ],
                    "notes": [
                        "All performance types are re-exported from codex_dashflow_core",
                        "Use PerformanceMetrics to track latency, throughput, and resource usage",
                        "BottleneckAnalysis helps identify slow nodes in agent workflows",
                        "ResourceUsage tracks API costs and token consumption"
                    ]
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&performance_info).unwrap()
                );
                return 0;
            }
            "scheduler" => {
                // Scheduler configuration and work-stealing capabilities
                // Documents DashFlow's WorkStealingScheduler and related types
                let scheduler_info = serde_json::json!({
                    "description": "DashFlow provides a work-stealing scheduler for parallel node execution",
                    "types": {
                        "WorkStealingScheduler": {
                            "description": "High-performance scheduler using work-stealing for load balancing",
                            "methods": {
                                "new": "Create with SchedulerConfig",
                                "spawn": "Submit work items for execution",
                                "run_until_complete": "Execute until all work is done"
                            }
                        },
                        "SchedulerConfig": {
                            "description": "Configuration for the work-stealing scheduler",
                            "fields": {
                                "max_workers": "Maximum number of worker threads",
                                "queue_capacity": "Work queue capacity per worker",
                                "selection_strategy": "Strategy for selecting work items",
                                "steal_batch_size": "Number of items to steal at once"
                            }
                        },
                        "SchedulerMetrics": {
                            "description": "Runtime metrics from the scheduler",
                            "fields": {
                                "tasks_completed": "Number of completed tasks",
                                "tasks_stolen": "Number of tasks stolen between workers",
                                "queue_depth": "Current queue depth",
                                "active_workers": "Currently active workers"
                            }
                        },
                        "SelectionStrategy": {
                            "description": "Strategy for selecting the next work item",
                            "variants": {
                                "Fifo": "First-in-first-out ordering",
                                "Lifo": "Last-in-first-out ordering (better cache locality)",
                                "Priority": "Priority-based selection"
                            }
                        }
                    },
                    "usage": {
                        "basic": "let scheduler = WorkStealingScheduler::new(SchedulerConfig::default())",
                        "custom": "let config = SchedulerConfig { max_workers: 4, selection_strategy: SelectionStrategy::Lifo, ..Default::default() }"
                    },
                    "exports": [
                        "WorkStealingScheduler",
                        "SchedulerConfig",
                        "SchedulerMetrics",
                        "SelectionStrategy"
                    ],
                    "notes": [
                        "Work-stealing provides automatic load balancing across workers",
                        "LIFO strategy often provides better cache locality",
                        "Used internally by DashFlow for parallel edge execution",
                        "All types are re-exported from codex_dashflow_core"
                    ]
                });
                println!("{}", serde_json::to_string_pretty(&scheduler_info).unwrap());
                return 0;
            }
            "retention" => {
                // Retention policy configuration for checkpoints
                // Documents DashFlow's RetentionPolicy and related types
                let retention_info = serde_json::json!({
                    "description": "DashFlow provides configurable retention policies for checkpoint management",
                    "types": {
                        "RetentionPolicy": {
                            "description": "Policy for managing checkpoint lifecycle and cleanup",
                            "fields": {
                                "max_checkpoints": "Maximum checkpoints to retain per thread",
                                "max_age_secs": "Maximum age in seconds before cleanup",
                                "preserve_latest": "Always preserve the most recent checkpoint",
                                "preserve_named": "Preserve explicitly named checkpoints"
                            }
                        },
                        "RetentionPolicyBuilder": {
                            "description": "Builder for creating RetentionPolicy instances",
                            "methods": {
                                "new": "Create a new builder",
                                "max_checkpoints": "Set maximum checkpoint count",
                                "max_age_secs": "Set maximum age",
                                "preserve_latest": "Enable/disable latest preservation",
                                "preserve_named": "Enable/disable named preservation",
                                "build": "Build the policy"
                            }
                        }
                    },
                    "usage": {
                        "builder": "RetentionPolicyBuilder::new().max_checkpoints(10).max_age_secs(3600).preserve_latest(true).build()",
                        "default": "RetentionPolicy::default() // Sensible defaults"
                    },
                    "exports": [
                        "RetentionPolicy",
                        "RetentionPolicyBuilder"
                    ],
                    "best_practices": [
                        "Use max_checkpoints to limit disk/memory usage",
                        "Set max_age_secs to auto-cleanup old sessions",
                        "Enable preserve_latest to always allow session resume",
                        "Use preserve_named for important checkpoint milestones"
                    ],
                    "notes": [
                        "Retention policies prevent unbounded checkpoint growth",
                        "Policies are applied during checkpoint save operations",
                        "All types are re-exported from codex_dashflow_core"
                    ]
                });
                println!("{}", serde_json::to_string_pretty(&retention_info).unwrap());
                return 0;
            }
            "dashoptimize" => {
                // DashOptimize: cost monitoring, A/B testing, and model distillation
                // Documents DashFlow's optimization capabilities
                let dashoptimize_info = serde_json::json!({
                    "description": "DashFlow's DashOptimize module provides cost monitoring, A/B testing, and model distillation",
                    "modules": {
                        "cost_monitoring": {
                            "description": "Track and control LLM API costs",
                            "types": {
                                "CostMonitor": "Track token usage and costs across requests",
                                "CostReport": "Generate cost reports by model, time period, or tenant",
                                "BudgetEnforcer": "Enforce cost limits with configurable alerts",
                                "BudgetConfig": "Configure budget limits and alert thresholds",
                                "AlertLevel": "Warning, Critical, or Blocking alert levels",
                                "ModelPricing": "Price database for LLM models",
                                "ModelPrice": "Per-model pricing configuration",
                                "TokenUsage": "Track input/output token counts",
                                "UsageRecord": "Individual usage record with timestamp and metadata"
                            },
                            "usage": {
                                "track_costs": "let monitor = CostMonitor::new(ModelPricing::default())",
                                "record_usage": "monitor.record(model, input_tokens, output_tokens)",
                                "get_report": "monitor.report_by_model()",
                                "enforce_budget": "let enforcer = BudgetEnforcer::new(BudgetConfig { daily_limit_usd: 10.0, alert_at: 0.8, ... })"
                            }
                        },
                        "ab_testing": {
                            "description": "A/B test different prompts, models, or configurations",
                            "types": {
                                "ABTest": "Define and run A/B tests with multiple variants",
                                "Variant": "Individual test variant with configuration",
                                "TrafficSplitter": "Control traffic distribution across variants",
                                "StatisticalAnalysis": "Analyze test results with statistical rigor",
                                "TTestResult": "T-test results for comparing variants",
                                "ConfidenceInterval": "Confidence interval for metric estimates",
                                "ResultsReport": "Full test results report",
                                "VariantReport": "Per-variant performance report"
                            },
                            "usage": {
                                "create_test": "let test = ABTest::new(\"prompt-test\").with_variant(variant_a).with_variant(variant_b)",
                                "assign_variant": "let variant = test.assign(user_id)",
                                "record_result": "test.record(variant_id, metric_value)",
                                "analyze": "let analysis = test.analyze()"
                            }
                        },
                        "distillation": {
                            "description": "Distill knowledge from large models to smaller, cheaper ones",
                            "types": {
                                "ModelDistillation": "Orchestrate model distillation process",
                                "DistillationConfig": "Configure distillation parameters",
                                "DistillationReport": "Report on distillation results",
                                "CostAnalysis": "Analyze cost savings from distillation",
                                "QualityGap": "Measure quality gap between teacher and student",
                                "ROIMetrics": "Return on investment metrics",
                                "SyntheticDataGenerator": "Generate training data from teacher model"
                            },
                            "workflow": [
                                "1. Configure teacher model (e.g., GPT-4)",
                                "2. Generate synthetic training data",
                                "3. Fine-tune student model (e.g., GPT-3.5)",
                                "4. Evaluate quality gap",
                                "5. Calculate ROI and cost savings"
                            ]
                        }
                    },
                    "exports": [
                        "CostMonitor",
                        "CostReport",
                        "BudgetEnforcer",
                        "BudgetConfig",
                        "AlertLevel",
                        "ModelPricing",
                        "ModelPrice",
                        "TokenUsage",
                        "UsageRecord",
                        "ABTest",
                        "Variant",
                        "TrafficSplitter",
                        "StatisticalAnalysis",
                        "TTestResult",
                        "ConfidenceInterval",
                        "ResultsReport",
                        "VariantReport",
                        "ModelDistillation",
                        "DistillationConfig",
                        "DistillationReport",
                        "CostAnalysis",
                        "QualityGap",
                        "ROIMetrics",
                        "SyntheticDataGenerator"
                    ],
                    "notes": [
                        "All types are re-exported from codex_dashflow_core",
                        "Cost monitoring helps prevent unexpected API bills",
                        "A/B testing enables data-driven prompt optimization",
                        "Model distillation reduces costs while maintaining quality"
                    ]
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&dashoptimize_info).unwrap()
                );
                return 0;
            }
            "optimizers" => {
                // DashOptimize prompt optimizers (SIMBA, GEPA, BootstrapFewShot, etc.)
                // Documents DashFlow's prompt optimization algorithms
                let optimizers_info = serde_json::json!({
                    "description": "DashFlow's prompt optimization algorithms for improving LLM performance",
                    "algorithms": {
                        "SIMBA": {
                            "description": "Self-Improving Model-Based Annotation - learns from model feedback",
                            "types": ["SIMBA", "SimbaStrategy", "SimbaOutput"],
                            "use_case": "Iteratively improve prompts using model self-evaluation"
                        },
                        "GEPA": {
                            "description": "Genetic Evolution of Prompt Annotations - evolutionary prompt optimization",
                            "types": ["GEPA", "GEPAConfig", "GEPAResult"],
                            "use_case": "Evolve prompts using genetic algorithms for optimal performance"
                        },
                        "BootstrapFewShot": {
                            "description": "Bootstrap few-shot examples from model outputs",
                            "types": ["BootstrapFewShot", "CandidateProgram"],
                            "use_case": "Generate high-quality few-shot examples automatically"
                        },
                        "BootstrapOptuna": {
                            "description": "Hyperparameter optimization for prompts using Optuna",
                            "types": ["BootstrapOptuna"],
                            "use_case": "Systematic hyperparameter search for prompt parameters"
                        },
                        "KNNFewShot": {
                            "description": "K-Nearest Neighbor selection of few-shot examples",
                            "types": ["KNNFewShot"],
                            "use_case": "Dynamically select most relevant examples for each query"
                        },
                        "LabeledFewShot": {
                            "description": "Use pre-labeled examples for few-shot learning",
                            "types": ["LabeledFewShot"],
                            "use_case": "Leverage human-labeled examples for consistent quality"
                        },
                        "AutoPrompt": {
                            "description": "Automatic prompt generation and optimization",
                            "types": ["AutoPrompt", "AutoPromptBuilder"],
                            "use_case": "Generate effective prompts automatically from task description"
                        },
                        "RandomSearch": {
                            "description": "Random search over prompt variations",
                            "types": ["RandomSearch", "OptimizerConfig"],
                            "use_case": "Baseline optimization via random sampling"
                        },
                        "GraphOptimizer": {
                            "description": "Optimize entire graph workflows",
                            "types": ["GraphOptimizer", "OptimizationStrategy"],
                            "use_case": "Optimize node configurations and edge routing"
                        }
                    },
                    "exports": [
                        "SIMBA",
                        "SimbaStrategy",
                        "SimbaOutput",
                        "StrategyContext",
                        "GEPA",
                        "GEPAConfig",
                        "GEPAResult",
                        "BootstrapFewShot",
                        "BootstrapOptuna",
                        "CandidateProgram",
                        "KNNFewShot",
                        "LabeledFewShot",
                        "AutoPrompt",
                        "AutoPromptBuilder",
                        "RandomSearch",
                        "OptimizerConfig",
                        "GraphOptimizer",
                        "OptimizationStrategy"
                    ],
                    "workflow": [
                        "1. Define evaluation metric for prompt quality",
                        "2. Create training examples or use auto-generation",
                        "3. Select optimizer (SIMBA for iterative, GEPA for evolutionary)",
                        "4. Run optimization with desired iterations",
                        "5. Apply optimized prompts to production"
                    ],
                    "notes": [
                        "All types are re-exported from codex_dashflow_core",
                        "SIMBA and GEPA are the most powerful optimizers",
                        "Use BootstrapFewShot for automatic example generation",
                        "GraphOptimizer can optimize entire agent workflows"
                    ]
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&optimizers_info).unwrap()
                );
                return 0;
            }
            "evals" => {
                // DashOptimize evaluation metrics for measuring LLM output quality
                let evals_info = serde_json::json!({
                    "description": "DashFlow's evaluation metrics for measuring LLM output quality",
                    "functions": {
                        "exact_match": {
                            "description": "Check if prediction exactly matches ground truth",
                            "returns": "1.0 if exact match, 0.0 otherwise"
                        },
                        "exact_match_any": {
                            "description": "Check if prediction matches any of multiple ground truths",
                            "returns": "1.0 if matches any, 0.0 otherwise"
                        },
                        "f1_score": {
                            "description": "Calculate F1 score between prediction and ground truth",
                            "returns": "Harmonic mean of precision and recall (0.0-1.0)"
                        },
                        "precision_score": {
                            "description": "Calculate precision (correct tokens / predicted tokens)"
                        },
                        "recall_score": {
                            "description": "Calculate recall (correct tokens / ground truth tokens)"
                        },
                        "max_f1": {
                            "description": "Maximum F1 score across multiple ground truths"
                        },
                        "normalize_text": {
                            "description": "Normalize text for comparison (lowercase, whitespace, punctuation)"
                        }
                    },
                    "json_metrics": {
                        "description": "Structured JSON comparison metrics",
                        "functions": ["json_exact_match", "json_f1_score", "json_precision_score", "json_recall_score", "compute_all_json_metrics"],
                        "config": "JsonMetricConfig configures comparison behavior"
                    },
                    "semantic_similarity": {
                        "types": ["SemanticF1", "SemanticF1Config", "SemanticF1Result"],
                        "use_case": "Compare meaning rather than exact tokens using embeddings"
                    },
                    "exports": [
                        "exact_match", "exact_match_any", "f1_score", "precision_score", "recall_score",
                        "max_f1", "normalize_text", "json_exact_match", "json_f1_score",
                        "json_precision_score", "json_recall_score", "compute_all_json_metrics",
                        "JsonMetricConfig", "MetricFn", "SemanticF1", "SemanticF1Config", "SemanticF1Result"
                    ],
                    "notes": [
                        "All metrics return f64 scores typically in range 0.0-1.0",
                        "Use f1_score for balanced precision/recall evaluation",
                        "JSON metrics handle structured data comparison"
                    ]
                });
                println!("{}", serde_json::to_string_pretty(&evals_info).unwrap());
                return 0;
            }
            "datacollection" => {
                // DashOptimize data collection and analysis
                let datacollection_info = serde_json::json!({
                    "description": "DashFlow's data collection and class balancing framework",
                    "types": {
                        "DataCollector": {
                            "description": "Captures input/output pairs from production execution",
                            "methods": ["collect", "load_dataset"]
                        },
                        "DataFormat": {
                            "description": "Defines the format for collected data",
                            "factory": "DataFormat::classification(input_field, label_field)"
                        },
                        "DataStore": {
                            "description": "Storage backend for collected data",
                            "variants": ["DataStore::memory()", "DataStore::file(path)"]
                        },
                        "DataSource": {
                            "description": "Source of training data (file, database, etc.)"
                        },
                        "DistributionAnalyzer": {
                            "description": "Analyzes label distributions and detects class imbalance",
                            "config": "with_min_examples_per_class(n)"
                        },
                        "DistributionAnalysis": {
                            "description": "Results of distribution analysis",
                            "fields": ["total_examples", "imbalance_ratio", "class_counts"]
                        },
                        "DashFlowTrainingExample": {
                            "description": "Single training data point with inputs and outputs"
                        }
                    },
                    "workflow": [
                        "1. Create DataFormat for your task (classification, QA, etc.)",
                        "2. Create DataStore for persistence",
                        "3. Use DataCollector to capture production data",
                        "4. Analyze distribution with DistributionAnalyzer",
                        "5. Use collected data for optimization"
                    ],
                    "exports": [
                        "DataCollector", "DataFormat", "DataSource", "DataStore",
                        "DistributionAnalysis", "DistributionAnalyzer", "DashFlowTrainingExample"
                    ],
                    "notes": [
                        "Collect production data to create high-quality training sets",
                        "DistributionAnalyzer helps detect class imbalance before optimization",
                        "DashFlowTrainingExample aliased to avoid conflict with local TrainingExample"
                    ]
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&datacollection_info).unwrap()
                );
                return 0;
            }
            "modules" => {
                // DashOptimize optimizable node patterns
                let modules_info = serde_json::json!({
                    "description": "DashFlow's pre-built optimizable node patterns for common LLM workflows",
                    "modules": {
                        "ChainOfThoughtNode": {
                            "description": "Adds step-by-step reasoning before answers",
                            "use_case": "Complex reasoning tasks where intermediate steps improve accuracy"
                        },
                        "ReActNode": {
                            "description": "Agent loop with tool use (Reason-Act pattern)",
                            "use_case": "Tool-augmented tasks requiring multi-step reasoning",
                            "helpers": ["SimpleTool", "Tool"]
                        },
                        "AvatarNode": {
                            "description": "Advanced agent with explicit action tracking",
                            "types": ["Action", "ActionOutput", "AvatarTool"],
                            "use_case": "Complex agents with optimizable instruction following"
                        },
                        "BestOfNNode": {
                            "description": "N-times sampling with reward-based selection",
                            "types": ["RewardFn"],
                            "use_case": "Improve output quality through sampling and selection"
                        },
                        "EnsembleNode": {
                            "description": "Parallel execution with result aggregation",
                            "types": ["AggregationStrategy"],
                            "use_case": "Combine multiple models/prompts for robust outputs"
                        },
                        "MultiChainComparisonNode": {
                            "description": "Compares multiple reasoning chains and synthesizes final answer",
                            "use_case": "Self-consistency and answer verification"
                        },
                        "RefineNode": {
                            "description": "Iterative refinement with feedback",
                            "types": ["FeedbackFn", "RefineableState"],
                            "use_case": "Incrementally improve outputs based on feedback"
                        }
                    },
                    "exports": [
                        "ChainOfThoughtNode", "ReActNode", "SimpleTool", "Tool",
                        "AvatarNode", "AvatarTool", "Action", "ActionOutput",
                        "BestOfNNode", "RewardFn",
                        "EnsembleNode", "AggregationStrategy",
                        "MultiChainComparisonNode",
                        "RefineNode", "FeedbackFn", "RefineableState"
                    ],
                    "notes": [
                        "All modules can be optimized with BootstrapFewShot and other optimizers",
                        "Modules are composable - combine them for complex workflows",
                        "ReActNode is the standard agent pattern with tool calling"
                    ]
                });
                println!("{}", serde_json::to_string_pretty(&modules_info).unwrap());
                return 0;
            }
            "multiobjective" => {
                // DashOptimize multi-objective optimization
                let multiobjective_info = serde_json::json!({
                    "description": "DashFlow's multi-objective optimization for balancing quality, cost, and latency",
                    "types": {
                        "MultiObjectiveOptimizer": {
                            "description": "Main optimizer for multi-objective tasks",
                            "methods": ["add_objective", "with_quality_metric", "evaluate_candidates"]
                        },
                        "MultiObjectiveConfig": {
                            "description": "Configuration for multi-objective optimization"
                        },
                        "Objective": {
                            "description": "Single objective definition with weight",
                            "factory": "Objective::new(ObjectiveType::Quality, weight)"
                        },
                        "ObjectiveType": {
                            "description": "Type of objective to optimize",
                            "variants": ["Quality", "Cost", "Latency", "TokenUsage"]
                        },
                        "ObjectiveValue": {
                            "description": "Computed value for an objective"
                        },
                        "Candidate": {
                            "description": "A model/prompt configuration to evaluate",
                            "methods": ["with_eval_fn"]
                        },
                        "ParetoFrontier": {
                            "description": "Set of non-dominated solutions",
                            "methods": ["select_by_budget", "select_by_quality"]
                        },
                        "ParetoSolution": {
                            "description": "Single solution on the Pareto frontier"
                        }
                    },
                    "workflow": [
                        "1. Create MultiObjectiveOptimizer with objectives (quality, cost)",
                        "2. Define candidates (different models/prompts)",
                        "3. Run evaluate_candidates to find Pareto frontier",
                        "4. Select best solution by budget or quality threshold"
                    ],
                    "use_cases": [
                        "Budget-constrained optimization: best quality within fixed cost",
                        "Quality-constrained cost minimization: cheapest model meeting quality bar",
                        "Tradeoff analysis: understand quality/cost curve"
                    ],
                    "exports": [
                        "MultiObjectiveOptimizer", "MultiObjectiveConfig", "MultiObjectiveError",
                        "Objective", "ObjectiveType", "ObjectiveValue",
                        "Candidate", "ParetoFrontier", "ParetoSolution", "ParetoError"
                    ],
                    "notes": [
                        "Pareto frontier shows all optimal tradeoffs",
                        "Use select_by_budget when you have a fixed cost target",
                        "Use select_by_quality when you need minimum quality guarantees"
                    ]
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&multiobjective_info).unwrap()
                );
                return 0;
            }
            "aggregation" => {
                // DashOptimize aggregation utilities
                let aggregation_info = serde_json::json!({
                    "description": "DashFlow's aggregation utilities for combining multiple outputs",
                    "functions": {
                        "majority": {
                            "description": "Select the most common answer from multiple outputs",
                            "use_case": "Self-consistency voting across multiple reasoning chains"
                        },
                        "default_normalize": {
                            "description": "Default text normalization for comparison",
                            "use_case": "Standardize text before aggregation"
                        }
                    },
                    "exports": ["majority", "default_normalize"],
                    "notes": [
                        "Use with EnsembleNode or MultiChainComparisonNode",
                        "Majority voting improves accuracy on reasoning tasks"
                    ]
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&aggregation_info).unwrap()
                );
                return 0;
            }
            "knn" => {
                // DashOptimize KNN for example selection
                let knn_info = serde_json::json!({
                    "description": "DashFlow's KNN implementation for few-shot example selection",
                    "types": {
                        "KNN": {
                            "description": "K-Nearest Neighbors for finding similar examples",
                            "use_case": "Select most relevant few-shot examples based on input similarity"
                        },
                        "DashFlowExample": {
                            "description": "Example data point for KNN indexing",
                            "note": "Aliased as DashFlowExample to avoid conflict with local Example type"
                        }
                    },
                    "workflow": [
                        "1. Create KNN index with training examples",
                        "2. For each input, find k most similar examples",
                        "3. Use selected examples as few-shot prompts"
                    ],
                    "exports": ["KNN", "DashFlowExample"],
                    "notes": [
                        "Used internally by KNNFewShot optimizer",
                        "Embeddings determine similarity between examples"
                    ]
                });
                println!("{}", serde_json::to_string_pretty(&knn_info).unwrap());
                return 0;
            }
            "signatures" => {
                // DashOptimize signature and content types for structured LLM interactions
                let signatures_info = serde_json::json!({
                    "description": "DashFlow's signature and content types for structured LLM interactions",
                    "signature_types": {
                        "Signature": {
                            "description": "Task definition specifying inputs, outputs, and semantic description",
                            "use_case": "Define what the LLM should produce given specific inputs"
                        },
                        "Field": {
                            "description": "A field in a signature (input or output)",
                            "properties": ["name", "description", "kind"]
                        },
                        "FieldKind": {
                            "description": "Type of field (Input or Output)",
                            "variants": ["Input", "Output"]
                        },
                        "make_signature": {
                            "description": "Helper function to create signatures from shorthand notation",
                            "example": "make_signature(\"question, context -> answer\", \"Answer questions from context\")"
                        }
                    },
                    "content_types": {
                        "LlmContent": {
                            "description": "Content that can be sent to or received from LLMs",
                            "variants": ["Text", "Image", "Audio", "Document", "Code", "ToolCall", "ToolResult"]
                        },
                        "DashFlowOptMessage": {
                            "description": "A message in an LLM conversation (renamed to avoid conflict)",
                            "fields": ["role", "content"]
                        },
                        "DashFlowOptRole": {
                            "description": "Role of a message sender (renamed to avoid conflict)",
                            "variants": ["System", "User", "Assistant", "Tool"]
                        },
                        "Image": "Image content with format and data",
                        "Audio": "Audio content with format and data",
                        "Document": "Document content (PDF, etc.)",
                        "Code": "Code content with language",
                        "Citation": "Citation reference",
                        "File": "File content",
                        "History": "Conversation history",
                        "Reasoning": "Reasoning/chain-of-thought content",
                        "ReasoningEffort": "Level of reasoning effort",
                        "ReasoningOutput": "Output of reasoning process",
                        "ReasoningStep": "A single step in reasoning chain"
                    },
                    "tool_types": {
                        "DashFlowOptToolCall": "Tool call in optimization context (renamed to avoid conflict)",
                        "ToolCalls": "Collection of tool calls",
                        "DashFlowOptToolResult": "Tool result in optimization context (renamed to avoid conflict)"
                    },
                    "traits": {
                        "ToLlmContent": {
                            "description": "Convert types to LlmContent for LLM interactions",
                            "method": "to_llm_content() -> LlmContent"
                        }
                    },
                    "extension_traits": {
                        "DspyGraphExt": {
                            "description": "Extension trait for StateGraph to add DSPy-style optimization",
                            "methods": ["add_llm_node", "optimize"]
                        }
                    },
                    "exports": [
                        "make_signature", "Field", "FieldKind", "Signature",
                        "Audio", "AudioFormat", "Citation", "Code", "Document", "File", "FileType",
                        "History", "Image", "ImageFormat", "Language", "LlmContent",
                        "DashFlowOptMessage", "Reasoning", "ReasoningEffort", "ReasoningOutput",
                        "ReasoningStep", "DashFlowOptRole", "ToLlmContent",
                        "DashFlowOptToolCall", "ToolCalls", "DashFlowOptToolResult", "DspyGraphExt"
                    ],
                    "notes": [
                        "Message, Role, ToolCall, ToolResult are renamed with DashFlowOpt prefix to avoid conflicts",
                        "Signatures enable type-safe prompt optimization",
                        "Content types support multimodal LLM interactions"
                    ]
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&signatures_info).unwrap()
                );
                return 0;
            }
            "optimizerext" => {
                // Additional DashOptimize optimizer types beyond the main optimizers section
                let optimizer_ext_info = serde_json::json!({
                    "description": "Additional DashFlow optimizer types and traits",
                    "optimizer_types": {
                        "COPROv2": {
                            "description": "Contextual Prompt Optimization v2 - advanced instruction optimization",
                            "use_case": "Automatically improve system prompts based on task performance"
                        },
                        "COPROv2Builder": "Builder for COPROv2 optimizer configuration",
                        "COPROv2MetricFn": "Metric function type for COPROv2 optimization",
                        "AutoPromptMetricFn": "Metric function type for AutoPrompt optimization",
                        "GEPAMetricFn": "Metric function type for GEPA optimization"
                    },
                    "result_types": {
                        "DashFlowOptimizationResult": {
                            "description": "Result of an optimization run (renamed to avoid conflict)",
                            "fields": ["best_score", "best_program", "iterations", "history"]
                        },
                        "ScoreWithFeedback": {
                            "description": "Score with optional feedback for improvement",
                            "fields": ["score", "feedback"]
                        }
                    },
                    "strategy_types": {
                        "OptSelectStrategy": {
                            "description": "Selection strategy for optimization (renamed to avoid conflict with scheduler)",
                            "use_case": "Configure how candidates are selected during optimization"
                        },
                        "OptTraceStep": {
                            "description": "Trace step from optimizers (renamed to avoid conflict with debug)",
                            "use_case": "Record optimization progress"
                        }
                    },
                    "distillation_types": {
                        "DistillationConfigBuilder": "Builder for DistillationConfig",
                        "DistillationResult": "Result of model distillation process",
                        "SyntheticDataConfig": "Configuration for synthetic data generation"
                    },
                    "error_types": {
                        "CostMonitorError": "Error type for cost monitoring operations"
                    },
                    "traits": {
                        "Optimizable": {
                            "description": "Trait for nodes that can be optimized with training data",
                            "methods": ["optimize", "get_optimization_state", "set_optimization_state"]
                        }
                    },
                    "state_types": {
                        "OptimizationState": {
                            "description": "Captured state during optimization (instruction, examples, metadata)",
                            "use_case": "Save/load optimized prompt configurations"
                        },
                        "DashFlowFewShotExample": {
                            "description": "Few-shot example for optimized prompts (renamed to avoid conflict)",
                            "fields": ["input", "output", "reasoning"]
                        }
                    },
                    "exports": [
                        "COPROv2", "COPROv2Builder", "COPROv2MetricFn",
                        "AutoPromptMetricFn", "GEPAMetricFn",
                        "DashFlowOptimizationResult", "ScoreWithFeedback",
                        "OptSelectStrategy", "OptTraceStep",
                        "DistillationConfigBuilder", "DistillationResult", "SyntheticDataConfig",
                        "CostMonitorError", "Optimizable", "OptimizationState", "DashFlowFewShotExample"
                    ],
                    "notes": [
                        "These extend the main 'optimizers' section with additional types",
                        "Types are renamed where they conflict with existing codex_dashflow types",
                        "Use with main optimizers for complete prompt optimization workflows"
                    ]
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&optimizer_ext_info).unwrap()
                );
                return 0;
            }
            "debug" => {
                // DashFlow Debug module for graph visualization and execution tracing
                let debug_info = serde_json::json!({
                    "description": "DashFlow debug utilities for graph visualization and execution tracing",
                    "modules": {
                        "mermaid_export": {
                            "description": "Generate Mermaid diagrams from StateGraph structures",
                            "types": {
                                "MermaidConfig": {
                                    "description": "Configuration for Mermaid diagram export",
                                    "fields": ["direction", "include_fence", "node_shape", "terminal_shape", "conditional_shape", "node_labels", "node_styles", "edge_labels", "styles", "title"]
                                },
                                "MermaidDirection": {
                                    "description": "Mermaid diagram direction",
                                    "variants": ["TopToBottom (TD)", "LeftToRight (LR)", "BottomToTop (BT)", "RightToLeft (RL)"]
                                },
                                "MermaidNodeShape": {
                                    "description": "Mermaid node shape styles",
                                    "variants": ["Rectangle", "Stadium", "Subroutine", "Cylinder", "Circle", "Asymmetric", "Rhombus", "Hexagon", "Parallelogram", "Trapezoid", "DoubleCircle"]
                                },
                                "GraphStructure": {
                                    "description": "Simplified graph structure for Mermaid export",
                                    "fields": ["nodes", "edges", "conditional_edges", "parallel_edges", "entry_point"]
                                },
                                "MermaidExport": {
                                    "description": "Trait for StateGraph to enable Mermaid export",
                                    "methods": ["to_graph_structure", "to_mermaid", "to_mermaid_with_config"]
                                }
                            }
                        },
                        "execution_tracing": {
                            "description": "Record execution paths with state snapshots for debugging",
                            "types": {
                                "ExecutionTracer": {
                                    "description": "Thread-safe execution tracer that collects trace data",
                                    "methods": ["new", "with_state_capture", "get_trace", "clear", "start_execution", "complete_execution", "start_node", "complete_node"]
                                },
                                "ExecutionTrace": {
                                    "description": "Complete execution trace with all steps",
                                    "fields": ["graph_name", "thread_id", "started_at", "total_duration", "steps", "completed", "final_error"],
                                    "methods": ["steps", "step_count", "total_time", "node_path", "to_mermaid_sequence", "to_json"]
                                },
                                "TraceStep": {
                                    "description": "A single step in the execution trace",
                                    "fields": ["step_number", "node", "started_at", "duration", "edge_taken", "state_before", "state_after", "error"]
                                },
                                "EdgeTaken": {
                                    "description": "Information about an edge taken during execution",
                                    "fields": ["edge_type", "from", "to", "condition_result"]
                                },
                                "TracedEdgeType": {
                                    "description": "Type of edge in a trace",
                                    "variants": ["Simple", "Conditional", "Parallel"]
                                },
                                "TracingCallback": {
                                    "description": "Event callback that records execution traces",
                                    "usage": "Attach to compiled graph with .with_callback(TracingCallback::new(tracer))"
                                }
                            }
                        }
                    },
                    "exports": [
                        "MermaidConfig",
                        "MermaidDirection",
                        "MermaidNodeShape",
                        "MermaidExport",
                        "GraphStructure",
                        "ExecutionTracer",
                        "ExecutionTrace",
                        "TraceStep",
                        "EdgeTaken",
                        "TracedEdgeType",
                        "TracingCallback"
                    ],
                    "usage_examples": {
                        "mermaid_export": [
                            "use codex_dashflow_core::{MermaidExport, MermaidConfig, MermaidDirection};",
                            "",
                            "// Export graph as Mermaid diagram",
                            "let mermaid = graph.to_mermaid();",
                            "",
                            "// With custom config",
                            "let config = MermaidConfig::new()",
                            "    .direction(MermaidDirection::LeftToRight)",
                            "    .title(\"My Agent Graph\");",
                            "let mermaid = graph.to_mermaid_with_config(&config);"
                        ],
                        "execution_tracing": [
                            "use codex_dashflow_core::{ExecutionTracer, TracingCallback};",
                            "",
                            "let tracer = ExecutionTracer::new();",
                            "let app = graph.compile()?",
                            "    .with_callback(TracingCallback::new(tracer.clone()));",
                            "",
                            "let result = app.invoke(state).await?;",
                            "",
                            "// Get execution trace",
                            "let trace = tracer.get_trace();",
                            "for step in trace.steps() {",
                            "    println!(\"Node: {}, Duration: {:?}\", step.node, step.duration);",
                            "}",
                            "",
                            "// Export as Mermaid sequence diagram",
                            "println!(\"{}\", trace.to_mermaid_sequence());"
                        ]
                    },
                    "notes": [
                        "MermaidExport trait is implemented for StateGraph",
                        "ExecutionTracer is thread-safe and can be cloned",
                        "State capture can significantly increase memory usage for large states",
                        "TracingCallback integrates with DashFlow's EventCallback system"
                    ]
                });
                println!("{}", serde_json::to_string_pretty(&debug_info).unwrap());
                return 0;
            }
            "approval" => {
                // DashFlow Approval module for human-in-the-loop patterns
                let approval_info = serde_json::json!({
                    "description": "DashFlow's built-in approval flow for human-in-the-loop patterns",
                    "note": "These complement codex_dashflow's own approval_presets module",
                    "types": {
                        "ApprovalRequest": {
                            "description": "A request for approval from the agent",
                            "fields": ["id", "action", "context", "risk_level", "timestamp"]
                        },
                        "ApprovalResponse": {
                            "description": "Response to an approval request",
                            "variants": ["Approved", "Rejected { reason: String }", "Modified { changes: String }"]
                        },
                        "PendingApproval": {
                            "description": "An approval request awaiting response",
                            "fields": ["request", "created_at", "response_sender"]
                        },
                        "ApprovalChannel": {
                            "description": "Channel for sending approval requests",
                            "methods": ["new", "request_approval"]
                        },
                        "ApprovalReceiver": {
                            "description": "Receiver end of approval channel",
                            "methods": ["recv", "try_recv"]
                        },
                        "ApprovalNode": {
                            "description": "Graph node that gates execution on approval",
                            "usage": "Wraps any action that requires human approval"
                        },
                        "AutoApprovalPolicy": {
                            "description": "Policy for automatic approval decisions",
                            "variants": ["AlwaysApprove", "AlwaysReject", "ApproveByRisk { max_risk: RiskLevel }", "Custom"]
                        },
                        "DashFlowRiskLevel": {
                            "description": "Risk level for approval decisions",
                            "variants": ["None", "Low", "Medium", "High", "Critical"],
                            "note": "Aliased as DashFlowRiskLevel to avoid conflict with codex RiskLevel"
                        }
                    },
                    "functions": {
                        "auto_approval_handler": {
                            "description": "Async handler that automatically processes approvals based on policy",
                            "signature": "async fn auto_approval_handler(receiver: ApprovalReceiver, policy: AutoApprovalPolicy)"
                        }
                    },
                    "exports": [
                        "ApprovalRequest",
                        "ApprovalResponse",
                        "PendingApproval",
                        "ApprovalChannel",
                        "ApprovalReceiver",
                        "ApprovalNode",
                        "AutoApprovalPolicy",
                        "DashFlowRiskLevel",
                        "auto_approval_handler"
                    ],
                    "integration": {
                        "codex_approval_presets": {
                            "description": "codex_dashflow's own approval system (approval_presets module)",
                            "types": ["ApprovalPreset", "ApprovalMode"],
                            "note": "Use codex presets for CLI/TUI approval flow, DashFlow types for advanced graph-based approval"
                        }
                    },
                    "notes": [
                        "DashFlow approval types enable graph-level human-in-the-loop patterns",
                        "DashFlowRiskLevel is aliased to avoid conflict with codex's RiskLevel enum",
                        "auto_approval_handler can be spawned as a background task for testing/automation",
                        "For most codex_dashflow use cases, use the approval_presets module instead"
                    ]
                });
                println!("{}", serde_json::to_string_pretty(&approval_info).unwrap());
                return 0;
            }
            "streaming" => {
                // DashFlow Streaming observability types
                // Documents streaming telemetry, metrics monitoring, and quality observability
                let streaming_info = serde_json::json!({
                    "description": "DashFlow Streaming observability for telemetry monitoring and quality tracking",
                    "feature_flag": "Requires 'dashstream' feature to be enabled",
                    "modules": {
                        "metrics_monitor": {
                            "description": "Prometheus metrics monitoring and message loss detection",
                            "types": {
                                "MetricsSnapshot": {
                                    "description": "Snapshot of current streaming metrics",
                                    "fields": ["messages_sent", "messages_received", "send_failures", "decode_failures", "loss_rate"]
                                }
                            },
                            "functions": {
                                "get_metrics_text": "Get all metrics in Prometheus text format",
                                "calculate_loss_rate": "Calculate message loss rate (sent - received) / sent",
                                "check_for_high_loss": "Check for high message loss and log alerts"
                            }
                        },
                        "quality_monitor": {
                            "description": "LLM-as-judge quality evaluation integrated into streaming telemetry",
                            "types": {
                                "QualityMonitor": {
                                    "description": "Quality monitor with integrated LLM-as-judge evaluation",
                                    "methods": ["new", "with_judge", "evaluate_and_emit"]
                                },
                                "QualityJudge": {
                                    "description": "Trait for implementing quality judge models (GPT-4, Claude, custom)",
                                    "methods": ["judge_response", "detect_issues"]
                                },
                                "StreamingQualityScore": {
                                    "description": "Quality scores from LLM judge (accuracy, relevance, completeness)",
                                    "note": "Aliased as StreamingQualityScore to avoid conflict with dashflow::quality::QualityScore"
                                },
                                "QualityIssue": {
                                    "description": "Quality issues detected in responses",
                                    "variants": ["ToolResultsIgnored", "IncompleteCoverage", "LowAccuracy", "LowRelevance", "LowCompleteness"]
                                }
                            }
                        },
                        "quality_gate": {
                            "description": "Self-correcting retry loops for guaranteed quality",
                            "types": {
                                "StreamingQualityGate": {
                                    "description": "Quality gate with self-correcting retry loops",
                                    "note": "Aliased as StreamingQualityGate to avoid conflict with dashflow::quality::QualityGate"
                                },
                                "StreamingQualityConfig": {
                                    "description": "Configuration for streaming quality gate",
                                    "fields": ["quality_threshold", "max_retries", "verbose", "judge"],
                                    "note": "Aliased as StreamingQualityConfig"
                                },
                                "QualityGateError": {
                                    "description": "Errors from quality gate execution",
                                    "variants": ["QualityThresholdNotMet", "ExecutionFailed", "JudgeFailed", "NoJudgeConfigured"]
                                }
                            }
                        }
                    },
                    "agent_events": {
                        "QualityGateStart": {
                            "description": "Emitted when quality validation begins",
                            "fields": ["session_id", "attempt", "max_retries", "threshold"]
                        },
                        "QualityGateResult": {
                            "description": "Emitted when quality validation completes",
                            "fields": ["session_id", "attempt", "passed", "accuracy", "relevance", "completeness", "average_score", "is_final", "reason"]
                        }
                    },
                    "exports": [
                        "MetricsSnapshot",
                        "get_metrics_text",
                        "calculate_loss_rate",
                        "check_for_high_loss",
                        "QualityMonitor",
                        "QualityJudge",
                        "StreamingQualityScore",
                        "QualityIssue",
                        "StreamingQualityGate",
                        "StreamingQualityConfig",
                        "QualityGateError",
                        "DashStreamCallback",
                        "DashFlowDashStreamConfig",
                        "DEFAULT_MAX_STATE_DIFF_SIZE",
                        "DEFAULT_MAX_CONCURRENT_TELEMETRY_SENDS",
                        "DEFAULT_TELEMETRY_BATCH_SIZE",
                        "DEFAULT_TELEMETRY_BATCH_TIMEOUT_MS"
                    ],
                    "dashstream_callback": {
                        "description": "Native DashFlow streaming callback for graph events",
                        "types": {
                            "DashStreamCallback": {
                                "description": "Callback that sends graph events to Kafka for dashstream observability",
                                "methods": ["new", "with_config", "flush", "telemetry_dropped_count", "pending_task_count"],
                                "features": ["Event batching", "State diffing", "Flow control", "Graceful shutdown"]
                            },
                            "DashFlowDashStreamConfig": {
                                "description": "Full configuration for DashStreamCallback (renamed to avoid conflict)",
                                "fields": ["bootstrap_servers", "topic", "tenant_id", "thread_id", "enable_state_diff", "compression_threshold", "max_state_diff_size", "max_concurrent_telemetry_sends", "telemetry_batch_size", "telemetry_batch_timeout_ms"]
                            }
                        },
                        "constants": {
                            "DEFAULT_MAX_STATE_DIFF_SIZE": "10MB - States larger than this skip diffing to prevent OOM",
                            "DEFAULT_MAX_CONCURRENT_TELEMETRY_SENDS": "64 - Flow control limit for concurrent telemetry sends",
                            "DEFAULT_TELEMETRY_BATCH_SIZE": "1 - Events per batch (1 = no batching)",
                            "DEFAULT_TELEMETRY_BATCH_TIMEOUT_MS": "100 - Flush timeout when batching"
                        }
                    },
                    "notes": [
                        "All types require the 'dashstream' feature flag",
                        "Types are conditionally re-exported from codex_dashflow_core",
                        "StreamingQuality* types are aliased to avoid conflicts with dashflow::quality types",
                        "QualityGateStart and QualityGateResult events are emitted during reasoning node execution",
                        "DashStreamCallback provides native DashFlow graph-to-Kafka integration",
                        "DashFlowDashStreamConfig is renamed from DashStreamConfig to avoid conflict with CLI's simpler config"
                    ]
                });
                println!("{}", serde_json::to_string_pretty(&streaming_info).unwrap());
                return 0;
            }
            "checkpoint" => {
                // DashFlow Checkpoint module for advanced checkpointing features
                let checkpoint_info = serde_json::json!({
                    "description": "DashFlow's advanced checkpointing features for state persistence and recovery",
                    "core_types": {
                        "Checkpoint": {
                            "description": "A checkpoint representing saved state at a point in time",
                            "fields": ["id", "thread_id", "state", "metadata", "version"]
                        },
                        "CheckpointId": {
                            "description": "Unique identifier for a checkpoint",
                            "type": "Uuid"
                        },
                        "Checkpointer": {
                            "description": "Trait for checkpoint storage backends",
                            "methods": ["save", "load", "delete", "list_checkpoints", "get_metadata"]
                        }
                    },
                    "file_checkpointers": {
                        "FileCheckpointer": {
                            "description": "Simple file-based checkpoint storage",
                            "storage": "One JSON file per checkpoint"
                        },
                        "CompressedFileCheckpointer": {
                            "description": "File checkpointer with compression support",
                            "algorithms": ["Gzip", "Zstd", "Lz4"],
                            "fields": ["base_dir", "algorithm"]
                        },
                        "VersionedFileCheckpointer": {
                            "description": "File checkpointer with version tracking",
                            "features": ["Version history", "Schema migrations"]
                        }
                    },
                    "database_checkpointers": {
                        "SqliteCheckpointer": {
                            "description": "SQLite-based checkpoint storage",
                            "features": ["Local database", "ACID transactions"]
                        },
                        "MemoryCheckpointer": {
                            "description": "In-memory checkpoint storage (non-persistent)",
                            "use_case": "Testing and development"
                        }
                    },
                    "distributed": {
                        "DistributedCheckpointCoordinator": {
                            "description": "Coordinates checkpointing across multiple nodes",
                            "features": ["Leader election", "Conflict resolution", "Replication"]
                        },
                        "MultiTierCheckpointer": {
                            "description": "Tiered storage with multiple backends",
                            "tiers": ["Hot (memory)", "Warm (local file)", "Cold (remote storage)"]
                        }
                    },
                    "migration": {
                        "StateMigration": {
                            "description": "Defines a migration from one state version to another",
                            "fields": ["from_version", "to_version", "migrate_fn"]
                        },
                        "MigrationChain": {
                            "description": "Chain of migrations to upgrade state across versions",
                            "methods": ["add_migration", "migrate_to", "current_version"]
                        }
                    },
                    "resume": {
                        "ResumeRunner": {
                            "description": "Runs graph execution with resume support",
                            "methods": ["run", "run_with_checkpointer"]
                        },
                        "ResumeValidator": {
                            "description": "Validates checkpoint compatibility before resuming",
                            "checks": ["Version compatibility", "State schema", "Graph structure"]
                        },
                        "ResumeEnvironment": {
                            "description": "Environment context for resume operations"
                        },
                        "ResumeError": {
                            "description": "Errors that can occur during resume",
                            "variants": ["CheckpointNotFound", "IncompatibleState", "MigrationFailed"]
                        },
                        "ResumeOutcome": {
                            "description": "Result of a resume operation",
                            "variants": ["Resumed", "StartedFresh", "RequiresMigration"]
                        }
                    },
                    "versioning": {
                        "Version": {
                            "description": "Semantic version for state schema",
                            "fields": ["major", "minor", "patch"]
                        },
                        "VersionedCheckpoint": {
                            "description": "Checkpoint with version information",
                            "features": ["Schema version tracking", "Migration support"]
                        },
                        "WritePolicy": {
                            "description": "Policy for checkpoint write operations",
                            "variants": ["Always", "OnChange", "OnInterval"]
                        }
                    },
                    "thread_types": {
                        "ThreadId": {
                            "description": "Unique identifier for a checkpoint thread/session",
                            "type": "String wrapper"
                        },
                        "DashFlowThreadInfo": {
                            "description": "Metadata about a checkpoint thread (renamed from ThreadInfo to avoid conflict)",
                            "fields": ["thread_id", "checkpoint_count", "latest_checkpoint_id", "updated_at"]
                        },
                        "DashFlowCheckpointMetadata": {
                            "description": "Metadata for a checkpoint (renamed from CheckpointMetadata to avoid conflict)",
                            "fields": ["checkpoint_id", "thread_id", "created_at", "version", "tags"]
                        }
                    },
                    "notes": [
                        "All checkpointers implement the Checkpointer trait",
                        "Use CompressedFileCheckpointer for production deployments",
                        "MigrationChain enables safe state schema evolution",
                        "DistributedCheckpointCoordinator requires external coordination service",
                        "ThreadId, DashFlowThreadInfo, DashFlowCheckpointMetadata are DashFlow types (prefixed to avoid name conflicts with our own types)"
                    ]
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&checkpoint_info).unwrap()
                );
                return 0;
            }
            "alerts" => {
                // DashFlow Introspection alerts and budget monitoring
                let alerts_info = serde_json::json!({
                    "description": "DashFlow's alert system for monitoring performance and budget",
                    "alert_types": {
                        "AlertSeverity": {
                            "description": "Severity level for alerts",
                            "variants": ["Info", "Warning", "Error", "Critical"]
                        },
                        "AlertType": {
                            "description": "Type of alert condition",
                            "variants": ["Performance", "Budget", "Quality", "Resource"]
                        },
                        "PerformanceAlert": {
                            "description": "Alert for performance threshold violations",
                            "fields": ["severity", "metric", "threshold", "actual_value", "message"]
                        }
                    },
                    "budget_alerts": {
                        "BudgetAlert": {
                            "description": "Alert for budget-related conditions",
                            "fields": ["severity", "alert_type", "current_usage", "limit", "message"]
                        },
                        "BudgetAlertSeverity": {
                            "description": "Budget-specific severity levels",
                            "variants": ["Notice", "Warning", "Critical", "Exceeded"]
                        },
                        "BudgetAlertType": {
                            "description": "Type of budget alert",
                            "variants": ["TokenUsage", "CostLimit", "RateLimit", "DailyLimit"]
                        }
                    },
                    "capabilities": {
                        "CapabilityManifest": {
                            "description": "Declares the capabilities of an AI agent or system",
                            "fields": ["name", "version", "model_capabilities", "features", "limitations"]
                        },
                        "CapabilityManifestBuilder": {
                            "description": "Builder pattern for CapabilityManifest",
                            "methods": ["name", "version", "add_capability", "add_feature", "build"]
                        },
                        "ModelCapability": {
                            "description": "Specific capability of an LLM model",
                            "examples": ["TextGeneration", "CodeGeneration", "Reasoning", "ToolUse"]
                        },
                        "ModelFeature": {
                            "description": "Feature supported by a model",
                            "examples": ["Streaming", "FunctionCalling", "Vision", "LongContext"]
                        }
                    },
                    "decision_logging": {
                        "DecisionLog": {
                            "description": "Log of decisions made during execution",
                            "fields": ["timestamp", "node", "decision", "reasoning", "context"]
                        },
                        "DecisionLogBuilder": {
                            "description": "Builder for DecisionLog entries"
                        },
                        "DecisionHistory": {
                            "description": "Complete history of decisions for a thread",
                            "features": ["Searchable", "Filterable by node", "Exportable"]
                        }
                    },
                    "optimization_suggestions": {
                        "OptimizationAnalysis": {
                            "description": "Analysis results with optimization suggestions",
                            "fields": ["suggestions", "estimated_improvement", "priority"]
                        },
                        "OptimizationSuggestion": {
                            "description": "A specific optimization recommendation",
                            "fields": ["category", "priority", "description", "expected_impact"]
                        },
                        "OptimizationCategory": {
                            "description": "Category of optimization",
                            "variants": ["Latency", "Cost", "Quality", "Throughput", "Memory"]
                        },
                        "OptimizationPriority": {
                            "description": "Priority of optimization suggestion",
                            "variants": ["Low", "Medium", "High", "Critical"]
                        }
                    },
                    "graph_reconfiguration": {
                        "GraphReconfiguration": {
                            "description": "Suggested reconfiguration for a graph",
                            "fields": ["type", "priority", "description", "affected_nodes"]
                        },
                        "ReconfigurationType": {
                            "description": "Type of graph reconfiguration",
                            "variants": ["AddNode", "RemoveNode", "ReorderEdges", "ParallelizeNodes"]
                        },
                        "ReconfigurationPriority": {
                            "description": "Priority of reconfiguration suggestion"
                        }
                    },
                    "pattern_detection": {
                        "Pattern": {
                            "description": "A detected pattern in execution",
                            "fields": ["type", "confidence", "occurrences", "context"]
                        },
                        "PatternAnalysis": {
                            "description": "Analysis of patterns across executions"
                        },
                        "PatternType": {
                            "description": "Type of detected pattern",
                            "variants": ["Bottleneck", "Loop", "Redundancy", "ErrorProne", "Opportunity"]
                        }
                    },
                    "notes": [
                        "Alerts are generated based on configurable thresholds",
                        "BudgetAlerts help prevent unexpected API costs",
                        "OptimizationSuggestions are AI-generated recommendations",
                        "Pattern detection requires multiple executions to identify trends"
                    ]
                });
                println!("{}", serde_json::to_string_pretty(&alerts_info).unwrap());
                return 0;
            }
            "stategraph" => {
                // DashFlow Core StateGraph types documentation
                let stategraph_info = serde_json::json!({
                    "description": "DashFlow's core StateGraph types for building graph-based workflows",
                    "edge_types": {
                        "Edge": {
                            "description": "Basic edge connecting two nodes",
                            "usage": "graph.add_edge(\"node_a\", \"node_b\")"
                        },
                        "ConditionalEdge": {
                            "description": "Edge with routing function based on state",
                            "usage": "graph.add_conditional_edges(\"node\", router_fn)"
                        },
                        "ParallelEdge": {
                            "description": "Edge for parallel execution of multiple targets",
                            "usage": "Enables fork-join patterns"
                        },
                        "END": {
                            "description": "Sentinel node marking graph completion",
                            "type": "constant"
                        },
                        "START": {
                            "description": "Sentinel node marking graph entry",
                            "type": "constant"
                        }
                    },
                    "event_types": {
                        "GraphEvent": {
                            "description": "Events emitted during graph execution",
                            "variants": ["NodeStart", "NodeEnd", "EdgeTaken", "Error"]
                        },
                        "EventCallback": {
                            "description": "Trait for receiving graph execution events"
                        },
                        "CollectingCallback": {
                            "description": "Callback that collects all events for later analysis"
                        },
                        "PrintCallback": {
                            "description": "Callback that prints events to stdout for debugging"
                        },
                        "FnTracer": {
                            "description": "Function-based tracer for custom event handling"
                        },
                        "TracerEvent": {
                            "description": "Low-level tracer events for detailed monitoring"
                        },
                        "EdgeType": {
                            "description": "Type of edge transition (Normal, Conditional, Parallel)"
                        }
                    },
                    "executor_types": {
                        "CompiledGraph": {
                            "description": "Compiled graph ready for execution",
                            "methods": ["invoke", "stream", "manifest", "validate"]
                        },
                        "ExecutionResult": {
                            "description": "Result of graph execution with final state and metrics"
                        },
                        "GraphIntrospection": {
                            "description": "Trait for introspecting compiled graphs"
                        },
                        "GraphValidationResult": {
                            "description": "Result of graph validation before execution"
                        },
                        "GraphValidationWarning": {
                            "description": "Warnings from graph validation"
                        },
                        "DEFAULT_GRAPH_TIMEOUT": {
                            "description": "Default timeout for graph execution",
                            "type": "constant"
                        },
                        "DEFAULT_NODE_TIMEOUT": {
                            "description": "Default timeout for individual node execution",
                            "type": "constant"
                        },
                        "DEFAULT_MAX_STATE_SIZE": {
                            "description": "Maximum allowed state size",
                            "type": "constant"
                        }
                    },
                    "graph_builders": {
                        "StateGraph": {
                            "description": "Main graph type for defining workflows",
                            "methods": ["new", "add_node", "add_edge", "set_entry_point", "compile"]
                        },
                        "GraphBuilder": {
                            "description": "Fluent builder API for constructing graphs",
                            "usage": "GraphBuilder::new().add_node(...).add_edge(...)"
                        }
                    },
                    "integration_types": {
                        "AgentNode": {
                            "description": "Node type for LLM-based agents"
                        },
                        "ToolNode": {
                            "description": "Node type for tool execution"
                        },
                        "RunnableNode": {
                            "description": "Generic runnable node wrapper"
                        },
                        "auto_tool_executor": {
                            "description": "Automatic tool execution based on agent output",
                            "type": "function"
                        },
                        "tools_condition": {
                            "description": "Condition function for tool-based routing",
                            "type": "function"
                        }
                    },
                    "node_types": {
                        "Node": {
                            "description": "Trait that all graph nodes must implement"
                        }
                    },
                    "prebuilt_patterns": {
                        "create_react_agent": {
                            "description": "Create a ReAct agent with reasoning and action loop",
                            "type": "function"
                        },
                        "DashFlowAgentState": {
                            "description": "Default agent state for prebuilt patterns (renamed to avoid conflict)"
                        }
                    },
                    "reducer_types": {
                        "Reducer": {
                            "description": "Trait for merging state updates"
                        },
                        "AddMessagesReducer": {
                            "description": "Reducer that appends messages to a list"
                        },
                        "add_messages": {
                            "description": "Helper function for message list reduction",
                            "type": "function"
                        },
                        "MessageExt": {
                            "description": "Extension trait for message types"
                        }
                    },
                    "state_types": {
                        "GraphState": {
                            "description": "Trait for types that can be graph state"
                        },
                        "MergeableState": {
                            "description": "Trait for states that support merging"
                        },
                        "JsonState": {
                            "description": "Dynamic JSON-based state type"
                        },
                        "JsonStateIter": {
                            "description": "Iterator over JsonState fields"
                        }
                    },
                    "stream_types": {
                        "StreamEvent": {
                            "description": "Events emitted during streaming execution"
                        },
                        "StreamMode": {
                            "description": "Mode for streaming (Values, Updates, Debug)",
                            "variants": ["Values", "Updates", "Debug"]
                        },
                        "DEFAULT_STREAM_CHANNEL_CAPACITY": {
                            "description": "Default channel capacity for streaming",
                            "type": "constant"
                        },
                        "stream_dropped_count": {
                            "description": "Get count of dropped stream events",
                            "type": "function"
                        },
                        "reset_stream_dropped_count": {
                            "description": "Reset the dropped stream event counter",
                            "type": "function"
                        }
                    },
                    "subgraph_types": {
                        "SubgraphNode": {
                            "description": "Node that wraps another compiled graph as a subgraph"
                        }
                    },
                    "derive_macros": {
                        "DeriveGraphState": {
                            "description": "Derive macro for GraphState trait"
                        },
                        "DeriveMergeableState": {
                            "description": "Derive macro for MergeableState trait"
                        },
                        "GraphStateDerive": {
                            "description": "Legacy derive macro (backwards compatibility)"
                        }
                    },
                    "error_types": {
                        "DashFlowError": {
                            "description": "Main DashFlow error type"
                        },
                        "CheckpointError": {
                            "description": "Checkpoint-specific errors"
                        }
                    },
                    "notes": [
                        "StateGraph is the core abstraction for defining agent workflows",
                        "Use GraphBuilder for fluent API, StateGraph for imperative style",
                        "CompiledGraph is immutable and thread-safe after compilation",
                        "StreamMode controls granularity of streaming updates",
                        "DashFlowAgentState is renamed from DashFlow's AgentState to avoid conflict with our AgentState"
                    ]
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&stategraph_info).unwrap()
                );
                return 0;
            }
            unknown => {
                eprintln!(
                    "Unknown section: {}. Valid sections: graph, nodes, edges, tools, registry, metrics, performance, quality, templates, scheduler, retention, dashoptimize, optimizers, evals, datacollection, modules, multiobjective, aggregation, knn, signatures, optimizerext, debug, approval, streaming, checkpoint, alerts, stategraph",
                    unknown
                );
                return 1;
            }
        }
    }

    // Brief summary output (for `codex architecture --brief`)
    if args.brief {
        // Build node flow order (topological from entry point)
        let node_names: Vec<&str> = [
            "user_input",
            "reasoning",
            "tool_selection",
            "tool_execution",
            "result_analysis",
        ]
        .into_iter()
        .filter(|n| manifest.nodes.contains_key(*n))
        .collect();

        // Extract tools from reasoning node
        let tools = manifest
            .nodes
            .get("reasoning")
            .map(|n| n.tools_available.clone())
            .unwrap_or_default();

        println!("Agent Architecture (DashFlow StateGraph)");
        println!("========================================");
        println!("Nodes: {}", node_names.join(" → "));
        println!("Entry: {}", manifest.entry_point);

        // Find exit conditions
        let exit_nodes: Vec<String> = manifest
            .edges
            .iter()
            .filter_map(|(from, edges)| {
                if edges.iter().any(|e| e.to == "__end__") {
                    Some(format!(
                        "{}{}",
                        from,
                        if edges.iter().any(|e| e.to == "__end__" && e.is_conditional) {
                            " (conditional)"
                        } else {
                            ""
                        }
                    ))
                } else {
                    None
                }
            })
            .collect();
        if !exit_nodes.is_empty() {
            println!("Exit: {}", exit_nodes.join(", "));
        }

        println!();
        println!("Capabilities:");
        if tools.iter().any(|t| t == "shell") {
            println!("  - Shell execution (sandboxed)");
        }
        if tools
            .iter()
            .any(|t| t == "read_file" || t == "write_file" || t == "apply_patch")
        {
            println!("  - File operations (read, write, patch)");
        }
        if tools.iter().any(|t| t == "list_dir" || t == "search_files") {
            println!("  - Directory navigation and search");
        }
        // Check if MCP is available in the manifest or always note it
        println!("  - MCP tool integration");

        return 0;
    }

    // Full output based on format
    match args.format {
        IntrospectFormat::Json => {
            let mut output = serde_json::to_value(&manifest).unwrap();

            // Add platform registry if requested
            if args.platform {
                use codex_dashflow_core::PlatformRegistry;
                let platform = PlatformRegistry::discover();
                if let Ok(platform_json) = serde_json::to_value(&platform) {
                    output["platform"] = platform_json;
                }
            }

            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        IntrospectFormat::Mermaid => {
            let mermaid = get_agent_graph_mermaid();
            println!("{}", mermaid);
        }
        IntrospectFormat::Text => {
            println!("=== Agent Introspection ===\n");
            println!(
                "Graph: {} ({})",
                manifest.graph_name.as_deref().unwrap_or("unknown"),
                manifest.graph_id.as_deref().unwrap_or("unknown")
            );
            println!("Entry Point: {}\n", manifest.entry_point);

            println!("Nodes ({}):", manifest.nodes.len());
            for (name, node) in &manifest.nodes {
                println!("  {} ({:?})", name, node.node_type);
                if let Some(ref desc) = node.description {
                    println!("    {}", desc);
                }
                if !node.tools_available.is_empty() {
                    println!("    Tools: {:?}", node.tools_available);
                }
            }

            println!("\nEdges:");
            for (from, edges) in &manifest.edges {
                for edge in edges {
                    let edge_type = if edge.is_conditional {
                        "conditional"
                    } else {
                        "direct"
                    };
                    println!("  {} -> {} ({})", from, edge.to, edge_type);
                    if let Some(ref desc) = edge.description {
                        println!("    {}", desc);
                    }
                }
            }

            if args.platform {
                use codex_dashflow_core::PlatformRegistry;
                let platform = PlatformRegistry::discover();
                println!("\n=== Platform ===");
                println!("DashFlow Version: {}", platform.version);
                println!("Modules: {}", platform.modules.len());
                println!("Features: {}", platform.features.len());
            }
        }
    }

    0
}

// ============================================================================
// Capabilities Command Implementation
// ============================================================================

/// Execute the capabilities subcommand.
///
/// Displays DashFlow platform capabilities, including available features,
/// modules, and version information. Uses PlatformRegistry::discover()
/// to query the DashFlow platform for its capabilities.
///
/// Output example (text format):
/// ```text
/// DashFlow Platform Capabilities
/// ==============================
/// Version: 0.4.0
///
/// Features:
///   - StateGraph: Graph-based workflow orchestration
///   - Streaming: Real-time event callbacks
///   - Checkpointing: Session persistence (Memory, File, PostgreSQL)
///   - Introspection: AI self-awareness
///   - DashOptimize: Prompt optimization and cost monitoring
/// ```
pub fn run_capabilities_command(args: &CapabilitiesArgs) -> i32 {
    use codex_dashflow_core::PlatformRegistry;

    let platform = PlatformRegistry::discover();

    match args.format {
        CapabilitiesFormat::Json => {
            // JSON output
            match serde_json::to_string_pretty(&platform) {
                Ok(json) => {
                    println!("{}", json);
                }
                Err(e) => {
                    eprintln!("Error serializing platform registry: {}", e);
                    return 1;
                }
            }
        }
        CapabilitiesFormat::Text => {
            // Human-readable text output
            println!("DashFlow Platform Capabilities");
            println!("==============================");
            println!("Version: {}", platform.version);
            println!();

            // Features
            if !platform.features.is_empty() {
                println!("Features:");
                for feature in &platform.features {
                    println!("  - {}: {}", feature.name, feature.description);
                }
                println!();
            }

            // Modules
            if !platform.modules.is_empty() {
                println!("Modules:");
                for module in &platform.modules {
                    println!("  - {}: {}", module.name, module.description);
                }
                println!();
            }

            // Crates
            if !platform.crates.is_empty() {
                println!("Available Crates ({}):", platform.crates.len());
                for krate in &platform.crates {
                    println!("  - {}: {}", krate.name, krate.description);
                }
            }
        }
    }

    0
}

// ============================================================================
// Features Command Implementation
// ============================================================================

/// Feature flag information for display
#[derive(Debug, Clone, serde::Serialize)]
pub struct FeatureInfo {
    /// Feature name
    pub name: String,
    /// Whether the feature is enabled (compiled in)
    pub enabled: bool,
    /// Description of the feature
    pub description: String,
    /// Dependencies or requirements
    pub requires: Option<String>,
}

/// Run the features command to list compiled feature flags
pub fn run_features_command(args: &FeaturesArgs) -> i32 {
    // Collect information about all known feature flags
    let features = vec![
        FeatureInfo {
            name: "dashstream".to_string(),
            enabled: cfg!(feature = "dashstream"),
            description: "DashFlow Streaming integration for real-time telemetry".to_string(),
            requires: Some("protoc (Protocol Buffers compiler)".to_string()),
        },
        FeatureInfo {
            name: "postgres".to_string(),
            enabled: cfg!(feature = "postgres"),
            description: "PostgreSQL checkpointing for production session persistence".to_string(),
            requires: Some("PostgreSQL server".to_string()),
        },
        FeatureInfo {
            name: "llm-judge".to_string(),
            enabled: cfg!(feature = "llm-judge"),
            description: "LLM-as-judge quality scoring using dashflow-evals".to_string(),
            requires: None,
        },
    ];

    match args.format {
        FeaturesFormat::Json => {
            // JSON output
            let output = serde_json::json!({
                "features": features,
                "version": env!("CARGO_PKG_VERSION"),
            });
            match serde_json::to_string_pretty(&output) {
                Ok(json) => {
                    println!("{}", json);
                }
                Err(e) => {
                    eprintln!("Error serializing features: {}", e);
                    return 1;
                }
            }
        }
        FeaturesFormat::Text => {
            // Human-readable text output
            println!("Codex DashFlow Feature Flags");
            println!("============================");
            println!("Version: {}", env!("CARGO_PKG_VERSION"));
            println!();
            println!("{:<15} {:<10} DESCRIPTION", "FEATURE", "STATUS");
            println!("{}", "-".repeat(70));

            for feature in &features {
                let status = if feature.enabled {
                    "enabled".green()
                } else {
                    "disabled".red()
                };
                println!(
                    "{:<15} {:<10} {}",
                    feature.name, status, feature.description
                );
                if let Some(ref req) = feature.requires {
                    println!("{:>25} Requires: {}", "", req.dimmed());
                }
            }

            println!();
            println!("To enable a feature, rebuild with: cargo build --features <feature>");
            println!("Example: cargo build --features dashstream,postgres");
        }
    }

    0
}

// ============================================================================
// Sessions Command Implementation
// ============================================================================

/// Session information for display
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionInfo {
    /// Session (thread) ID
    pub session_id: String,
    /// ID of the most recent checkpoint
    pub latest_checkpoint_id: String,
    /// When the session was last updated (ISO 8601 format)
    pub updated_at: String,
    /// Number of checkpoints (if available)
    pub checkpoint_count: Option<usize>,
}

/// Exit codes for the sessions command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionsExitCode {
    /// Success - sessions listed
    Success = 0,
    /// Error - failed to list sessions
    Error = 1,
    /// No checkpoint storage configured
    NoStorage = 2,
}

impl SessionsExitCode {
    /// Get the numeric exit code
    pub fn code(self) -> i32 {
        self as i32
    }
}

/// Execute the sessions subcommand, listing or deleting sessions.
///
/// This uses the core's `list_sessions()` and `delete_session()` functions
/// to manage sessions that have checkpoints saved.
///
/// Returns an exit code:
/// - 0: Success - sessions listed/deleted successfully
/// - 1: Error - failed to list/delete sessions
/// - 2: No checkpoint storage configured
pub async fn run_sessions_command(args: &SessionsArgs, config: &Config) -> SessionsExitCode {
    use codex_dashflow_core::{
        delete_all_sessions, delete_session, get_session_info, list_sessions, RunnerConfig,
    };

    // Build runner config from CLI args and file config
    // CLI args take precedence over file config
    let checkpoint_path = args
        .checkpoint_path
        .clone()
        .or_else(|| config.dashflow.checkpoint_path.clone());

    // Build the runner config to use for listing sessions
    let runner_config = if let Some(ref path) = checkpoint_path {
        RunnerConfig::with_file_checkpointing(path)
    } else {
        // Check if checkpointing is enabled in config
        if config.dashflow.checkpointing_enabled && config.dashflow.checkpoint_path.is_none() {
            // No checkpoint storage configured
            match args.format {
                SessionsFormat::Json => {
                    let output = serde_json::json!({
                        "error": "no_storage",
                        "message": "No checkpoint storage configured. Use --checkpoint-path or configure in config file."
                    });
                    println!("{}", serde_json::to_string_pretty(&output).unwrap());
                }
                SessionsFormat::Table => {
                    eprintln!("Error: No checkpoint storage configured.");
                    eprintln!(
                        "Use --checkpoint-path or configure 'checkpoint_path' in your config file."
                    );
                }
            }
            return SessionsExitCode::NoStorage;
        }
        RunnerConfig::default()
    };

    // Handle --delete flag if specified
    if let Some(ref session_id) = args.delete {
        // Check for confirmation unless --force is specified
        if !args.force {
            // In non-force mode, we warn and require --force
            match args.format {
                SessionsFormat::Json => {
                    let output = serde_json::json!({
                        "error": "confirmation_required",
                        "message": format!(
                            "Deleting session '{}' will remove all checkpoints. Use --force to confirm.",
                            session_id
                        )
                    });
                    println!("{}", serde_json::to_string_pretty(&output).unwrap());
                }
                SessionsFormat::Table => {
                    eprintln!(
                        "Warning: Deleting session '{}' will remove all checkpoints.",
                        session_id
                    );
                    eprintln!("Use --force to confirm deletion.");
                }
            }
            return SessionsExitCode::Error;
        }

        // Perform the deletion
        match delete_session(&runner_config, session_id).await {
            Ok(()) => {
                match args.format {
                    SessionsFormat::Json => {
                        let output = serde_json::json!({
                            "success": true,
                            "message": format!("Session '{}' deleted successfully.", session_id)
                        });
                        println!("{}", serde_json::to_string_pretty(&output).unwrap());
                    }
                    SessionsFormat::Table => {
                        println!("Session '{}' deleted successfully.", session_id);
                    }
                }
                return SessionsExitCode::Success;
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("checkpointing is not enabled")
                    || error_msg.contains("memory checkpointer")
                {
                    match args.format {
                        SessionsFormat::Json => {
                            let output = serde_json::json!({
                                "error": "no_storage",
                                "message": "No checkpoint storage configured. Use --checkpoint-path or configure in config file."
                            });
                            println!("{}", serde_json::to_string_pretty(&output).unwrap());
                        }
                        SessionsFormat::Table => {
                            eprintln!("Error: No checkpoint storage configured.");
                            eprintln!(
                                "Use --checkpoint-path or configure 'checkpoint_path' in your config file."
                            );
                        }
                    }
                    return SessionsExitCode::NoStorage;
                } else {
                    match args.format {
                        SessionsFormat::Json => {
                            let output = serde_json::json!({
                                "error": "delete_failed",
                                "message": format!("Failed to delete session: {}", e)
                            });
                            println!("{}", serde_json::to_string_pretty(&output).unwrap());
                        }
                        SessionsFormat::Table => {
                            eprintln!("Error: Failed to delete session: {}", e);
                        }
                    }
                    return SessionsExitCode::Error;
                }
            }
        }
    }

    // Handle --delete-all flag if specified
    if args.delete_all {
        // Check for confirmation unless --force is specified
        if !args.force {
            // In non-force mode, we warn and require --force
            match args.format {
                SessionsFormat::Json => {
                    let output = serde_json::json!({
                        "error": "confirmation_required",
                        "message": "Deleting ALL sessions will remove all checkpoints. Use --force to confirm."
                    });
                    println!("{}", serde_json::to_string_pretty(&output).unwrap());
                }
                SessionsFormat::Table => {
                    eprintln!("Warning: This will delete ALL sessions and their checkpoints.");
                    eprintln!("Use --force to confirm deletion.");
                }
            }
            return SessionsExitCode::Error;
        }

        // Perform the bulk deletion
        match delete_all_sessions(&runner_config).await {
            Ok(count) => {
                match args.format {
                    SessionsFormat::Json => {
                        let output = serde_json::json!({
                            "success": true,
                            "deleted_count": count,
                            "message": format!("Deleted {} session(s) successfully.", count)
                        });
                        println!("{}", serde_json::to_string_pretty(&output).unwrap());
                    }
                    SessionsFormat::Table => {
                        println!("Deleted {} session(s) successfully.", count);
                    }
                }
                return SessionsExitCode::Success;
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("checkpointing is not enabled")
                    || error_msg.contains("memory checkpointer")
                {
                    match args.format {
                        SessionsFormat::Json => {
                            let output = serde_json::json!({
                                "error": "no_storage",
                                "message": "No checkpoint storage configured. Use --checkpoint-path or configure in config file."
                            });
                            println!("{}", serde_json::to_string_pretty(&output).unwrap());
                        }
                        SessionsFormat::Table => {
                            eprintln!("Error: No checkpoint storage configured.");
                            eprintln!(
                                "Use --checkpoint-path or configure 'checkpoint_path' in your config file."
                            );
                        }
                    }
                    return SessionsExitCode::NoStorage;
                } else {
                    match args.format {
                        SessionsFormat::Json => {
                            let output = serde_json::json!({
                                "error": "delete_all_failed",
                                "message": format!("Failed to delete sessions: {}", e)
                            });
                            println!("{}", serde_json::to_string_pretty(&output).unwrap());
                        }
                        SessionsFormat::Table => {
                            eprintln!("Error: Failed to delete sessions: {}", e);
                        }
                    }
                    return SessionsExitCode::Error;
                }
            }
        }
    }

    // Handle --show flag if specified
    if let Some(ref session_id) = args.show {
        match get_session_info(&runner_config, session_id).await {
            Ok(details) => {
                match args.format {
                    SessionsFormat::Json => {
                        // Serialize with formatted timestamps for JSON output
                        let checkpoints: Vec<serde_json::Value> = details
                            .checkpoints
                            .iter()
                            .map(|cp| {
                                serde_json::json!({
                                    "id": cp.id,
                                    "thread_id": cp.thread_id,
                                    "node": cp.node,
                                    "timestamp": format_timestamp(cp.timestamp),
                                    "parent_id": cp.parent_id,
                                    "metadata": cp.metadata
                                })
                            })
                            .collect();

                        let output = serde_json::json!({
                            "session_id": details.session_id,
                            "checkpoint_count": details.checkpoint_count,
                            "created_at": details.created_at.map(format_timestamp),
                            "latest_update": details.latest_update.map(format_timestamp),
                            "checkpoints": checkpoints
                        });
                        println!("{}", serde_json::to_string_pretty(&output).unwrap());
                    }
                    SessionsFormat::Table => {
                        println!("Session Details: {}\n", details.session_id);
                        println!("Checkpoint Count: {}", details.checkpoint_count);
                        if let Some(created) = details.created_at {
                            let created_str = format_timestamp(created);
                            let display_time = created_str
                                .get(..19)
                                .unwrap_or(&created_str)
                                .replace('T', " ");
                            println!("Created:         {}", display_time);
                        }
                        if let Some(updated) = details.latest_update {
                            let updated_str = format_timestamp(updated);
                            let display_time = updated_str
                                .get(..19)
                                .unwrap_or(&updated_str)
                                .replace('T', " ");
                            println!("Last Updated:    {}", display_time);
                        }

                        if details.checkpoint_count == 0 {
                            println!("\nNo checkpoints found for this session.");
                        } else {
                            println!("\nCheckpoints (newest first):\n");
                            println!(
                                "{:<36}  {:<15}  {:<20}",
                                "CHECKPOINT ID", "NODE", "TIMESTAMP"
                            );
                            println!("{}", "-".repeat(75));
                            for cp in &details.checkpoints {
                                let ts_str = format_timestamp(cp.timestamp);
                                let display_time =
                                    ts_str.get(..19).unwrap_or(&ts_str).replace('T', " ");
                                // Truncate checkpoint ID if too long
                                let id_display = if cp.id.len() > 36 {
                                    format!("{}...", &cp.id[..33])
                                } else {
                                    cp.id.clone()
                                };
                                // Truncate node name if too long
                                let node_display = if cp.node.len() > 15 {
                                    format!("{}...", &cp.node[..12])
                                } else {
                                    cp.node.clone()
                                };
                                println!(
                                    "{:<36}  {:<15}  {:<20}",
                                    id_display, node_display, display_time
                                );
                            }
                        }
                        println!(
                            "\nTo resume this session: codex-dashflow --session {}",
                            details.session_id
                        );
                        println!(
                            "To delete this session: codex-dashflow sessions --delete {} --force",
                            details.session_id
                        );
                    }
                }
                return SessionsExitCode::Success;
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("checkpointing is not enabled")
                    || error_msg.contains("memory checkpointer")
                {
                    match args.format {
                        SessionsFormat::Json => {
                            let output = serde_json::json!({
                                "error": "no_storage",
                                "message": "No checkpoint storage configured. Use --checkpoint-path or configure in config file."
                            });
                            println!("{}", serde_json::to_string_pretty(&output).unwrap());
                        }
                        SessionsFormat::Table => {
                            eprintln!("Error: No checkpoint storage configured.");
                            eprintln!(
                                "Use --checkpoint-path or configure 'checkpoint_path' in your config file."
                            );
                        }
                    }
                    return SessionsExitCode::NoStorage;
                } else {
                    match args.format {
                        SessionsFormat::Json => {
                            let output = serde_json::json!({
                                "error": "show_failed",
                                "message": format!("Failed to get session info: {}", e)
                            });
                            println!("{}", serde_json::to_string_pretty(&output).unwrap());
                        }
                        SessionsFormat::Table => {
                            eprintln!("Error: Failed to get session info: {}", e);
                        }
                    }
                    return SessionsExitCode::Error;
                }
            }
        }
    }

    // List sessions using the core function
    match list_sessions(&runner_config).await {
        Ok(threads) => {
            let sessions: Vec<SessionInfo> = threads
                .into_iter()
                .map(|t| {
                    // Format timestamp as readable string
                    let updated_at = format_timestamp(t.updated_at);

                    SessionInfo {
                        session_id: t.thread_id.to_string(),
                        latest_checkpoint_id: t.latest_checkpoint_id.to_string(),
                        updated_at,
                        checkpoint_count: t.checkpoint_count,
                    }
                })
                .collect();

            match args.format {
                SessionsFormat::Json => {
                    let output = serde_json::json!({
                        "sessions": sessions,
                        "count": sessions.len()
                    });
                    println!("{}", serde_json::to_string_pretty(&output).unwrap());
                }
                SessionsFormat::Table => {
                    if sessions.is_empty() {
                        println!("No saved sessions found.");
                    } else {
                        println!("Saved Sessions ({} found):\n", sessions.len());
                        println!(
                            "{:<36}  {:<20}  {:>6}",
                            "SESSION ID", "LAST UPDATED", "CHECKPOINTS"
                        );
                        println!("{}", "-".repeat(70));
                        for session in &sessions {
                            // Format timestamp for display (just date and time)
                            let display_time = if session.updated_at.len() > 19 {
                                &session.updated_at[..19]
                            } else {
                                &session.updated_at
                            };
                            // Replace T with space for readability
                            let display_time = display_time.replace('T', " ");

                            let count_str = session
                                .checkpoint_count
                                .map(|c| c.to_string())
                                .unwrap_or_else(|| "-".to_string());

                            println!(
                                "{:<36}  {:<20}  {:>6}",
                                session.session_id, display_time, count_str
                            );
                        }
                        println!("\nTo resume a session: codex-dashflow --session <SESSION_ID>");
                    }
                }
            }
            SessionsExitCode::Success
        }
        Err(e) => {
            let error_msg = e.to_string();
            // Check if it's a "no storage" error
            if error_msg.contains("checkpointing is not enabled")
                || error_msg.contains("memory checkpointer")
            {
                match args.format {
                    SessionsFormat::Json => {
                        let output = serde_json::json!({
                            "error": "no_storage",
                            "message": "No checkpoint storage configured. Use --checkpoint-path or configure in config file."
                        });
                        println!("{}", serde_json::to_string_pretty(&output).unwrap());
                    }
                    SessionsFormat::Table => {
                        eprintln!("Error: No checkpoint storage configured.");
                        eprintln!("Use --checkpoint-path or configure 'checkpoint_path' in your config file.");
                    }
                }
                SessionsExitCode::NoStorage
            } else {
                match args.format {
                    SessionsFormat::Json => {
                        let output = serde_json::json!({
                            "error": "list_failed",
                            "message": format!("Failed to list sessions: {}", e)
                        });
                        println!("{}", serde_json::to_string_pretty(&output).unwrap());
                    }
                    SessionsFormat::Table => {
                        eprintln!("Error: Failed to list sessions: {}", e);
                    }
                }
                SessionsExitCode::Error
            }
        }
    }
}

/// Format a SystemTime as a readable ISO 8601 timestamp string
fn format_timestamp(time: std::time::SystemTime) -> String {
    use std::time::UNIX_EPOCH;

    time.duration_since(UNIX_EPOCH)
        .map(|d| {
            // Format as ISO 8601 (YYYY-MM-DDTHH:MM:SS)
            let secs = d.as_secs();
            let days = secs / 86400;
            let time_secs = secs % 86400;
            let hours = time_secs / 3600;
            let mins = (time_secs % 3600) / 60;
            let secs_rem = time_secs % 60;

            // Calculate date from days since epoch (1970-01-01)
            // This is a simplified calculation - good enough for display
            let mut year = 1970i64;
            let mut remaining_days = days as i64;

            loop {
                let days_in_year = if is_leap_year(year) { 366 } else { 365 };
                if remaining_days < days_in_year {
                    break;
                }
                remaining_days -= days_in_year;
                year += 1;
            }

            let (month, day) =
                day_of_year_to_month_day(remaining_days as u32 + 1, is_leap_year(year));

            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                year, month, day, hours, mins, secs_rem
            )
        })
        .unwrap_or_else(|_| "unknown".to_string())
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn day_of_year_to_month_day(day_of_year: u32, is_leap: bool) -> (u32, u32) {
    let days_in_months: [u32; 12] = if is_leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut remaining = day_of_year;
    for (i, &days) in days_in_months.iter().enumerate() {
        if remaining <= days {
            return ((i + 1) as u32, remaining);
        }
        remaining -= days;
    }
    (12, 31) // Fallback
}

/// Execute the doctor subcommand, checking system configuration.
///
/// Returns an exit code indicating the overall health status:
/// - 0: All checks passed
/// - 1: Some checks had warnings but no errors
/// - 2: Some checks had errors
pub async fn run_doctor_command(args: &DoctorArgs, config: &Config) -> DoctorExitCode {
    use std::time::Instant;

    let total_start = Instant::now();

    // Get effective slow threshold from CLI or config
    let slow_threshold = args.effective_slow_threshold(config);

    // Run all checks and collect results
    let raw_checks = run_doctor_checks().await;
    let total_elapsed = total_start.elapsed();

    // Apply slow threshold to each check
    let timed_checks: Vec<TimedDoctorCheckResult> = raw_checks
        .into_iter()
        .map(|c| c.with_slow_threshold(slow_threshold))
        .collect();

    // Count errors, warnings, and slow checks
    let mut errors = 0;
    let mut warnings = 0;
    let mut slow_checks = 0;
    for timed in &timed_checks {
        match timed.result.status {
            DoctorCheckStatus::Ok => {}
            DoctorCheckStatus::Warn => warnings += 1,
            DoctorCheckStatus::Error => errors += 1,
        }
        if timed.slow == Some(true) {
            slow_checks += 1;
        }
    }

    // Determine exit code
    let exit_code = if errors > 0 {
        DoctorExitCode::Errors
    } else if warnings > 0 {
        DoctorExitCode::Warnings
    } else {
        DoctorExitCode::Ok
    };

    // In quiet mode, skip all output
    if args.quiet {
        return exit_code;
    }

    // Output in JSON format if requested
    if args.json {
        let overall_status = match exit_code {
            DoctorExitCode::Ok => "ok",
            DoctorExitCode::Warnings => "warnings",
            DoctorExitCode::Errors => "errors",
        };

        // Build config summary (Audit #24)
        let config_summary = DoctorConfigSummary {
            collect_training: config.collect_training,
            model: config.model.clone(),
            sandbox_mode: config.sandbox_mode.map(|s| format!("{:?}", s)),
            streaming_enabled: config.dashflow.streaming_enabled,
            checkpointing_enabled: config.dashflow.checkpointing_enabled,
            introspection_enabled: config.dashflow.introspection_enabled,
            auto_resume_enabled: config.dashflow.auto_resume,
            auto_resume_max_age_secs: config.dashflow.auto_resume_max_age_secs,
        };

        let output = DoctorJsonOutput {
            version: env!("CARGO_PKG_VERSION").to_string(),
            total_checks: timed_checks.len(),
            errors,
            warnings,
            slow_checks,
            slow_threshold_ms: slow_threshold,
            total_duration_us: total_elapsed.as_micros() as u64,
            checks: timed_checks,
            overall_status: overall_status.to_string(),
            config_summary,
        };

        // Use pretty-printed JSON for readability
        println!(
            "{}",
            serde_json::to_string_pretty(&output)
                .unwrap_or_else(|e| format!(r#"{{"error": "Failed to serialize JSON: {}"}}"#, e))
        );
        return exit_code;
    }

    // Human-readable output
    println!("{}", "Checking system configuration...".bold());
    println!();

    let threshold_us = slow_threshold * 1000; // Convert ms to us

    for timed in &timed_checks {
        let check = &timed.result;
        let is_slow = timed.duration_us >= threshold_us;

        let (icon, colored_icon) = match check.status {
            DoctorCheckStatus::Ok => ("✓", "✓".green().bold()),
            DoctorCheckStatus::Warn => ("!", "!".yellow().bold()),
            DoctorCheckStatus::Error => ("✗", "✗".red().bold()),
        };

        // Use colored icon for terminal, plain for non-terminal
        let display_icon = if colored::control::SHOULD_COLORIZE.should_colorize() {
            format!("[{}]", colored_icon)
        } else {
            format!("[{}]", icon)
        };

        let name_display = match check.status {
            DoctorCheckStatus::Ok => check.name.green(),
            DoctorCheckStatus::Warn => check.name.yellow(),
            DoctorCheckStatus::Error => check.name.red(),
        };

        // Format timing for verbose mode, with slow indicator
        let timing_str = if args.verbose {
            let base_timing = format_check_duration(timed.duration_us);
            if is_slow {
                format!("{} {}", base_timing, "[SLOW]".yellow().bold())
            } else {
                base_timing
            }
        } else {
            String::new()
        };

        if args.verbose {
            println!(
                "{:8} {}: {} {}",
                display_icon,
                name_display,
                check.message,
                if is_slow {
                    timing_str
                } else {
                    timing_str.dimmed().to_string()
                }
            );
        } else {
            println!("{:8} {}: {}", display_icon, name_display, check.message);
        }
    }

    println!();
    if errors > 0 {
        println!(
            "{}",
            format!(
                "Found {} error(s) and {} warning(s). Please fix errors to use codex-dashflow.",
                errors, warnings
            )
            .red()
            .bold()
        );
    } else if warnings > 0 {
        println!(
            "{}",
            format!(
                "All checks passed with {} warning(s). codex-dashflow is ready to use.",
                warnings
            )
            .yellow()
        );
    } else {
        println!(
            "{}",
            "All checks passed. codex-dashflow is ready to use."
                .green()
                .bold()
        );
    }

    // Show configuration summary (Audit #24: collect_training visibility)
    println!();
    println!("{}", "[Configuration]".bold());
    println!(
        "  Collect training:  {}",
        format_bool(config.collect_training)
    );
    println!("  Model:             {}", config.model.cyan());
    if let Some(sandbox) = config.sandbox_mode {
        println!("  Sandbox mode:      {}", format!("{:?}", sandbox).cyan());
    }
    println!(
        "  Streaming:         {}",
        format_bool(config.dashflow.streaming_enabled)
    );
    println!(
        "  Checkpointing:     {}",
        format_bool(config.dashflow.checkpointing_enabled)
    );
    println!(
        "  Introspection:     {}",
        format_bool(config.dashflow.introspection_enabled)
    );
    println!(
        "  Auto-Resume:       {}",
        format_bool(config.dashflow.auto_resume)
    );
    if let Some(max_age) = config.dashflow.auto_resume_max_age_secs {
        println!(
            "  Auto-Resume Max Age: {}",
            format_duration_human(max_age).cyan()
        );
    }

    // Show slow check summary in verbose mode
    if args.verbose && slow_checks > 0 {
        println!(
            "{}",
            format!(
                "{} check(s) exceeded {}ms threshold.",
                slow_checks, slow_threshold
            )
            .yellow()
        );
    }

    // Show total time in verbose mode
    if args.verbose {
        println!();
        println!(
            "{}",
            format!("Total time: {:.2}ms", total_elapsed.as_secs_f64() * 1000.0).dimmed()
        );
    }

    exit_code
}

/// Run all doctor checks and return timed results
async fn run_doctor_checks() -> Vec<TimedDoctorCheckResult> {
    use std::time::Instant;

    let mut timed_checks = Vec::new();

    // Authentication Status (checks stored credentials or env var)
    let start = Instant::now();
    let result = check_auth_status();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // Config File
    let start = Instant::now();
    let result = check_config_file();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // Environment Variables
    let start = Instant::now();
    let result = check_environment_variables();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // Working Directory
    let start = Instant::now();
    let result = check_working_directory();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // Disk Space
    let start = Instant::now();
    let result = check_disk_space();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // Memory Usage
    let start = Instant::now();
    let result = check_memory_usage();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // Shell Access
    let start = Instant::now();
    let result = check_shell_access();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // Sandbox Availability (Audit #64, #67)
    let start = Instant::now();
    let result = check_sandbox_availability();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // Protoc Availability (Audit #79)
    let start = Instant::now();
    let result = check_protoc_availability();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // Network Connectivity (DNS)
    let start = Instant::now();
    let result = check_network_connectivity();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // API Connectivity (HTTP) - async
    let start = Instant::now();
    let result = check_api_connectivity_async().await;
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // Git Repository
    let start = Instant::now();
    let result = check_git_repository();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // Rust Toolchain
    let start = Instant::now();
    let result = check_rust_version();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // Training Data
    let start = Instant::now();
    let result = check_training_data();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    // Prompt Registry
    let start = Instant::now();
    let result = check_prompt_registry();
    timed_checks.push(TimedDoctorCheckResult::new(result, start.elapsed()));

    timed_checks
}

/// Format a duration in microseconds to a human-readable string
fn format_check_duration(us: u64) -> String {
    if us < 1000 {
        format!("({}µs)", us)
    } else if us < 1_000_000 {
        format!("({:.2}ms)", us as f64 / 1000.0)
    } else {
        format!("({:.2}s)", us as f64 / 1_000_000.0)
    }
}

fn check_auth_status() -> DoctorCheckResult {
    use codex_dashflow_core::{AuthCredentialsStoreMode, AuthManager, AuthStatus};

    // Try to get auth status from stored credentials
    match AuthManager::new(AuthCredentialsStoreMode::Auto) {
        Ok(auth) => {
            let status = auth.get_status();
            match status {
                AuthStatus::ChatGpt { email: Some(e) } => DoctorCheckResult::ok(
                    "Authentication",
                    format!("Signed in with ChatGPT ({})", e),
                ),
                AuthStatus::ChatGpt { email: None } => {
                    DoctorCheckResult::ok("Authentication", "Signed in with ChatGPT")
                }
                AuthStatus::ApiKey => {
                    DoctorCheckResult::ok("Authentication", "Using stored API key")
                }
                AuthStatus::EnvApiKey => {
                    DoctorCheckResult::ok("Authentication", "Using OPENAI_API_KEY from environment")
                }
                AuthStatus::NotAuthenticated => DoctorCheckResult::error(
                    "Authentication",
                    "Not authenticated. Run 'codex-dashflow login' or set OPENAI_API_KEY",
                ),
            }
        }
        Err(_) => {
            // Fall back to checking env var
            if std::env::var("OPENAI_API_KEY").is_ok() {
                DoctorCheckResult::ok("Authentication", "OPENAI_API_KEY is set")
            } else {
                DoctorCheckResult::error(
                    "Authentication",
                    "Not authenticated. Run 'codex-dashflow login' or set OPENAI_API_KEY",
                )
            }
        }
    }
}

fn check_config_file() -> DoctorCheckResult {
    let config_path = dirs::home_dir()
        .map(|h| h.join(".codex-dashflow").join("config.toml"))
        .unwrap_or_default();

    if config_path.exists() {
        match Config::load_from_path(&config_path) {
            Ok(config) => {
                // Validate the config for semantic issues
                let validation = config.validate();
                format_config_validation_result(&config_path, &validation)
            }
            Err(e) => DoctorCheckResult::error("Config File", format!("Parse error: {}", e)),
        }
    } else {
        DoctorCheckResult::ok("Config File", "Not found (using defaults)")
    }
}

/// Format config validation result as a doctor check result
fn format_config_validation_result(
    config_path: &std::path::Path,
    validation: &ConfigValidationResult,
) -> DoctorCheckResult {
    if validation.is_valid() && !validation.has_warnings() {
        DoctorCheckResult::ok("Config File", format!("Valid at {}", config_path.display()))
    } else if validation.is_valid() && validation.has_warnings() {
        // Config is valid but has warnings
        let warning_count = validation.warning_count();
        let first_warning = validation.warnings().next();
        let msg = if warning_count == 1 {
            if let Some(w) = first_warning {
                format!(
                    "Valid with 1 warning at {}: {}",
                    config_path.display(),
                    w.message
                )
            } else {
                format!("Valid with 1 warning at {}", config_path.display())
            }
        } else {
            let details: Vec<String> = validation
                .warnings()
                .take(2)
                .map(|w| format!("{}: {}", w.field, w.message))
                .collect();
            format!(
                "Valid with {} warnings at {}: {}{}",
                warning_count,
                config_path.display(),
                details.join("; "),
                if warning_count > 2 { "; ..." } else { "" }
            )
        };
        DoctorCheckResult::warn("Config File", msg)
    } else {
        // Config has errors
        let error_count = validation.error_count();
        let first_error = validation.errors().next();
        let msg = if let Some(e) = first_error {
            format!(
                "{} error(s) at {}: {}: {}",
                error_count,
                config_path.display(),
                e.field,
                e.message
            )
        } else {
            format!("{} error(s) at {}", error_count, config_path.display())
        };
        DoctorCheckResult::error("Config File", msg)
    }
}

fn check_working_directory() -> DoctorCheckResult {
    match std::env::current_dir() {
        Ok(path) => {
            if path.exists() && path.is_dir() {
                DoctorCheckResult::ok("Working Directory", format!("{}", path.display()))
            } else {
                DoctorCheckResult::error("Working Directory", "Current directory not accessible")
            }
        }
        Err(e) => DoctorCheckResult::error("Working Directory", format!("Cannot determine: {}", e)),
    }
}

fn check_training_data() -> DoctorCheckResult {
    let training_path = dirs::data_dir()
        .map(|d| d.join("codex-dashflow").join("training.json"))
        .unwrap_or_default();

    if training_path.exists() {
        match TrainingData::load(&training_path) {
            Ok(data) => DoctorCheckResult::ok(
                "Training Data",
                format!(
                    "{} examples at {}",
                    data.examples.len(),
                    training_path.display()
                ),
            ),
            Err(e) => DoctorCheckResult::warn(
                "Training Data",
                format!("File exists but error loading: {}", e),
            ),
        }
    } else {
        DoctorCheckResult::ok("Training Data", "Not found (none collected yet)")
    }
}

fn check_prompt_registry() -> DoctorCheckResult {
    let prompts_path = dirs::data_dir()
        .map(|d| d.join("codex-dashflow").join("prompts.json"))
        .unwrap_or_default();

    if prompts_path.exists() {
        match PromptRegistry::load(&prompts_path) {
            Ok(registry) => {
                let count = registry.prompts.len();
                DoctorCheckResult::ok(
                    "Prompt Registry",
                    format!("{} prompts at {}", count, prompts_path.display()),
                )
            }
            Err(e) => DoctorCheckResult::warn(
                "Prompt Registry",
                format!("File exists but error loading: {}", e),
            ),
        }
    } else {
        DoctorCheckResult::ok("Prompt Registry", "Not found (using defaults)")
    }
}

fn check_disk_space() -> DoctorCheckResult {
    let check_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Use statvfs on Unix systems to get disk space info
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let path_cstr = CString::new(check_path.as_os_str().as_bytes()).unwrap_or_default();

        unsafe {
            let mut stat: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(path_cstr.as_ptr(), &mut stat) == 0 {
                let block_size = stat.f_frsize;
                let available_bytes = stat.f_bavail as u64 * block_size;
                let available_gb = available_bytes / (1024 * 1024 * 1024);
                let available_mb = (available_bytes / (1024 * 1024)) % 1024;

                // Warn if less than 1GB, error if less than 100MB
                if available_bytes < 100 * 1024 * 1024 {
                    return DoctorCheckResult::error(
                        "Disk Space",
                        format!(
                            "Only {} MB available (minimum 100 MB recommended)",
                            available_bytes / (1024 * 1024)
                        ),
                    );
                } else if available_bytes < 1024 * 1024 * 1024 {
                    return DoctorCheckResult::warn(
                        "Disk Space",
                        format!(
                            "{} MB available (1 GB+ recommended)",
                            available_bytes / (1024 * 1024)
                        ),
                    );
                } else {
                    return DoctorCheckResult::ok(
                        "Disk Space",
                        format!("{}.{} GB available", available_gb, available_mb / 100),
                    );
                }
            }
        }
    }

    // Fallback for non-Unix or if statvfs fails
    DoctorCheckResult::ok("Disk Space", "Check not available on this platform")
}

fn check_rust_version() -> DoctorCheckResult {
    // Get the Rust version that compiled this binary
    let rust_version = env!("CARGO_PKG_RUST_VERSION", "unknown");

    // Check if we're running on a system with rustc available
    match std::process::Command::new("rustc")
        .arg("--version")
        .output()
    {
        Ok(output) if output.status.success() => {
            let version_str = String::from_utf8_lossy(&output.stdout);
            let version = version_str.trim();
            DoctorCheckResult::ok("Rust Toolchain", version.to_string())
        }
        _ => {
            if rust_version != "unknown" {
                DoctorCheckResult::ok(
                    "Rust Toolchain",
                    format!("Compiled with Rust {} (rustc not in PATH)", rust_version),
                )
            } else {
                DoctorCheckResult::ok(
                    "Rust Toolchain",
                    "Not required (pre-compiled binary)".to_string(),
                )
            }
        }
    }
}

fn check_shell_access() -> DoctorCheckResult {
    // Check if we can spawn a shell process
    let shell = if cfg!(windows) { "cmd" } else { "sh" };
    let args: &[&str] = if cfg!(windows) {
        &["/c", "echo ok"]
    } else {
        &["-c", "echo ok"]
    };

    match std::process::Command::new(shell).args(args).output() {
        Ok(output) if output.status.success() => {
            let shell_name = if cfg!(windows) { "cmd.exe" } else { "sh" };
            DoctorCheckResult::ok("Shell Access", format!("{} available", shell_name))
        }
        Ok(_) => {
            DoctorCheckResult::warn("Shell Access", "Shell command returned non-zero exit code")
        }
        Err(e) => DoctorCheckResult::error("Shell Access", format!("Cannot spawn shell: {}", e)),
    }
}

/// Check sandbox availability (Audit #64, #67)
///
/// Verifies that platform-specific sandbox support is available.
/// On macOS this checks for Seatbelt, on Linux for Landlock.
/// On Windows or other platforms, warns that sandboxing is not supported.
fn check_sandbox_availability() -> DoctorCheckResult {
    // Platform-specific checks
    #[cfg(target_os = "macos")]
    {
        if SandboxExecutor::is_available() {
            DoctorCheckResult::ok("Sandbox", "Seatbelt available (macOS sandbox)")
        } else {
            DoctorCheckResult::warn(
                "Sandbox",
                "Seatbelt binary not found at /usr/bin/sandbox-exec. \
                 Shell commands will run without sandbox protection. \
                 Ensure macOS sandbox-exec is available or use --sandbox danger-full-access",
            )
        }
    }

    #[cfg(target_os = "linux")]
    {
        if SandboxExecutor::is_available() {
            DoctorCheckResult::ok("Sandbox", "Landlock available (Linux sandbox)")
        } else {
            DoctorCheckResult::warn(
                "Sandbox",
                "Landlock not available (requires Linux kernel 5.13+). \
                 Shell commands will run without sandbox protection. \
                 Consider using a container or --sandbox danger-full-access",
            )
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Audit #67: Windows-specific warning
        DoctorCheckResult::warn(
            "Sandbox",
            "Sandbox not supported on Windows. \
             Shell commands will run without sandbox protection. \
             Consider running in a container or VM for added security",
        )
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        DoctorCheckResult::warn(
            "Sandbox",
            format!(
                "Sandbox not supported on {} platform. \
                 Shell commands will run without sandbox protection",
                std::env::consts::OS
            ),
        )
    }
}

/// Check protoc availability for dashstream feature (Audit #79)
///
/// The dashstream feature requires protoc (Protocol Buffers compiler) to be
/// installed for building DashFlow streaming support. This check warns when:
/// - The dashstream feature is enabled but protoc is not available
/// - Protoc is available but the dashstream feature is not compiled in
fn check_protoc_availability() -> DoctorCheckResult {
    // Check if protoc is available in PATH
    let protoc_available = which::which("protoc").is_ok();

    // Check if dashstream feature is compiled in
    #[cfg(feature = "dashstream")]
    let dashstream_compiled = true;
    #[cfg(not(feature = "dashstream"))]
    let dashstream_compiled = false;

    match (protoc_available, dashstream_compiled) {
        (true, true) => {
            // Get protoc version for display
            let version = std::process::Command::new("protoc")
                .arg("--version")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "unknown version".to_string());
            DoctorCheckResult::ok(
                "Protoc",
                format!("Available ({}) with dashstream feature enabled", version),
            )
        }
        (false, true) => {
            // Feature enabled but protoc missing - this is an error
            DoctorCheckResult::error(
                "Protoc",
                "Not found but dashstream feature is enabled. \
                 Install protoc (Protocol Buffers compiler) or rebuild without --features dashstream. \
                 On macOS: brew install protobuf, on Ubuntu/Debian: apt install protobuf-compiler",
            )
        }
        (true, false) => {
            // Protoc available but feature not compiled - informational
            DoctorCheckResult::ok(
                "Protoc",
                "Available (dashstream feature not compiled in - rebuild with --features dashstream to enable streaming)",
            )
        }
        (false, false) => {
            // Neither available - ok since feature is not needed
            DoctorCheckResult::ok(
                "Protoc",
                "Not found (not required unless --features dashstream is enabled)",
            )
        }
    }
}

fn check_network_connectivity() -> DoctorCheckResult {
    // Perform a synchronous DNS lookup to check network connectivity
    // We check if we can resolve api.openai.com
    use std::net::ToSocketAddrs;

    match "api.openai.com:443".to_socket_addrs() {
        Ok(mut addrs) => {
            if addrs.next().is_some() {
                DoctorCheckResult::ok("Network", "Can resolve api.openai.com")
            } else {
                DoctorCheckResult::warn("Network", "DNS resolved but no addresses returned")
            }
        }
        Err(e) => DoctorCheckResult::warn(
            "Network",
            format!(
                "Cannot resolve api.openai.com: {} (network may be unavailable)",
                e
            ),
        ),
    }
}

/// Check HTTP connectivity to OpenAI API by making a HEAD request.
/// This verifies that we can actually reach the API endpoint, not just resolve DNS.
pub async fn check_api_connectivity_async() -> DoctorCheckResult {
    use std::time::Duration;

    // Build a minimal client with a short timeout
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return DoctorCheckResult::warn(
                "API Connectivity",
                format!("Cannot create HTTP client: {}", e),
            );
        }
    };

    // Make a HEAD request to the API (doesn't require auth)
    // We use the models endpoint which returns 401 without auth, proving connectivity
    match client.head("https://api.openai.com/v1/models").send().await {
        Ok(response) => {
            let status = response.status();
            if status.is_success() || status.as_u16() == 401 {
                // 401 is expected without auth - proves connectivity works
                DoctorCheckResult::ok(
                    "API Connectivity",
                    format!("Can reach api.openai.com (HTTP {})", status.as_u16()),
                )
            } else if status.is_server_error() {
                DoctorCheckResult::warn(
                    "API Connectivity",
                    format!("API returned server error (HTTP {})", status.as_u16()),
                )
            } else {
                // Other status codes (3xx, 4xx except 401) indicate connectivity works
                DoctorCheckResult::ok(
                    "API Connectivity",
                    format!("Can reach api.openai.com (HTTP {})", status.as_u16()),
                )
            }
        }
        Err(e) => {
            if e.is_timeout() {
                DoctorCheckResult::warn("API Connectivity", "Request timed out (5s)")
            } else if e.is_connect() {
                DoctorCheckResult::warn("API Connectivity", format!("Connection failed: {}", e))
            } else {
                DoctorCheckResult::warn("API Connectivity", format!("HTTP request failed: {}", e))
            }
        }
    }
}

fn check_git_repository() -> DoctorCheckResult {
    // Check if current directory is a git repository and get status info
    match std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
    {
        Ok(output) if output.status.success() => {
            // We're in a git repo, get more details
            let branch = std::process::Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            // Check for uncommitted changes
            let status_output = std::process::Command::new("git")
                .args(["status", "--porcelain"])
                .output();

            let (changes_count, has_changes) = match status_output {
                Ok(output) if output.status.success() => {
                    let status = String::from_utf8_lossy(&output.stdout);
                    let count = status.lines().count();
                    (count, count > 0)
                }
                _ => (0, false),
            };

            if has_changes {
                DoctorCheckResult::ok(
                    "Git Repository",
                    format!(
                        "On branch '{}' ({} uncommitted changes)",
                        branch, changes_count
                    ),
                )
            } else {
                DoctorCheckResult::ok("Git Repository", format!("On branch '{}' (clean)", branch))
            }
        }
        Ok(_) => {
            // git command succeeded but we're not in a repo
            DoctorCheckResult::ok("Git Repository", "Not a git repository (that's OK)")
        }
        Err(_) => {
            // git command not found
            DoctorCheckResult::warn("Git Repository", "git not found in PATH")
        }
    }
}

fn check_memory_usage() -> DoctorCheckResult {
    // Get current process memory usage
    #[cfg(unix)]
    {
        // Use /proc/self/statm on Linux or similar approaches
        #[cfg(target_os = "linux")]
        {
            if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
                let parts: Vec<&str> = statm.split_whitespace().collect();
                if parts.len() >= 2 {
                    // First value is total program size, second is resident set size
                    // Values are in pages (typically 4KB)
                    if let Ok(rss_pages) = parts[1].parse::<u64>() {
                        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 };
                        let rss_mb = (rss_pages * page_size) / (1024 * 1024);
                        return DoctorCheckResult::ok(
                            "Memory",
                            format!("{} MB resident memory", rss_mb),
                        );
                    }
                }
            }
        }

        // macOS fallback - use rusage
        #[cfg(target_os = "macos")]
        {
            unsafe {
                let mut rusage: libc::rusage = std::mem::zeroed();
                if libc::getrusage(libc::RUSAGE_SELF, &mut rusage) == 0 {
                    // ru_maxrss is in bytes on macOS
                    let rss_mb = rusage.ru_maxrss / (1024 * 1024);
                    return DoctorCheckResult::ok("Memory", format!("{} MB peak memory", rss_mb));
                }
            }
        }
    }

    // Fallback for other platforms or if checks fail
    DoctorCheckResult::ok("Memory", "Check not available on this platform")
}

fn check_environment_variables() -> DoctorCheckResult {
    // Check for relevant environment variables that affect codex-dashflow
    let mut set_vars = Vec::new();
    let mut unset_vars = Vec::new();

    // Core environment variables
    // Note: OPENAI_API_KEY is optional since auth can come from stored credentials
    let important_vars = [
        ("OPENAI_API_KEY", false),  // Optional - auth can use stored credentials
        ("OPENAI_BASE_URL", false), // Optional - custom endpoint
        ("OPENAI_ORG_ID", false),   // Optional - organization
        ("CODEX_DASHFLOW_CONFIG", false), // Optional - config path
        ("RUST_LOG", false),        // Optional - logging level
        ("NO_COLOR", false),        // Optional - disable colors
    ];

    for (var, required) in important_vars {
        if std::env::var(var).is_ok() {
            set_vars.push(var);
        } else if required {
            unset_vars.push(var);
        }
    }

    // Build result message
    if set_vars.is_empty() && !unset_vars.is_empty() {
        DoctorCheckResult::warn(
            "Environment",
            format!("Missing required: {}", unset_vars.join(", ")),
        )
    } else if !set_vars.is_empty() {
        let msg = if unset_vars.is_empty() {
            format!("{} variable(s) set", set_vars.len())
        } else {
            format!(
                "{} set (missing: {})",
                set_vars.len(),
                unset_vars.join(", ")
            )
        };
        DoctorCheckResult::ok("Environment", msg)
    } else {
        DoctorCheckResult::ok("Environment", "No relevant variables set (using defaults)")
    }
}

// ============================================================================
// MCP Server Subcommand Implementation
// ============================================================================

/// Execute the mcp-server subcommand
pub async fn run_mcp_server_command(args: &McpServerArgs) -> anyhow::Result<()> {
    use codex_dashflow_mcp_server::{run_mcp_server, McpServerConfig};

    let working_dir = args
        .working_dir
        .clone()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let config = McpServerConfig::default()
        .with_working_dir(working_dir)
        .with_sandbox_mode(args.sandbox.into())
        .with_mock_llm(args.mock);

    run_mcp_server(config)
        .await
        .map_err(|e| anyhow::anyhow!("MCP server error: {}", e))
}

// ============================================================================
// Optimize Subcommand Implementation
// ============================================================================

/// Execute the optimize subcommand
pub fn run_optimize_command(args: &OptimizeArgs) -> anyhow::Result<()> {
    match &args.action {
        OptimizeAction::Run {
            few_shot_count,
            min_score: _,
            training_file,
            prompts_file,
        } => run_optimize(
            *few_shot_count,
            training_file.as_ref(),
            prompts_file.as_ref(),
        ),
        OptimizeAction::Stats { training_file } => show_training_stats(training_file.as_ref()),
        OptimizeAction::Add {
            input,
            output,
            score,
            tools,
            training_file,
        } => add_training_example(
            input,
            output,
            *score,
            tools.as_ref(),
            training_file.as_ref(),
        ),
        OptimizeAction::Show { prompts_file } => show_prompts(prompts_file.as_ref()),
    }
}

/// Run prompt optimization
fn run_optimize(
    few_shot_count: usize,
    training_file: Option<&PathBuf>,
    prompts_file: Option<&PathBuf>,
) -> anyhow::Result<()> {
    // Load training data
    let training = if let Some(path) = training_file {
        TrainingData::load(path)
            .map_err(|e| anyhow::anyhow!("Failed to load training data: {}", e))?
    } else {
        TrainingData::load_default()
            .map_err(|e| anyhow::anyhow!("Failed to load training data: {}", e))?
    };

    if training.is_empty() {
        println!("No training data found. Add examples with 'codex-dashflow optimize add'");
        return Ok(());
    }

    // Load prompt registry
    let mut registry = if let Some(path) = prompts_file {
        PromptRegistry::load(path).unwrap_or_else(|_| PromptRegistry::with_defaults())
    } else {
        PromptRegistry::load_default().unwrap_or_else(|_| PromptRegistry::with_defaults())
    };

    // Create config
    let config = OptimizeConfig {
        few_shot_count,
        ..Default::default()
    };

    // Run optimization
    println!(
        "Running optimization with {} training examples...",
        training.len()
    );
    let result = optimize_prompts(&mut registry, &training, &config)
        .map_err(|e| anyhow::anyhow!("Optimization failed: {}", e))?;

    // Save optimized prompts
    if let Some(path) = prompts_file {
        registry
            .save(path)
            .map_err(|e| anyhow::anyhow!("Failed to save prompts: {}", e))?;
        println!("Saved optimized prompts to: {}", path.display());
    } else {
        registry
            .save_default()
            .map_err(|e| anyhow::anyhow!("Failed to save prompts: {}", e))?;
        println!("Saved optimized prompts to default location");
    }

    // Print results
    println!("\nOptimization Results:");
    println!("  Initial score: {:.2}", result.initial_score);
    println!("  Final score:   {:.2}", result.final_score);
    println!(
        "  Improvement:   {:.2} ({:.1}%)",
        result.improvement,
        result.improvement_percent()
    );
    println!("  Examples generated: {}", result.examples_generated);
    println!("  Duration: {:.2}s", result.duration_secs);

    Ok(())
}

/// Show training data statistics
fn show_training_stats(training_file: Option<&PathBuf>) -> anyhow::Result<()> {
    let training = if let Some(path) = training_file {
        TrainingData::load(path)
            .map_err(|e| anyhow::anyhow!("Failed to load training data: {}", e))?
    } else {
        TrainingData::load_default()
            .map_err(|e| anyhow::anyhow!("Failed to load training data: {}", e))?
    };

    if training.is_empty() {
        println!("No training data found.");
        println!("Add examples with 'codex-dashflow optimize add -i <input> -o <output>'");
        return Ok(());
    }

    // Compute statistics
    let total = training.len();
    let avg_score = training.average_score();
    let high_quality = training.filter_by_score(0.7).len();
    let top_examples = training.top_examples(3);

    println!("Training Data Statistics:");
    println!("  Total examples: {}", total);
    println!("  Average score:  {:.2}", avg_score);
    println!("  High-quality (>=0.7): {}", high_quality);

    // Score distribution
    let low = training.examples.iter().filter(|e| e.score < 0.5).count();
    let medium = training
        .examples
        .iter()
        .filter(|e| e.score >= 0.5 && e.score < 0.7)
        .count();
    let high = training.examples.iter().filter(|e| e.score >= 0.7).count();
    println!("\nScore Distribution:");
    println!("  Low (<0.5):    {}", low);
    println!("  Medium (0.5-0.7): {}", medium);
    println!("  High (>=0.7):  {}", high);

    // Top examples
    if !top_examples.is_empty() {
        println!("\nTop Examples:");
        for (i, ex) in top_examples.iter().enumerate() {
            let input_preview = if ex.user_input.len() > 50 {
                format!("{}...", &ex.user_input[..50])
            } else {
                ex.user_input.clone()
            };
            println!("  {}. [score: {:.2}] {}", i + 1, ex.score, input_preview);
        }
    }

    Ok(())
}

/// Add a training example
fn add_training_example(
    input: &str,
    output: &str,
    score: f64,
    tools: Option<&String>,
    training_file: Option<&PathBuf>,
) -> anyhow::Result<()> {
    // Validate score
    if !(0.0..=1.0).contains(&score) {
        return Err(anyhow::anyhow!("Score must be between 0.0 and 1.0"));
    }

    // Load existing training data
    let mut training = if let Some(path) = training_file {
        TrainingData::load(path).unwrap_or_else(|_| TrainingData::new())
    } else {
        TrainingData::load_default().unwrap_or_else(|_| TrainingData::new())
    };

    // Parse tools if provided
    let tool_calls: Vec<String> = tools
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Add the example
    let example = TrainingExample::new(input, output, score).with_tool_calls(tool_calls.clone());
    training.examples.push(example);

    // Save
    if let Some(path) = training_file {
        training
            .save(path)
            .map_err(|e| anyhow::anyhow!("Failed to save training data: {}", e))?;
        println!("Added training example to: {}", path.display());
    } else {
        training
            .save_default()
            .map_err(|e| anyhow::anyhow!("Failed to save training data: {}", e))?;
        println!("Added training example to default location");
    }

    println!("  Input: {}", input);
    println!("  Output: {}", output);
    println!("  Score: {}", score);
    if !tool_calls.is_empty() {
        println!("  Tools: {}", tool_calls.join(", "));
    }
    println!("Total examples: {}", training.len());

    Ok(())
}

/// Show current optimized prompts
fn show_prompts(prompts_file: Option<&PathBuf>) -> anyhow::Result<()> {
    let registry = if let Some(path) = prompts_file {
        PromptRegistry::load(path).map_err(|e| anyhow::anyhow!("Failed to load prompts: {}", e))?
    } else {
        PromptRegistry::load_default().unwrap_or_else(|_| PromptRegistry::with_defaults())
    };

    println!("Prompt Registry (version {}):", registry.version);
    println!("  Registered prompts: {}", registry.prompts.len());

    for (name, config) in &registry.prompts {
        println!("\n[{}]", name);

        // Show instruction preview
        let instruction_preview = if config.instruction.len() > 100 {
            format!("{}...", &config.instruction[..100])
        } else {
            config.instruction.clone()
        };
        println!("  Instruction: {}", instruction_preview);

        // Show few-shot examples
        if !config.few_shot_examples.is_empty() {
            println!("  Few-shot examples: {}", config.few_shot_examples.len());
            for (i, ex) in config.few_shot_examples.iter().enumerate() {
                let input_preview = if ex.user_input.len() > 40 {
                    format!("{}...", &ex.user_input[..40])
                } else {
                    ex.user_input.clone()
                };
                println!("    {}. [score: {:.2}] {}", i + 1, ex.score, input_preview);
            }
        }

        // Show metadata if optimization has been run
        if config.metadata.optimizer.is_some() {
            println!("  Optimization:");
            println!(
                "    Optimizer: {}",
                config.metadata.optimizer.as_ref().unwrap()
            );
            println!("    Best score: {:.2}", config.metadata.best_score);
            println!("    Iterations: {}", config.metadata.iterations);
            println!("    Training size: {}", config.metadata.training_size);
        }
    }

    Ok(())
}

/// Load configuration, merging file config with CLI args
pub fn load_config(args: &Args) -> Config {
    // Load from file if specified, otherwise try default path
    let file_config = if let Some(ref path) = args.config {
        load_config_with_detailed_errors(path)
    } else {
        Config::load().unwrap_or_else(|e| {
            tracing::debug!("Using default config: {}", e);
            Config::default()
        })
    };

    file_config
}

/// Load config from a specific path with detailed error messages for TOML syntax errors
fn load_config_with_detailed_errors(path: &std::path::Path) -> Config {
    use codex_dashflow_core::config::validate_toml_syntax;

    // First try to read the file
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "Warning: Failed to read config from {}: {}",
                path.display(),
                e
            );
            return Config::default();
        }
    };

    // Try to parse with detailed error messages
    if let Err(detail) = validate_toml_syntax(&content) {
        eprintln!("Warning: Config file has syntax errors: {}", path.display());
        eprintln!("{}", detail);
        eprintln!("\nUsing default configuration instead.");
        return Config::default();
    }

    // If validation passed, actually load the config
    Config::load_from_path(path).unwrap_or_else(|e| {
        eprintln!(
            "Warning: Failed to load config from {}: {}",
            path.display(),
            e
        );
        Config::default()
    })
}

/// Resolved configuration after merging CLI args with file config
#[derive(Debug, Clone, Default)]
pub struct ResolvedConfig {
    pub model: String,
    pub max_turns: u32,
    pub working_dir: String,
    pub session_id: Option<String>,
    pub use_mock_llm: bool,
    pub verbose: bool,
    /// Suppress all non-essential output (only print final result)
    pub quiet: bool,
    /// Show resolved configuration and exit without running the agent
    pub dry_run: bool,
    /// Validate configuration and exit with status
    pub check: bool,
    /// Output in JSON format (for use with --check or --dry-run)
    pub json: bool,
    /// Read prompt from stdin (for multiline prompts or piping)
    pub stdin: bool,
    /// Path to file containing prompt
    pub prompt_file: Option<PathBuf>,
    pub exec_prompt: Option<String>,
    /// DashFlow Streaming configuration
    pub dashstream: Option<DashStreamConfig>,
    /// Tool approval mode (CLI overrides file config)
    pub approval_mode: ApprovalMode,
    /// Policy configuration from config file
    pub policy_config: PolicyConfig,
    /// Whether to collect training data from successful runs
    pub collect_training: bool,
    /// Whether to load optimized prompts from PromptRegistry
    pub load_optimized_prompts: bool,
    /// Custom system prompt (overrides default and optimized prompts)
    pub system_prompt: Option<String>,
    /// Path to file containing system prompt
    pub system_prompt_file: Option<PathBuf>,
    /// Sandbox mode for command execution
    pub sandbox_mode: SandboxMode,
    /// Additional writable roots for sandbox (Audit #70)
    pub sandbox_writable_roots: Vec<PathBuf>,
    /// PostgreSQL connection string for session checkpointing
    pub postgres: Option<String>,
    /// OpenAI API base URL (audit item #16)
    pub api_base: String,
    /// Whether streaming telemetry is enabled (audit item #14)
    pub streaming_enabled: bool,
    /// Whether checkpointing is enabled (audit item #14)
    pub checkpointing_enabled: bool,
    /// Path for file-based checkpointing (audit item #15)
    pub checkpoint_path: Option<PathBuf>,
    /// MCP server configurations (audit item #17, #22, #90)
    pub mcp_servers: Vec<codex_dashflow_mcp::McpServerConfig>,
    /// Whether AI introspection is enabled (graph manifest in system prompt)
    pub introspection_enabled: bool,
    /// Whether auto-resume of most recent session is enabled
    pub auto_resume_enabled: bool,
    /// Maximum age in seconds for auto-resume sessions (None = no limit)
    pub auto_resume_max_age_secs: Option<u64>,
}

/// DashFlow Streaming configuration
#[derive(Debug, Clone)]
pub struct DashStreamConfig {
    pub bootstrap_servers: String,
    pub topic: String,
}

/// Merge CLI arguments with file configuration
/// CLI args take precedence over file config
pub fn resolve_config(args: &Args, file_config: &Config) -> ResolvedConfig {
    // Build DashStream config if enabled (Audit #71: CLI args override file config)
    // Priority: CLI --dashstream flags > config file kafka_bootstrap_servers
    let dashstream = if args.dashstream {
        // CLI flag explicitly enabled
        args.dashstream_bootstrap
            .clone()
            .map(|bootstrap_servers| DashStreamConfig {
                bootstrap_servers,
                topic: args.dashstream_topic.clone(),
            })
    } else {
        // Fall back to config file kafka settings
        file_config
            .dashflow
            .kafka_bootstrap_servers
            .as_ref()
            .map(|bootstrap| DashStreamConfig {
                bootstrap_servers: bootstrap.clone(),
                topic: file_config.dashflow.kafka_topic.clone(),
            })
    };

    // CLI approval mode overrides file config's approval mode
    // Start with file config's policy, then apply CLI override for approval mode
    let mut policy_config = file_config.policy.clone();
    policy_config.approval_mode = args.approval_mode.into();

    // CLI --collect-training flag OR file config's collect_training
    // (CLI flag enables it; if not set, fall back to file config)
    let collect_training = args.collect_training || file_config.collect_training;

    // Sandbox mode: CLI overrides file config, default is ReadOnly
    let sandbox_mode = args
        .sandbox
        .map(SandboxMode::from)
        .or(file_config.sandbox_mode)
        .unwrap_or_default();

    ResolvedConfig {
        model: args
            .model
            .clone()
            .unwrap_or_else(|| file_config.model.clone()),
        max_turns: args.max_turns.unwrap_or(file_config.max_turns),
        working_dir: args
            .working_dir
            .clone()
            .or_else(|| file_config.working_dir.clone())
            .unwrap_or_else(|| ".".to_string()),
        session_id: args.session.clone(),
        use_mock_llm: args.mock,
        verbose: args.verbose,
        quiet: args.quiet,
        dry_run: args.dry_run,
        check: args.check,
        json: args.json,
        stdin: args.stdin,
        prompt_file: args.prompt_file.clone(),
        exec_prompt: args.exec.clone(),
        dashstream,
        approval_mode: args.approval_mode.into(),
        policy_config,
        collect_training,
        load_optimized_prompts: args.load_optimized_prompts,
        system_prompt: args.system_prompt.clone(),
        system_prompt_file: args.system_prompt_file.clone(),
        sandbox_mode,
        // Audit #70: Additional writable roots for sandbox
        sandbox_writable_roots: file_config.sandbox_writable_roots.clone(),
        postgres: args.postgres.clone(),
        // Audit item #16: API base from config
        api_base: file_config.api_base.clone(),
        // Audit item #14: DashFlow streaming/checkpointing flags from config
        streaming_enabled: file_config.dashflow.streaming_enabled,
        checkpointing_enabled: file_config.dashflow.checkpointing_enabled,
        // Audit item #15, #83: Checkpoint path (CLI overrides config)
        checkpoint_path: args
            .checkpoint_path
            .clone()
            .or_else(|| file_config.dashflow.checkpoint_path.clone()),
        // Audit item #17, #22, #90: MCP server configs from file config
        mcp_servers: file_config.mcp_servers.clone(),
        // AI introspection: graph manifest in system prompt
        // CLI flags override config: --introspection enables, --no-introspection disables
        introspection_enabled: if args.no_introspection {
            false
        } else if args.introspection {
            true
        } else {
            file_config.dashflow.introspection_enabled
        },
        // Auto-resume: CLI flags override config
        // --auto-resume enables, --no-auto-resume disables
        auto_resume_enabled: if args.no_auto_resume {
            false
        } else if args.auto_resume {
            true
        } else {
            file_config.dashflow.auto_resume
        },
        // Auto-resume max age: CLI --auto-resume-max-age overrides config
        auto_resume_max_age_secs: args
            .auto_resume_max_age
            .or(file_config.dashflow.auto_resume_max_age_secs),
    }
}

/// Resolve the special "latest" session value to an actual session ID.
///
/// When `--session` is used without an argument, clap sets the value to "latest".
/// This function resolves that to the most recent session ID, or returns `None`
/// if no sessions exist.
///
/// For any other session value, it returns the value unchanged.
///
/// # Arguments
///
/// * `session_id` - The session ID from CLI args (may be "latest" or an actual ID)
/// * `runner_config` - Runner configuration with checkpointing settings
///
/// # Returns
///
/// Returns the resolved session ID, or `None` if "latest" was requested but no sessions exist.
pub async fn resolve_session_id(
    session_id: Option<&str>,
    runner_config: &RunnerConfig,
) -> Option<String> {
    match session_id {
        Some("latest") => {
            // Try to get the most recent session
            match codex_dashflow_core::get_latest_session(runner_config).await {
                Ok(Some(id)) => {
                    tracing::info!(session_id = %id, "Resolved --session to latest session");
                    Some(id)
                }
                Ok(None) => {
                    tracing::warn!("No sessions found when resolving --session (latest)");
                    None
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to resolve latest session, will start fresh");
                    None
                }
            }
        }
        Some(id) => Some(id.to_string()),
        None => None,
    }
}

/// Resolve the special "latest" session value with max age filtering.
///
/// Similar to `resolve_session_id`, but when resolving "latest", will skip
/// sessions older than `max_age_secs` (if specified). This is intended for
/// auto-resume scenarios where stale sessions should be skipped.
///
/// # Arguments
///
/// * `session_id` - The session ID from CLI args (may be "latest" or an actual ID)
/// * `runner_config` - Runner configuration with checkpointing settings
/// * `max_age_secs` - Maximum session age in seconds. `None` means no limit.
///
/// # Returns
///
/// Returns the resolved session ID, or `None` if:
/// - "latest" was requested but no sessions exist
/// - "latest" was requested but all sessions are older than max_age_secs
pub async fn resolve_session_id_with_max_age(
    session_id: Option<&str>,
    runner_config: &RunnerConfig,
    max_age_secs: Option<u64>,
) -> Option<String> {
    match session_id {
        Some("latest") => {
            // Try to get the most recent session, respecting max age
            match codex_dashflow_core::get_latest_session_with_max_age(runner_config, max_age_secs)
                .await
            {
                Ok(Some(id)) => {
                    tracing::info!(session_id = %id, "Resolved --session to latest session");
                    Some(id)
                }
                Ok(None) => {
                    if max_age_secs.is_some() {
                        tracing::info!(
                            max_age_secs = ?max_age_secs,
                            "No sessions found within max age limit"
                        );
                    } else {
                        tracing::warn!("No sessions found when resolving --session (latest)");
                    }
                    None
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to resolve latest session, will start fresh");
                    None
                }
            }
        }
        Some(id) => Some(id.to_string()),
        None => None,
    }
}

/// Build an AgentState from resolved configuration
pub fn build_agent_state(config: &ResolvedConfig) -> AgentState {
    use codex_dashflow_core::{llm::LlmConfig, AuthCredentialsStoreMode, AuthManager, AuthStatus};

    let mut state = AgentState::new();
    state.session_id = config
        .session_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Configure LLM based on authentication mode
    // ChatGPT OAuth users need to use the ChatGPT backend at chatgpt.com/backend-api/codex
    // API key users use the standard OpenAI API at api.openai.com/v1
    let llm_config = if let Ok(auth) = AuthManager::new(AuthCredentialsStoreMode::Auto) {
        let status = auth.get_status();
        tracing::debug!(?status, "Auth status detected");

        match status {
            AuthStatus::ChatGpt { .. } => {
                // ChatGPT OAuth mode - get tokens and configure ChatGPT backend
                if let (Ok(Some(token)), Ok(Some((account_id, _)))) =
                    (auth.get_access_token(), auth.get_account_info())
                {
                    tracing::info!("Configuring ChatGPT backend authentication");
                    LlmConfig::chatgpt(&config.model, token, account_id)
                } else {
                    tracing::warn!(
                        "ChatGPT auth detected but missing credentials, using default API config"
                    );
                    LlmConfig::with_model(&config.model)
                }
            }
            _ => {
                tracing::debug!("Using standard OpenAI API authentication");
                LlmConfig::with_model(&config.model)
            }
        }
    } else {
        tracing::debug!("No auth manager available, using default config");
        LlmConfig::with_model(&config.model)
    };

    // Apply api_base from config if non-default (audit item #16)
    // Only apply if ChatGPT auth mode is not active (ChatGPT uses its own endpoint)
    let default_api_base = "https://api.openai.com/v1";
    let mut llm_config = llm_config;
    if config.api_base != default_api_base && llm_config.api_base.is_none() {
        tracing::debug!(api_base = %config.api_base, "Applying custom API base from config");
        llm_config.api_base = Some(config.api_base.clone());
    }

    state.llm_config = llm_config;

    if config.max_turns > 0 {
        state.max_turns = config.max_turns;
    }

    if config.use_mock_llm {
        state = state.with_mock_llm();
    }

    // Build exec policy from policy config (includes custom rules and approval mode)
    let policy = config.policy_config.build_policy();
    state = state.with_exec_policy(Arc::new(policy));

    // Set working directory - audit item #11
    state.working_directory = config.working_dir.clone();

    // Set sandbox mode - audit item #12
    state.sandbox_mode = config.sandbox_mode;

    // Set writable roots - audit #70
    state.sandbox_writable_roots = config.sandbox_writable_roots.clone();

    // AI Introspection: Conditionally attach graph manifest for AI self-awareness
    // This enables the AI to understand its own structure and capabilities
    // Controlled by config.dashflow.introspection_enabled (defaults to true)
    if config.introspection_enabled {
        use std::sync::Arc;
        let manifest = codex_dashflow_core::build_agent_graph_manifest();
        state.graph_manifest = Some(Arc::new(manifest));
    }

    state
}

/// Build an MCP client from resolved configuration.
///
/// Connects to all configured MCP servers. If any connection fails,
/// a warning is logged but the client is still returned with partial connections.
/// Returns None if no MCP servers are configured.
///
/// Audit items #17, #22, #90: Wire MCP servers in exec mode
pub async fn build_mcp_client(
    config: &ResolvedConfig,
) -> Option<Arc<codex_dashflow_mcp::McpClient>> {
    if config.mcp_servers.is_empty() {
        return None;
    }

    let client = codex_dashflow_mcp::McpClient::new();

    for server_config in &config.mcp_servers {
        tracing::info!(server = %server_config.name, "Connecting to MCP server");
        match client.connect(server_config).await {
            Ok(()) => {
                tracing::info!(server = %server_config.name, "Connected to MCP server");
            }
            Err(e) => {
                tracing::warn!(
                    server = %server_config.name,
                    error = %e,
                    "Failed to connect to MCP server, continuing without it"
                );
            }
        }
    }

    Some(Arc::new(client))
}

/// Build an approval callback appropriate for exec mode based on approval mode.
///
/// In exec mode (non-interactive), we cannot prompt the user for approval.
/// Therefore:
/// - `Never` -> AutoApproveCallback (approve all non-forbidden tools)
/// - `OnFirstUse`, `OnDangerous`, `Always` -> AutoRejectCallback (reject tools needing approval)
///
/// This ensures that approval modes that require user interaction don't
/// silently auto-approve in exec mode.
///
/// Audit items #18, #61: Wire approval callback in exec mode
pub fn build_exec_approval_callback(
    approval_mode: ApprovalMode,
) -> Arc<dyn codex_dashflow_core::state::ApprovalCallback> {
    use codex_dashflow_core::state::{AutoApproveCallback, AutoRejectCallback};

    match approval_mode {
        ApprovalMode::Never => {
            // User explicitly chose to auto-approve everything
            Arc::new(AutoApproveCallback)
        }
        ApprovalMode::OnFirstUse | ApprovalMode::OnDangerous | ApprovalMode::Always => {
            // These modes require user interaction which exec mode can't provide
            // Reject tools that need approval to maintain security
            tracing::info!(
                approval_mode = ?approval_mode,
                "Exec mode: tools requiring approval will be rejected (no interactive prompt available)"
            );
            Arc::new(AutoRejectCallback)
        }
    }
}

/// Resolve the effective system prompt from CLI options.
///
/// Precedence (highest to lowest):
/// 1. --system-prompt (direct string)
/// 2. --system-prompt-file (path to file)
/// 3. None (use default or optimized prompts)
///
/// Returns an error if the file cannot be read.
pub fn resolve_system_prompt(
    system_prompt: Option<&str>,
    system_prompt_file: Option<&PathBuf>,
) -> Result<Option<String>, std::io::Error> {
    // Direct --system-prompt takes highest precedence
    if let Some(prompt) = system_prompt {
        return Ok(Some(prompt.to_string()));
    }

    // --system-prompt-file is second priority
    if let Some(path) = system_prompt_file {
        let content = std::fs::read_to_string(path)?;
        return Ok(Some(content.trim().to_string()));
    }

    Ok(None)
}

// ============================================================================
// Prompt Validation
// ============================================================================

/// Maximum prompt length in characters (approximately 100KB of text)
pub const MAX_PROMPT_LENGTH: usize = 100_000;

/// Minimum prompt length in characters (must have some content)
pub const MIN_PROMPT_LENGTH: usize = 1;

/// Validation error for prompts
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptValidationError {
    /// Prompt is empty or contains only whitespace
    Empty,
    /// Prompt exceeds maximum length
    TooLong { length: usize, max: usize },
    /// Prompt contains invalid characters (e.g., null bytes)
    InvalidCharacters(String),
}

impl std::fmt::Display for PromptValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "prompt is empty"),
            Self::TooLong { length, max } => {
                write!(f, "prompt is too long ({} characters, max {})", length, max)
            }
            Self::InvalidCharacters(desc) => {
                write!(f, "prompt contains invalid characters: {}", desc)
            }
        }
    }
}

impl std::error::Error for PromptValidationError {}

/// Validate a prompt string.
///
/// Checks for:
/// - Empty or whitespace-only prompts
/// - Excessive length (>100K characters)
/// - Invalid characters (null bytes, etc.)
///
/// Returns Ok(()) if valid, or a PromptValidationError if invalid.
pub fn validate_prompt(prompt: &str) -> Result<(), PromptValidationError> {
    // Check for empty prompt (after trimming)
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Err(PromptValidationError::Empty);
    }

    // Check for excessive length (use original length, not trimmed)
    if prompt.len() > MAX_PROMPT_LENGTH {
        return Err(PromptValidationError::TooLong {
            length: prompt.len(),
            max: MAX_PROMPT_LENGTH,
        });
    }

    // Check for null bytes (can cause issues in some systems)
    if prompt.contains('\0') {
        return Err(PromptValidationError::InvalidCharacters(
            "null bytes not allowed".to_string(),
        ));
    }

    Ok(())
}

/// Read prompt from stdin.
///
/// Reads all input until EOF, trimming leading/trailing whitespace.
/// Returns an error if stdin cannot be read or if the input is empty.
pub fn read_prompt_from_stdin() -> Result<String, std::io::Error> {
    use std::io::Read;

    let mut buffer = String::new();
    std::io::stdin().read_to_string(&mut buffer)?;

    let prompt = buffer.trim().to_string();

    if prompt.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "stdin prompt is empty",
        ));
    }

    Ok(prompt)
}

/// Build a RunnerConfig with the appropriate stream callback and training collection
pub fn build_runner_config(
    verbose: bool,
    collect_training: bool,
    load_optimized_prompts: bool,
    system_prompt: Option<String>,
    postgres_connection_string: Option<String>,
) -> RunnerConfig {
    build_runner_config_with_dashstream(
        verbose,
        collect_training,
        load_optimized_prompts,
        system_prompt,
        postgres_connection_string,
        None, // No dashstream config
    )
}

/// Build a RunnerConfig with optional DashStream support (audit item #13).
///
/// When dashstream config is provided, the callback will emit events to both
/// the console/exec callback AND the DashFlow streaming adapter.
///
/// Checkpointing priority (audit items #14, #15):
/// 1. PostgreSQL (if postgres_connection_string provided)
/// 2. File-based (if checkpoint_path provided AND checkpointing_enabled)
/// 3. Memory (default, no persistence)
pub fn build_runner_config_with_dashstream(
    verbose: bool,
    collect_training: bool,
    load_optimized_prompts: bool,
    system_prompt: Option<String>,
    postgres_connection_string: Option<String>,
    dashstream: Option<&DashStreamConfig>,
) -> RunnerConfig {
    build_runner_config_full(
        verbose,
        collect_training,
        load_optimized_prompts,
        system_prompt,
        postgres_connection_string,
        dashstream,
        false, // checkpointing_enabled
        None,  // checkpoint_path
        None,  // working_dir - use with_dashstream_and_project_docs for project doc support
    )
}

/// Build a RunnerConfig with full checkpointing support (audit items #14, #15).
///
/// Checkpointing priority:
/// 1. PostgreSQL (if postgres_connection_string provided)
/// 2. File-based (if checkpoint_path provided AND checkpointing_enabled)
/// 3. Memory (if checkpointing_enabled but no path)
/// 4. Default (no checkpointing)
///
/// Note: This synchronous version does not initialize DashFlowStreamAdapter.
/// For full DashFlow Streaming support, use `build_runner_config_full_async`.
#[allow(clippy::too_many_arguments)]
pub fn build_runner_config_full(
    verbose: bool,
    collect_training: bool,
    load_optimized_prompts: bool,
    system_prompt: Option<String>,
    postgres_connection_string: Option<String>,
    dashstream: Option<&DashStreamConfig>,
    checkpointing_enabled: bool,
    checkpoint_path: Option<&PathBuf>,
    working_dir: Option<&str>,
) -> RunnerConfig {
    use codex_dashflow_core::streaming::{ConsoleStreamCallback, MultiStreamCallback};

    // Build the primary callback (console or exec style)
    let exec_callback: Arc<dyn StreamCallback> = Arc::new(ExecStreamCallback::new(verbose));

    // If dashstream is configured, add a console callback as well for telemetry
    // Note: Full DashFlowStreamAdapter requires the 'dashstream' feature and async init.
    // Use build_runner_config_full_async for full Kafka integration.
    let callback: Arc<dyn StreamCallback> = if dashstream.is_some() {
        // When dashstream is enabled, combine exec callback with console for visibility
        let console_callback: Arc<dyn StreamCallback> = Arc::new(ConsoleStreamCallback::new());
        Arc::new(MultiStreamCallback::new(vec![
            exec_callback,
            console_callback,
        ]))
    } else {
        exec_callback
    };

    // Checkpointing priority: PostgreSQL > File > Memory > Default
    let mut config = if let Some(ref conn_str) = postgres_connection_string {
        // Priority 1: PostgreSQL checkpointing
        RunnerConfig::with_postgres_checkpointing(conn_str)
    } else if let Some(path) = checkpoint_path {
        // Priority 2: File-based checkpointing (audit item #15)
        RunnerConfig::with_file_checkpointing(path)
    } else if checkpointing_enabled {
        // Priority 3: Memory checkpointing (audit item #14)
        RunnerConfig::with_memory_checkpointing()
    } else {
        // Priority 4: Default (no checkpointing)
        RunnerConfig::default()
    };

    config = config
        .with_stream_callback(callback)
        .with_collect_training(collect_training)
        .with_load_optimized_prompts(load_optimized_prompts);

    if let Some(prompt) = system_prompt {
        config = config.with_system_prompt(prompt);
    }

    // Enable project documentation discovery (AGENTS.md) - audit item #20, #23
    if let Some(dir) = working_dir {
        config = config.with_project_docs(dir);
    }

    config
}

/// Async version of `build_runner_config_full` with full DashFlow Streaming support.
///
/// When the `dashstream` feature is enabled and a DashStreamConfig is provided,
/// this function initializes the DashFlowStreamAdapter for Kafka telemetry export.
///
/// Checkpointing priority:
/// 1. PostgreSQL (if postgres_connection_string provided)
/// 2. File-based (if checkpoint_path provided AND checkpointing_enabled)
/// 3. Memory (if checkpointing_enabled but no path)
/// 4. Default (no checkpointing)
#[allow(clippy::too_many_arguments)]
pub async fn build_runner_config_full_async(
    verbose: bool,
    collect_training: bool,
    load_optimized_prompts: bool,
    system_prompt: Option<String>,
    postgres_connection_string: Option<String>,
    dashstream: Option<&DashStreamConfig>,
    checkpointing_enabled: bool,
    checkpoint_path: Option<&PathBuf>,
    working_dir: Option<&str>,
    session_id: Option<&str>,
    streaming_enabled: bool, // Audit item #14: Config-driven streaming
) -> RunnerConfig {
    use codex_dashflow_core::streaming::{ConsoleStreamCallback, MultiStreamCallback};

    // Build the primary callback (console or exec style)
    let exec_callback: Arc<dyn StreamCallback> = Arc::new(ExecStreamCallback::new(verbose));

    // Build the callback stack
    // Priority: dashstream CLI flag > config streaming_enabled > default (exec callback only)
    let callback: Arc<dyn StreamCallback> = if let Some(ds_config) = dashstream {
        // When dashstream is configured, try to create the full streaming adapter
        #[cfg(feature = "dashstream")]
        {
            use codex_dashflow_core::streaming::{DashFlowStreamAdapter, DashFlowStreamConfig};

            let stream_config = DashFlowStreamConfig {
                bootstrap_servers: ds_config.bootstrap_servers.clone(),
                topic: ds_config.topic.clone(),
                tenant_id: "codex-dashflow".to_string(),
                enable_state_diff: true,
                compression_threshold: 512,
                compression_level: 3,
                enable_compression: true,
            };

            let session = session_id.unwrap_or("default-session");

            match DashFlowStreamAdapter::new(stream_config, session).await {
                Ok(adapter) => {
                    tracing::info!(
                        "DashFlow streaming enabled: {} -> {}",
                        ds_config.bootstrap_servers,
                        ds_config.topic
                    );
                    Arc::new(MultiStreamCallback::new(vec![
                        exec_callback,
                        Arc::new(adapter),
                    ]))
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to initialize DashFlow streaming (falling back to console): {}",
                        e
                    );
                    let console_callback: Arc<dyn StreamCallback> =
                        Arc::new(ConsoleStreamCallback::new());
                    Arc::new(MultiStreamCallback::new(vec![
                        exec_callback,
                        console_callback,
                    ]))
                }
            }
        }

        #[cfg(not(feature = "dashstream"))]
        {
            // Feature not enabled - use console callback as fallback
            // Suppress unused variable warnings for feature-gated parameters
            let _ = (ds_config, session_id);
            tracing::debug!(
                "DashFlow streaming requested but 'dashstream' feature not enabled; using console"
            );
            let console_callback: Arc<dyn StreamCallback> = Arc::new(ConsoleStreamCallback::new());
            Arc::new(MultiStreamCallback::new(vec![
                exec_callback,
                console_callback,
            ]))
        }
    } else if streaming_enabled {
        // Audit item #14: Config streaming_enabled enables console streaming
        // when dashstream CLI flag is not provided
        tracing::debug!("Console streaming enabled via config (streaming_enabled = true)");
        let console_callback: Arc<dyn StreamCallback> = Arc::new(ConsoleStreamCallback::new());
        Arc::new(MultiStreamCallback::new(vec![
            exec_callback,
            console_callback,
        ]))
    } else {
        exec_callback
    };

    // Checkpointing priority: PostgreSQL > File > Memory > Default
    let mut config = if let Some(ref conn_str) = postgres_connection_string {
        // Priority 1: PostgreSQL checkpointing
        RunnerConfig::with_postgres_checkpointing(conn_str)
    } else if let Some(path) = checkpoint_path {
        // Priority 2: File-based checkpointing (audit item #15)
        RunnerConfig::with_file_checkpointing(path)
    } else if checkpointing_enabled {
        // Priority 3: Memory checkpointing (audit item #14)
        RunnerConfig::with_memory_checkpointing()
    } else {
        // Priority 4: Default (no checkpointing)
        RunnerConfig::default()
    };

    config = config
        .with_stream_callback(callback)
        .with_collect_training(collect_training)
        .with_load_optimized_prompts(load_optimized_prompts);

    if let Some(prompt) = system_prompt {
        config = config.with_system_prompt(prompt);
    }

    // Enable project documentation discovery (AGENTS.md) - audit item #20, #23
    if let Some(dir) = working_dir {
        config = config.with_project_docs(dir);
    }

    config
}

// ============================================================================
// Dry Run Configuration Output
// ============================================================================

/// JSON-serializable representation of dry-run configuration output.
///
/// This struct mirrors the human-readable output of `print_dry_run_config`
/// in a structured format suitable for machine parsing.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DryRunConfig {
    /// Path to the configuration file (if specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_file: Option<String>,

    /// Execution settings
    pub execution: DryRunExecution,

    /// Mode information
    pub mode: DryRunMode,

    /// Output settings
    pub output: DryRunOutput,

    /// Security settings
    pub security: DryRunSecurity,

    /// Prompt settings
    pub prompts: DryRunPrompts,

    /// Training settings
    pub training: DryRunTraining,

    /// DashStream settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dashstream: Option<DryRunDashStream>,
}

/// Execution-related configuration for dry-run output
#[derive(Debug, Clone, serde::Serialize)]
pub struct DryRunExecution {
    pub model: String,
    pub max_turns: DryRunMaxTurns,
    pub working_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub mock_llm: bool,
}

/// Max turns can be a number or unlimited
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum DryRunMaxTurns {
    Limited(u32),
    Unlimited,
}

impl DryRunMaxTurns {
    fn from_u32(value: u32) -> Self {
        if value == 0 {
            DryRunMaxTurns::Unlimited
        } else {
            DryRunMaxTurns::Limited(value)
        }
    }
}

impl std::fmt::Display for DryRunMaxTurns {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DryRunMaxTurns::Limited(n) => write!(f, "{}", n),
            DryRunMaxTurns::Unlimited => write!(f, "unlimited"),
        }
    }
}

/// Mode information for dry-run output
#[derive(Debug, Clone, serde::Serialize)]
pub struct DryRunMode {
    /// "exec" or "tui"
    pub mode: String,
    /// Source of prompt: "stdin", "file", "inline", or null for TUI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_source: Option<String>,
    /// Path to prompt file if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_file: Option<String>,
    /// Preview of inline prompt if applicable (truncated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_preview: Option<String>,
}

/// Output settings for dry-run output
#[derive(Debug, Clone, serde::Serialize)]
pub struct DryRunOutput {
    pub verbose: bool,
    pub quiet: bool,
}

/// Security settings for dry-run output
#[derive(Debug, Clone, serde::Serialize)]
pub struct DryRunSecurity {
    pub approval_mode: String,
    pub sandbox_mode: String,
    pub policy_rule_count: usize,
    pub dangerous_patterns_included: bool,
}

/// Prompt settings for dry-run output
#[derive(Debug, Clone, serde::Serialize)]
pub struct DryRunPrompts {
    pub load_optimized: bool,
    /// "custom", "file", or "default"
    pub system_prompt_source: String,
    /// Path to system prompt file if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_file: Option<String>,
}

/// Training settings for dry-run output
#[derive(Debug, Clone, serde::Serialize)]
pub struct DryRunTraining {
    pub collect_training: bool,
}

/// DashStream settings for dry-run output
#[derive(Debug, Clone, serde::Serialize)]
pub struct DryRunDashStream {
    pub bootstrap_servers: String,
    pub topic: String,
}

impl DryRunConfig {
    /// Build a DryRunConfig from ResolvedConfig
    fn from_resolved(config: &ResolvedConfig, config_path: Option<&std::path::Path>) -> Self {
        // Determine mode and prompt information
        let is_exec_mode =
            config.stdin || config.prompt_file.is_some() || config.exec_prompt.is_some();

        let (mode_str, prompt_source, prompt_file, prompt_preview) = if is_exec_mode {
            let (source, file, preview) = if config.stdin {
                ("stdin".to_string(), None, None)
            } else if let Some(ref path) = config.prompt_file {
                ("file".to_string(), Some(path.display().to_string()), None)
            } else if let Some(ref prompt) = config.exec_prompt {
                let preview = if prompt.len() > 50 {
                    format!("{}...", &prompt[..50])
                } else {
                    prompt.clone()
                };
                ("inline".to_string(), None, Some(preview))
            } else {
                ("unknown".to_string(), None, None)
            };
            ("exec".to_string(), Some(source), file, preview)
        } else {
            ("tui".to_string(), None, None, None)
        };

        // System prompt source
        let system_prompt_source = if config.system_prompt.is_some() {
            "custom"
        } else if config.system_prompt_file.is_some() {
            "file"
        } else {
            "default"
        }
        .to_string();

        DryRunConfig {
            config_file: config_path.map(|p| p.display().to_string()),
            execution: DryRunExecution {
                model: config.model.clone(),
                max_turns: DryRunMaxTurns::from_u32(config.max_turns),
                working_dir: config.working_dir.clone(),
                session_id: config.session_id.clone(),
                mock_llm: config.use_mock_llm,
            },
            mode: DryRunMode {
                mode: mode_str,
                prompt_source,
                prompt_file,
                prompt_preview,
            },
            output: DryRunOutput {
                verbose: config.verbose,
                quiet: config.quiet,
            },
            security: DryRunSecurity {
                approval_mode: format!("{:?}", config.approval_mode),
                sandbox_mode: format!("{:?}", config.sandbox_mode),
                policy_rule_count: config.policy_config.rules.len(),
                dangerous_patterns_included: config.policy_config.include_dangerous_patterns,
            },
            prompts: DryRunPrompts {
                load_optimized: config.load_optimized_prompts,
                system_prompt_source,
                system_prompt_file: config
                    .system_prompt_file
                    .as_ref()
                    .map(|p| p.display().to_string()),
            },
            training: DryRunTraining {
                collect_training: config.collect_training,
            },
            dashstream: config.dashstream.as_ref().map(|ds| DryRunDashStream {
                bootstrap_servers: ds.bootstrap_servers.clone(),
                topic: ds.topic.clone(),
            }),
        }
    }
}

/// Format a boolean value with color for display
fn format_bool(value: bool) -> String {
    if value {
        "true".green().to_string()
    } else {
        "false".dimmed().to_string()
    }
}

/// Format a duration in seconds to a human-readable string
fn format_duration_human(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        let mins = secs / 60;
        let remaining = secs % 60;
        if remaining == 0 {
            format!("{}m", mins)
        } else {
            format!("{}m {}s", mins, remaining)
        }
    } else if secs < 86400 {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        if mins == 0 {
            format!("{}h", hours)
        } else {
            format!("{}h {}m", hours, mins)
        }
    } else {
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        if hours == 0 {
            format!("{}d", days)
        } else {
            format!("{}d {}h", days, hours)
        }
    }
}

/// Print a concise security posture banner for exec mode.
///
/// Displays sandbox mode, approval mode, and MCP server status in a
/// human-readable format directly to stderr. This ensures operators
/// can see the security posture immediately when running in exec mode,
/// without needing to read trace logs.
///
/// Audit item #11: Exec-mode surface should display security posture at start.
pub fn print_exec_security_posture(config: &ResolvedConfig) {
    use std::io::Write;

    let sandbox_desc = match config.sandbox_mode {
        SandboxMode::ReadOnly => "read-only".green(),
        SandboxMode::WorkspaceWrite => "workspace-write".yellow(),
        SandboxMode::DangerFullAccess => "DANGER-FULL-ACCESS".red().bold(),
    };

    let approval_desc = match config.approval_mode {
        ApprovalMode::Never => "never (auto-approve all)".red(),
        ApprovalMode::OnFirstUse => "on-first-use (auto-reject in exec)".yellow(),
        ApprovalMode::OnDangerous => "on-dangerous (auto-reject in exec)".cyan(),
        ApprovalMode::Always => "always (auto-reject in exec)".cyan(),
    };

    let mcp_desc = if config.mcp_servers.is_empty() {
        "none".dimmed()
    } else {
        format!("{} server(s) configured", config.mcp_servers.len()).cyan()
    };

    // Check sandbox availability
    let sandbox_warning =
        if !config.sandbox_mode.is_unrestricted() && !SandboxExecutor::is_available() {
            Some(
                "\n  WARNING: Sandbox not available, commands run WITHOUT protection!"
                    .red()
                    .bold(),
            )
        } else {
            None
        };

    let _ = writeln!(
        std::io::stderr(),
        "{}",
        "[Security Posture]".bold().underline()
    );
    let _ = writeln!(
        std::io::stderr(),
        "  Sandbox:       {}{}",
        sandbox_desc,
        sandbox_warning
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_default()
    );
    let _ = writeln!(std::io::stderr(), "  Approval mode: {}", approval_desc);
    let _ = writeln!(std::io::stderr(), "  MCP servers:   {}", mcp_desc);
    let _ = writeln!(std::io::stderr());
}

/// Print the resolved configuration in dry-run mode.
///
/// Displays all configuration values that would be used when running
/// the agent, allowing users to verify their configuration without
/// actually executing anything.
///
/// If `json` is true, outputs a machine-readable JSON format instead
/// of the human-readable text format.
pub fn print_dry_run_config(
    config: &ResolvedConfig,
    config_path: Option<&std::path::Path>,
    json: bool,
) {
    if json {
        let dry_run_config = DryRunConfig::from_resolved(config, config_path);
        match serde_json::to_string_pretty(&dry_run_config) {
            Ok(json_str) => println!("{}", json_str),
            Err(e) => eprintln!("Error serializing configuration: {}", e),
        }
        return;
    }

    println!("{}", "Dry-run mode: showing resolved configuration".bold());
    println!(
        "{}",
        "=============================================".dimmed()
    );
    println!();

    // Config file source
    if let Some(path) = config_path {
        println!("Config file:         {}", path.display().to_string().cyan());
    } else {
        println!("Config file:         {}", "(default or none)".dimmed());
    }
    println!();

    // Model and execution settings
    println!("{}", "[Execution]".bold());
    println!("  Model:             {}", config.model.cyan());
    println!(
        "  Max turns:         {}",
        if config.max_turns == 0 {
            "unlimited".yellow().to_string()
        } else {
            config.max_turns.to_string()
        }
    );
    println!("  Working directory: {}", config.working_dir.cyan());
    println!(
        "  Session ID:        {}",
        config
            .session_id
            .as_deref()
            .map(|s| s.cyan().to_string())
            .unwrap_or_else(|| "(auto-generated)".dimmed().to_string())
    );
    println!("  Mock LLM:          {}", format_bool(config.use_mock_llm));
    println!();

    // Mode
    println!("{}", "[Mode]".bold());
    // Determine mode and prompt source
    let is_exec_mode = config.stdin || config.prompt_file.is_some() || config.exec_prompt.is_some();
    if is_exec_mode {
        println!("  Mode:              {}", "exec (non-interactive)".cyan());
        // Show prompt source
        if config.stdin {
            println!(
                "  Prompt source:     {}",
                "stdin (read on execution)".yellow()
            );
        } else if let Some(ref path) = config.prompt_file {
            println!(
                "  Prompt source:     file ({})",
                path.display().to_string().cyan()
            );
        } else if let Some(prompt) = &config.exec_prompt {
            let preview = if prompt.len() > 50 {
                format!("{}...", &prompt[..50])
            } else {
                prompt.clone()
            };
            println!("  Prompt source:     {}", "inline (--exec)".cyan());
            println!("  Prompt:            \"{}\"", preview.dimmed());
        }
    } else {
        println!("  Mode:              {}", "TUI (interactive)".green());
    }
    println!();

    // Output settings
    println!("{}", "[Output]".bold());
    println!("  Verbose:           {}", format_bool(config.verbose));
    println!("  Quiet:             {}", format_bool(config.quiet));
    println!();

    // Security settings
    println!("{}", "[Security]".bold());
    println!(
        "  Approval mode:     {}",
        format!("{:?}", config.approval_mode).cyan()
    );
    println!(
        "  Sandbox mode:      {}",
        format!("{:?}", config.sandbox_mode).cyan()
    );
    println!(
        "  Policy rules:      {}",
        format!("{} custom rules", config.policy_config.rules.len()).dimmed()
    );
    if config.policy_config.include_dangerous_patterns {
        println!("  Dangerous patterns: {}", "included".green());
    }
    println!();

    // Prompt settings
    println!("{}", "[Prompts]".bold());
    println!(
        "  Load optimized:    {}",
        format_bool(config.load_optimized_prompts)
    );
    println!(
        "  System prompt:     {}",
        if config.system_prompt.is_some() {
            "custom (--system-prompt)".cyan().to_string()
        } else if config.system_prompt_file.is_some() {
            "from file (--system-prompt-file)".cyan().to_string()
        } else {
            "default".dimmed().to_string()
        }
    );
    if let Some(path) = &config.system_prompt_file {
        println!(
            "  System prompt file: {}",
            path.display().to_string().cyan()
        );
    }
    println!();

    // Training settings
    println!("{}", "[Training]".bold());
    println!(
        "  Collect training:  {}",
        format_bool(config.collect_training)
    );
    println!();

    // DashStream settings
    println!("{}", "[DashStream]".bold());
    if let Some(ds) = &config.dashstream {
        println!("  Enabled:           {}", "true".green());
        println!("  Bootstrap servers: {}", ds.bootstrap_servers.cyan());
        println!("  Topic:             {}", ds.topic.cyan());
    } else {
        println!("  Enabled:           {}", "false".dimmed());
    }
    println!();

    println!(
        "{}",
        "=============================================".dimmed()
    );
    println!(
        "{}",
        "Configuration valid. Use without --dry-run to execute.".green()
    );
}

// ============================================================================
// Check Command Implementation
// ============================================================================

/// Exit codes for the check command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckExitCode {
    /// Configuration is valid with no issues
    Valid = 0,
    /// Configuration has warnings but is usable
    Warnings = 1,
    /// Configuration has errors and is not usable
    Errors = 2,
}

impl CheckExitCode {
    /// Convert to process exit code
    pub fn code(self) -> i32 {
        self as i32
    }
}

/// Result of a configuration check
#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckResult {
    /// Whether the configuration file was found
    pub config_found: bool,
    /// Path to the configuration file (if found)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    /// Whether the configuration file could be parsed
    pub config_parsed: bool,
    /// Parse error message (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    /// Validation issues from semantic checks
    pub issues: Vec<CheckIssue>,
    /// Total number of errors
    pub error_count: usize,
    /// Total number of warnings
    pub warning_count: usize,
    /// Overall status: "valid", "warnings", or "errors"
    pub status: String,
}

/// A single issue found during configuration check
#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckIssue {
    /// Severity: "error" or "warning"
    pub severity: String,
    /// Field or area where the issue was found
    pub field: String,
    /// Description of the issue
    pub message: String,
    /// Suggested fix (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Check environment variables and add issues to the list.
///
/// This validates environment variables needed for operation:
/// - OPENAI_API_KEY: Required for API access (error if missing)
/// - OPENAI_BASE_URL: If set, validates URL format (warning if invalid)
/// - HTTP_PROXY/HTTPS_PROXY: If set, validates URL format (warning if invalid)
fn check_environment_for_issues(issues: &mut Vec<CheckIssue>) {
    use codex_dashflow_core::{AuthCredentialsStoreMode, AuthManager, AuthStatus};

    // Check for authentication (stored credentials OR env var)
    let has_auth = if let Ok(auth) = AuthManager::new(AuthCredentialsStoreMode::Auto) {
        !matches!(auth.get_status(), AuthStatus::NotAuthenticated)
    } else {
        std::env::var("OPENAI_API_KEY").is_ok()
    };

    if !has_auth {
        issues.push(CheckIssue {
            severity: "error".to_string(),
            field: "Authentication".to_string(),
            message: "Not authenticated".to_string(),
            suggestion: Some("Run 'codex-dashflow login' or set OPENAI_API_KEY".to_string()),
        });
    }

    // Check OPENAI_BASE_URL format if set
    if let Ok(base_url) = std::env::var("OPENAI_BASE_URL") {
        if !base_url.is_empty() {
            match url::Url::parse(&base_url) {
                Ok(url) => {
                    // Warn if using HTTP instead of HTTPS
                    if url.scheme() == "http" {
                        issues.push(CheckIssue {
                            severity: "warning".to_string(),
                            field: "OPENAI_BASE_URL".to_string(),
                            message: "Using HTTP instead of HTTPS for API endpoint".to_string(),
                            suggestion: Some(
                                "Consider using HTTPS for secure API communication".to_string(),
                            ),
                        });
                    }
                }
                Err(_) => {
                    issues.push(CheckIssue {
                        severity: "warning".to_string(),
                        field: "OPENAI_BASE_URL".to_string(),
                        message: format!("Invalid URL format: {}", base_url),
                        suggestion: Some(
                            "Use a valid URL like https://api.openai.com/v1".to_string(),
                        ),
                    });
                }
            }
        }
    }

    // Check HTTP_PROXY format if set
    check_proxy_env_var(issues, "HTTP_PROXY");
    check_proxy_env_var(issues, "http_proxy");

    // Check HTTPS_PROXY format if set
    check_proxy_env_var(issues, "HTTPS_PROXY");
    check_proxy_env_var(issues, "https_proxy");

    // Check NO_PROXY format if set (informational)
    if let Ok(no_proxy) = std::env::var("NO_PROXY").or_else(|_| std::env::var("no_proxy")) {
        if !no_proxy.is_empty() {
            // NO_PROXY is a comma-separated list of hosts/domains, validate basic format
            let entries: Vec<&str> = no_proxy.split(',').map(|s| s.trim()).collect();
            for entry in &entries {
                if entry.is_empty() {
                    issues.push(CheckIssue {
                        severity: "warning".to_string(),
                        field: "NO_PROXY".to_string(),
                        message: "NO_PROXY contains empty entry (consecutive commas)".to_string(),
                        suggestion: Some(
                            "Remove empty entries from NO_PROXY: localhost,.internal.domain"
                                .to_string(),
                        ),
                    });
                    break;
                }
            }
        }
    }
}

/// Validate a proxy environment variable (HTTP_PROXY, HTTPS_PROXY, etc.)
fn check_proxy_env_var(issues: &mut Vec<CheckIssue>, var_name: &str) {
    if let Ok(proxy_url) = std::env::var(var_name) {
        if !proxy_url.is_empty() {
            match url::Url::parse(&proxy_url) {
                Ok(url) => {
                    let scheme = url.scheme();
                    // Valid proxy schemes are http, https, socks4, socks5
                    if !["http", "https", "socks4", "socks5"].contains(&scheme) {
                        issues.push(CheckIssue {
                            severity: "warning".to_string(),
                            field: var_name.to_string(),
                            message: format!("Unusual proxy scheme '{}' in {}", scheme, var_name),
                            suggestion: Some(
                                "Common proxy schemes are http, https, socks4, socks5".to_string(),
                            ),
                        });
                    }

                    // Warn if HTTPS_PROXY uses http:// (credential exposure risk)
                    if (var_name == "HTTPS_PROXY" || var_name == "https_proxy") && scheme == "http"
                    {
                        issues.push(CheckIssue {
                            severity: "warning".to_string(),
                            field: var_name.to_string(),
                            message: "HTTPS_PROXY uses HTTP scheme - credentials may be sent unencrypted to proxy".to_string(),
                            suggestion: Some(
                                "Consider using https:// or socks5:// for the proxy URL if credentials are used".to_string(),
                            ),
                        });
                    }
                }
                Err(_) => {
                    issues.push(CheckIssue {
                        severity: "warning".to_string(),
                        field: var_name.to_string(),
                        message: format!("Invalid URL format in {}: {}", var_name, proxy_url),
                        suggestion: Some(
                            "Use a valid URL like http://proxy.example.com:8080".to_string(),
                        ),
                    });
                }
            }
        }
    }
}

/// Run configuration validation and return exit code.
///
/// Performs comprehensive validation including:
/// - Config file existence and accessibility
/// - TOML parsing
/// - Semantic validation (model names, URLs, paths, etc.)
/// - System prompt file accessibility (if specified)
/// - Prompt file accessibility (if specified)
/// - Environment variable validation
///
/// Returns CheckExitCode indicating the result:
/// - Valid (0): No issues found
/// - Warnings (1): Issues found but config is usable
/// - Errors (2): Critical issues that prevent operation
pub fn run_check_command(
    args: &Args,
    file_config: &Config,
    config_path: Option<&std::path::Path>,
    quiet: bool,
    json: bool,
) -> CheckExitCode {
    let mut issues = Vec::new();
    let mut config_found = false;
    let config_parsed = true;
    let parse_error: Option<String> = None;

    // Check config file
    let actual_config_path = config_path
        .map(|p| p.to_path_buf())
        .or_else(|| dirs::home_dir().map(|h| h.join(".codex-dashflow").join("config.toml")));

    if let Some(ref path) = actual_config_path {
        if path.exists() {
            config_found = true;
            // Config was already loaded, but check for validation issues
            let validation = file_config.validate();
            // Collect errors
            for issue in validation.errors() {
                issues.push(CheckIssue {
                    severity: "error".to_string(),
                    field: issue.field.clone(),
                    message: issue.message.clone(),
                    suggestion: issue.suggestion.clone(),
                });
            }
            // Collect warnings
            for issue in validation.warnings() {
                issues.push(CheckIssue {
                    severity: "warning".to_string(),
                    field: issue.field.clone(),
                    message: issue.message.clone(),
                    suggestion: issue.suggestion.clone(),
                });
            }
        }
    }

    // Check system prompt file accessibility
    if let Some(ref path) = args.system_prompt_file {
        if !path.exists() {
            issues.push(CheckIssue {
                severity: "error".to_string(),
                field: "system_prompt_file".to_string(),
                message: format!("System prompt file not found: {}", path.display()),
                suggestion: Some("Ensure the file exists and is accessible".to_string()),
            });
        } else if let Err(e) = std::fs::read_to_string(path) {
            issues.push(CheckIssue {
                severity: "error".to_string(),
                field: "system_prompt_file".to_string(),
                message: format!("Cannot read system prompt file: {}", e),
                suggestion: Some("Check file permissions".to_string()),
            });
        }
    }

    // Check prompt file accessibility
    if let Some(ref path) = args.prompt_file {
        if !path.exists() {
            issues.push(CheckIssue {
                severity: "error".to_string(),
                field: "prompt_file".to_string(),
                message: format!("Prompt file not found: {}", path.display()),
                suggestion: Some("Ensure the file exists and is accessible".to_string()),
            });
        } else if let Err(e) = std::fs::read_to_string(path) {
            issues.push(CheckIssue {
                severity: "error".to_string(),
                field: "prompt_file".to_string(),
                message: format!("Cannot read prompt file: {}", e),
                suggestion: Some("Check file permissions".to_string()),
            });
        }
    }

    // Check working directory
    if let Some(ref dir) = args.working_dir {
        let path = std::path::Path::new(dir);
        if !path.exists() {
            issues.push(CheckIssue {
                severity: "error".to_string(),
                field: "working_dir".to_string(),
                message: format!("Working directory does not exist: {}", dir),
                suggestion: Some("Create the directory or specify an existing one".to_string()),
            });
        } else if !path.is_dir() {
            issues.push(CheckIssue {
                severity: "error".to_string(),
                field: "working_dir".to_string(),
                message: format!("Working directory is not a directory: {}", dir),
                suggestion: Some("Specify a directory path, not a file".to_string()),
            });
        }
    }

    // Check DashStream configuration
    if args.dashstream && args.dashstream_bootstrap.is_none() {
        issues.push(CheckIssue {
            severity: "error".to_string(),
            field: "dashstream".to_string(),
            message: "--dashstream requires --dashstream-bootstrap to be set".to_string(),
            suggestion: Some("Add --dashstream-bootstrap localhost:9092".to_string()),
        });
    }

    // Audit item #25: Guard when --dashstream used without compiled feature
    #[cfg(not(feature = "dashstream"))]
    if args.dashstream {
        issues.push(CheckIssue {
            severity: "error".to_string(),
            field: "dashstream".to_string(),
            message: "--dashstream flag used but 'dashstream' feature is not compiled".to_string(),
            suggestion: Some(
                "Rebuild with: cargo build --features dashstream (requires protoc)".to_string(),
            ),
        });
    }

    // Check conflicting options
    if args.quiet && args.verbose {
        issues.push(CheckIssue {
            severity: "warning".to_string(),
            field: "output".to_string(),
            message: "Both --quiet and --verbose are set".to_string(),
            suggestion: Some("--quiet will take precedence".to_string()),
        });
    }

    // Check environment variables
    check_environment_for_issues(&mut issues);

    // Count issues by severity
    let error_count = issues.iter().filter(|i| i.severity == "error").count();
    let warning_count = issues.iter().filter(|i| i.severity == "warning").count();

    // Determine exit code
    let exit_code = if error_count > 0 {
        CheckExitCode::Errors
    } else if warning_count > 0 {
        CheckExitCode::Warnings
    } else {
        CheckExitCode::Valid
    };

    let status = match exit_code {
        CheckExitCode::Valid => "valid",
        CheckExitCode::Warnings => "warnings",
        CheckExitCode::Errors => "errors",
    };

    // In quiet mode, return early
    if quiet {
        return exit_code;
    }

    // Build result
    let result = CheckResult {
        config_found,
        config_path: actual_config_path.as_ref().map(|p| p.display().to_string()),
        config_parsed,
        parse_error,
        issues: issues.clone(),
        error_count,
        warning_count,
        status: status.to_string(),
    };

    // Output
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result)
                .unwrap_or_else(|e| format!(r#"{{"error": "Failed to serialize: {}"}}"#, e))
        );
    } else {
        println!("{}", "Configuration Check".bold());
        println!();

        // Config file status
        if config_found {
            println!(
                "{} Config file found: {}",
                "[✓]".green().bold(),
                actual_config_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            );
        } else {
            println!("{} No config file (using defaults)", "[i]".blue().bold());
        }

        // Show issues
        if !issues.is_empty() {
            println!();
            println!("{}:", "Issues Found".bold());
            for issue in &issues {
                let (icon, colored_icon) = if issue.severity == "error" {
                    ("[✗]", "[✗]".red().bold())
                } else {
                    ("[!]", "[!]".yellow().bold())
                };

                let icon_display = if colored::control::SHOULD_COLORIZE.should_colorize() {
                    format!("{}", colored_icon)
                } else {
                    icon.to_string()
                };

                let field_display = if issue.severity == "error" {
                    issue.field.red()
                } else {
                    issue.field.yellow()
                };

                println!("  {} {}: {}", icon_display, field_display, issue.message);
                if let Some(ref suggestion) = issue.suggestion {
                    println!("      {}: {}", "Suggestion".dimmed(), suggestion);
                }
            }
        }

        // Summary
        println!();
        if error_count > 0 {
            println!(
                "{}",
                format!(
                    "Found {} error(s) and {} warning(s). Configuration is invalid.",
                    error_count, warning_count
                )
                .red()
                .bold()
            );
        } else if warning_count > 0 {
            println!(
                "{}",
                format!("Configuration valid with {} warning(s).", warning_count).yellow()
            );
        } else {
            println!("{}", "Configuration is valid. Ready to run.".green().bold());
        }
    }

    exit_code
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use serial_test::serial;

    #[test]
    fn test_args_default() {
        // Empty args should parse to defaults
        let args = Args::try_parse_from(["codex-dashflow"]).unwrap();
        assert!(args.exec.is_none());
        assert!(!args.stdin);
        assert!(args.prompt_file.is_none());
        assert!(args.working_dir.is_none());
        assert!(args.max_turns.is_none());
        assert!(args.session.is_none());
        assert!(!args.mock);
        assert!(args.model.is_none());
        assert!(!args.verbose);
        assert!(!args.quiet);
        assert!(!args.dry_run);
        assert!(!args.check);
        assert!(!args.json);
        assert!(args.config.is_none());
        assert!(!args.dashstream);
        assert!(args.dashstream_bootstrap.is_none());
        assert_eq!(args.dashstream_topic, "codex-events");
        assert_eq!(args.approval_mode, CliApprovalMode::OnDangerous);
        assert!(args.sandbox.is_none());
    }

    #[test]
    fn test_args_exec_mode() {
        let args = Args::try_parse_from([
            "codex-dashflow",
            "--exec",
            "list files in the current directory",
        ])
        .unwrap();
        assert_eq!(
            args.exec,
            Some("list files in the current directory".to_string())
        );
    }

    #[test]
    fn test_args_exec_short_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "-e", "hello world"]).unwrap();
        assert_eq!(args.exec, Some("hello world".to_string()));
    }

    #[test]
    fn test_args_working_dir() {
        let args = Args::try_parse_from(["codex-dashflow", "--working-dir", "/tmp/test"]).unwrap();
        assert_eq!(args.working_dir, Some("/tmp/test".to_string()));
    }

    #[test]
    fn test_args_working_dir_short_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "-d", "/home/user"]).unwrap();
        assert_eq!(args.working_dir, Some("/home/user".to_string()));
    }

    #[test]
    fn test_args_max_turns() {
        let args = Args::try_parse_from(["codex-dashflow", "--max-turns", "10"]).unwrap();
        assert_eq!(args.max_turns, Some(10));
    }

    #[test]
    fn test_args_max_turns_short_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "-t", "5"]).unwrap();
        assert_eq!(args.max_turns, Some(5));
    }

    #[test]
    fn test_args_session() {
        let args = Args::try_parse_from(["codex-dashflow", "--session", "abc123"]).unwrap();
        assert_eq!(args.session, Some("abc123".to_string()));
    }

    #[test]
    fn test_args_session_without_value() {
        // --session without argument uses "latest" as default_missing_value
        let args = Args::try_parse_from(["codex-dashflow", "--session"]).unwrap();
        assert_eq!(args.session, Some("latest".to_string()));
    }

    #[test]
    fn test_args_session_short_flag_without_value() {
        // -s without argument uses "latest" as default_missing_value
        let args = Args::try_parse_from(["codex-dashflow", "-s"]).unwrap();
        assert_eq!(args.session, Some("latest".to_string()));
    }

    #[test]
    fn test_args_session_short_flag_with_value() {
        let args = Args::try_parse_from(["codex-dashflow", "-s", "my-session"]).unwrap();
        assert_eq!(args.session, Some("my-session".to_string()));
    }

    #[test]
    fn test_args_mock_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "--mock"]).unwrap();
        assert!(args.mock);
    }

    #[test]
    fn test_args_model() {
        let args = Args::try_parse_from(["codex-dashflow", "--model", "gpt-4-turbo"]).unwrap();
        assert_eq!(args.model, Some("gpt-4-turbo".to_string()));
    }

    #[test]
    fn test_args_model_short_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "-m", "claude-3"]).unwrap();
        assert_eq!(args.model, Some("claude-3".to_string()));
    }

    #[test]
    fn test_args_verbose_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "--verbose"]).unwrap();
        assert!(args.verbose);
    }

    #[test]
    fn test_args_verbose_short_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "-v"]).unwrap();
        assert!(args.verbose);
    }

    #[test]
    fn test_args_quiet_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "--quiet"]).unwrap();
        assert!(args.quiet);
    }

    #[test]
    fn test_args_quiet_short_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "-q"]).unwrap();
        assert!(args.quiet);
    }

    #[test]
    fn test_args_quiet_and_verbose_together() {
        // Both flags can be set together (quiet takes precedence in implementation)
        let args = Args::try_parse_from(["codex-dashflow", "-q", "-v"]).unwrap();
        assert!(args.quiet);
        assert!(args.verbose);
    }

    #[test]
    fn test_resolve_config_quiet_flag() {
        let args = Args {
            quiet: true,
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(resolved.quiet);
    }

    #[test]
    fn test_resolve_config_quiet_default_false() {
        let args = Args::default();
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(!resolved.quiet);
    }

    /// Test that quiet and streaming_enabled are both preserved in resolved config.
    /// The actual suppression of streaming when quiet=true happens in main.rs:
    /// `let effective_streaming = resolved.streaming_enabled && !resolved.quiet;`
    /// This test documents that both flags are available for that calculation.
    #[test]
    fn test_resolve_config_quiet_and_streaming_both_preserved() {
        // Config enables streaming
        let mut file_config = Config::default();
        file_config.dashflow.streaming_enabled = true;

        // CLI sets quiet flag
        let args = Args {
            quiet: true,
            ..Default::default()
        };

        let resolved = resolve_config(&args, &file_config);

        // Both flags are preserved - main.rs uses effective_streaming = streaming && !quiet
        assert!(resolved.quiet);
        assert!(resolved.streaming_enabled);

        // Document the expected effective_streaming calculation
        let effective_streaming = resolved.streaming_enabled && !resolved.quiet;
        assert!(
            !effective_streaming,
            "When quiet=true, streaming should be suppressed"
        );
    }

    /// Test that streaming is effective when quiet is false.
    #[test]
    fn test_resolve_config_streaming_enabled_without_quiet() {
        // Config enables streaming
        let mut file_config = Config::default();
        file_config.dashflow.streaming_enabled = true;

        // CLI does not set quiet flag
        let args = Args::default();

        let resolved = resolve_config(&args, &file_config);

        assert!(!resolved.quiet);
        assert!(resolved.streaming_enabled);

        // Without quiet, streaming should be effective
        let effective_streaming = resolved.streaming_enabled && !resolved.quiet;
        assert!(
            effective_streaming,
            "Without quiet, streaming should be active"
        );
    }

    #[test]
    fn test_args_dry_run_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "--dry-run"]).unwrap();
        assert!(args.dry_run);
    }

    #[test]
    fn test_args_dry_run_default_false() {
        let args = Args::try_parse_from(["codex-dashflow"]).unwrap();
        assert!(!args.dry_run);
    }

    #[test]
    fn test_args_dry_run_with_exec() {
        // Can combine --dry-run with --exec to verify exec mode config
        let args =
            Args::try_parse_from(["codex-dashflow", "--dry-run", "--exec", "test prompt"]).unwrap();
        assert!(args.dry_run);
        assert_eq!(args.exec, Some("test prompt".to_string()));
    }

    #[test]
    fn test_args_dry_run_with_multiple_flags() {
        // Can combine --dry-run with various config options
        let args = Args::try_parse_from([
            "codex-dashflow",
            "--dry-run",
            "--model",
            "gpt-4",
            "--max-turns",
            "10",
            "--verbose",
        ])
        .unwrap();
        assert!(args.dry_run);
        assert_eq!(args.model, Some("gpt-4".to_string()));
        assert_eq!(args.max_turns, Some(10));
        assert!(args.verbose);
    }

    #[test]
    fn test_resolve_config_dry_run_flag() {
        let args = Args {
            dry_run: true,
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(resolved.dry_run);
    }

    #[test]
    fn test_resolve_config_dry_run_default_false() {
        let args = Args::default();
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(!resolved.dry_run);
    }

    // ========================================================================
    // Check Flag Tests
    // ========================================================================

    #[test]
    fn test_args_check_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "--check"]).unwrap();
        assert!(args.check);
    }

    #[test]
    fn test_args_check_default_false() {
        let args = Args::try_parse_from(["codex-dashflow"]).unwrap();
        assert!(!args.check);
    }

    #[test]
    fn test_args_check_with_config() {
        let args = Args::try_parse_from([
            "codex-dashflow",
            "--check",
            "--config",
            "/path/to/config.toml",
        ])
        .unwrap();
        assert!(args.check);
        assert_eq!(args.config, Some(PathBuf::from("/path/to/config.toml")));
    }

    #[test]
    fn test_args_check_with_verbose() {
        // Can combine --check with --verbose for detailed output
        let args = Args::try_parse_from(["codex-dashflow", "--check", "--verbose"]).unwrap();
        assert!(args.check);
        assert!(args.verbose);
    }

    #[test]
    fn test_args_check_with_quiet() {
        // Can combine --check with --quiet for silent validation
        let args = Args::try_parse_from(["codex-dashflow", "--check", "--quiet"]).unwrap();
        assert!(args.check);
        assert!(args.quiet);
    }

    #[test]
    fn test_resolve_config_check_flag() {
        let args = Args {
            check: true,
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(resolved.check);
    }

    #[test]
    fn test_resolve_config_check_default_false() {
        let args = Args::default();
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(!resolved.check);
    }

    #[test]
    fn test_check_exit_code_values() {
        assert_eq!(CheckExitCode::Valid.code(), 0);
        assert_eq!(CheckExitCode::Warnings.code(), 1);
        assert_eq!(CheckExitCode::Errors.code(), 2);
    }

    #[test]
    fn test_check_exit_code_equality() {
        assert_eq!(CheckExitCode::Valid, CheckExitCode::Valid);
        assert_eq!(CheckExitCode::Warnings, CheckExitCode::Warnings);
        assert_eq!(CheckExitCode::Errors, CheckExitCode::Errors);
        assert_ne!(CheckExitCode::Valid, CheckExitCode::Warnings);
        assert_ne!(CheckExitCode::Valid, CheckExitCode::Errors);
    }

    #[test]
    #[serial]
    fn test_run_check_command_valid_config() {
        // Save all env vars that check_environment_for_issues examines
        let original = std::env::var("OPENAI_API_KEY").ok();
        let original_base = std::env::var("OPENAI_BASE_URL").ok();
        let orig_http = std::env::var("HTTP_PROXY").ok();
        let orig_http_lower = std::env::var("http_proxy").ok();
        let orig_https = std::env::var("HTTPS_PROXY").ok();
        let orig_https_lower = std::env::var("https_proxy").ok();
        let orig_no_proxy = std::env::var("NO_PROXY").ok();
        let orig_no_proxy_lower = std::env::var("no_proxy").ok();

        // Set up clean environment
        std::env::set_var("OPENAI_API_KEY", "test-key");
        std::env::remove_var("OPENAI_BASE_URL");
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("http_proxy");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("https_proxy");
        std::env::remove_var("NO_PROXY");
        std::env::remove_var("no_proxy");

        // Test with default config (no config file)
        let args = Args::default();
        let file_config = Config::default();

        let exit_code = run_check_command(&args, &file_config, None, true, false);
        assert_eq!(exit_code, CheckExitCode::Valid);

        // Restore all original values
        if let Some(val) = original {
            std::env::set_var("OPENAI_API_KEY", val);
        } else {
            std::env::remove_var("OPENAI_API_KEY");
        }
        if let Some(val) = original_base {
            std::env::set_var("OPENAI_BASE_URL", val);
        }
        if let Some(val) = orig_http {
            std::env::set_var("HTTP_PROXY", val);
        }
        if let Some(val) = orig_http_lower {
            std::env::set_var("http_proxy", val);
        }
        if let Some(val) = orig_https {
            std::env::set_var("HTTPS_PROXY", val);
        }
        if let Some(val) = orig_https_lower {
            std::env::set_var("https_proxy", val);
        }
        if let Some(val) = orig_no_proxy {
            std::env::set_var("NO_PROXY", val);
        }
        if let Some(val) = orig_no_proxy_lower {
            std::env::set_var("no_proxy", val);
        }
    }

    #[test]
    fn test_run_check_command_missing_system_prompt_file() {
        let args = Args {
            system_prompt_file: Some(PathBuf::from("/nonexistent/prompt.txt")),
            ..Default::default()
        };
        let file_config = Config::default();

        let exit_code = run_check_command(&args, &file_config, None, true, false);
        assert_eq!(exit_code, CheckExitCode::Errors);
    }

    #[test]
    fn test_run_check_command_missing_prompt_file() {
        let args = Args {
            prompt_file: Some(PathBuf::from("/nonexistent/prompt.txt")),
            ..Default::default()
        };
        let file_config = Config::default();

        let exit_code = run_check_command(&args, &file_config, None, true, false);
        assert_eq!(exit_code, CheckExitCode::Errors);
    }

    #[test]
    fn test_run_check_command_missing_working_dir() {
        let args = Args {
            working_dir: Some("/nonexistent/dir".to_string()),
            ..Default::default()
        };
        let file_config = Config::default();

        let exit_code = run_check_command(&args, &file_config, None, true, false);
        assert_eq!(exit_code, CheckExitCode::Errors);
    }

    #[test]
    fn test_run_check_command_dashstream_without_bootstrap() {
        let args = Args {
            dashstream: true,
            dashstream_bootstrap: None,
            ..Default::default()
        };
        let file_config = Config::default();

        let exit_code = run_check_command(&args, &file_config, None, true, false);
        assert_eq!(exit_code, CheckExitCode::Errors);
    }

    #[test]
    #[serial]
    fn test_run_check_command_quiet_verbose_warning() {
        // Ensure OPENAI_API_KEY is set for this test
        let original = std::env::var("OPENAI_API_KEY").ok();
        std::env::set_var("OPENAI_API_KEY", "test-key");
        // Remove OPENAI_BASE_URL to avoid URL warnings
        let original_base = std::env::var("OPENAI_BASE_URL").ok();
        std::env::remove_var("OPENAI_BASE_URL");

        let args = Args {
            quiet: true,
            verbose: true,
            ..Default::default()
        };
        let file_config = Config::default();

        let exit_code = run_check_command(&args, &file_config, None, true, false);
        assert_eq!(exit_code, CheckExitCode::Warnings);

        // Restore original values
        if let Some(val) = original {
            std::env::set_var("OPENAI_API_KEY", val);
        } else {
            std::env::remove_var("OPENAI_API_KEY");
        }
        if let Some(val) = original_base {
            std::env::set_var("OPENAI_BASE_URL", val);
        }
    }

    #[test]
    #[serial]
    fn test_check_environment_for_issues_missing_auth() {
        // Temporarily remove OPENAI_API_KEY if set
        let original = std::env::var("OPENAI_API_KEY").ok();
        std::env::remove_var("OPENAI_API_KEY");

        // Also clear any stored auth by pointing to non-existent dir
        let home = std::env::var("CODEX_DASHFLOW_HOME").ok();
        let temp_path = std::env::temp_dir().join(format!("codex_test_{}", std::process::id()));
        std::env::set_var("CODEX_DASHFLOW_HOME", &temp_path);

        let mut issues = Vec::new();
        check_environment_for_issues(&mut issues);

        // Should have an error for missing authentication
        assert!(issues
            .iter()
            .any(|i| i.field == "Authentication" && i.severity == "error"));

        // Restore original values
        if let Some(val) = original {
            std::env::set_var("OPENAI_API_KEY", val);
        }
        if let Some(val) = home {
            std::env::set_var("CODEX_DASHFLOW_HOME", val);
        } else {
            std::env::remove_var("CODEX_DASHFLOW_HOME");
        }
    }

    #[test]
    #[serial]
    fn test_check_environment_for_issues_invalid_base_url() {
        // Set an invalid OPENAI_BASE_URL
        let original = std::env::var("OPENAI_BASE_URL").ok();
        std::env::set_var("OPENAI_BASE_URL", "not-a-valid-url");

        let mut issues = Vec::new();
        check_environment_for_issues(&mut issues);

        // Should have a warning for invalid URL
        assert!(issues
            .iter()
            .any(|i| i.field == "OPENAI_BASE_URL" && i.severity == "warning"));

        // Restore original value
        if let Some(val) = original {
            std::env::set_var("OPENAI_BASE_URL", val);
        } else {
            std::env::remove_var("OPENAI_BASE_URL");
        }
    }

    #[test]
    #[serial]
    fn test_check_environment_for_issues_http_base_url() {
        // Save and clear all environment variables that check_environment_for_issues checks
        let original_base = std::env::var("OPENAI_BASE_URL").ok();
        let original_key = std::env::var("OPENAI_API_KEY").ok();
        let orig_http = std::env::var("HTTP_PROXY").ok();
        let orig_http_lower = std::env::var("http_proxy").ok();
        let orig_https = std::env::var("HTTPS_PROXY").ok();
        let orig_https_lower = std::env::var("https_proxy").ok();
        let orig_no_proxy = std::env::var("NO_PROXY").ok();
        let orig_no_proxy_lower = std::env::var("no_proxy").ok();

        // Clear proxy vars to avoid interference
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("http_proxy");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("https_proxy");
        std::env::remove_var("NO_PROXY");
        std::env::remove_var("no_proxy");

        // Set an HTTP OPENAI_BASE_URL (should warn about not using HTTPS)
        std::env::set_var("OPENAI_BASE_URL", "http://localhost:8080/v1");
        // Set API key to avoid error masking the warning
        std::env::set_var("OPENAI_API_KEY", "test-key");

        let mut issues = Vec::new();
        check_environment_for_issues(&mut issues);

        // Should have a warning for using HTTP
        assert!(
            issues.iter().any(|i| i.field == "OPENAI_BASE_URL"
                && i.severity == "warning"
                && i.message.contains("HTTP")),
            "Expected OPENAI_BASE_URL HTTP warning, got issues: {:?}",
            issues
        );

        // Restore original values
        if let Some(val) = original_base {
            std::env::set_var("OPENAI_BASE_URL", val);
        } else {
            std::env::remove_var("OPENAI_BASE_URL");
        }
        if let Some(val) = original_key {
            std::env::set_var("OPENAI_API_KEY", val);
        } else {
            std::env::remove_var("OPENAI_API_KEY");
        }
        if let Some(val) = orig_http {
            std::env::set_var("HTTP_PROXY", val);
        }
        if let Some(val) = orig_http_lower {
            std::env::set_var("http_proxy", val);
        }
        if let Some(val) = orig_https {
            std::env::set_var("HTTPS_PROXY", val);
        }
        if let Some(val) = orig_https_lower {
            std::env::set_var("https_proxy", val);
        }
        if let Some(val) = orig_no_proxy {
            std::env::set_var("NO_PROXY", val);
        }
        if let Some(val) = orig_no_proxy_lower {
            std::env::set_var("no_proxy", val);
        }
    }

    #[test]
    #[serial]
    fn test_check_environment_for_issues_https_base_url() {
        // Save and clear all environment variables that check_environment_for_issues checks
        let original_base = std::env::var("OPENAI_BASE_URL").ok();
        let original_key = std::env::var("OPENAI_API_KEY").ok();
        let orig_http = std::env::var("HTTP_PROXY").ok();
        let orig_http_lower = std::env::var("http_proxy").ok();
        let orig_https = std::env::var("HTTPS_PROXY").ok();
        let orig_https_lower = std::env::var("https_proxy").ok();
        let orig_no_proxy = std::env::var("NO_PROXY").ok();
        let orig_no_proxy_lower = std::env::var("no_proxy").ok();

        // Clear proxy vars to avoid interference
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("http_proxy");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("https_proxy");
        std::env::remove_var("NO_PROXY");
        std::env::remove_var("no_proxy");

        // Set a valid HTTPS OPENAI_BASE_URL (should not warn)
        std::env::set_var("OPENAI_BASE_URL", "https://api.example.com/v1");
        std::env::set_var("OPENAI_API_KEY", "test-key");

        let mut issues = Vec::new();
        check_environment_for_issues(&mut issues);

        // Should NOT have any OPENAI_BASE_URL warnings
        assert!(
            !issues.iter().any(|i| i.field == "OPENAI_BASE_URL"),
            "Unexpected OPENAI_BASE_URL issues: {:?}",
            issues
        );

        // Restore original values
        if let Some(val) = original_base {
            std::env::set_var("OPENAI_BASE_URL", val);
        } else {
            std::env::remove_var("OPENAI_BASE_URL");
        }
        if let Some(val) = original_key {
            std::env::set_var("OPENAI_API_KEY", val);
        } else {
            std::env::remove_var("OPENAI_API_KEY");
        }
        if let Some(val) = orig_http {
            std::env::set_var("HTTP_PROXY", val);
        }
        if let Some(val) = orig_http_lower {
            std::env::set_var("http_proxy", val);
        }
        if let Some(val) = orig_https {
            std::env::set_var("HTTPS_PROXY", val);
        }
        if let Some(val) = orig_https_lower {
            std::env::set_var("https_proxy", val);
        }
        if let Some(val) = orig_no_proxy {
            std::env::set_var("NO_PROXY", val);
        }
        if let Some(val) = orig_no_proxy_lower {
            std::env::set_var("no_proxy", val);
        }
    }

    #[test]
    fn test_check_issue_serialization() {
        let issue = CheckIssue {
            severity: "error".to_string(),
            field: "test_field".to_string(),
            message: "Test message".to_string(),
            suggestion: Some("Test suggestion".to_string()),
        };

        let json = serde_json::to_string(&issue).unwrap();
        assert!(json.contains(r#""severity":"error""#));
        assert!(json.contains(r#""field":"test_field""#));
        assert!(json.contains(r#""message":"Test message""#));
        assert!(json.contains(r#""suggestion":"Test suggestion""#));
    }

    #[test]
    fn test_check_issue_serialization_no_suggestion() {
        let issue = CheckIssue {
            severity: "warning".to_string(),
            field: "test_field".to_string(),
            message: "Test message".to_string(),
            suggestion: None,
        };

        let json = serde_json::to_string(&issue).unwrap();
        assert!(json.contains(r#""severity":"warning""#));
        // suggestion should be omitted when None
        assert!(!json.contains("suggestion"));
    }

    #[test]
    fn test_check_result_serialization() {
        let result = CheckResult {
            config_found: true,
            config_path: Some("/path/to/config.toml".to_string()),
            config_parsed: true,
            parse_error: None,
            issues: vec![],
            error_count: 0,
            warning_count: 0,
            status: "valid".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains(r#""config_found":true"#));
        assert!(json.contains(r#""config_path":"/path/to/config.toml""#));
        assert!(json.contains(r#""config_parsed":true"#));
        assert!(json.contains(r#""status":"valid""#));
        // parse_error should be omitted when None
        assert!(!json.contains("parse_error"));
    }

    // ========================================================================
    // JSON Flag Tests (for --check and --dry-run)
    // ========================================================================

    #[test]
    fn test_args_json_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "--json"]).unwrap();
        assert!(args.json);
    }

    #[test]
    fn test_args_json_default_false() {
        let args = Args::try_parse_from(["codex-dashflow"]).unwrap();
        assert!(!args.json);
    }

    #[test]
    fn test_args_json_with_check() {
        let args = Args::try_parse_from(["codex-dashflow", "--check", "--json"]).unwrap();
        assert!(args.check);
        assert!(args.json);
    }

    #[test]
    fn test_args_json_with_dry_run() {
        let args = Args::try_parse_from(["codex-dashflow", "--dry-run", "--json"]).unwrap();
        assert!(args.dry_run);
        assert!(args.json);
    }

    #[test]
    fn test_resolve_config_json_flag() {
        let args = Args {
            json: true,
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(resolved.json);
    }

    #[test]
    fn test_resolve_config_json_default_false() {
        let args = Args::default();
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(!resolved.json);
    }

    #[test]
    #[serial]
    fn test_run_check_command_json_output() {
        // Save all env vars that check_environment_for_issues examines
        let original = std::env::var("OPENAI_API_KEY").ok();
        let original_base = std::env::var("OPENAI_BASE_URL").ok();
        let orig_http = std::env::var("HTTP_PROXY").ok();
        let orig_http_lower = std::env::var("http_proxy").ok();
        let orig_https = std::env::var("HTTPS_PROXY").ok();
        let orig_https_lower = std::env::var("https_proxy").ok();
        let orig_no_proxy = std::env::var("NO_PROXY").ok();
        let orig_no_proxy_lower = std::env::var("no_proxy").ok();

        // Set up clean environment
        std::env::set_var("OPENAI_API_KEY", "test-key");
        std::env::remove_var("OPENAI_BASE_URL");
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("http_proxy");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("https_proxy");
        std::env::remove_var("NO_PROXY");
        std::env::remove_var("no_proxy");

        // Test that JSON output is correctly formatted
        let args = Args::default();
        let file_config = Config::default();

        // Run with json=true, quiet=false to get output
        // We can't easily capture stdout in tests, but we can verify it doesn't panic
        let exit_code = run_check_command(&args, &file_config, None, false, true);
        assert_eq!(exit_code, CheckExitCode::Valid);

        // Restore all original values
        if let Some(val) = original {
            std::env::set_var("OPENAI_API_KEY", val);
        } else {
            std::env::remove_var("OPENAI_API_KEY");
        }
        if let Some(val) = original_base {
            std::env::set_var("OPENAI_BASE_URL", val);
        }
        if let Some(val) = orig_http {
            std::env::set_var("HTTP_PROXY", val);
        }
        if let Some(val) = orig_http_lower {
            std::env::set_var("http_proxy", val);
        }
        if let Some(val) = orig_https {
            std::env::set_var("HTTPS_PROXY", val);
        }
        if let Some(val) = orig_https_lower {
            std::env::set_var("https_proxy", val);
        }
        if let Some(val) = orig_no_proxy {
            std::env::set_var("NO_PROXY", val);
        }
        if let Some(val) = orig_no_proxy_lower {
            std::env::set_var("no_proxy", val);
        }
    }

    // ========================================================================
    // DryRunConfig JSON Serialization Tests
    // ========================================================================

    #[test]
    fn test_dry_run_config_serialization_basic() {
        let resolved = ResolvedConfig {
            model: "gpt-4o".to_string(),
            max_turns: 10,
            working_dir: "/tmp".to_string(),
            session_id: None,
            use_mock_llm: false,
            verbose: false,
            quiet: false,
            dry_run: true,
            check: false,
            json: true,
            stdin: false,
            prompt_file: None,
            exec_prompt: None,
            dashstream: None,
            approval_mode: ApprovalMode::OnDangerous,
            policy_config: PolicyConfig::default(),
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::default(),
            postgres: None,
            ..Default::default()
        };

        let dry_run_config = DryRunConfig::from_resolved(&resolved, None);
        let json = serde_json::to_string_pretty(&dry_run_config).unwrap();

        // Verify it serializes without error and contains expected fields
        assert!(json.contains("\"model\": \"gpt-4o\""));
        assert!(json.contains("\"working_dir\": \"/tmp\""));
        assert!(json.contains("\"mode\": \"tui\""));
        assert!(json.contains("\"mock_llm\": false"));
    }

    #[test]
    fn test_dry_run_config_serialization_with_exec_mode() {
        let resolved = ResolvedConfig {
            model: "gpt-4".to_string(),
            max_turns: 5,
            working_dir: "/home/user".to_string(),
            session_id: Some("test-session".to_string()),
            use_mock_llm: true,
            verbose: true,
            quiet: false,
            dry_run: true,
            check: false,
            json: true,
            stdin: false,
            prompt_file: None,
            exec_prompt: Some("Hello, world!".to_string()),
            dashstream: None,
            approval_mode: ApprovalMode::OnFirstUse,
            policy_config: PolicyConfig::default(),
            collect_training: true,
            load_optimized_prompts: true,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::default(),
            postgres: None,
            ..Default::default()
        };

        let dry_run_config = DryRunConfig::from_resolved(&resolved, None);
        let json = serde_json::to_string_pretty(&dry_run_config).unwrap();

        assert!(json.contains("\"mode\": \"exec\""));
        assert!(json.contains("\"prompt_source\": \"inline\""));
        assert!(json.contains("\"prompt_preview\": \"Hello, world!\""));
        assert!(json.contains("\"session_id\": \"test-session\""));
        assert!(json.contains("\"mock_llm\": true"));
        assert!(json.contains("\"verbose\": true"));
    }

    #[test]
    fn test_dry_run_config_serialization_with_stdin() {
        let resolved = ResolvedConfig {
            model: "gpt-4o-mini".to_string(),
            max_turns: 0, // unlimited
            working_dir: ".".to_string(),
            session_id: None,
            use_mock_llm: false,
            verbose: false,
            quiet: true,
            dry_run: true,
            check: false,
            json: true,
            stdin: true,
            prompt_file: None,
            exec_prompt: None,
            dashstream: None,
            approval_mode: ApprovalMode::OnDangerous,
            policy_config: PolicyConfig::default(),
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::default(),
            postgres: None,
            ..Default::default()
        };

        let dry_run_config = DryRunConfig::from_resolved(&resolved, None);
        let json = serde_json::to_string_pretty(&dry_run_config).unwrap();

        assert!(json.contains("\"mode\": \"exec\""));
        assert!(json.contains("\"prompt_source\": \"stdin\""));
        assert!(json.contains("\"quiet\": true"));
        // max_turns: 0 should serialize as null for unlimited (via DryRunMaxTurns::Unlimited)
        assert!(json.contains("\"max_turns\": null"));
    }

    #[test]
    fn test_dry_run_config_serialization_with_prompt_file() {
        let resolved = ResolvedConfig {
            model: "claude-3".to_string(),
            max_turns: 100,
            working_dir: "/project".to_string(),
            session_id: None,
            use_mock_llm: false,
            verbose: false,
            quiet: false,
            dry_run: true,
            check: false,
            json: true,
            stdin: false,
            prompt_file: Some(PathBuf::from("/path/to/prompt.txt")),
            exec_prompt: None,
            dashstream: None,
            approval_mode: ApprovalMode::Never,
            policy_config: PolicyConfig::default(),
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::default(),
            postgres: None,
            ..Default::default()
        };

        let dry_run_config = DryRunConfig::from_resolved(&resolved, None);
        let json = serde_json::to_string_pretty(&dry_run_config).unwrap();

        assert!(json.contains("\"mode\": \"exec\""));
        assert!(json.contains("\"prompt_source\": \"file\""));
        assert!(json.contains("\"/path/to/prompt.txt\""));
    }

    #[test]
    fn test_dry_run_config_serialization_with_dashstream() {
        let resolved = ResolvedConfig {
            model: "gpt-4o".to_string(),
            max_turns: 10,
            working_dir: "/tmp".to_string(),
            session_id: None,
            use_mock_llm: false,
            verbose: false,
            quiet: false,
            dry_run: true,
            check: false,
            json: true,
            stdin: false,
            prompt_file: None,
            exec_prompt: None,
            dashstream: Some(DashStreamConfig {
                bootstrap_servers: "localhost:9092".to_string(),
                topic: "codex-events".to_string(),
            }),
            approval_mode: ApprovalMode::OnDangerous,
            policy_config: PolicyConfig::default(),
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::default(),
            postgres: None,
            ..Default::default()
        };

        let dry_run_config = DryRunConfig::from_resolved(&resolved, None);
        let json = serde_json::to_string_pretty(&dry_run_config).unwrap();

        assert!(json.contains("\"bootstrap_servers\": \"localhost:9092\""));
        assert!(json.contains("\"topic\": \"codex-events\""));
    }

    #[test]
    fn test_dry_run_config_serialization_with_config_path() {
        let resolved = ResolvedConfig {
            model: "gpt-4o".to_string(),
            max_turns: 10,
            working_dir: "/tmp".to_string(),
            session_id: None,
            use_mock_llm: false,
            verbose: false,
            quiet: false,
            dry_run: true,
            check: false,
            json: true,
            stdin: false,
            prompt_file: None,
            exec_prompt: None,
            dashstream: None,
            approval_mode: ApprovalMode::OnDangerous,
            policy_config: PolicyConfig::default(),
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::default(),
            postgres: None,
            ..Default::default()
        };

        let config_path = std::path::Path::new("/home/user/.codex/config.toml");
        let dry_run_config = DryRunConfig::from_resolved(&resolved, Some(config_path));
        let json = serde_json::to_string_pretty(&dry_run_config).unwrap();

        assert!(json.contains("\"config_file\": \"/home/user/.codex/config.toml\""));
    }

    #[test]
    fn test_dry_run_config_serialization_long_prompt_truncation() {
        let long_prompt = "a".repeat(100);
        let resolved = ResolvedConfig {
            model: "gpt-4o".to_string(),
            max_turns: 10,
            working_dir: "/tmp".to_string(),
            session_id: None,
            use_mock_llm: false,
            verbose: false,
            quiet: false,
            dry_run: true,
            check: false,
            json: true,
            stdin: false,
            prompt_file: None,
            exec_prompt: Some(long_prompt),
            dashstream: None,
            approval_mode: ApprovalMode::OnDangerous,
            policy_config: PolicyConfig::default(),
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::default(),
            postgres: None,
            ..Default::default()
        };

        let dry_run_config = DryRunConfig::from_resolved(&resolved, None);
        let json = serde_json::to_string_pretty(&dry_run_config).unwrap();

        // Prompt preview should be truncated to 50 chars + "..."
        assert!(json.contains(&"a".repeat(50)));
        assert!(json.contains("..."));
        // Full 100 char prompt should not be present
        assert!(!json.contains(&"a".repeat(51)));
    }

    #[test]
    fn test_dry_run_max_turns_serialization() {
        // Test limited turns
        let limited = DryRunMaxTurns::Limited(42);
        let json = serde_json::to_value(&limited).unwrap();
        assert_eq!(json, serde_json::json!(42));

        // Test unlimited turns
        let unlimited = DryRunMaxTurns::Unlimited;
        let json = serde_json::to_value(&unlimited).unwrap();
        assert!(json.is_null());
    }

    #[test]
    fn test_dry_run_max_turns_display() {
        assert_eq!(format!("{}", DryRunMaxTurns::Limited(10)), "10");
        assert_eq!(format!("{}", DryRunMaxTurns::Unlimited), "unlimited");
    }

    #[test]
    fn test_dry_run_config_system_prompt_sources() {
        // Test custom system prompt
        let resolved_custom = ResolvedConfig {
            system_prompt: Some("Custom prompt".to_string()),
            system_prompt_file: None,
            ..Default::default()
        };
        let config = DryRunConfig::from_resolved(&resolved_custom, None);
        assert_eq!(config.prompts.system_prompt_source, "custom");

        // Test file system prompt
        let resolved_file = ResolvedConfig {
            system_prompt: None,
            system_prompt_file: Some(PathBuf::from("/path/to/system.txt")),
            ..Default::default()
        };
        let config = DryRunConfig::from_resolved(&resolved_file, None);
        assert_eq!(config.prompts.system_prompt_source, "file");
        assert_eq!(
            config.prompts.system_prompt_file,
            Some("/path/to/system.txt".to_string())
        );

        // Test default system prompt
        let resolved_default = ResolvedConfig::default();
        let config = DryRunConfig::from_resolved(&resolved_default, None);
        assert_eq!(config.prompts.system_prompt_source, "default");
    }

    // ========================================================================
    // Proxy Environment Variable Tests
    // ========================================================================

    #[test]
    #[serial]
    fn test_check_proxy_env_var_valid_http() {
        // Save and clear existing proxy vars
        let orig_http = std::env::var("HTTP_PROXY").ok();
        let orig_https = std::env::var("HTTPS_PROXY").ok();
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("http_proxy");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("https_proxy");
        std::env::remove_var("NO_PROXY");
        std::env::remove_var("no_proxy");

        // Set a valid HTTP proxy
        std::env::set_var("HTTP_PROXY", "http://proxy.example.com:8080");

        let mut issues = Vec::new();
        check_proxy_env_var(&mut issues, "HTTP_PROXY");

        // Valid http:// proxy should not generate any issues
        assert!(
            issues.is_empty(),
            "Valid HTTP_PROXY should not generate issues: {:?}",
            issues
        );

        // Cleanup
        std::env::remove_var("HTTP_PROXY");
        if let Some(val) = orig_http {
            std::env::set_var("HTTP_PROXY", val);
        }
        if let Some(val) = orig_https {
            std::env::set_var("HTTPS_PROXY", val);
        }
    }

    #[test]
    #[serial]
    fn test_check_proxy_env_var_invalid_url() {
        let orig = std::env::var("HTTP_PROXY").ok();
        std::env::set_var("HTTP_PROXY", "not-a-valid-url");

        let mut issues = Vec::new();
        check_proxy_env_var(&mut issues, "HTTP_PROXY");

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].field, "HTTP_PROXY");
        assert!(issues[0].message.contains("Invalid URL format"));

        // Cleanup
        std::env::remove_var("HTTP_PROXY");
        if let Some(val) = orig {
            std::env::set_var("HTTP_PROXY", val);
        }
    }

    #[test]
    #[serial]
    fn test_check_proxy_env_var_unusual_scheme() {
        let orig = std::env::var("HTTP_PROXY").ok();
        std::env::set_var("HTTP_PROXY", "ftp://proxy.example.com:21");

        let mut issues = Vec::new();
        check_proxy_env_var(&mut issues, "HTTP_PROXY");

        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("Unusual proxy scheme"));

        // Cleanup
        std::env::remove_var("HTTP_PROXY");
        if let Some(val) = orig {
            std::env::set_var("HTTP_PROXY", val);
        }
    }

    #[test]
    #[serial]
    fn test_check_proxy_env_var_https_with_http_scheme() {
        let orig = std::env::var("HTTPS_PROXY").ok();
        std::env::set_var("HTTPS_PROXY", "http://proxy.example.com:8080");

        let mut issues = Vec::new();
        check_proxy_env_var(&mut issues, "HTTPS_PROXY");

        // Should warn about using HTTP scheme for HTTPS_PROXY
        assert_eq!(issues.len(), 1);
        assert!(issues[0]
            .message
            .contains("credentials may be sent unencrypted"));

        // Cleanup
        std::env::remove_var("HTTPS_PROXY");
        if let Some(val) = orig {
            std::env::set_var("HTTPS_PROXY", val);
        }
    }

    #[test]
    #[serial]
    fn test_check_proxy_env_var_valid_socks5() {
        let orig = std::env::var("HTTP_PROXY").ok();
        std::env::set_var("HTTP_PROXY", "socks5://localhost:1080");

        let mut issues = Vec::new();
        check_proxy_env_var(&mut issues, "HTTP_PROXY");

        // socks5 is a valid proxy scheme
        assert!(
            issues.is_empty(),
            "Valid socks5 proxy should not generate issues: {:?}",
            issues
        );

        // Cleanup
        std::env::remove_var("HTTP_PROXY");
        if let Some(val) = orig {
            std::env::set_var("HTTP_PROXY", val);
        }
    }

    #[test]
    #[serial]
    fn test_check_environment_no_proxy_empty_entry() {
        // Save original values
        let orig_no_proxy = std::env::var("NO_PROXY").ok();
        let orig_api_key = std::env::var("OPENAI_API_KEY").ok();
        let orig_http = std::env::var("HTTP_PROXY").ok();
        let orig_https = std::env::var("HTTPS_PROXY").ok();

        // Clear proxy-related vars to isolate the test
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("http_proxy");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("https_proxy");
        std::env::remove_var("OPENAI_BASE_URL");

        // Set required key and problematic NO_PROXY
        std::env::set_var("OPENAI_API_KEY", "test-key");
        std::env::set_var("NO_PROXY", "localhost,,example.com");

        let mut issues = Vec::new();
        check_environment_for_issues(&mut issues);

        // Should warn about empty entry in NO_PROXY
        let no_proxy_issues: Vec<_> = issues.iter().filter(|i| i.field == "NO_PROXY").collect();
        assert_eq!(no_proxy_issues.len(), 1);
        assert!(no_proxy_issues[0].message.contains("empty entry"));

        // Cleanup
        std::env::remove_var("NO_PROXY");
        if let Some(val) = orig_no_proxy {
            std::env::set_var("NO_PROXY", val);
        }
        if let Some(val) = orig_api_key {
            std::env::set_var("OPENAI_API_KEY", val);
        } else {
            std::env::remove_var("OPENAI_API_KEY");
        }
        if let Some(val) = orig_http {
            std::env::set_var("HTTP_PROXY", val);
        }
        if let Some(val) = orig_https {
            std::env::set_var("HTTPS_PROXY", val);
        }
    }

    #[test]
    fn test_args_stdin_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "--stdin"]).unwrap();
        assert!(args.stdin);
    }

    #[test]
    fn test_args_stdin_default_false() {
        let args = Args::try_parse_from(["codex-dashflow"]).unwrap();
        assert!(!args.stdin);
    }

    #[test]
    fn test_args_stdin_with_exec() {
        // Can use --stdin without --exec (stdin IS the exec prompt)
        let args = Args::try_parse_from(["codex-dashflow", "--stdin"]).unwrap();
        assert!(args.stdin);
        assert!(args.exec.is_none());
    }

    #[test]
    fn test_args_stdin_with_exec_together() {
        // Can use --stdin with --exec (stdin overrides --exec when stdin is set)
        let args =
            Args::try_parse_from(["codex-dashflow", "--stdin", "--exec", "ignored"]).unwrap();
        assert!(args.stdin);
        assert_eq!(args.exec, Some("ignored".to_string()));
    }

    #[test]
    fn test_resolve_config_stdin_flag() {
        let args = Args {
            stdin: true,
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(resolved.stdin);
    }

    #[test]
    fn test_resolve_config_stdin_default_false() {
        let args = Args::default();
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(!resolved.stdin);
    }

    #[test]
    fn test_args_prompt_file_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "--prompt-file", "/path/to/prompt.txt"])
            .unwrap();
        assert_eq!(args.prompt_file, Some(PathBuf::from("/path/to/prompt.txt")));
    }

    #[test]
    fn test_args_prompt_file_default_none() {
        let args = Args::try_parse_from(["codex-dashflow"]).unwrap();
        assert!(args.prompt_file.is_none());
    }

    #[test]
    fn test_args_prompt_file_with_exec() {
        // Both can be set (prompt_file takes precedence in main.rs)
        let args = Args::try_parse_from([
            "codex-dashflow",
            "--prompt-file",
            "/path/to/prompt.txt",
            "--exec",
            "inline prompt",
        ])
        .unwrap();
        assert_eq!(args.prompt_file, Some(PathBuf::from("/path/to/prompt.txt")));
        assert_eq!(args.exec, Some("inline prompt".to_string()));
    }

    #[test]
    fn test_resolve_config_prompt_file_flag() {
        let args = Args {
            prompt_file: Some(PathBuf::from("/path/to/prompt.txt")),
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert_eq!(
            resolved.prompt_file,
            Some(PathBuf::from("/path/to/prompt.txt"))
        );
    }

    #[test]
    fn test_resolve_config_prompt_file_default_none() {
        let args = Args::default();
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(resolved.prompt_file.is_none());
    }

    #[test]
    fn test_args_config_path() {
        let args =
            Args::try_parse_from(["codex-dashflow", "--config", "/path/to/config.toml"]).unwrap();
        assert_eq!(args.config, Some(PathBuf::from("/path/to/config.toml")));
    }

    #[test]
    fn test_args_combined() {
        let args = Args::try_parse_from([
            "codex-dashflow",
            "-e",
            "test prompt",
            "-d",
            "/tmp",
            "-t",
            "15",
            "-s",
            "session123",
            "--mock",
            "-m",
            "gpt-4",
            "-v",
        ])
        .unwrap();

        assert_eq!(args.exec, Some("test prompt".to_string()));
        assert_eq!(args.working_dir, Some("/tmp".to_string()));
        assert_eq!(args.max_turns, Some(15));
        assert_eq!(args.session, Some("session123".to_string()));
        assert!(args.mock);
        assert_eq!(args.model, Some("gpt-4".to_string()));
        assert!(args.verbose);
    }

    #[test]
    fn test_resolve_config_defaults() {
        let args = Args::default();
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);

        // Should use file_config defaults when args are None
        assert_eq!(resolved.model, file_config.model);
        assert_eq!(resolved.max_turns, file_config.max_turns);
        assert_eq!(resolved.working_dir, ".");
        assert!(resolved.session_id.is_none());
        assert!(!resolved.use_mock_llm);
        assert!(!resolved.verbose);
        assert!(resolved.exec_prompt.is_none());
        assert!(resolved.dashstream.is_none());
        assert_eq!(resolved.approval_mode, ApprovalMode::OnDangerous);
        assert_eq!(resolved.sandbox_mode, SandboxMode::ReadOnly); // Default
    }

    #[test]
    fn test_resolve_config_args_override_file() {
        let args = Args {
            model: Some("custom-model".to_string()),
            max_turns: Some(20),
            working_dir: Some("/custom/path".to_string()),
            session: Some("my-session".to_string()),
            mock: true,
            verbose: true,
            exec: Some("test prompt".to_string()),
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);

        // CLI args should override file config
        assert_eq!(resolved.model, "custom-model");
        assert_eq!(resolved.max_turns, 20);
        assert_eq!(resolved.working_dir, "/custom/path");
        assert_eq!(resolved.session_id, Some("my-session".to_string()));
        assert!(resolved.use_mock_llm);
        assert!(resolved.verbose);
        assert_eq!(resolved.exec_prompt, Some("test prompt".to_string()));
    }

    #[test]
    fn test_resolve_config_partial_override() {
        let args = Args {
            model: Some("custom-model".to_string()),
            // max_turns not set, should use file_config
            ..Default::default()
        };

        let file_config = Config {
            max_turns: 50,
            working_dir: Some("/file/path".to_string()),
            ..Default::default()
        };

        let resolved = resolve_config(&args, &file_config);

        assert_eq!(resolved.model, "custom-model"); // from args
        assert_eq!(resolved.max_turns, 50); // from file_config
        assert_eq!(resolved.working_dir, "/file/path"); // from file_config
    }

    #[test]
    fn test_build_agent_state_basic() {
        let config = ResolvedConfig {
            model: "test-model".to_string(),
            max_turns: 10,
            working_dir: ".".to_string(),
            session_id: Some("test-session".to_string()),
            use_mock_llm: false,
            verbose: false,
            quiet: false,
            dry_run: false,
            check: false,
            json: false,
            stdin: false,
            prompt_file: None,
            exec_prompt: None,
            dashstream: None,
            approval_mode: ApprovalMode::OnDangerous,
            policy_config: PolicyConfig::default(),
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::ReadOnly,
            postgres: None,
            ..Default::default()
        };

        let state = build_agent_state(&config);

        assert_eq!(state.session_id, "test-session");
        assert_eq!(state.llm_config.model, "test-model");
        assert_eq!(state.max_turns, 10);
        assert!(state.has_exec_policy()); // Policy should be set
    }

    #[test]
    fn test_build_agent_state_with_mock() {
        let config = ResolvedConfig {
            model: "test-model".to_string(),
            max_turns: 0, // unlimited
            working_dir: ".".to_string(),
            session_id: None, // will generate UUID
            use_mock_llm: true,
            verbose: false,
            quiet: false,
            dry_run: false,
            check: false,
            json: false,
            stdin: false,
            prompt_file: None,
            exec_prompt: None,
            dashstream: None,
            approval_mode: ApprovalMode::OnDangerous,
            policy_config: PolicyConfig::default(),
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::ReadOnly,
            postgres: None,
            ..Default::default()
        };

        let state = build_agent_state(&config);

        // Session ID should be auto-generated UUID
        assert!(!state.session_id.is_empty());
        assert!(state.session_id.contains('-')); // UUID format

        // Mock LLM should be enabled (stored on AgentState, not LlmConfig)
        assert!(state.use_mock_llm);
    }

    #[test]
    fn test_build_agent_state_max_turns_zero_means_unlimited() {
        let config = ResolvedConfig {
            model: "test-model".to_string(),
            max_turns: 0, // 0 means unlimited
            working_dir: ".".to_string(),
            session_id: None,
            use_mock_llm: false,
            verbose: false,
            quiet: false,
            dry_run: false,
            check: false,
            json: false,
            stdin: false,
            prompt_file: None,
            exec_prompt: None,
            dashstream: None,
            approval_mode: ApprovalMode::OnDangerous,
            policy_config: PolicyConfig::default(),
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::ReadOnly,
            postgres: None,
            ..Default::default()
        };

        let state = build_agent_state(&config);

        // max_turns = 0 means unlimited, which is the default
        // The code skips setting when max_turns is 0, leaving default (0 = unlimited)
        assert_eq!(state.max_turns, 0);
    }

    #[test]
    fn test_build_agent_state_custom_max_turns() {
        let config = ResolvedConfig {
            model: "test-model".to_string(),
            max_turns: 25,
            working_dir: ".".to_string(),
            session_id: None,
            use_mock_llm: false,
            verbose: false,
            quiet: false,
            dry_run: false,
            check: false,
            json: false,
            stdin: false,
            prompt_file: None,
            exec_prompt: None,
            dashstream: None,
            approval_mode: ApprovalMode::OnDangerous,
            policy_config: PolicyConfig::default(),
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::ReadOnly,
            postgres: None,
            ..Default::default()
        };

        let state = build_agent_state(&config);

        // Custom max_turns should be set
        assert_eq!(state.max_turns, 25);
    }

    #[test]
    fn test_exec_stream_callback_new() {
        let callback = ExecStreamCallback::new(true);
        assert!(callback.verbose);

        let callback = ExecStreamCallback::new(false);
        assert!(!callback.verbose);
    }

    #[test]
    fn test_args_dashstream_flag() {
        let args = Args::try_parse_from([
            "codex-dashflow",
            "--dashstream",
            "--dashstream-bootstrap",
            "localhost:9092",
        ])
        .unwrap();
        assert!(args.dashstream);
        assert_eq!(
            args.dashstream_bootstrap,
            Some("localhost:9092".to_string())
        );
        assert_eq!(args.dashstream_topic, "codex-events"); // default
    }

    #[test]
    fn test_args_dashstream_custom_topic() {
        let args = Args::try_parse_from([
            "codex-dashflow",
            "--dashstream",
            "--dashstream-bootstrap",
            "kafka.example.com:9093",
            "--dashstream-topic",
            "my-custom-topic",
        ])
        .unwrap();
        assert!(args.dashstream);
        assert_eq!(
            args.dashstream_bootstrap,
            Some("kafka.example.com:9093".to_string())
        );
        assert_eq!(args.dashstream_topic, "my-custom-topic");
    }

    #[test]
    fn test_resolve_config_with_dashstream() {
        let args = Args {
            dashstream: true,
            dashstream_bootstrap: Some("localhost:9092".to_string()),
            dashstream_topic: "test-topic".to_string(),
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);

        assert!(resolved.dashstream.is_some());
        let ds = resolved.dashstream.unwrap();
        assert_eq!(ds.bootstrap_servers, "localhost:9092");
        assert_eq!(ds.topic, "test-topic");
    }

    #[test]
    fn test_resolve_config_dashstream_requires_bootstrap() {
        // If --dashstream is set but no bootstrap servers, dashstream should be None
        let args = Args {
            dashstream: true,
            dashstream_bootstrap: None, // No bootstrap servers
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);

        // Should be None because bootstrap servers are required
        assert!(resolved.dashstream.is_none());
    }

    #[test]
    fn test_resolve_config_kafka_from_file_config() {
        // Audit #71: Kafka config from file should be used when CLI flags not set
        let args = Args::default(); // No dashstream CLI flags

        let mut file_config = Config::default();
        file_config.dashflow.kafka_bootstrap_servers = Some("kafka.example.com:9092".to_string());
        file_config.dashflow.kafka_topic = "file-events".to_string();

        let resolved = resolve_config(&args, &file_config);

        assert!(resolved.dashstream.is_some());
        let ds = resolved.dashstream.unwrap();
        assert_eq!(ds.bootstrap_servers, "kafka.example.com:9092");
        assert_eq!(ds.topic, "file-events");
    }

    #[test]
    fn test_resolve_config_cli_dashstream_overrides_file_kafka() {
        // CLI --dashstream flags should override file config kafka settings
        let args = Args {
            dashstream: true,
            dashstream_bootstrap: Some("cli-kafka:9092".to_string()),
            dashstream_topic: "cli-events".to_string(),
            ..Default::default()
        };

        let mut file_config = Config::default();
        file_config.dashflow.kafka_bootstrap_servers = Some("file-kafka:9092".to_string());
        file_config.dashflow.kafka_topic = "file-events".to_string();

        let resolved = resolve_config(&args, &file_config);

        assert!(resolved.dashstream.is_some());
        let ds = resolved.dashstream.unwrap();
        // CLI should take precedence
        assert_eq!(ds.bootstrap_servers, "cli-kafka:9092");
        assert_eq!(ds.topic, "cli-events");
    }

    #[test]
    fn test_resolve_config_inherits_policy_from_file() {
        // Verify that policy rules from file config are inherited
        use codex_dashflow_core::{Decision, PolicyRule};

        let args = Args::default();

        // Create file config with custom policy
        let mut file_config = Config::default();
        file_config
            .policy
            .rules
            .push(PolicyRule::new("custom_tool", Decision::Allow));
        file_config.policy.include_dangerous_patterns = false; // No default dangerous patterns

        let resolved = resolve_config(&args, &file_config);

        // Should have the custom rule
        assert_eq!(resolved.policy_config.rules.len(), 1);
        assert_eq!(resolved.policy_config.rules[0].pattern, "custom_tool");
        assert!(!resolved.policy_config.include_dangerous_patterns);
    }

    #[test]
    fn test_resolve_config_cli_overrides_file_approval_mode() {
        // CLI --approval-mode should override file config's approval mode
        let args = Args {
            approval_mode: CliApprovalMode::Always, // CLI sets Always
            ..Default::default()
        };

        let mut file_config = Config::default();
        file_config.policy.approval_mode = ApprovalMode::Never; // File sets Never

        let resolved = resolve_config(&args, &file_config);

        // CLI should override
        assert_eq!(resolved.approval_mode, ApprovalMode::Always);
        assert_eq!(resolved.policy_config.approval_mode, ApprovalMode::Always);
    }

    #[test]
    fn test_build_agent_state_with_custom_rules_from_file() {
        // Verify that custom policy rules from file config are applied
        use codex_dashflow_core::{Decision, PolicyRule};

        let policy_config = PolicyConfig {
            include_dangerous_patterns: false, // No default patterns
            rules: vec![
                PolicyRule::new("my_tool", Decision::Allow).with_reason("Custom allowed tool"),
                PolicyRule::new("dangerous_tool", Decision::Forbidden).with_reason("No way"),
            ],
            ..PolicyConfig::default()
        };

        let config = ResolvedConfig {
            model: "test-model".to_string(),
            max_turns: 0,
            working_dir: ".".to_string(),
            session_id: None,
            use_mock_llm: false,
            verbose: false,
            quiet: false,
            dry_run: false,
            check: false,
            json: false,
            stdin: false,
            prompt_file: None,
            exec_prompt: None,
            dashstream: None,
            approval_mode: ApprovalMode::OnDangerous,
            policy_config,
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::ReadOnly,
            postgres: None,
            ..Default::default()
        };

        let state = build_agent_state(&config);
        let policy = state.exec_policy();

        // my_tool should be allowed
        let tc = codex_dashflow_core::ToolCall::new("my_tool", serde_json::json!({}));
        assert!(policy.evaluate(&tc).is_approved());

        // dangerous_tool should be forbidden
        let tc = codex_dashflow_core::ToolCall::new("dangerous_tool", serde_json::json!({}));
        assert!(policy.evaluate(&tc).is_forbidden());
    }

    #[test]
    fn test_args_approval_mode_flag() {
        // Test parsing various approval modes
        let args = Args::try_parse_from(["codex-dashflow", "--approval-mode", "never"]).unwrap();
        assert_eq!(args.approval_mode, CliApprovalMode::Never);

        let args = Args::try_parse_from(["codex-dashflow", "--approval-mode", "always"]).unwrap();
        assert_eq!(args.approval_mode, CliApprovalMode::Always);

        let args =
            Args::try_parse_from(["codex-dashflow", "--approval-mode", "on-first-use"]).unwrap();
        assert_eq!(args.approval_mode, CliApprovalMode::OnFirstUse);

        let args =
            Args::try_parse_from(["codex-dashflow", "--approval-mode", "on-dangerous"]).unwrap();
        assert_eq!(args.approval_mode, CliApprovalMode::OnDangerous);
    }

    #[test]
    fn test_cli_approval_mode_conversion() {
        // Test conversion from CliApprovalMode to ApprovalMode
        assert_eq!(
            ApprovalMode::from(CliApprovalMode::Never),
            ApprovalMode::Never
        );
        assert_eq!(
            ApprovalMode::from(CliApprovalMode::Always),
            ApprovalMode::Always
        );
        assert_eq!(
            ApprovalMode::from(CliApprovalMode::OnFirstUse),
            ApprovalMode::OnFirstUse
        );
        assert_eq!(
            ApprovalMode::from(CliApprovalMode::OnDangerous),
            ApprovalMode::OnDangerous
        );
    }

    #[test]
    fn test_build_agent_state_with_approval_mode() {
        // Test that approval mode is correctly applied to exec policy
        let policy_config = PolicyConfig {
            approval_mode: ApprovalMode::Never, // Permissive mode
            ..PolicyConfig::default()
        };
        let config = ResolvedConfig {
            model: "test-model".to_string(),
            max_turns: 0,
            working_dir: ".".to_string(),
            session_id: None,
            use_mock_llm: false,
            verbose: false,
            quiet: false,
            dry_run: false,
            check: false,
            json: false,
            stdin: false,
            prompt_file: None,
            exec_prompt: None,
            dashstream: None,
            approval_mode: ApprovalMode::Never,
            policy_config,
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::ReadOnly,
            postgres: None,
            ..Default::default()
        };

        let state = build_agent_state(&config);

        // Should have exec policy set
        assert!(state.has_exec_policy());
        // In permissive (Never) mode, dangerous tools should be auto-approved
        let policy = state.exec_policy();
        let tool_call =
            codex_dashflow_core::ToolCall::new("shell", serde_json::json!({"command": "ls -la"}));
        let requirement = policy.evaluate(&tool_call);
        assert!(requirement.is_approved());
    }

    #[test]
    fn test_build_agent_state_strict_approval_mode() {
        // Test strict (Always) approval mode
        let policy_config = PolicyConfig {
            approval_mode: ApprovalMode::Always, // Strict mode
            ..PolicyConfig::default()
        };
        let config = ResolvedConfig {
            model: "test-model".to_string(),
            max_turns: 0,
            working_dir: ".".to_string(),
            session_id: None,
            use_mock_llm: false,
            verbose: false,
            quiet: false,
            dry_run: false,
            check: false,
            json: false,
            stdin: false,
            prompt_file: None,
            exec_prompt: None,
            dashstream: None,
            approval_mode: ApprovalMode::Always,
            policy_config,
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::ReadOnly,
            postgres: None,
            ..Default::default()
        };

        let state = build_agent_state(&config);

        // In strict mode, tools without explicit allow rules need approval
        // Note: read_file has an explicit allow rule in dangerous_patterns policy,
        // so we test with a tool that doesn't have an explicit rule
        let policy = state.exec_policy();
        let tool_call = codex_dashflow_core::ToolCall::new(
            "custom_tool", // No explicit rule for this tool
            serde_json::json!({}),
        );
        let requirement = policy.evaluate(&tool_call);
        assert!(requirement.needs_approval());
    }

    // ========================================================================
    // Optimize Subcommand Tests
    // ========================================================================

    #[test]
    fn test_args_optimize_run_subcommand() {
        let args =
            Args::try_parse_from(["codex-dashflow", "optimize", "run", "--few-shot-count", "5"])
                .unwrap();

        assert!(args.command.is_some());
        if let Some(Command::Optimize(opt_args)) = args.command {
            if let OptimizeAction::Run { few_shot_count, .. } = opt_args.action {
                assert_eq!(few_shot_count, 5);
            } else {
                panic!("Expected Run action");
            }
        } else {
            panic!("Expected Optimize command");
        }
    }

    #[test]
    fn test_args_optimize_stats_subcommand() {
        let args = Args::try_parse_from(["codex-dashflow", "optimize", "stats"]).unwrap();

        assert!(args.command.is_some());
        if let Some(Command::Optimize(opt_args)) = args.command {
            assert!(matches!(opt_args.action, OptimizeAction::Stats { .. }));
        } else {
            panic!("Expected Optimize command");
        }
    }

    #[test]
    fn test_args_optimize_add_subcommand() {
        let args = Args::try_parse_from([
            "codex-dashflow",
            "optimize",
            "add",
            "-i",
            "List files",
            "-o",
            "Here are the files...",
            "-s",
            "0.9",
            "--tools",
            "shell,read_file",
        ])
        .unwrap();

        assert!(args.command.is_some());
        if let Some(Command::Optimize(opt_args)) = args.command {
            if let OptimizeAction::Add {
                input,
                output,
                score,
                tools,
                ..
            } = opt_args.action
            {
                assert_eq!(input, "List files");
                assert_eq!(output, "Here are the files...");
                assert_eq!(score, 0.9);
                assert_eq!(tools, Some("shell,read_file".to_string()));
            } else {
                panic!("Expected Add action");
            }
        } else {
            panic!("Expected Optimize command");
        }
    }

    #[test]
    fn test_args_optimize_show_subcommand() {
        let args = Args::try_parse_from(["codex-dashflow", "optimize", "show"]).unwrap();

        assert!(args.command.is_some());
        if let Some(Command::Optimize(opt_args)) = args.command {
            assert!(matches!(opt_args.action, OptimizeAction::Show { .. }));
        } else {
            panic!("Expected Optimize command");
        }
    }

    #[test]
    fn test_args_optimize_run_with_custom_files() {
        let args = Args::try_parse_from([
            "codex-dashflow",
            "optimize",
            "run",
            "--training-file",
            "/path/to/training.toml",
            "--prompts-file",
            "/path/to/prompts.toml",
        ])
        .unwrap();

        if let Some(Command::Optimize(opt_args)) = args.command {
            if let OptimizeAction::Run {
                training_file,
                prompts_file,
                ..
            } = opt_args.action
            {
                assert_eq!(training_file, Some(PathBuf::from("/path/to/training.toml")));
                assert_eq!(prompts_file, Some(PathBuf::from("/path/to/prompts.toml")));
            } else {
                panic!("Expected Run action");
            }
        } else {
            panic!("Expected Optimize command");
        }
    }

    #[test]
    fn test_args_optimize_add_default_score() {
        // Test that default score is 0.8
        let args = Args::try_parse_from([
            "codex-dashflow",
            "optimize",
            "add",
            "-i",
            "input",
            "-o",
            "output",
        ])
        .unwrap();

        if let Some(Command::Optimize(opt_args)) = args.command {
            if let OptimizeAction::Add { score, .. } = opt_args.action {
                assert_eq!(score, 0.8);
            } else {
                panic!("Expected Add action");
            }
        } else {
            panic!("Expected Optimize command");
        }
    }

    #[test]
    fn test_args_no_subcommand_means_agent_mode() {
        // When no subcommand, args.command is None (agent mode)
        let args = Args::try_parse_from(["codex-dashflow"]).unwrap();
        assert!(args.command.is_none());

        // Even with other flags, command should be None
        let args = Args::try_parse_from(["codex-dashflow", "--exec", "hello"]).unwrap();
        assert!(args.command.is_none());
        assert_eq!(args.exec, Some("hello".to_string()));
    }

    #[test]
    fn test_args_collect_training_flag() {
        // Default is false
        let args = Args::try_parse_from(["codex-dashflow"]).unwrap();
        assert!(!args.collect_training);

        // Can enable with --collect-training
        let args = Args::try_parse_from(["codex-dashflow", "--collect-training"]).unwrap();
        assert!(args.collect_training);
    }

    #[test]
    fn test_resolve_config_collect_training_from_cli() {
        let args = Args {
            collect_training: true,
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(resolved.collect_training);
    }

    #[test]
    fn test_resolve_config_collect_training_from_file() {
        let args = Args::default(); // CLI flag not set
        let file_config = Config {
            collect_training: true, // But file config enables it
            ..Default::default()
        };

        let resolved = resolve_config(&args, &file_config);
        assert!(resolved.collect_training);
    }

    #[test]
    fn test_resolve_config_collect_training_cli_overrides_file() {
        // CLI flag takes precedence (OR semantics: either enables it)
        let args = Args {
            collect_training: true,
            ..Default::default()
        };
        let file_config = Config {
            collect_training: false,
            ..Default::default()
        };

        let resolved = resolve_config(&args, &file_config);
        assert!(resolved.collect_training);
    }

    #[test]
    fn test_resolve_config_collect_training_disabled_both() {
        let args = Args::default();
        let file_config = Config::default(); // Both default to false

        let resolved = resolve_config(&args, &file_config);
        assert!(!resolved.collect_training);
    }

    // ========================================================================
    // Sandbox Mode Tests
    // ========================================================================

    #[test]
    fn test_args_sandbox_flag_read_only() {
        let args = Args::try_parse_from(["codex-dashflow", "--sandbox", "read-only"]).unwrap();
        assert_eq!(args.sandbox, Some(CliSandboxMode::ReadOnly));
    }

    #[test]
    fn test_args_sandbox_flag_workspace_write() {
        let args =
            Args::try_parse_from(["codex-dashflow", "--sandbox", "workspace-write"]).unwrap();
        assert_eq!(args.sandbox, Some(CliSandboxMode::WorkspaceWrite));
    }

    #[test]
    fn test_args_sandbox_flag_danger_full_access() {
        let args =
            Args::try_parse_from(["codex-dashflow", "--sandbox", "danger-full-access"]).unwrap();
        assert_eq!(args.sandbox, Some(CliSandboxMode::DangerFullAccess));
    }

    #[test]
    fn test_args_sandbox_short_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "-S", "workspace-write"]).unwrap();
        assert_eq!(args.sandbox, Some(CliSandboxMode::WorkspaceWrite));
    }

    #[test]
    fn test_cli_sandbox_mode_conversion() {
        assert_eq!(
            SandboxMode::from(CliSandboxMode::ReadOnly),
            SandboxMode::ReadOnly
        );
        assert_eq!(
            SandboxMode::from(CliSandboxMode::WorkspaceWrite),
            SandboxMode::WorkspaceWrite
        );
        assert_eq!(
            SandboxMode::from(CliSandboxMode::DangerFullAccess),
            SandboxMode::DangerFullAccess
        );
    }

    #[test]
    fn test_resolve_config_sandbox_from_cli() {
        let args = Args {
            sandbox: Some(CliSandboxMode::WorkspaceWrite),
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert_eq!(resolved.sandbox_mode, SandboxMode::WorkspaceWrite);
    }

    #[test]
    fn test_resolve_config_sandbox_from_file() {
        let args = Args::default(); // No CLI sandbox
        let file_config = Config {
            sandbox_mode: Some(SandboxMode::DangerFullAccess),
            ..Default::default()
        };

        let resolved = resolve_config(&args, &file_config);
        assert_eq!(resolved.sandbox_mode, SandboxMode::DangerFullAccess);
    }

    #[test]
    fn test_resolve_config_sandbox_cli_overrides_file() {
        let args = Args {
            sandbox: Some(CliSandboxMode::ReadOnly), // CLI sets ReadOnly
            ..Default::default()
        };
        let file_config = Config {
            sandbox_mode: Some(SandboxMode::DangerFullAccess), // File sets full access
            ..Default::default()
        };

        let resolved = resolve_config(&args, &file_config);
        // CLI should override
        assert_eq!(resolved.sandbox_mode, SandboxMode::ReadOnly);
    }

    // ========================================================================
    // Load Optimized Prompts Tests
    // ========================================================================

    #[test]
    fn test_args_load_optimized_prompts_flag() {
        // Default is false
        let args = Args::try_parse_from(["codex-dashflow"]).unwrap();
        assert!(!args.load_optimized_prompts);

        // Can enable with --load-optimized-prompts
        let args = Args::try_parse_from(["codex-dashflow", "--load-optimized-prompts"]).unwrap();
        assert!(args.load_optimized_prompts);
    }

    #[test]
    fn test_resolve_config_load_optimized_prompts_from_cli() {
        let args = Args {
            load_optimized_prompts: true,
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(resolved.load_optimized_prompts);
    }

    #[test]
    fn test_resolve_config_load_optimized_prompts_disabled_default() {
        let args = Args::default();
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(!resolved.load_optimized_prompts);
    }

    // ========================================================================
    // MCP Server Subcommand Tests
    // ========================================================================

    #[test]
    fn test_args_mcp_server_subcommand() {
        let args = Args::try_parse_from(["codex-dashflow", "mcp-server"]).unwrap();

        assert!(args.command.is_some());
        if let Some(Command::McpServer(mcp_args)) = args.command {
            assert!(mcp_args.working_dir.is_none());
            assert_eq!(mcp_args.sandbox, CliSandboxMode::WorkspaceWrite); // default
            assert!(!mcp_args.mock);
        } else {
            panic!("Expected McpServer command");
        }
    }

    #[test]
    fn test_args_mcp_server_with_working_dir() {
        let args = Args::try_parse_from([
            "codex-dashflow",
            "mcp-server",
            "--working-dir",
            "/path/to/project",
        ])
        .unwrap();

        if let Some(Command::McpServer(mcp_args)) = args.command {
            assert_eq!(mcp_args.working_dir, Some("/path/to/project".to_string()));
        } else {
            panic!("Expected McpServer command");
        }
    }

    #[test]
    fn test_args_mcp_server_with_sandbox() {
        let args = Args::try_parse_from(["codex-dashflow", "mcp-server", "--sandbox", "read-only"])
            .unwrap();

        if let Some(Command::McpServer(mcp_args)) = args.command {
            assert_eq!(mcp_args.sandbox, CliSandboxMode::ReadOnly);
        } else {
            panic!("Expected McpServer command");
        }
    }

    #[test]
    fn test_args_mcp_server_with_mock() {
        let args = Args::try_parse_from(["codex-dashflow", "mcp-server", "--mock"]).unwrap();

        if let Some(Command::McpServer(mcp_args)) = args.command {
            assert!(mcp_args.mock);
        } else {
            panic!("Expected McpServer command");
        }
    }

    #[test]
    fn test_args_mcp_server_all_options() {
        let args = Args::try_parse_from([
            "codex-dashflow",
            "mcp-server",
            "-d",
            "/home/user/code",
            "-S",
            "danger-full-access",
            "--mock",
        ])
        .unwrap();

        if let Some(Command::McpServer(mcp_args)) = args.command {
            assert_eq!(mcp_args.working_dir, Some("/home/user/code".to_string()));
            assert_eq!(mcp_args.sandbox, CliSandboxMode::DangerFullAccess);
            assert!(mcp_args.mock);
        } else {
            panic!("Expected McpServer command");
        }
    }

    // ========================================================================
    // System Prompt Tests
    // ========================================================================

    #[test]
    fn test_args_system_prompt_flag() {
        // Default is None
        let args = Args::try_parse_from(["codex-dashflow"]).unwrap();
        assert!(args.system_prompt.is_none());

        // Can set with --system-prompt
        let args = Args::try_parse_from([
            "codex-dashflow",
            "--system-prompt",
            "You are a helpful coding assistant.",
        ])
        .unwrap();
        assert_eq!(
            args.system_prompt,
            Some("You are a helpful coding assistant.".to_string())
        );
    }

    #[test]
    fn test_resolve_config_system_prompt_from_cli() {
        let args = Args {
            system_prompt: Some("Custom system prompt".to_string()),
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert_eq!(
            resolved.system_prompt,
            Some("Custom system prompt".to_string())
        );
    }

    #[test]
    fn test_resolve_config_system_prompt_none_by_default() {
        let args = Args::default();
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert!(resolved.system_prompt.is_none());
    }

    #[test]
    fn test_build_runner_config_with_system_prompt() {
        let config =
            build_runner_config(false, false, false, Some("Custom prompt".to_string()), None);
        assert_eq!(config.system_prompt, Some("Custom prompt".to_string()));
    }

    #[test]
    fn test_build_runner_config_without_system_prompt() {
        let config = build_runner_config(false, false, false, None, None);
        assert!(config.system_prompt.is_none());
    }

    #[test]
    fn test_build_runner_config_with_postgres() {
        let config = build_runner_config(
            false,
            false,
            false,
            None,
            Some("host=localhost dbname=test".to_string()),
        );
        assert!(config.enable_checkpointing);
        assert_eq!(
            config.postgres_connection_string,
            Some("host=localhost dbname=test".to_string())
        );
    }

    // ========================================================================
    // System Prompt File Tests
    // ========================================================================

    #[test]
    fn test_args_system_prompt_file_flag() {
        // Default is None
        let args = Args::try_parse_from(["codex-dashflow"]).unwrap();
        assert!(args.system_prompt_file.is_none());

        // Can set with --system-prompt-file
        let args = Args::try_parse_from([
            "codex-dashflow",
            "--system-prompt-file",
            "/path/to/prompt.txt",
        ])
        .unwrap();
        assert_eq!(
            args.system_prompt_file,
            Some(PathBuf::from("/path/to/prompt.txt"))
        );
    }

    #[test]
    fn test_resolve_config_system_prompt_file_from_cli() {
        let args = Args {
            system_prompt_file: Some(PathBuf::from("/path/to/prompt.txt")),
            ..Default::default()
        };
        let file_config = Config::default();

        let resolved = resolve_config(&args, &file_config);
        assert_eq!(
            resolved.system_prompt_file,
            Some(PathBuf::from("/path/to/prompt.txt"))
        );
    }

    #[test]
    fn test_resolve_system_prompt_direct_takes_precedence() {
        // When both --system-prompt and --system-prompt-file are provided,
        // --system-prompt should take precedence
        let result = resolve_system_prompt(
            Some("Direct prompt"),
            Some(&PathBuf::from("/nonexistent/path.txt")),
        )
        .unwrap();

        assert_eq!(result, Some("Direct prompt".to_string()));
    }

    #[test]
    fn test_resolve_system_prompt_from_file() {
        // Create a temporary file with a prompt
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_prompt.txt");
        let prompt_content = "You are a specialized assistant for Rust programming.";

        std::fs::write(&temp_file, prompt_content).unwrap();

        let result = resolve_system_prompt(None, Some(&temp_file)).unwrap();

        assert_eq!(result, Some(prompt_content.to_string()));

        // Cleanup
        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_resolve_system_prompt_from_file_trims_whitespace() {
        // Create a temporary file with a prompt with whitespace
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_prompt_whitespace.txt");
        let prompt_content = "  \n  You are a helpful assistant.  \n\n";

        std::fs::write(&temp_file, prompt_content).unwrap();

        let result = resolve_system_prompt(None, Some(&temp_file)).unwrap();

        assert_eq!(result, Some("You are a helpful assistant.".to_string()));

        // Cleanup
        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_resolve_system_prompt_none_when_neither_set() {
        let result = resolve_system_prompt(None, None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_system_prompt_file_not_found_error() {
        let result = resolve_system_prompt(None, Some(&PathBuf::from("/nonexistent/file.txt")));
        assert!(result.is_err());
    }

    // ========================================================================
    // Shell Completions Tests
    // ========================================================================

    #[test]
    fn test_args_completions_subcommand_bash() {
        let args = Args::try_parse_from(["codex-dashflow", "completions", "bash"]).unwrap();

        assert!(args.command.is_some());
        if let Some(Command::Completions(completions_args)) = args.command {
            assert_eq!(completions_args.shell, CliShell::Bash);
        } else {
            panic!("Expected Completions command");
        }
    }

    #[test]
    fn test_args_completions_subcommand_zsh() {
        let args = Args::try_parse_from(["codex-dashflow", "completions", "zsh"]).unwrap();

        if let Some(Command::Completions(completions_args)) = args.command {
            assert_eq!(completions_args.shell, CliShell::Zsh);
        } else {
            panic!("Expected Completions command");
        }
    }

    #[test]
    fn test_args_completions_subcommand_fish() {
        let args = Args::try_parse_from(["codex-dashflow", "completions", "fish"]).unwrap();

        if let Some(Command::Completions(completions_args)) = args.command {
            assert_eq!(completions_args.shell, CliShell::Fish);
        } else {
            panic!("Expected Completions command");
        }
    }

    #[test]
    fn test_args_completions_subcommand_powershell() {
        let args = Args::try_parse_from(["codex-dashflow", "completions", "powershell"]).unwrap();

        if let Some(Command::Completions(completions_args)) = args.command {
            assert_eq!(completions_args.shell, CliShell::PowerShell);
        } else {
            panic!("Expected Completions command");
        }
    }

    #[test]
    fn test_args_completions_subcommand_elvish() {
        let args = Args::try_parse_from(["codex-dashflow", "completions", "elvish"]).unwrap();

        if let Some(Command::Completions(completions_args)) = args.command {
            assert_eq!(completions_args.shell, CliShell::Elvish);
        } else {
            panic!("Expected Completions command");
        }
    }

    #[test]
    fn test_cli_shell_conversion() {
        // Test conversion from CliShell to clap_complete::Shell
        assert_eq!(Shell::from(CliShell::Bash), Shell::Bash);
        assert_eq!(Shell::from(CliShell::Zsh), Shell::Zsh);
        assert_eq!(Shell::from(CliShell::Fish), Shell::Fish);
        assert_eq!(Shell::from(CliShell::PowerShell), Shell::PowerShell);
        assert_eq!(Shell::from(CliShell::Elvish), Shell::Elvish);
    }

    #[test]
    fn test_args_completions_requires_shell_argument() {
        // Completions subcommand requires a shell argument
        let result = Args::try_parse_from(["codex-dashflow", "completions"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_args_completions_invalid_shell() {
        // Invalid shell should fail
        let result = Args::try_parse_from(["codex-dashflow", "completions", "invalid-shell"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_args_value_hints_present() {
        // Verify that value hints are properly configured for shell completion
        // This test generates completions and checks for expected hint markers
        let cmd = Args::command();

        // Get the arguments from the command
        let args: Vec<_> = cmd.get_arguments().collect();

        // Check that path arguments have FilePath or DirPath hints
        let prompt_file = args.iter().find(|a| a.get_id() == "prompt_file");
        assert!(prompt_file.is_some(), "prompt_file argument should exist");
        assert_eq!(
            prompt_file.unwrap().get_value_hint(),
            ValueHint::FilePath,
            "prompt_file should have FilePath hint"
        );

        let working_dir = args.iter().find(|a| a.get_id() == "working_dir");
        assert!(working_dir.is_some(), "working_dir argument should exist");
        assert_eq!(
            working_dir.unwrap().get_value_hint(),
            ValueHint::DirPath,
            "working_dir should have DirPath hint"
        );

        let config = args.iter().find(|a| a.get_id() == "config");
        assert!(config.is_some(), "config argument should exist");
        assert_eq!(
            config.unwrap().get_value_hint(),
            ValueHint::FilePath,
            "config should have FilePath hint"
        );

        let system_prompt_file = args.iter().find(|a| a.get_id() == "system_prompt_file");
        assert!(
            system_prompt_file.is_some(),
            "system_prompt_file argument should exist"
        );
        assert_eq!(
            system_prompt_file.unwrap().get_value_hint(),
            ValueHint::FilePath,
            "system_prompt_file should have FilePath hint"
        );

        let dashstream_bootstrap = args.iter().find(|a| a.get_id() == "dashstream_bootstrap");
        assert!(
            dashstream_bootstrap.is_some(),
            "dashstream_bootstrap argument should exist"
        );
        assert_eq!(
            dashstream_bootstrap.unwrap().get_value_hint(),
            ValueHint::Hostname,
            "dashstream_bootstrap should have Hostname hint"
        );
    }

    #[test]
    fn test_mcp_server_args_value_hints() {
        // Verify McpServerArgs has proper value hints
        let cmd = Args::command();
        let mcp_cmd = cmd.get_subcommands().find(|c| c.get_name() == "mcp-server");
        assert!(mcp_cmd.is_some(), "mcp-server subcommand should exist");

        let mcp_args: Vec<_> = mcp_cmd.unwrap().get_arguments().collect();
        let working_dir = mcp_args.iter().find(|a| a.get_id() == "working_dir");
        assert!(
            working_dir.is_some(),
            "working_dir argument should exist in mcp-server"
        );
        assert_eq!(
            working_dir.unwrap().get_value_hint(),
            ValueHint::DirPath,
            "mcp-server working_dir should have DirPath hint"
        );
    }

    #[test]
    fn test_optimize_args_value_hints() {
        // Verify optimize subcommand args have proper value hints
        let cmd = Args::command();
        let optimize_cmd = cmd.get_subcommands().find(|c| c.get_name() == "optimize");
        assert!(optimize_cmd.is_some(), "optimize subcommand should exist");

        // Check the 'run' subcommand
        let run_cmd = optimize_cmd
            .unwrap()
            .get_subcommands()
            .find(|c| c.get_name() == "run");
        assert!(run_cmd.is_some(), "optimize run subcommand should exist");

        let run_args: Vec<_> = run_cmd.unwrap().get_arguments().collect();
        let training_file = run_args.iter().find(|a| a.get_id() == "training_file");
        assert!(
            training_file.is_some(),
            "training_file argument should exist in optimize run"
        );
        assert_eq!(
            training_file.unwrap().get_value_hint(),
            ValueHint::FilePath,
            "optimize run training_file should have FilePath hint"
        );

        let prompts_file = run_args.iter().find(|a| a.get_id() == "prompts_file");
        assert!(
            prompts_file.is_some(),
            "prompts_file argument should exist in optimize run"
        );
        assert_eq!(
            prompts_file.unwrap().get_value_hint(),
            ValueHint::FilePath,
            "optimize run prompts_file should have FilePath hint"
        );
    }

    #[test]
    fn test_zsh_completions_include_file_hints() {
        // Generate zsh completions and verify they include _files completion
        let mut cmd = Args::command();
        let mut output = Vec::new();
        generate(Shell::Zsh, &mut cmd, "codex-dashflow", &mut output);
        let completions = String::from_utf8(output).unwrap();

        // Zsh uses _files for FilePath hints
        assert!(
            completions.contains("_files"),
            "Zsh completions should include _files for path arguments"
        );

        // Zsh uses _files -/ for DirPath hints
        assert!(
            completions.contains("_files -/"),
            "Zsh completions should include _files -/ for directory arguments"
        );

        // Zsh uses _hosts for Hostname hints
        assert!(
            completions.contains("_hosts"),
            "Zsh completions should include _hosts for hostname arguments"
        );
    }

    // ========================================================================
    // Init Subcommand Tests
    // ========================================================================

    #[test]
    fn test_args_init_subcommand() {
        let args = Args::try_parse_from(["codex-dashflow", "init"]).unwrap();
        assert!(args.command.is_some());
        match args.command {
            Some(Command::Init(init_args)) => {
                assert!(!init_args.force);
                assert!(!init_args.stdout);
                assert!(init_args.output.is_none());
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_args_init_with_force() {
        let args = Args::try_parse_from(["codex-dashflow", "init", "--force"]).unwrap();
        match args.command {
            Some(Command::Init(init_args)) => {
                assert!(init_args.force);
                assert!(!init_args.stdout);
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_args_init_with_stdout() {
        let args = Args::try_parse_from(["codex-dashflow", "init", "--stdout"]).unwrap();
        match args.command {
            Some(Command::Init(init_args)) => {
                assert!(!init_args.force);
                assert!(init_args.stdout);
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_args_init_with_output_path() {
        let args =
            Args::try_parse_from(["codex-dashflow", "init", "-o", "/tmp/my-config.toml"]).unwrap();
        match args.command {
            Some(Command::Init(init_args)) => {
                assert_eq!(init_args.output, Some(PathBuf::from("/tmp/my-config.toml")));
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_args_init_combined_flags() {
        let args = Args::try_parse_from([
            "codex-dashflow",
            "init",
            "--force",
            "-o",
            "/custom/path.toml",
        ])
        .unwrap();
        match args.command {
            Some(Command::Init(init_args)) => {
                assert!(init_args.force);
                assert!(!init_args.stdout);
                assert_eq!(init_args.output, Some(PathBuf::from("/custom/path.toml")));
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_generate_sample_config_valid_toml() {
        // Verify that generated config is valid TOML (using standard template)
        let config_str = generate_sample_config(ConfigTemplate::Standard);

        // Should start with the header
        assert!(config_str.contains("# Codex DashFlow Configuration"));

        // Should contain key fields
        assert!(config_str.contains("model ="));
        assert!(config_str.contains("api_base ="));
        assert!(config_str.contains("[dashflow]"));
        assert!(config_str.contains("[policy]"));
        assert!(config_str.contains("[doctor]"));

        // Should contain examples (commented out)
        assert!(config_str.contains("# [[mcp_servers]]"));
        assert!(config_str.contains("# [[policy.rules]]"));
    }

    #[test]
    fn test_init_exit_code_values() {
        // Verify exit code integer values
        assert_eq!(InitExitCode::Success.code(), 0);
        assert_eq!(InitExitCode::FileExists.code(), 1);
        assert_eq!(InitExitCode::WriteError.code(), 2);
    }

    #[test]
    fn test_init_stdout_mode() {
        // Test that --stdout mode returns success without writing
        let args = InitArgs {
            force: false,
            stdout: true,
            json: false,
            template: ConfigTemplate::Standard,
            output: None,
            list_templates: false,
        };
        // Note: This would print to stdout in a real run
        // We can't easily capture stdout here, but we verify it doesn't error
        let exit_code = run_init_command(&args);
        assert_eq!(exit_code, InitExitCode::Success);
    }

    #[test]
    fn test_init_to_temp_file() {
        // Test writing to a temporary file
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join(format!("codex_test_init_{}.toml", std::process::id()));

        // Clean up if it exists from a previous failed test
        let _ = std::fs::remove_file(&test_file);

        let args = InitArgs {
            force: false,
            stdout: false,
            json: false,
            template: ConfigTemplate::Standard,
            output: Some(test_file.clone()),
            list_templates: false,
        };

        let exit_code = run_init_command(&args);
        assert_eq!(exit_code, InitExitCode::Success);
        assert!(test_file.exists());

        // Verify the file contains valid config
        let content = std::fs::read_to_string(&test_file).unwrap();
        assert!(content.contains("# Codex DashFlow Configuration"));
        assert!(content.contains("model ="));

        // Clean up
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_init_file_exists_without_force() {
        // Test that existing file returns FileExists without --force
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join(format!(
            "codex_test_init_exists_{}.toml",
            std::process::id()
        ));

        // Create the file first
        std::fs::write(&test_file, "existing content").unwrap();

        let args = InitArgs {
            force: false,
            stdout: false,
            json: false,
            template: ConfigTemplate::Standard,
            output: Some(test_file.clone()),
            list_templates: false,
        };

        let exit_code = run_init_command(&args);
        assert_eq!(exit_code, InitExitCode::FileExists);

        // Verify file wasn't overwritten
        let content = std::fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, "existing content");

        // Clean up
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_init_file_exists_with_force() {
        // Test that existing file is overwritten with --force
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join(format!("codex_test_init_force_{}.toml", std::process::id()));

        // Create the file first
        std::fs::write(&test_file, "existing content").unwrap();

        let args = InitArgs {
            force: true,
            stdout: false,
            json: false,
            template: ConfigTemplate::Standard,
            output: Some(test_file.clone()),
            list_templates: false,
        };

        let exit_code = run_init_command(&args);
        assert_eq!(exit_code, InitExitCode::Success);

        // Verify file was overwritten
        let content = std::fs::read_to_string(&test_file).unwrap();
        assert!(content.contains("# Codex DashFlow Configuration"));

        // Clean up
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_init_creates_parent_directories() {
        // Test that init creates parent directories if needed
        let temp_dir = std::env::temp_dir();
        let nested_dir = temp_dir.join(format!("codex_test_nested_{}", std::process::id()));
        let test_file = nested_dir.join("subdir").join("config.toml");

        // Ensure directory doesn't exist
        let _ = std::fs::remove_dir_all(&nested_dir);

        let args = InitArgs {
            force: false,
            stdout: false,
            json: false,
            template: ConfigTemplate::Standard,
            output: Some(test_file.clone()),
            list_templates: false,
        };

        let exit_code = run_init_command(&args);
        assert_eq!(exit_code, InitExitCode::Success);
        assert!(test_file.exists());

        // Clean up
        let _ = std::fs::remove_dir_all(&nested_dir);
    }

    #[test]
    fn test_init_value_hint_for_output() {
        // Verify that output argument has FilePath hint for shell completion
        let cmd = Args::command();
        let init_cmd = cmd.get_subcommands().find(|c| c.get_name() == "init");
        assert!(init_cmd.is_some(), "init subcommand should exist");

        let init_args: Vec<_> = init_cmd.unwrap().get_arguments().collect();
        let output_arg = init_args.iter().find(|a| a.get_id() == "output");
        assert!(output_arg.is_some(), "output argument should exist in init");
        assert_eq!(
            output_arg.unwrap().get_value_hint(),
            ValueHint::FilePath,
            "init output should have FilePath hint"
        );
    }

    #[test]
    fn test_args_init_with_json() {
        let args = Args::try_parse_from(["codex-dashflow", "init", "--json"]).unwrap();
        match args.command {
            Some(Command::Init(init_args)) => {
                assert!(init_args.json);
                assert!(!init_args.force);
                assert!(!init_args.stdout);
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_args_init_with_json_and_force() {
        let args = Args::try_parse_from(["codex-dashflow", "init", "--json", "--force"]).unwrap();
        match args.command {
            Some(Command::Init(init_args)) => {
                assert!(init_args.json);
                assert!(init_args.force);
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_init_json_success() {
        // Test JSON output on successful init
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join(format!("codex_test_init_json_{}.toml", std::process::id()));

        // Clean up if it exists from a previous failed test
        let _ = std::fs::remove_file(&test_file);

        let args = InitArgs {
            force: false,
            stdout: false,
            json: true,
            template: ConfigTemplate::Standard,
            output: Some(test_file.clone()),
            list_templates: false,
        };

        let exit_code = run_init_command(&args);
        assert_eq!(exit_code, InitExitCode::Success);
        assert!(test_file.exists());

        // Clean up
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_init_json_file_exists() {
        // Test JSON output when file already exists
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join(format!(
            "codex_test_init_json_exists_{}.toml",
            std::process::id()
        ));

        // Create the file first
        std::fs::write(&test_file, "existing content").unwrap();

        let args = InitArgs {
            force: false,
            stdout: false,
            json: true,
            template: ConfigTemplate::Standard,
            output: Some(test_file.clone()),
            list_templates: false,
        };

        let exit_code = run_init_command(&args);
        assert_eq!(exit_code, InitExitCode::FileExists);

        // Verify file wasn't overwritten
        let content = std::fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, "existing content");

        // Clean up
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_init_json_with_force_overwrites() {
        // Test JSON output when overwriting with --force
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join(format!(
            "codex_test_init_json_force_{}.toml",
            std::process::id()
        ));

        // Create the file first
        std::fs::write(&test_file, "existing content").unwrap();

        let args = InitArgs {
            force: true,
            stdout: false,
            json: true,
            template: ConfigTemplate::Standard,
            output: Some(test_file.clone()),
            list_templates: false,
        };

        let exit_code = run_init_command(&args);
        assert_eq!(exit_code, InitExitCode::Success);

        // Verify file was overwritten
        let content = std::fs::read_to_string(&test_file).unwrap();
        assert!(content.contains("# Codex DashFlow Configuration"));

        // Clean up
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_init_result_serialization_success() {
        let result = InitResult {
            success: true,
            path: Some("/path/to/config.toml".to_string()),
            message: "Created configuration file".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains(r#""success":true"#));
        assert!(json.contains(r#""path":"/path/to/config.toml""#));
        assert!(json.contains(r#""message":"Created configuration file""#));
    }

    #[test]
    fn test_init_result_serialization_failure() {
        let result = InitResult {
            success: false,
            path: Some("/path/to/config.toml".to_string()),
            message: "File already exists".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains(r#""success":false"#));
        assert!(json.contains(r#""message":"File already exists""#));
    }

    #[test]
    fn test_init_result_serialization_no_path() {
        let result = InitResult {
            success: false,
            path: None,
            message: "Error message".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        // path should be omitted when None
        assert!(!json.contains("path"));
        assert!(json.contains(r#""success":false"#));
    }

    // ========================================================================
    // Init Template Tests
    // ========================================================================

    #[test]
    fn test_args_init_default_template() {
        let args = Args::try_parse_from(["codex-dashflow", "init"]).unwrap();
        if let Some(Command::Init(init_args)) = args.command {
            assert_eq!(init_args.template, ConfigTemplate::Standard);
        } else {
            panic!("Expected init command");
        }
    }

    #[test]
    fn test_args_init_template_minimal() {
        let args =
            Args::try_parse_from(["codex-dashflow", "init", "--template", "minimal"]).unwrap();
        if let Some(Command::Init(init_args)) = args.command {
            assert_eq!(init_args.template, ConfigTemplate::Minimal);
        } else {
            panic!("Expected init command");
        }
    }

    #[test]
    fn test_args_init_template_standard() {
        let args = Args::try_parse_from(["codex-dashflow", "init", "-t", "standard"]).unwrap();
        if let Some(Command::Init(init_args)) = args.command {
            assert_eq!(init_args.template, ConfigTemplate::Standard);
        } else {
            panic!("Expected init command");
        }
    }

    #[test]
    fn test_args_init_template_full() {
        let args = Args::try_parse_from(["codex-dashflow", "init", "--template", "full"]).unwrap();
        if let Some(Command::Init(init_args)) = args.command {
            assert_eq!(init_args.template, ConfigTemplate::Full);
        } else {
            panic!("Expected init command");
        }
    }

    #[test]
    fn test_args_init_template_development() {
        let args = Args::try_parse_from(["codex-dashflow", "init", "-t", "development"]).unwrap();
        if let Some(Command::Init(init_args)) = args.command {
            assert_eq!(init_args.template, ConfigTemplate::Development);
        } else {
            panic!("Expected init command");
        }
    }

    #[test]
    fn test_generate_minimal_config_content() {
        let content = generate_minimal_config();
        // Should contain minimal header
        assert!(content.contains("Minimal"));
        assert!(content.contains("essential settings only"));
        // Should contain core settings
        assert!(content.contains("model = \"gpt-4o\""));
        assert!(content.contains("max_turns = 10"));
        assert!(content.contains("sandbox_mode = \"read-only\""));
        // Should NOT contain complex sections
        assert!(!content.contains("[policy]"));
        assert!(!content.contains("[dashflow]"));
        assert!(!content.contains("[[mcp_servers]]"));
    }

    #[test]
    fn test_generate_standard_config_content() {
        let content = generate_standard_config();
        // Should contain standard header
        assert!(content.contains("Codex DashFlow Configuration"));
        assert!(content.contains("Environment variables"));
        // Should contain serialized default config
        assert!(content.contains("model"));
        // Should contain example sections as comments
        assert!(content.contains("# MCP Server Configuration"));
        assert!(content.contains("# Custom Policy Rules"));
    }

    #[test]
    fn test_generate_full_config_content() {
        let content = generate_full_config();
        // Should contain full header
        assert!(content.contains("(Full)"));
        assert!(content.contains("Complete configuration"));
        // Should contain all environment variables documented
        assert!(content.contains("OPENAI_API_KEY"));
        assert!(content.contains("OPENAI_BASE_URL"));
        assert!(content.contains("RUST_LOG"));
        assert!(content.contains("NO_COLOR"));
        // Should contain all sections
        assert!(content.contains("[policy]"));
        assert!(content.contains("[dashflow]"));
        assert!(content.contains("[doctor]"));
        // Should contain MCP examples
        assert!(content.contains("MCP (Model Context Protocol)"));
        // Should contain detailed policy examples
        assert!(content.contains("approval_mode"));
    }

    #[test]
    fn test_generate_development_config_content() {
        let content = generate_development_config();
        // Should contain development header
        assert!(content.contains("(Development)"));
        assert!(content.contains("NOT recommended for production"));
        // Should use faster/cheaper model
        assert!(content.contains("gpt-4o-mini"));
        // Should have permissive settings
        assert!(content.contains("approval_mode = \"never\""));
        assert!(content.contains("sandbox_mode = \"workspace-write\""));
        // Should have streaming enabled for debugging
        assert!(content.contains("streaming_enabled = true"));
        // Should have lower slow threshold
        assert!(content.contains("slow_threshold_ms = 50"));
        // Should have pre-configured allow rules for dev tools
        assert!(content.contains("pattern = \"cargo\""));
        assert!(content.contains("pattern = \"git\""));
    }

    #[test]
    fn test_init_with_template_stdout() {
        let args = InitArgs {
            force: false,
            stdout: true,
            json: false,
            template: ConfigTemplate::Minimal,
            output: None,
            list_templates: false,
        };
        let result = run_init_command(&args);
        assert_eq!(result, InitExitCode::Success);
    }

    #[test]
    fn test_init_templates_are_valid_toml() {
        // All template outputs should be parseable as TOML
        for template in [
            ConfigTemplate::Minimal,
            ConfigTemplate::Standard,
            ConfigTemplate::Full,
            ConfigTemplate::Development,
        ] {
            let content = generate_sample_config(template);
            // Skip comment-only lines and try to parse the rest
            let toml_content: String = content
                .lines()
                .filter(|line| !line.trim().starts_with('#') && !line.trim().is_empty())
                .collect::<Vec<&str>>()
                .join("\n");

            // Should be valid TOML (at least parseable)
            let result: Result<toml::Value, _> = toml::from_str(&toml_content);
            assert!(
                result.is_ok(),
                "Template {:?} produced invalid TOML: {}",
                template,
                result.unwrap_err()
            );
        }
    }

    #[test]
    fn test_init_template_with_output_file() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join(format!(
            "codex_test_init_template_{}.toml",
            std::process::id()
        ));

        // Clean up any existing test file
        let _ = std::fs::remove_file(&test_file);

        let args = InitArgs {
            force: false,
            stdout: false,
            json: false,
            template: ConfigTemplate::Development,
            output: Some(test_file.clone()),
            list_templates: false,
        };

        let result = run_init_command(&args);
        assert_eq!(result, InitExitCode::Success);

        // Verify file was created with development template content
        let content = std::fs::read_to_string(&test_file).unwrap();
        assert!(content.contains("(Development)"));
        assert!(content.contains("gpt-4o-mini"));

        // Clean up
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_args_init_list_templates() {
        let args = Args::try_parse_from(["codex-dashflow", "init", "--list-templates"]).unwrap();
        if let Some(Command::Init(init_args)) = args.command {
            assert!(init_args.list_templates);
        } else {
            panic!("Expected Init command");
        }
    }

    #[test]
    fn test_args_init_list_templates_with_json() {
        let args =
            Args::try_parse_from(["codex-dashflow", "init", "--list-templates", "--json"]).unwrap();
        if let Some(Command::Init(init_args)) = args.command {
            assert!(init_args.list_templates);
            assert!(init_args.json);
        } else {
            panic!("Expected Init command");
        }
    }

    #[test]
    fn test_init_list_templates_returns_success() {
        let args = InitArgs {
            force: false,
            stdout: false,
            json: false,
            template: ConfigTemplate::Standard,
            output: None,
            list_templates: true,
        };

        let result = run_init_command(&args);
        assert_eq!(result, InitExitCode::Success);
    }

    #[test]
    fn test_init_list_templates_json_output() {
        let args = InitArgs {
            force: false,
            stdout: false,
            json: true,
            template: ConfigTemplate::Standard,
            output: None,
            list_templates: true,
        };

        // This should succeed and output JSON
        let result = run_init_command(&args);
        assert_eq!(result, InitExitCode::Success);
    }

    #[test]
    fn test_template_info_serialization() {
        let info = TemplateInfo {
            name: "test".to_string(),
            description: "A test template".to_string(),
            is_default: true,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"description\":\"A test template\""));
        assert!(json.contains("\"is_default\":true"));
    }

    #[test]
    fn test_config_template_all() {
        let templates = ConfigTemplate::all();
        assert_eq!(templates.len(), 4);
        assert!(templates.contains(&ConfigTemplate::Minimal));
        assert!(templates.contains(&ConfigTemplate::Standard));
        assert!(templates.contains(&ConfigTemplate::Full));
        assert!(templates.contains(&ConfigTemplate::Development));
    }

    #[test]
    fn test_config_template_name() {
        assert_eq!(ConfigTemplate::Minimal.name(), "minimal");
        assert_eq!(ConfigTemplate::Standard.name(), "standard");
        assert_eq!(ConfigTemplate::Full.name(), "full");
        assert_eq!(ConfigTemplate::Development.name(), "development");
    }

    #[test]
    fn test_config_template_description() {
        assert!(!ConfigTemplate::Minimal.description().is_empty());
        assert!(!ConfigTemplate::Standard.description().is_empty());
        assert!(!ConfigTemplate::Full.description().is_empty());
        assert!(!ConfigTemplate::Development.description().is_empty());
    }

    #[test]
    fn test_config_template_is_default() {
        assert!(!ConfigTemplate::Minimal.is_default());
        assert!(ConfigTemplate::Standard.is_default());
        assert!(!ConfigTemplate::Full.is_default());
        assert!(!ConfigTemplate::Development.is_default());
    }

    // ========================================================================
    // Version Subcommand Tests
    // ========================================================================

    #[test]
    fn test_args_version_subcommand() {
        let args = Args::try_parse_from(["codex-dashflow", "version"]).unwrap();
        assert!(args.command.is_some());
        assert!(matches!(args.command, Some(Command::Version(_))));
    }

    #[test]
    fn test_args_version_with_agent_flag() {
        let args = Args::try_parse_from(["codex-dashflow", "version", "--agent"]).unwrap();
        match &args.command {
            Some(Command::Version(version_args)) => {
                assert!(version_args.agent);
            }
            _ => panic!("Expected Version command"),
        }
    }

    #[test]
    fn test_args_version_with_json_format() {
        let args = Args::try_parse_from(["codex-dashflow", "version", "--format", "json"]).unwrap();
        match &args.command {
            Some(Command::Version(version_args)) => {
                assert!(matches!(version_args.format, VersionFormat::Json));
            }
            _ => panic!("Expected Version command"),
        }
    }

    #[test]
    fn test_args_version_with_agent_and_json() {
        let args =
            Args::try_parse_from(["codex-dashflow", "version", "--agent", "--format", "json"])
                .unwrap();
        match &args.command {
            Some(Command::Version(version_args)) => {
                assert!(version_args.agent);
                assert!(matches!(version_args.format, VersionFormat::Json));
            }
            _ => panic!("Expected Version command"),
        }
    }

    #[test]
    fn test_run_version_command_basic_text() {
        // Test basic version command (no --agent) with text format
        let args = VersionArgs::default();
        let exit_code = run_version_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_version_command_basic_json() {
        // Test basic version command (no --agent) with JSON format
        let args = VersionArgs {
            agent: false,
            format: VersionFormat::Json,
        };
        let exit_code = run_version_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_version_command_agent_text() {
        // Test version --agent with text format
        let args = VersionArgs {
            agent: true,
            format: VersionFormat::Text,
        };
        let exit_code = run_version_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_version_command_agent_json() {
        // Test version --agent with JSON format
        let args = VersionArgs {
            agent: true,
            format: VersionFormat::Json,
        };
        let exit_code = run_version_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_version_info_current() {
        // Verify that VersionInfo::current() returns valid data
        let info = VersionInfo::current();

        // Version should match Cargo.toml
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));

        // Git hash should not be empty
        assert!(!info.git_hash.is_empty());

        // Git date should be in YYYY-MM-DD format or "unknown"
        assert!(info.git_date == "unknown" || info.git_date.contains('-'));

        // Build timestamp should not be empty
        assert!(!info.build_timestamp.is_empty());

        // Build target should not be empty
        assert!(!info.build_target.is_empty());
    }

    #[test]
    fn test_version_info_display() {
        let info = VersionInfo::current();
        let output = format!("{}", info);

        // Should contain key information
        assert!(output.contains("codex-dashflow"));
        assert!(output.contains(info.version));
        assert!(output.contains("Git commit:"));
        assert!(output.contains("Build time:"));
        assert!(output.contains("Target:"));
        assert!(output.contains("DashFlow"));
        assert!(output.contains("Repository:"));
    }

    // ========================================================================
    // Prompt Validation Tests
    // ========================================================================

    #[test]
    fn test_validate_prompt_valid() {
        assert!(validate_prompt("Hello, world!").is_ok());
        assert!(validate_prompt("A").is_ok()); // minimum valid
        assert!(validate_prompt("Multi\nline\nprompt").is_ok());
        assert!(validate_prompt("  trimmed  ").is_ok()); // whitespace around is ok
    }

    #[test]
    fn test_validate_prompt_empty() {
        assert_eq!(validate_prompt(""), Err(PromptValidationError::Empty));
        assert_eq!(validate_prompt("   "), Err(PromptValidationError::Empty));
        assert_eq!(validate_prompt("\n\n"), Err(PromptValidationError::Empty));
        assert_eq!(validate_prompt("\t\t"), Err(PromptValidationError::Empty));
    }

    #[test]
    fn test_validate_prompt_too_long() {
        let long_prompt = "a".repeat(MAX_PROMPT_LENGTH + 1);
        let result = validate_prompt(&long_prompt);
        assert!(matches!(result, Err(PromptValidationError::TooLong { .. })));

        if let Err(PromptValidationError::TooLong { length, max }) = result {
            assert_eq!(length, MAX_PROMPT_LENGTH + 1);
            assert_eq!(max, MAX_PROMPT_LENGTH);
        }
    }

    #[test]
    fn test_validate_prompt_at_max_length() {
        let max_prompt = "a".repeat(MAX_PROMPT_LENGTH);
        assert!(validate_prompt(&max_prompt).is_ok());
    }

    #[test]
    fn test_validate_prompt_null_bytes() {
        let result = validate_prompt("hello\0world");
        assert!(matches!(
            result,
            Err(PromptValidationError::InvalidCharacters(_))
        ));
    }

    #[test]
    fn test_prompt_validation_error_display() {
        assert_eq!(
            format!("{}", PromptValidationError::Empty),
            "prompt is empty"
        );

        let too_long = PromptValidationError::TooLong {
            length: 150000,
            max: 100000,
        };
        assert!(format!("{}", too_long).contains("150000"));
        assert!(format!("{}", too_long).contains("100000"));

        let invalid = PromptValidationError::InvalidCharacters("null bytes".to_string());
        assert!(format!("{}", invalid).contains("null bytes"));
    }

    #[test]
    fn test_max_prompt_length_constant() {
        // Verify the constant is reasonable (approximately 100KB)
        assert_eq!(MAX_PROMPT_LENGTH, 100_000);
    }

    // ========================================================================
    // Doctor Subcommand Tests
    // ========================================================================

    #[test]
    fn test_args_doctor_subcommand() {
        let args = Args::try_parse_from(["codex-dashflow", "doctor"]).unwrap();
        assert!(args.command.is_some());
        if let Some(Command::Doctor(doctor_args)) = args.command {
            assert!(!doctor_args.verbose);
        } else {
            panic!("Expected Doctor command");
        }
    }

    #[test]
    fn test_args_doctor_subcommand_verbose() {
        let args = Args::try_parse_from(["codex-dashflow", "doctor", "--verbose"]).unwrap();
        assert!(args.command.is_some());
        if let Some(Command::Doctor(doctor_args)) = args.command {
            assert!(doctor_args.verbose);
        } else {
            panic!("Expected Doctor command");
        }
    }

    #[test]
    fn test_args_doctor_subcommand_verbose_short() {
        let args = Args::try_parse_from(["codex-dashflow", "doctor", "-v"]).unwrap();
        if let Some(Command::Doctor(doctor_args)) = args.command {
            assert!(doctor_args.verbose);
        } else {
            panic!("Expected Doctor command");
        }
    }

    #[test]
    fn test_doctor_args_default() {
        let args = DoctorArgs::default();
        assert!(!args.verbose);
        assert!(!args.json);
        assert!(!args.quiet);
        assert!(args.slow_threshold.is_none()); // CLI default is None, uses config value
    }

    #[test]
    fn test_args_doctor_subcommand_json() {
        let args = Args::try_parse_from(["codex-dashflow", "doctor", "--json"]).unwrap();
        if let Some(Command::Doctor(doctor_args)) = args.command {
            assert!(doctor_args.json);
            assert!(!doctor_args.verbose);
        } else {
            panic!("Expected Doctor command");
        }
    }

    #[test]
    fn test_args_doctor_subcommand_json_and_verbose() {
        // Both flags can be set together (JSON takes precedence)
        let args =
            Args::try_parse_from(["codex-dashflow", "doctor", "--json", "--verbose"]).unwrap();
        if let Some(Command::Doctor(doctor_args)) = args.command {
            assert!(doctor_args.json);
            assert!(doctor_args.verbose);
        } else {
            panic!("Expected Doctor command");
        }
    }

    #[test]
    fn test_timed_doctor_check_result() {
        use std::time::Duration;
        let check = DoctorCheckResult::ok("Test", "Test message");
        let timed = TimedDoctorCheckResult::new(check.clone(), Duration::from_micros(1234));
        assert_eq!(timed.duration_us, 1234);
        assert_eq!(timed.result.name, "Test");
        assert_eq!(timed.result.status, DoctorCheckStatus::Ok);
        assert!(timed.slow.is_none()); // Not set until threshold applied
    }

    #[test]
    fn test_timed_doctor_check_result_with_slow_threshold() {
        use std::time::Duration;

        // Test check under threshold (100ms = 100000us)
        let check = DoctorCheckResult::ok("Fast", "Fast check");
        let timed = TimedDoctorCheckResult::new(check, Duration::from_micros(50_000)) // 50ms
            .with_slow_threshold(100);
        assert_eq!(timed.slow, Some(false));

        // Test check at threshold
        let check = DoctorCheckResult::ok("AtThreshold", "At threshold check");
        let timed = TimedDoctorCheckResult::new(check, Duration::from_micros(100_000)) // 100ms
            .with_slow_threshold(100);
        assert_eq!(timed.slow, Some(true)); // >= threshold is slow

        // Test check over threshold
        let check = DoctorCheckResult::ok("Slow", "Slow check");
        let timed = TimedDoctorCheckResult::new(check, Duration::from_micros(150_000)) // 150ms
            .with_slow_threshold(100);
        assert_eq!(timed.slow, Some(true));

        // Test with custom threshold
        let check = DoctorCheckResult::ok("Custom", "Custom threshold");
        let timed = TimedDoctorCheckResult::new(check, Duration::from_micros(200_000)) // 200ms
            .with_slow_threshold(250); // 250ms threshold
        assert_eq!(timed.slow, Some(false)); // 200ms < 250ms threshold
    }

    #[test]
    fn test_format_check_duration_microseconds() {
        assert_eq!(format_check_duration(500), "(500µs)");
        assert_eq!(format_check_duration(999), "(999µs)");
    }

    #[test]
    fn test_format_check_duration_milliseconds() {
        assert_eq!(format_check_duration(1000), "(1.00ms)");
        assert_eq!(format_check_duration(5000), "(5.00ms)");
        assert_eq!(format_check_duration(999_999), "(1000.00ms)");
    }

    #[test]
    fn test_format_check_duration_seconds() {
        assert_eq!(format_check_duration(1_000_000), "(1.00s)");
        assert_eq!(format_check_duration(2_500_000), "(2.50s)");
    }

    #[test]
    fn test_doctor_check_result_ok() {
        let check = DoctorCheckResult::ok("Test", "Everything is fine");
        assert_eq!(check.name, "Test");
        assert_eq!(check.status, DoctorCheckStatus::Ok);
        assert_eq!(check.message, "Everything is fine");
    }

    #[test]
    fn test_doctor_check_result_warn() {
        let check = DoctorCheckResult::warn("Test", "Minor issue");
        assert_eq!(check.name, "Test");
        assert_eq!(check.status, DoctorCheckStatus::Warn);
        assert_eq!(check.message, "Minor issue");
    }

    #[test]
    fn test_doctor_check_result_error() {
        let check = DoctorCheckResult::error("Test", "Critical failure");
        assert_eq!(check.name, "Test");
        assert_eq!(check.status, DoctorCheckStatus::Error);
        assert_eq!(check.message, "Critical failure");
    }

    #[test]
    fn test_doctor_check_status_equality() {
        assert_eq!(DoctorCheckStatus::Ok, DoctorCheckStatus::Ok);
        assert_eq!(DoctorCheckStatus::Warn, DoctorCheckStatus::Warn);
        assert_eq!(DoctorCheckStatus::Error, DoctorCheckStatus::Error);
        assert_ne!(DoctorCheckStatus::Ok, DoctorCheckStatus::Error);
    }

    #[test]
    fn test_doctor_check_disk_space() {
        let result = check_disk_space();
        assert_eq!(result.name, "Disk Space");
        // On most systems, we should have some disk space
        // The check should not error unless disk is nearly full
        assert!(
            result.status == DoctorCheckStatus::Ok
                || result.status == DoctorCheckStatus::Warn
                || result.message.contains("not available")
        );
    }

    #[test]
    fn test_doctor_check_shell_access() {
        let result = check_shell_access();
        assert_eq!(result.name, "Shell Access");
        // On most systems, we should be able to run shell commands
        assert_eq!(result.status, DoctorCheckStatus::Ok);
        assert!(result.message.contains("available"));
    }

    #[test]
    fn test_doctor_check_rust_version() {
        let result = check_rust_version();
        assert_eq!(result.name, "Rust Toolchain");
        // Should always return Ok - either rustc is in PATH or we report compiled version
        assert_eq!(result.status, DoctorCheckStatus::Ok);
        assert!(
            result.message.contains("rustc")
                || result.message.contains("Rust")
                || result.message.contains("Not required")
        );
    }

    #[test]
    fn test_doctor_check_network_connectivity() {
        let result = check_network_connectivity();
        assert_eq!(result.name, "Network");
        // Network check should return Ok or Warn (not Error)
        // It only warns if DNS resolution fails (network may be unavailable)
        assert!(result.status == DoctorCheckStatus::Ok || result.status == DoctorCheckStatus::Warn);
        // Message should mention either success or the reason for failure
        assert!(result.message.contains("api.openai.com") || result.message.contains("network"));
    }

    #[test]
    fn test_doctor_check_memory_usage() {
        let result = check_memory_usage();
        assert_eq!(result.name, "Memory");
        // Memory check should always return Ok
        assert_eq!(result.status, DoctorCheckStatus::Ok);
        // Message should contain MB or "not available"
        assert!(result.message.contains("MB") || result.message.contains("not available"));
    }

    #[test]
    fn test_doctor_check_git_repository() {
        let result = check_git_repository();
        assert_eq!(result.name, "Git Repository");
        // Git check should return Ok or Warn (not Error)
        // Ok if in a git repo, Ok if not a git repo, Warn if git not found
        assert!(result.status == DoctorCheckStatus::Ok || result.status == DoctorCheckStatus::Warn);
        // Message should indicate repo status
        assert!(
            result.message.contains("branch")
                || result.message.contains("Not a git repository")
                || result.message.contains("git not found")
        );
    }

    #[test]
    fn test_doctor_check_environment_variables() {
        let result = check_environment_variables();
        assert_eq!(result.name, "Environment");
        // Should return Ok or Warn (never Error)
        assert!(result.status == DoctorCheckStatus::Ok || result.status == DoctorCheckStatus::Warn);
        // Message should mention variables
        assert!(
            result.message.contains("variable")
                || result.message.contains("set")
                || result.message.contains("Missing")
        );
    }

    #[tokio::test]
    async fn test_doctor_check_api_connectivity_async() {
        let result = check_api_connectivity_async().await;
        assert_eq!(result.name, "API Connectivity");
        // Should return Ok or Warn (never Error) - network issues are warnings
        assert!(result.status == DoctorCheckStatus::Ok || result.status == DoctorCheckStatus::Warn);
        // Message should indicate connectivity status
        assert!(
            result.message.contains("api.openai.com")
                || result.message.contains("HTTP")
                || result.message.contains("timed out")
                || result.message.contains("failed")
                || result.message.contains("client")
        );
    }

    // ========================================================================
    // Doctor JSON Output Tests
    // ========================================================================

    #[test]
    fn test_doctor_check_status_serialization() {
        // Verify status serializes to lowercase strings
        assert_eq!(
            serde_json::to_string(&DoctorCheckStatus::Ok).unwrap(),
            r#""ok""#
        );
        assert_eq!(
            serde_json::to_string(&DoctorCheckStatus::Warn).unwrap(),
            r#""warn""#
        );
        assert_eq!(
            serde_json::to_string(&DoctorCheckStatus::Error).unwrap(),
            r#""error""#
        );
    }

    #[test]
    fn test_doctor_check_result_serialization() {
        let check = DoctorCheckResult::ok("Test Check", "All good");
        let json = serde_json::to_string(&check).unwrap();
        assert!(json.contains(r#""name":"Test Check""#));
        assert!(json.contains(r#""status":"ok""#));
        assert!(json.contains(r#""message":"All good""#));
    }

    #[test]
    fn test_timed_doctor_check_result_serialization() {
        use std::time::Duration;
        let check = DoctorCheckResult::warn("Test", "Warning message");
        let timed = TimedDoctorCheckResult::new(check, Duration::from_micros(5000));
        let json = serde_json::to_string(&timed).unwrap();
        // Check flattening works - fields from result should be at top level
        assert!(json.contains(r#""name":"Test""#));
        assert!(json.contains(r#""status":"warn""#));
        assert!(json.contains(r#""message":"Warning message""#));
        assert!(json.contains(r#""duration_us":5000"#));
        // slow field should NOT appear when None (skip_serializing_if)
        assert!(!json.contains(r#""slow""#));
    }

    #[test]
    fn test_timed_doctor_check_result_serialization_with_slow() {
        use std::time::Duration;

        // Test with slow=true
        let check = DoctorCheckResult::ok("SlowCheck", "This was slow");
        let timed = TimedDoctorCheckResult::new(check, Duration::from_micros(150_000))
            .with_slow_threshold(100);
        let json = serde_json::to_string(&timed).unwrap();
        assert!(json.contains(r#""slow":true"#));

        // Test with slow=false
        let check = DoctorCheckResult::ok("FastCheck", "This was fast");
        let timed = TimedDoctorCheckResult::new(check, Duration::from_micros(50_000))
            .with_slow_threshold(100);
        let json = serde_json::to_string(&timed).unwrap();
        assert!(json.contains(r#""slow":false"#));
    }

    #[test]
    fn test_doctor_json_output_serialization() {
        use std::time::Duration;

        let checks = vec![
            TimedDoctorCheckResult::new(
                DoctorCheckResult::ok("Check 1", "OK message"),
                Duration::from_micros(100),
            )
            .with_slow_threshold(100),
            TimedDoctorCheckResult::new(
                DoctorCheckResult::warn("Check 2", "Warn message"),
                Duration::from_micros(200_000), // 200ms - slow
            )
            .with_slow_threshold(100),
            TimedDoctorCheckResult::new(
                DoctorCheckResult::error("Check 3", "Error message"),
                Duration::from_micros(300),
            )
            .with_slow_threshold(100),
        ];

        let output = DoctorJsonOutput {
            version: "0.1.0".to_string(),
            total_checks: 3,
            errors: 1,
            warnings: 1,
            slow_checks: 1,
            slow_threshold_ms: 100,
            total_duration_us: 200_600,
            checks,
            overall_status: "errors".to_string(),
            config_summary: DoctorConfigSummary {
                collect_training: true,
                model: "gpt-4o".to_string(),
                sandbox_mode: Some("ReadOnly".to_string()),
                streaming_enabled: false,
                checkpointing_enabled: true,
                introspection_enabled: true,
                auto_resume_enabled: false,
                auto_resume_max_age_secs: None,
            },
        };

        let json = serde_json::to_string_pretty(&output).unwrap();

        // Verify structure
        assert!(json.contains(r#""version": "0.1.0""#));
        assert!(json.contains(r#""total_checks": 3"#));
        assert!(json.contains(r#""errors": 1"#));
        assert!(json.contains(r#""warnings": 1"#));
        assert!(json.contains(r#""slow_checks": 1"#));
        assert!(json.contains(r#""slow_threshold_ms": 100"#));
        assert!(json.contains(r#""total_duration_us": 200600"#));
        assert!(json.contains(r#""overall_status": "errors""#));
        assert!(json.contains(r#""checks": ["#));
        assert!(json.contains(r#""name": "Check 1""#));
        assert!(json.contains(r#""status": "ok""#));
        assert!(json.contains(r#""status": "warn""#));
        assert!(json.contains(r#""status": "error""#));
        // Verify slow field appears for checks with threshold applied
        assert!(json.contains(r#""slow": true"#)); // Check 2 was slow
        assert!(json.contains(r#""slow": false"#)); // Check 1 and 3 were fast
                                                    // Verify config summary (Audit #24)
        assert!(json.contains(r#""collect_training": true"#));
        assert!(json.contains(r#""model": "gpt-4o""#));
    }

    #[test]
    fn test_doctor_json_output_overall_status_variants() {
        use std::time::Duration;

        // Test "ok" status
        let ok_output = DoctorJsonOutput {
            version: "0.1.0".to_string(),
            total_checks: 1,
            errors: 0,
            warnings: 0,
            slow_checks: 0,
            slow_threshold_ms: 100,
            total_duration_us: 100,
            checks: vec![TimedDoctorCheckResult::new(
                DoctorCheckResult::ok("Test", "OK"),
                Duration::from_micros(100),
            )],
            overall_status: "ok".to_string(),
            config_summary: DoctorConfigSummary {
                collect_training: false,
                model: "gpt-4".to_string(),
                sandbox_mode: None,
                streaming_enabled: false,
                checkpointing_enabled: false,
                introspection_enabled: true,
                auto_resume_enabled: false,
                auto_resume_max_age_secs: None,
            },
        };
        let json = serde_json::to_string(&ok_output).unwrap();
        assert!(json.contains(r#""overall_status":"ok""#));

        // Test "warnings" status
        let warn_output = DoctorJsonOutput {
            version: "0.1.0".to_string(),
            total_checks: 1,
            errors: 0,
            warnings: 1,
            slow_checks: 0,
            slow_threshold_ms: 100,
            total_duration_us: 100,
            checks: vec![TimedDoctorCheckResult::new(
                DoctorCheckResult::warn("Test", "Warn"),
                Duration::from_micros(100),
            )],
            overall_status: "warnings".to_string(),
            config_summary: DoctorConfigSummary {
                collect_training: false,
                model: "gpt-4".to_string(),
                sandbox_mode: None,
                streaming_enabled: false,
                checkpointing_enabled: false,
                introspection_enabled: true,
                auto_resume_enabled: false,
                auto_resume_max_age_secs: None,
            },
        };
        let json = serde_json::to_string(&warn_output).unwrap();
        assert!(json.contains(r#""overall_status":"warnings""#));
    }

    #[tokio::test]
    async fn test_run_doctor_checks_returns_all_checks() {
        let checks = run_doctor_checks().await;
        // Should return 15 checks (audit #79 added Protoc check)
        assert_eq!(checks.len(), 15);
        // Verify some expected check names
        let names: Vec<&str> = checks.iter().map(|c| c.result.name).collect();
        assert!(names.contains(&"Authentication"));
        assert!(names.contains(&"Config File"));
        assert!(names.contains(&"Environment"));
        assert!(names.contains(&"Disk Space"));
        assert!(names.contains(&"Memory"));
        assert!(names.contains(&"Shell Access"));
        assert!(names.contains(&"Sandbox")); // Audit #64
        assert!(names.contains(&"Protoc")); // Audit #79
        assert!(names.contains(&"Network"));
        assert!(names.contains(&"API Connectivity"));
        assert!(names.contains(&"Git Repository"));
        assert!(names.contains(&"Rust Toolchain"));
        assert!(names.contains(&"Training Data"));
        assert!(names.contains(&"Prompt Registry"));
    }

    // ========================================================================
    // Doctor --quiet Flag Tests
    // ========================================================================

    #[test]
    fn test_args_doctor_subcommand_quiet() {
        let args = Args::try_parse_from(["codex-dashflow", "doctor", "--quiet"]).unwrap();
        if let Some(Command::Doctor(doctor_args)) = args.command {
            assert!(doctor_args.quiet);
            assert!(!doctor_args.verbose);
            assert!(!doctor_args.json);
        } else {
            panic!("Expected Doctor command");
        }
    }

    #[test]
    fn test_args_doctor_subcommand_quiet_short() {
        let args = Args::try_parse_from(["codex-dashflow", "doctor", "-q"]).unwrap();
        if let Some(Command::Doctor(doctor_args)) = args.command {
            assert!(doctor_args.quiet);
        } else {
            panic!("Expected Doctor command");
        }
    }

    #[test]
    fn test_args_doctor_subcommand_quiet_and_json() {
        // Both flags can be set together (quiet takes precedence - no output)
        let args = Args::try_parse_from(["codex-dashflow", "doctor", "--quiet", "--json"]).unwrap();
        if let Some(Command::Doctor(doctor_args)) = args.command {
            assert!(doctor_args.quiet);
            assert!(doctor_args.json);
        } else {
            panic!("Expected Doctor command");
        }
    }

    #[test]
    fn test_args_doctor_subcommand_quiet_and_verbose() {
        // Both flags can be set together (quiet takes precedence - no output)
        let args =
            Args::try_parse_from(["codex-dashflow", "doctor", "--quiet", "--verbose"]).unwrap();
        if let Some(Command::Doctor(doctor_args)) = args.command {
            assert!(doctor_args.quiet);
            assert!(doctor_args.verbose);
        } else {
            panic!("Expected Doctor command");
        }
    }

    #[test]
    fn test_args_doctor_subcommand_slow_threshold() {
        // Default is None (uses config value)
        let args = Args::try_parse_from(["codex-dashflow", "doctor"]).unwrap();
        if let Some(Command::Doctor(doctor_args)) = args.command {
            assert!(doctor_args.slow_threshold.is_none());
        } else {
            panic!("Expected Doctor command");
        }

        // Custom threshold from CLI
        let args =
            Args::try_parse_from(["codex-dashflow", "doctor", "--slow-threshold", "250"]).unwrap();
        if let Some(Command::Doctor(doctor_args)) = args.command {
            assert_eq!(doctor_args.slow_threshold, Some(250));
        } else {
            panic!("Expected Doctor command");
        }
    }

    #[test]
    fn test_args_doctor_subcommand_slow_threshold_with_verbose() {
        let args = Args::try_parse_from([
            "codex-dashflow",
            "doctor",
            "--verbose",
            "--slow-threshold",
            "50",
        ])
        .unwrap();
        if let Some(Command::Doctor(doctor_args)) = args.command {
            assert!(doctor_args.verbose);
            assert_eq!(doctor_args.slow_threshold, Some(50));
        } else {
            panic!("Expected Doctor command");
        }
    }

    #[test]
    fn test_doctor_args_effective_slow_threshold() {
        // Test that effective_slow_threshold falls back to config
        let args = DoctorArgs::default();
        let config = Config::default();

        // When CLI not provided, use config value
        assert_eq!(args.effective_slow_threshold(&config), 100);

        // When CLI provided, use CLI value
        let args_with_threshold = DoctorArgs {
            slow_threshold: Some(250),
            ..Default::default()
        };
        assert_eq!(args_with_threshold.effective_slow_threshold(&config), 250);
    }

    #[test]
    fn test_doctor_args_effective_slow_threshold_custom_config() {
        let args = DoctorArgs::default();
        let mut config = Config::default();
        config.doctor.slow_threshold_ms = 500;

        // Should use custom config value
        assert_eq!(args.effective_slow_threshold(&config), 500);

        // CLI override should still take precedence
        let args_with_threshold = DoctorArgs {
            slow_threshold: Some(50),
            ..Default::default()
        };
        assert_eq!(args_with_threshold.effective_slow_threshold(&config), 50);
    }

    // ========================================================================
    // DoctorExitCode Tests
    // ========================================================================

    #[test]
    fn test_doctor_exit_code_values() {
        assert_eq!(DoctorExitCode::Ok.code(), 0);
        assert_eq!(DoctorExitCode::Warnings.code(), 1);
        assert_eq!(DoctorExitCode::Errors.code(), 2);
    }

    #[test]
    fn test_doctor_exit_code_equality() {
        assert_eq!(DoctorExitCode::Ok, DoctorExitCode::Ok);
        assert_eq!(DoctorExitCode::Warnings, DoctorExitCode::Warnings);
        assert_eq!(DoctorExitCode::Errors, DoctorExitCode::Errors);
        assert_ne!(DoctorExitCode::Ok, DoctorExitCode::Warnings);
        assert_ne!(DoctorExitCode::Ok, DoctorExitCode::Errors);
        assert_ne!(DoctorExitCode::Warnings, DoctorExitCode::Errors);
    }

    #[test]
    fn test_doctor_exit_code_debug() {
        // Verify Debug derive works
        let _ = format!("{:?}", DoctorExitCode::Ok);
        let _ = format!("{:?}", DoctorExitCode::Warnings);
        let _ = format!("{:?}", DoctorExitCode::Errors);
    }

    #[test]
    fn test_doctor_exit_code_clone() {
        let code = DoctorExitCode::Warnings;
        let cloned = code;
        assert_eq!(code, cloned);
    }

    #[tokio::test]
    async fn test_run_doctor_command_returns_exit_code() {
        // Run doctor in quiet mode to avoid output during tests
        let args = DoctorArgs {
            verbose: false,
            json: false,
            quiet: true,
            slow_threshold: Some(100),
        };
        let config = Config::default();
        let exit_code = run_doctor_command(&args, &config).await;
        // Exit code should be one of the valid values
        assert!(
            exit_code == DoctorExitCode::Ok
                || exit_code == DoctorExitCode::Warnings
                || exit_code == DoctorExitCode::Errors
        );
    }

    #[tokio::test]
    async fn test_run_doctor_command_quiet_mode_returns_valid_code() {
        // Verify quiet mode still returns proper exit codes
        let args = DoctorArgs {
            verbose: false,
            json: false,
            quiet: true,
            slow_threshold: None, // Use config default
        };
        let config = Config::default();
        let exit_code = run_doctor_command(&args, &config).await;
        // The code should be valid regardless of output mode
        let code_value = exit_code.code();
        assert!((0..=2).contains(&code_value));
    }

    #[test]
    fn test_build_agent_state_introspection_enabled() {
        // Test that introspection_enabled=true attaches graph manifest
        let config = ResolvedConfig {
            model: "test-model".to_string(),
            max_turns: 10,
            working_dir: ".".to_string(),
            session_id: Some("test-session".to_string()),
            use_mock_llm: false,
            verbose: false,
            quiet: false,
            dry_run: false,
            check: false,
            json: false,
            stdin: false,
            prompt_file: None,
            exec_prompt: None,
            dashstream: None,
            approval_mode: ApprovalMode::OnDangerous,
            policy_config: PolicyConfig::default(),
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::ReadOnly,
            postgres: None,
            introspection_enabled: true,
            ..Default::default()
        };

        let state = build_agent_state(&config);

        // When introspection is enabled, graph_manifest should be set
        assert!(state.graph_manifest.is_some());
        let manifest = state.graph_manifest.as_ref().unwrap();
        assert_eq!(
            manifest.graph_name,
            Some("codex_dashflow_agent".to_string())
        );
    }

    #[test]
    fn test_build_agent_state_introspection_disabled() {
        // Test that introspection_enabled=false does not attach graph manifest
        let config = ResolvedConfig {
            model: "test-model".to_string(),
            max_turns: 10,
            working_dir: ".".to_string(),
            session_id: Some("test-session".to_string()),
            use_mock_llm: false,
            verbose: false,
            quiet: false,
            dry_run: false,
            check: false,
            json: false,
            stdin: false,
            prompt_file: None,
            exec_prompt: None,
            dashstream: None,
            approval_mode: ApprovalMode::OnDangerous,
            policy_config: PolicyConfig::default(),
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
            sandbox_mode: SandboxMode::ReadOnly,
            postgres: None,
            introspection_enabled: false,
            ..Default::default()
        };

        let state = build_agent_state(&config);

        // When introspection is disabled, graph_manifest should be None
        assert!(state.graph_manifest.is_none());
    }

    #[test]
    fn test_resolve_config_introspection_from_file() {
        // Test that introspection_enabled is resolved from file config
        let args = Args::default();

        // With default config (introspection enabled by default)
        let file_config = Config::default();
        let resolved = resolve_config(&args, &file_config);
        assert!(resolved.introspection_enabled);

        // With introspection disabled in file config
        let mut file_config_disabled = Config::default();
        file_config_disabled.dashflow.introspection_enabled = false;
        let resolved = resolve_config(&args, &file_config_disabled);
        assert!(!resolved.introspection_enabled);
    }

    #[test]
    fn test_resolve_config_introspection_cli_override() {
        let file_config = Config::default();

        // CLI --introspection overrides config (even when config is default=true, explicit flag works)
        let args_enable = Args {
            introspection: true,
            ..Args::default()
        };
        let resolved = resolve_config(&args_enable, &file_config);
        assert!(resolved.introspection_enabled);

        // CLI --no-introspection disables even when config enables it
        let args_disable = Args {
            no_introspection: true,
            ..Args::default()
        };
        let resolved = resolve_config(&args_disable, &file_config);
        assert!(!resolved.introspection_enabled);

        // CLI --introspection enables when config disables it
        let mut file_config_disabled = Config::default();
        file_config_disabled.dashflow.introspection_enabled = false;
        let args_enable = Args {
            introspection: true,
            ..Args::default()
        };
        let resolved = resolve_config(&args_enable, &file_config_disabled);
        assert!(resolved.introspection_enabled);

        // --no-introspection takes precedence when both flags somehow set
        // (clap's overrides_with handles this, but test the logic)
        let args_both = Args {
            no_introspection: true,
            introspection: false, // no_introspection=true takes precedence in our logic
            ..Args::default()
        };
        let resolved = resolve_config(&args_both, &file_config);
        assert!(!resolved.introspection_enabled);
    }

    #[test]
    fn test_resolve_config_auto_resume_from_file() {
        // Test that auto_resume_enabled is resolved from file config
        let args = Args::default();

        // With default config (auto_resume disabled by default)
        let file_config = Config::default();
        let resolved = resolve_config(&args, &file_config);
        assert!(!resolved.auto_resume_enabled);

        // With auto_resume enabled in file config
        let mut file_config_enabled = Config::default();
        file_config_enabled.dashflow.auto_resume = true;
        let resolved = resolve_config(&args, &file_config_enabled);
        assert!(resolved.auto_resume_enabled);
    }

    #[test]
    fn test_resolve_config_auto_resume_cli_override() {
        let file_config = Config::default();

        // CLI --auto-resume enables auto resume
        let args_enable = Args {
            auto_resume: true,
            ..Args::default()
        };
        let resolved = resolve_config(&args_enable, &file_config);
        assert!(resolved.auto_resume_enabled);

        // CLI --no-auto-resume disables even when config enables it
        let mut file_config_enabled = Config::default();
        file_config_enabled.dashflow.auto_resume = true;
        let args_disable = Args {
            no_auto_resume: true,
            ..Args::default()
        };
        let resolved = resolve_config(&args_disable, &file_config_enabled);
        assert!(!resolved.auto_resume_enabled);

        // CLI --auto-resume enables when config disables it (default)
        let args_enable = Args {
            auto_resume: true,
            ..Args::default()
        };
        let resolved = resolve_config(&args_enable, &file_config);
        assert!(resolved.auto_resume_enabled);

        // --no-auto-resume takes precedence when both flags somehow set
        // (clap's overrides_with handles this, but test the logic)
        let args_both = Args {
            no_auto_resume: true,
            auto_resume: false, // no_auto_resume=true takes precedence in our logic
            ..Args::default()
        };
        let resolved = resolve_config(&args_both, &file_config);
        assert!(!resolved.auto_resume_enabled);
    }

    #[test]
    fn test_resolve_config_auto_resume_max_age_from_file() {
        // Test that auto_resume_max_age_secs is resolved from file config
        let args = Args::default();

        // With default config (no max age)
        let file_config = Config::default();
        let resolved = resolve_config(&args, &file_config);
        assert!(resolved.auto_resume_max_age_secs.is_none());

        // With max age set in file config
        let mut file_config_with_age = Config::default();
        file_config_with_age.dashflow.auto_resume_max_age_secs = Some(86400);
        let resolved = resolve_config(&args, &file_config_with_age);
        assert_eq!(resolved.auto_resume_max_age_secs, Some(86400));
    }

    #[test]
    fn test_resolve_config_auto_resume_max_age_cli_override() {
        let file_config = Config::default();

        // CLI --auto-resume-max-age sets the value
        let args_with_age = Args {
            auto_resume_max_age: Some(3600),
            ..Args::default()
        };
        let resolved = resolve_config(&args_with_age, &file_config);
        assert_eq!(resolved.auto_resume_max_age_secs, Some(3600));

        // CLI --auto-resume-max-age overrides file config
        let mut file_config_with_age = Config::default();
        file_config_with_age.dashflow.auto_resume_max_age_secs = Some(86400);
        let args_override = Args {
            auto_resume_max_age: Some(7200),
            ..Args::default()
        };
        let resolved = resolve_config(&args_override, &file_config_with_age);
        assert_eq!(resolved.auto_resume_max_age_secs, Some(7200));

        // When CLI doesn't set it, file config is used
        let args_no_age = Args::default();
        let resolved = resolve_config(&args_no_age, &file_config_with_age);
        assert_eq!(resolved.auto_resume_max_age_secs, Some(86400));
    }

    #[test]
    fn test_format_duration_human() {
        // Seconds
        assert_eq!(format_duration_human(0), "0s");
        assert_eq!(format_duration_human(30), "30s");
        assert_eq!(format_duration_human(59), "59s");

        // Minutes
        assert_eq!(format_duration_human(60), "1m");
        assert_eq!(format_duration_human(90), "1m 30s");
        assert_eq!(format_duration_human(120), "2m");
        assert_eq!(format_duration_human(3599), "59m 59s");

        // Hours
        assert_eq!(format_duration_human(3600), "1h");
        assert_eq!(format_duration_human(5400), "1h 30m");
        assert_eq!(format_duration_human(7200), "2h");
        assert_eq!(format_duration_human(86399), "23h 59m");

        // Days
        assert_eq!(format_duration_human(86400), "1d");
        assert_eq!(format_duration_human(90000), "1d 1h");
        assert_eq!(format_duration_human(172800), "2d");
        assert_eq!(format_duration_human(604800), "7d");
    }

    #[test]
    fn test_run_introspect_command_json_format() {
        // Default format is JSON
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: None,
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_mermaid_format() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Mermaid,
            platform: false,
            section: None,
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_text_format() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Text,
            platform: false,
            section: None,
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_nodes() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("nodes".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_edges() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("edges".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_tools() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("tools".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_graph() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("graph".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_invalid_section() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("invalid".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 1, "Invalid section should return error code");
    }

    #[test]
    fn test_run_introspect_command_section_registry() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("registry".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_metrics() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("metrics".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_performance() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("performance".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_quality() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("quality".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_templates() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("templates".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_scheduler() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("scheduler".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_retention() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("retention".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_dashoptimize() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("dashoptimize".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_optimizers() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("optimizers".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_evals() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("evals".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_datacollection() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("datacollection".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_modules() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("modules".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_multiobjective() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("multiobjective".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_aggregation() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("aggregation".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_knn() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("knn".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_signatures() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("signatures".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_optimizerext() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("optimizerext".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_streaming() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("streaming".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_debug() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("debug".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_approval() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("approval".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_with_platform() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: true,
            section: None,
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_checkpoint() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("checkpoint".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_alerts() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("alerts".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_section_stategraph() {
        let args = IntrospectArgs {
            format: IntrospectFormat::Json,
            platform: false,
            section: Some("stategraph".to_string()),
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_introspect_command_brief() {
        let args = IntrospectArgs {
            brief: true,
            ..Default::default()
        };
        let exit_code = run_introspect_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_architecture_alias_brief_flag() {
        // Test that `codex architecture --brief` works via CLI args
        let args = Args::try_parse_from(["codex-dashflow", "architecture", "--brief"]).unwrap();
        if let Some(Command::Introspect(introspect_args)) = args.command {
            assert!(introspect_args.brief);
        } else {
            panic!("Expected Introspect command");
        }
    }

    // ========================================================================
    // Sessions Subcommand Tests
    // ========================================================================

    #[test]
    fn test_args_sessions_subcommand() {
        let args = Args::try_parse_from(["codex-dashflow", "sessions"]).unwrap();
        assert!(args.command.is_some());
        if let Some(Command::Sessions(sessions_args)) = args.command {
            assert!(matches!(sessions_args.format, SessionsFormat::Table));
            assert!(sessions_args.checkpoint_path.is_none());
        } else {
            panic!("Expected Sessions command");
        }
    }

    #[test]
    fn test_args_sessions_subcommand_json_format() {
        let args =
            Args::try_parse_from(["codex-dashflow", "sessions", "--format", "json"]).unwrap();
        if let Some(Command::Sessions(sessions_args)) = args.command {
            assert!(matches!(sessions_args.format, SessionsFormat::Json));
        } else {
            panic!("Expected Sessions command");
        }
    }

    #[test]
    fn test_args_sessions_subcommand_table_format() {
        let args =
            Args::try_parse_from(["codex-dashflow", "sessions", "--format", "table"]).unwrap();
        if let Some(Command::Sessions(sessions_args)) = args.command {
            assert!(matches!(sessions_args.format, SessionsFormat::Table));
        } else {
            panic!("Expected Sessions command");
        }
    }

    #[test]
    fn test_args_sessions_subcommand_short_format() {
        let args = Args::try_parse_from(["codex-dashflow", "sessions", "-f", "json"]).unwrap();
        if let Some(Command::Sessions(sessions_args)) = args.command {
            assert!(matches!(sessions_args.format, SessionsFormat::Json));
        } else {
            panic!("Expected Sessions command");
        }
    }

    #[test]
    fn test_args_sessions_subcommand_with_checkpoint_path() {
        let args = Args::try_parse_from([
            "codex-dashflow",
            "sessions",
            "--checkpoint-path",
            "/tmp/sessions",
        ])
        .unwrap();
        if let Some(Command::Sessions(sessions_args)) = args.command {
            assert_eq!(
                sessions_args.checkpoint_path,
                Some(PathBuf::from("/tmp/sessions"))
            );
        } else {
            panic!("Expected Sessions command");
        }
    }

    #[test]
    fn test_args_sessions_subcommand_combined_flags() {
        let args = Args::try_parse_from([
            "codex-dashflow",
            "sessions",
            "--format",
            "json",
            "--checkpoint-path",
            "/data/checkpoints",
        ])
        .unwrap();
        if let Some(Command::Sessions(sessions_args)) = args.command {
            assert!(matches!(sessions_args.format, SessionsFormat::Json));
            assert_eq!(
                sessions_args.checkpoint_path,
                Some(PathBuf::from("/data/checkpoints"))
            );
        } else {
            panic!("Expected Sessions command");
        }
    }

    #[test]
    fn test_sessions_exit_code_values() {
        assert_eq!(SessionsExitCode::Success.code(), 0);
        assert_eq!(SessionsExitCode::Error.code(), 1);
        assert_eq!(SessionsExitCode::NoStorage.code(), 2);
    }

    #[test]
    fn test_sessions_exit_code_equality() {
        assert_eq!(SessionsExitCode::Success, SessionsExitCode::Success);
        assert_eq!(SessionsExitCode::Error, SessionsExitCode::Error);
        assert_eq!(SessionsExitCode::NoStorage, SessionsExitCode::NoStorage);
        assert_ne!(SessionsExitCode::Success, SessionsExitCode::Error);
        assert_ne!(SessionsExitCode::Success, SessionsExitCode::NoStorage);
        assert_ne!(SessionsExitCode::Error, SessionsExitCode::NoStorage);
    }

    #[test]
    fn test_sessions_format_default() {
        let args = SessionsArgs::default();
        assert!(matches!(args.format, SessionsFormat::Table));
    }

    #[test]
    fn test_session_info_serialization() {
        let session = SessionInfo {
            session_id: "test-123".to_string(),
            latest_checkpoint_id: "cp-456".to_string(),
            updated_at: "2025-01-15T10:30:00".to_string(),
            checkpoint_count: Some(5),
        };
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("test-123"));
        assert!(json.contains("cp-456"));
        assert!(json.contains("2025-01-15T10:30:00"));
        assert!(json.contains("5"));
    }

    #[test]
    fn test_format_timestamp() {
        use std::time::{Duration, UNIX_EPOCH};

        // Test a known timestamp: 2025-01-15 10:30:00 UTC
        // This is approximately 1736935800 seconds since epoch
        let timestamp = UNIX_EPOCH + Duration::from_secs(1736935800);
        let formatted = format_timestamp(timestamp);

        // Should produce a valid ISO 8601 format
        assert!(formatted.contains("2025"));
        assert!(formatted.contains("T"));
        assert_eq!(formatted.len(), 19); // YYYY-MM-DDTHH:MM:SS
    }

    #[test]
    fn test_is_leap_year() {
        // Leap years
        assert!(is_leap_year(2000)); // divisible by 400
        assert!(is_leap_year(2004)); // divisible by 4, not by 100
        assert!(is_leap_year(2024)); // divisible by 4, not by 100

        // Non-leap years
        assert!(!is_leap_year(1900)); // divisible by 100, not by 400
        assert!(!is_leap_year(2001)); // not divisible by 4
        assert!(!is_leap_year(2023)); // not divisible by 4
    }

    #[test]
    fn test_day_of_year_to_month_day() {
        // Non-leap year
        assert_eq!(day_of_year_to_month_day(1, false), (1, 1)); // Jan 1
        assert_eq!(day_of_year_to_month_day(31, false), (1, 31)); // Jan 31
        assert_eq!(day_of_year_to_month_day(32, false), (2, 1)); // Feb 1
        assert_eq!(day_of_year_to_month_day(59, false), (2, 28)); // Feb 28
        assert_eq!(day_of_year_to_month_day(60, false), (3, 1)); // Mar 1
        assert_eq!(day_of_year_to_month_day(365, false), (12, 31)); // Dec 31

        // Leap year
        assert_eq!(day_of_year_to_month_day(59, true), (2, 28)); // Feb 28
        assert_eq!(day_of_year_to_month_day(60, true), (2, 29)); // Feb 29
        assert_eq!(day_of_year_to_month_day(61, true), (3, 1)); // Mar 1
        assert_eq!(day_of_year_to_month_day(366, true), (12, 31)); // Dec 31
    }

    #[tokio::test]
    async fn test_run_sessions_command_no_storage_table() {
        // Test with default config (no checkpointing configured)
        let args = SessionsArgs::default();
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should return NoStorage since no checkpoint path is configured
        assert_eq!(exit_code, SessionsExitCode::NoStorage);
    }

    #[tokio::test]
    async fn test_run_sessions_command_no_storage_json() {
        // Test with JSON format and no checkpointing configured
        let args = SessionsArgs {
            format: SessionsFormat::Json,
            checkpoint_path: None,
            show: None,
            delete: None,
            delete_all: false,
            force: false,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should return NoStorage since no checkpoint path is configured
        assert_eq!(exit_code, SessionsExitCode::NoStorage);
    }

    #[tokio::test]
    async fn test_run_sessions_command_with_temp_path() {
        use std::fs;

        // Create a temporary directory for checkpoints
        let temp_dir = std::env::temp_dir().join(format!("sessions_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        let args = SessionsArgs {
            format: SessionsFormat::Table,
            checkpoint_path: Some(temp_dir.clone()),
            show: None,
            delete: None,
            delete_all: false,
            force: false,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should succeed (empty list is valid)
        assert_eq!(exit_code, SessionsExitCode::Success);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_run_sessions_command_json_empty_list() {
        use std::fs;

        // Create a temporary directory for checkpoints
        let temp_dir = std::env::temp_dir().join(format!("sessions_json_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        let args = SessionsArgs {
            format: SessionsFormat::Json,
            checkpoint_path: Some(temp_dir.clone()),
            show: None,
            delete: None,
            delete_all: false,
            force: false,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should succeed with empty list
        assert_eq!(exit_code, SessionsExitCode::Success);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sessions_args_delete_flag_parsing() {
        // Test that --delete requires a value
        let result =
            Args::try_parse_from(["codex-dashflow", "sessions", "--delete", "session-123"]);
        assert!(result.is_ok());
        let args = result.unwrap();
        if let Some(Command::Sessions(sessions_args)) = args.command {
            assert_eq!(sessions_args.delete, Some("session-123".to_string()));
            assert!(!sessions_args.force);
        } else {
            panic!("Expected Sessions command");
        }
    }

    #[test]
    fn test_sessions_args_delete_with_force() {
        // Test --delete with --force
        let result = Args::try_parse_from([
            "codex-dashflow",
            "sessions",
            "--delete",
            "session-456",
            "--force",
        ]);
        assert!(result.is_ok());
        let args = result.unwrap();
        if let Some(Command::Sessions(sessions_args)) = args.command {
            assert_eq!(sessions_args.delete, Some("session-456".to_string()));
            assert!(sessions_args.force);
        } else {
            panic!("Expected Sessions command");
        }
    }

    #[test]
    fn test_sessions_args_force_alone_is_valid() {
        // Test that --force without --delete or --delete-all is valid (but does nothing)
        // This is a design choice: --force is just a flag, not tied to any specific operation
        let result = Args::try_parse_from(["codex-dashflow", "sessions", "--force"]);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_sessions_delete_requires_force() {
        use std::fs;

        // Create a temporary directory for checkpoints
        let temp_dir =
            std::env::temp_dir().join(format!("sessions_delete_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // Try to delete without --force
        let args = SessionsArgs {
            format: SessionsFormat::Table,
            checkpoint_path: Some(temp_dir.clone()),
            show: None,
            delete: Some("nonexistent-session".to_string()),
            delete_all: false,
            force: false,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should fail because --force is not set
        assert_eq!(exit_code, SessionsExitCode::Error);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sessions_delete_with_force() {
        use std::fs;

        // Create a temporary directory for checkpoints
        let temp_dir =
            std::env::temp_dir().join(format!("sessions_delete_force_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // Try to delete with --force (session doesn't exist but delete should succeed silently)
        let args = SessionsArgs {
            format: SessionsFormat::Table,
            checkpoint_path: Some(temp_dir.clone()),
            show: None,
            delete: Some("nonexistent-session".to_string()),
            delete_all: false,
            force: true,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should succeed (deleting nonexistent session is not an error)
        assert_eq!(exit_code, SessionsExitCode::Success);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sessions_delete_json_requires_force() {
        use std::fs;

        // Create a temporary directory for checkpoints
        let temp_dir =
            std::env::temp_dir().join(format!("sessions_delete_json_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // Try to delete without --force in JSON format
        let args = SessionsArgs {
            format: SessionsFormat::Json,
            checkpoint_path: Some(temp_dir.clone()),
            show: None,
            delete: Some("test-session".to_string()),
            delete_all: false,
            force: false,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should fail because --force is not set
        assert_eq!(exit_code, SessionsExitCode::Error);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sessions_delete_no_storage() {
        // Try to delete with no checkpoint storage configured
        let args = SessionsArgs {
            format: SessionsFormat::Table,
            checkpoint_path: None,
            show: None,
            delete: Some("test-session".to_string()),
            delete_all: false,
            force: true,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should return NoStorage error
        assert_eq!(exit_code, SessionsExitCode::NoStorage);
    }

    // Tests for --show flag

    #[test]
    fn test_sessions_args_show_flag_parsing() {
        // Test that --show parses correctly
        let result = Args::try_parse_from(["codex-dashflow", "sessions", "--show", "session-123"]);
        assert!(result.is_ok());
        let args = result.unwrap();
        if let Some(Command::Sessions(sessions_args)) = args.command {
            assert_eq!(sessions_args.show, Some("session-123".to_string()));
            assert!(sessions_args.delete.is_none());
            assert!(!sessions_args.force);
        } else {
            panic!("Expected Sessions command");
        }
    }

    #[test]
    fn test_sessions_args_show_with_format() {
        // Test --show with --format json
        let result = Args::try_parse_from([
            "codex-dashflow",
            "sessions",
            "--show",
            "my-session",
            "--format",
            "json",
        ]);
        assert!(result.is_ok());
        let args = result.unwrap();
        if let Some(Command::Sessions(sessions_args)) = args.command {
            assert_eq!(sessions_args.show, Some("my-session".to_string()));
            assert!(matches!(sessions_args.format, SessionsFormat::Json));
        } else {
            panic!("Expected Sessions command");
        }
    }

    #[tokio::test]
    async fn test_sessions_show_no_storage() {
        // Test --show with no checkpoint storage configured
        let args = SessionsArgs {
            format: SessionsFormat::Table,
            checkpoint_path: None,
            show: Some("test-session".to_string()),
            delete: None,
            delete_all: false,
            force: false,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should return NoStorage error
        assert_eq!(exit_code, SessionsExitCode::NoStorage);
    }

    #[tokio::test]
    async fn test_sessions_show_no_storage_json() {
        // Test --show with JSON format and no storage
        let args = SessionsArgs {
            format: SessionsFormat::Json,
            checkpoint_path: None,
            show: Some("test-session".to_string()),
            delete: None,
            delete_all: false,
            force: false,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should return NoStorage error
        assert_eq!(exit_code, SessionsExitCode::NoStorage);
    }

    #[tokio::test]
    async fn test_sessions_show_with_temp_path() {
        use std::fs;

        // Create a temporary directory for checkpoints
        let temp_dir =
            std::env::temp_dir().join(format!("sessions_show_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // Test --show for a non-existent session (should succeed with empty checkpoints)
        let args = SessionsArgs {
            format: SessionsFormat::Table,
            checkpoint_path: Some(temp_dir.clone()),
            show: Some("nonexistent-session".to_string()),
            delete: None,
            delete_all: false,
            force: false,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should succeed (empty checkpoint list for nonexistent session is valid)
        assert_eq!(exit_code, SessionsExitCode::Success);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sessions_show_json_with_temp_path() {
        use std::fs;

        // Create a temporary directory for checkpoints
        let temp_dir =
            std::env::temp_dir().join(format!("sessions_show_json_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // Test --show with JSON format
        let args = SessionsArgs {
            format: SessionsFormat::Json,
            checkpoint_path: Some(temp_dir.clone()),
            show: Some("test-session".to_string()),
            delete: None,
            delete_all: false,
            force: false,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should succeed with JSON output
        assert_eq!(exit_code, SessionsExitCode::Success);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_resolve_session_id_none() {
        // When session_id is None, should return None
        let config = RunnerConfig::default();
        let result = resolve_session_id(None, &config).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_resolve_session_id_specific_value() {
        // When session_id is a specific value, should return it unchanged
        let config = RunnerConfig::default();
        let result = resolve_session_id(Some("my-session-123"), &config).await;
        assert_eq!(result, Some("my-session-123".to_string()));
    }

    #[tokio::test]
    async fn test_resolve_session_id_latest_no_storage() {
        // When session_id is "latest" but no storage configured, should return None
        let config = RunnerConfig::default();
        let result = resolve_session_id(Some("latest"), &config).await;
        // Should return None since no checkpointing configured
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_resolve_session_id_latest_empty_storage() {
        use std::fs;

        // Create a temporary directory for checkpoints
        let temp_dir =
            std::env::temp_dir().join(format!("resolve_session_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // With file checkpointing but no sessions, should return None
        let config = RunnerConfig::with_file_checkpointing(&temp_dir);
        let result = resolve_session_id(Some("latest"), &config).await;
        assert!(result.is_none());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // Tests for --delete-all flag

    #[test]
    fn test_sessions_args_delete_all_flag_parsing() {
        // Test that --delete-all parses correctly
        let result =
            Args::try_parse_from(["codex-dashflow", "sessions", "--delete-all", "--force"]);
        assert!(result.is_ok());
        let args = result.unwrap();
        if let Some(Command::Sessions(sessions_args)) = args.command {
            assert!(sessions_args.delete_all);
            assert!(sessions_args.force);
            assert!(sessions_args.delete.is_none());
        } else {
            panic!("Expected Sessions command");
        }
    }

    #[test]
    fn test_sessions_args_delete_all_requires_force() {
        // Test that --delete-all without --force should fail (clap requires_if)
        let result = Args::try_parse_from(["codex-dashflow", "sessions", "--delete-all"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_sessions_args_delete_all_conflicts_with_delete() {
        // Test that --delete-all conflicts with --delete
        let result = Args::try_parse_from([
            "codex-dashflow",
            "sessions",
            "--delete-all",
            "--delete",
            "session-123",
            "--force",
        ]);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_sessions_delete_all_requires_force() {
        use std::fs;

        // Create a temporary directory for checkpoints
        let temp_dir = std::env::temp_dir().join(format!(
            "sessions_delete_all_noforce_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&temp_dir).unwrap();

        // Try --delete-all without --force
        let args = SessionsArgs {
            format: SessionsFormat::Table,
            checkpoint_path: Some(temp_dir.clone()),
            show: None,
            delete: None,
            delete_all: true,
            force: false,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should fail because --force is not set
        assert_eq!(exit_code, SessionsExitCode::Error);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sessions_delete_all_with_force() {
        use std::fs;

        // Create a temporary directory for checkpoints
        let temp_dir = std::env::temp_dir().join(format!(
            "sessions_delete_all_force_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&temp_dir).unwrap();

        // --delete-all with --force should succeed (even with empty storage)
        let args = SessionsArgs {
            format: SessionsFormat::Table,
            checkpoint_path: Some(temp_dir.clone()),
            show: None,
            delete: None,
            delete_all: true,
            force: true,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should succeed (deleting 0 sessions is valid)
        assert_eq!(exit_code, SessionsExitCode::Success);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sessions_delete_all_json_with_force() {
        use std::fs;

        // Create a temporary directory for checkpoints
        let temp_dir =
            std::env::temp_dir().join(format!("sessions_delete_all_json_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // --delete-all with --force and JSON format
        let args = SessionsArgs {
            format: SessionsFormat::Json,
            checkpoint_path: Some(temp_dir.clone()),
            show: None,
            delete: None,
            delete_all: true,
            force: true,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should succeed with JSON output
        assert_eq!(exit_code, SessionsExitCode::Success);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_sessions_delete_all_no_storage() {
        // --delete-all with no checkpoint storage configured
        let args = SessionsArgs {
            format: SessionsFormat::Table,
            checkpoint_path: None,
            show: None,
            delete: None,
            delete_all: true,
            force: true,
        };
        let config = Config::default();

        let exit_code = run_sessions_command(&args, &config).await;
        // Should return NoStorage error
        assert_eq!(exit_code, SessionsExitCode::NoStorage);
    }

    // ========================================================================
    // Capabilities Subcommand Tests
    // ========================================================================

    #[test]
    fn test_args_capabilities_subcommand() {
        let args = Args::try_parse_from(["codex-dashflow", "capabilities"]).unwrap();
        assert!(args.command.is_some());
        if let Some(Command::Capabilities(caps_args)) = args.command {
            assert!(matches!(caps_args.format, CapabilitiesFormat::Text));
        } else {
            panic!("Expected Capabilities command");
        }
    }

    #[test]
    fn test_args_capabilities_with_json_format() {
        let args =
            Args::try_parse_from(["codex-dashflow", "capabilities", "--format", "json"]).unwrap();
        if let Some(Command::Capabilities(caps_args)) = args.command {
            assert!(matches!(caps_args.format, CapabilitiesFormat::Json));
        } else {
            panic!("Expected Capabilities command");
        }
    }

    #[test]
    fn test_args_capabilities_with_text_format() {
        let args =
            Args::try_parse_from(["codex-dashflow", "capabilities", "--format", "text"]).unwrap();
        if let Some(Command::Capabilities(caps_args)) = args.command {
            assert!(matches!(caps_args.format, CapabilitiesFormat::Text));
        } else {
            panic!("Expected Capabilities command");
        }
    }

    #[test]
    fn test_run_capabilities_command_text() {
        let args = CapabilitiesArgs {
            format: CapabilitiesFormat::Text,
        };
        let exit_code = run_capabilities_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_capabilities_command_json() {
        let args = CapabilitiesArgs {
            format: CapabilitiesFormat::Json,
        };
        let exit_code = run_capabilities_command(&args);
        assert_eq!(exit_code, 0);
    }

    // ========================================================================
    // Features Command Tests
    // ========================================================================

    #[test]
    fn test_args_features_subcommand() {
        let args = Args::try_parse_from(["codex-dashflow", "features"]).unwrap();
        assert!(matches!(args.command, Some(Command::Features(_))));
    }

    #[test]
    fn test_args_features_with_json_format() {
        let args =
            Args::try_parse_from(["codex-dashflow", "features", "--format", "json"]).unwrap();
        if let Some(Command::Features(features_args)) = args.command {
            assert!(matches!(features_args.format, FeaturesFormat::Json));
        } else {
            panic!("Expected Features command");
        }
    }

    #[test]
    fn test_args_features_with_text_format() {
        let args =
            Args::try_parse_from(["codex-dashflow", "features", "--format", "text"]).unwrap();
        if let Some(Command::Features(features_args)) = args.command {
            assert!(matches!(features_args.format, FeaturesFormat::Text));
        } else {
            panic!("Expected Features command");
        }
    }

    #[test]
    fn test_run_features_command_text() {
        let args = FeaturesArgs {
            format: FeaturesFormat::Text,
        };
        let exit_code = run_features_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_run_features_command_json() {
        let args = FeaturesArgs {
            format: FeaturesFormat::Json,
        };
        let exit_code = run_features_command(&args);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_feature_info_struct() {
        let feature = FeatureInfo {
            name: "test-feature".to_string(),
            enabled: true,
            description: "A test feature".to_string(),
            requires: Some("test-requirement".to_string()),
        };

        // Verify serialization works
        let json = serde_json::to_string(&feature).unwrap();
        assert!(json.contains("test-feature"));
        assert!(json.contains("true"));
        assert!(json.contains("A test feature"));
        assert!(json.contains("test-requirement"));
    }

    #[test]
    fn test_feature_info_without_requires() {
        let feature = FeatureInfo {
            name: "simple-feature".to_string(),
            enabled: false,
            description: "A simple feature".to_string(),
            requires: None,
        };

        // Verify serialization works with None
        let json = serde_json::to_string(&feature).unwrap();
        assert!(json.contains("simple-feature"));
        assert!(json.contains("false"));
        assert!(json.contains("null"));
    }
}
