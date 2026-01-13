// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! CLI commands for DashFlow module introspection.
//!
//! # Submodules
//!
//! - [`health`]: Runtime health checks for deployed instances
//!
//! Provides direct, CLI-first access to module discovery data.
//! This is the primary interface for AI workers working ON DashFlow.
//!
//! # Output Formats
//!
//! All introspect subcommands support `--format` for output format selection:
//! - `--format table` (default): Human-readable colored table output
//! - `--format json`: Machine-readable JSON output for automation
//!
//! # Examples
//!
//! ```bash
//! # List all modules
//! dashflow introspect list
//! dashflow introspect list --format json
//!
//! # Search for modules
//! dashflow introspect search distill
//! dashflow introspect search kafka --format json
//!
//! # Show details for a module
//! dashflow introspect show distillation
//! dashflow introspect show distillation --format json
//!
//! # Show CLI wiring status
//! dashflow introspect cli
//! dashflow introspect cli --stubs-only
//! dashflow introspect cli --format json
//!
//! # Health checks
//! dashflow introspect health
//! dashflow introspect health --format json
//! ```

use crate::output::{create_table, print_error, print_info, OutputFormat};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::Colorize;
use dashflow::introspection::ExecutionTrace;
use dashflow::introspection::{InterfaceConfig, IntrospectionInterface};
use dashflow::introspection::ModulePatternRegistry;
use dashflow::lint::{TypeIndex, TypeIndexCache};
use dashflow::unified_introspection::{DashFlowIntrospection, IntrospectionLevel};
use dashflow_module_discovery::{
    discover_all_types, discover_all_workspace_crates, discover_modules,
    discover_workspace_binaries, discover_workspace_modules, CliStatus, ModuleInfo, ModuleStatus,
};
use std::path::{Path, PathBuf};

mod health;
mod types;

/// M-606: Parse introspection level from command line argument
fn parse_introspection_level(s: &str) -> Result<IntrospectionLevel, String> {
    match s.to_lowercase().as_str() {
        "platform" => Ok(IntrospectionLevel::Platform),
        "application" | "app" => Ok(IntrospectionLevel::Application),
        "runtime" => Ok(IntrospectionLevel::Runtime),
        "network" => Ok(IntrospectionLevel::Network),
        _ => Err(format!(
            "Invalid level '{}'. Valid options: platform, application (or app), runtime, network",
            s
        )),
    }
}

/// Static CLI command registry - authoritative list of all CLI commands from main.rs Commands enum.
/// This ensures `dashflow introspect cli` shows all 29 commands regardless of @cli annotations.
struct CliCommandEntry {
    /// The command name (e.g., "tail", "watch")
    command: &'static str,
    /// Description from the Commands enum doc comment
    description: &'static str,
    /// Category for grouping
    category: &'static str,
}

/// All CLI commands from dashflow-cli/src/main.rs Commands enum
static CLI_COMMANDS: &[CliCommandEntry] = &[
    // === Unified Timeline Interface (M-38 - RECOMMENDED) ===
    CliCommandEntry {
        command: "timeline",
        description: "Unified timeline interface for graph execution (RECOMMENDED entry point)",
        category: "streaming",
    },
    // === Streaming Telemetry Commands ===
    CliCommandEntry {
        command: "tail",
        description: "Stream live events from Kafka",
        category: "streaming",
    },
    CliCommandEntry {
        command: "watch",
        description: "Watch live graph execution with TUI visualization",
        category: "streaming",
    },
    CliCommandEntry {
        command: "inspect",
        description: "Show thread details and execution history",
        category: "streaming",
    },
    CliCommandEntry {
        command: "replay",
        description: "Replay execution from a checkpoint (time-travel debugging)",
        category: "streaming",
    },
    CliCommandEntry {
        command: "diff",
        description: "Compare two checkpoints",
        category: "streaming",
    },
    CliCommandEntry {
        command: "export",
        description: "Export thread data to JSON",
        category: "streaming",
    },
    CliCommandEntry {
        command: "flamegraph",
        description: "Generate flamegraph for performance visualization (Kafka)",
        category: "analysis",
    },
    CliCommandEntry {
        command: "costs",
        description: "Analyze token costs across executions (Kafka)",
        category: "analysis",
    },
    CliCommandEntry {
        command: "profile",
        description: "Profile execution performance (Kafka)",
        category: "analysis",
    },
    CliCommandEntry {
        command: "analyze",
        description: "Analyze exported JSON files offline (no Kafka required)",
        category: "analysis",
    },
    // === Optimization Commands ===
    CliCommandEntry {
        command: "optimize",
        description: "Run prompt optimization on a graph",
        category: "optimization",
    },
    CliCommandEntry {
        command: "eval",
        description: "Evaluate graph performance on a test dataset",
        category: "optimization",
    },
    CliCommandEntry {
        command: "train",
        description: "Train or fine-tune models (distillation, RL)",
        category: "optimization",
    },
    CliCommandEntry {
        command: "dataset",
        description: "Dataset utilities (generate, validate, inspect)",
        category: "optimization",
    },
    CliCommandEntry {
        command: "evals",
        description: "Manage evaluation test cases and golden datasets (list, show, promote)",
        category: "optimization",
    },
    CliCommandEntry {
        command: "baseline",
        description: "Manage evaluation baselines (save, list, check, delete)",
        category: "optimization",
    },
    // === Developer Tools ===
    CliCommandEntry {
        command: "visualize",
        description: "Visualize DashFlow graphs with interactive web UI",
        category: "developer",
    },
    CliCommandEntry {
        command: "debug",
        description: "Interactive debugger for step-through graph execution",
        category: "developer",
    },
    CliCommandEntry {
        command: "lint",
        description: "Lint for platform feature reimplementations",
        category: "developer",
    },
    // === Pattern Detection ===
    CliCommandEntry {
        command: "patterns",
        description: "Detect patterns in execution traces (unified pattern engine)",
        category: "patterns",
    },
    // === Parallel AI Development ===
    CliCommandEntry {
        command: "locks",
        description: "Manage parallel AI development locks (list, acquire, release)",
        category: "infrastructure",
    },
    // === Infrastructure Health ===
    CliCommandEntry {
        command: "status",
        description: "Check DashFlow infrastructure health (Docker, Kafka, services)",
        category: "infrastructure",
    },
    CliCommandEntry {
        command: "executions",
        description: "Query persisted executions from EventStore (list, show, events)",
        category: "infrastructure",
    },
    // === Introspection ===
    CliCommandEntry {
        command: "introspect",
        description: "Query DashFlow module information directly (CLI-first introspection)",
        category: "meta",
    },
    CliCommandEntry {
        command: "mcp-server",
        description: "MCP server for AI introspection (HTTP API for external tools)",
        category: "meta",
    },
    // === Self-Improvement ===
    CliCommandEntry {
        command: "self-improve",
        description: "Self-improvement commands for AI agents (analyze, plans, approve)",
        category: "meta",
    },
    // === Project Scaffolding ===
    CliCommandEntry {
        command: "new",
        description: "Create a new DashFlow application with production defaults",
        category: "scaffolding",
    },
    // === Package Registry ===
    CliCommandEntry {
        command: "pkg",
        description: "Package registry operations (search, install, publish)",
        category: "registry",
    },
];

/// M-604: Synonym index for improved search results.
/// Maps common search terms to related keywords that should also be searched.
/// This helps find modules when users search for concepts rather than exact names.
///
/// Usage: When user searches for "consumer", we also search for related terms
/// like "kafka", "streaming", "queue" to surface relevant modules.
static SEARCH_SYNONYMS: &[(&str, &[&str])] = &[
    // Messaging/Streaming
    ("consumer", &["kafka", "streaming", "queue", "message", "subscriber", "event"]),
    ("producer", &["kafka", "streaming", "queue", "message", "publisher", "event"]),
    ("kafka", &["streaming", "consumer", "producer", "queue", "message", "event"]),
    ("streaming", &["kafka", "event", "realtime", "websocket", "dashstream"]),
    ("queue", &["kafka", "message", "consumer", "producer", "streaming"]),
    ("message", &["kafka", "queue", "event", "streaming"]),
    ("event", &["streaming", "kafka", "telemetry", "dashstream", "message"]),

    // Vector/Embeddings
    ("vector", &["embedding", "similarity", "search", "chroma", "pinecone", "qdrant"]),
    ("embedding", &["vector", "openai", "huggingface", "similarity", "encode"]),
    ("similarity", &["vector", "embedding", "search", "cosine", "distance"]),
    ("search", &["vector", "retriever", "rag", "query", "find"]),

    // Retrieval/RAG
    ("retriever", &["rag", "search", "vector", "document", "knowledge"]),
    ("rag", &["retriever", "vector", "document", "knowledge", "qa"]),
    ("document", &["loader", "parser", "text", "pdf", "retriever"]),

    // LLM/AI
    ("llm", &["openai", "anthropic", "model", "chat", "completion", "gpt", "claude"]),
    ("model", &["llm", "openai", "anthropic", "huggingface", "inference"]),
    ("chat", &["llm", "openai", "anthropic", "completion", "conversation"]),
    ("gpt", &["openai", "llm", "chat", "completion"]),
    ("claude", &["anthropic", "llm", "chat", "completion"]),

    // Optimization
    ("optimizer", &["optimize", "prompt", "distillation", "copro", "bootstrap"]),
    ("optimize", &["optimizer", "prompt", "distillation", "improvement"]),
    ("distillation", &["optimizer", "student", "teacher", "finetuning"]),
    ("prompt", &["optimizer", "template", "chain", "engineering"]),

    // Observability
    ("metrics", &["prometheus", "telemetry", "monitoring", "dashstream"]),
    ("telemetry", &["metrics", "trace", "span", "monitoring", "dashstream", "event"]),
    ("monitoring", &["metrics", "telemetry", "prometheus", "grafana", "health"]),
    ("trace", &["telemetry", "span", "observability", "debugging"]),

    // Tools
    ("tool", &["agent", "function", "call", "action"]),
    ("agent", &["tool", "langchain", "graph", "executor"]),

    // State/Graph
    ("state", &["graph", "checkpoint", "persistence", "memory"]),
    ("graph", &["state", "node", "edge", "executor", "flow"]),
    ("checkpoint", &["state", "persistence", "recovery", "snapshot"]),

    // Storage
    ("redis", &["cache", "storage", "memory", "persistence"]),
    ("cache", &["redis", "memory", "storage"]),
    ("database", &["storage", "persistence", "postgres", "sqlite", "mongodb"]),
    ("storage", &["persistence", "database", "cache", "redis"]),
];

/// M-604: Expand a search query with synonyms.
/// Returns a set of terms including the original and all related synonyms.
fn expand_search_query(query: &str) -> Vec<String> {
    let query_lower = query.to_lowercase();
    let mut terms = vec![query_lower.clone()];

    // Find all synonyms for the query
    for (term, synonyms) in SEARCH_SYNONYMS {
        if query_lower == *term || query_lower.contains(term) {
            for syn in *synonyms {
                if !terms.contains(&syn.to_string()) {
                    terms.push(syn.to_string());
                }
            }
        }
    }

    terms
}

/// Query DashFlow module information (CLI-first introspection)
#[derive(Args)]
pub struct IntrospectArgs {
    #[command(subcommand)]
    pub command: IntrospectCommand,
}

#[derive(Subcommand)]
pub enum IntrospectCommand {
    /// List all discovered modules
    List(ListArgs),

    /// Search for modules by name, path, or description
    Search(SearchArgs),

    /// Show details for a specific module
    Show(ShowArgs),

    /// Show CLI command wiring status
    Cli(CliArgs),

    /// Run runtime health checks on the deployed instance
    Health(HealthArgs),

    /// Ask a natural language question about execution traces
    Ask(AskArgs),

    /// Search and view API documentation
    Docs(DocsArgs),

    /// Query optimizer selection guidance and metadata
    Optimizers(OptimizersArgs),

    /// Select an optimizer based on context (examples, task type, constraints)
    Optimize(OptimizeArgs),

    /// View optimization history and past outcomes
    OptimizeHistory(OptimizeHistoryArgs),

    /// Get insights and patterns learned from optimization history
    OptimizeInsights(OptimizeInsightsArgs),

    /// List public types (structs, traits, fns) in a crate or workspace
    Types(TypesArgs),

    /// Manage the type index (status, rebuild)
    Index(TypeIndexArgs),

    /// Find types by capability tag (e.g., "bm25", "retriever", "embeddings")
    FindCapability(FindCapabilityArgs),

    /// List all available capability tags in the workspace
    Capabilities(CapabilitiesArgs),

    /// Find platform alternatives for a code snippet
    Alternatives(AlternativesArgs),

    /// Show all automatic behaviors (ON by default per DESIGN_INVARIANTS.md Invariant 6)
    ///
    /// Generates a markdown report of all features that are automatically enabled,
    /// their opt-out environment variables, and file locations. Output to stdout
    /// for piping to a file or direct reading.
    Automatic(AutomaticArgs),

    /// List module capabilities with lint patterns (Phase 938)
    ///
    /// Shows all modules registered with capability tags and replacement patterns.
    /// Use `--with-patterns` to include lint patterns for self-linting.
    Modules(ModulesArgs),
}

/// List all modules
#[derive(Args)]
pub struct ListArgs {
    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Filter by category (e.g., "optimize", "core")
    #[arg(long)]
    category: Option<String>,

    /// Path to dashflow/src directory (default: auto-detect)
    #[arg(long)]
    src_path: Option<PathBuf>,
}

/// Search for modules
#[derive(Args)]
pub struct SearchArgs {
    /// Search query (matches name, path, description, capability tags)
    pub query: Option<String>,

    /// Filter results to modules/types that have this capability tag (e.g., "kafka", "retriever")
    #[arg(long)]
    pub capability: Option<String>,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Also search individual types (structs, traits, fns)
    #[arg(long)]
    types: bool,

    /// Use semantic (TF-IDF based) similarity search instead of substring matching
    #[arg(long)]
    semantic: bool,

    /// Maximum number of results for semantic search (default: 20)
    #[arg(long, default_value_t = 20)]
    limit: usize,

    /// Minimum similarity score threshold for semantic search (0.0-1.0, default: 0.1)
    #[arg(long, default_value_t = 0.1)]
    min_score: f32,

    /// Path to dashflow/src directory (default: auto-detect)
    #[arg(long)]
    src_path: Option<PathBuf>,
}

/// Show details for a module
#[derive(Args)]
pub struct ShowArgs {
    /// Module name to show
    pub name: String,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Path to dashflow/src directory (default: auto-detect)
    #[arg(long)]
    src_path: Option<PathBuf>,
}

/// Show CLI wiring status
#[derive(Args)]
pub struct CliArgs {
    /// Show only stub (unwired) commands
    #[arg(long)]
    stubs_only: bool,

    /// Show only wired commands
    #[arg(long)]
    wired_only: bool,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Path to dashflow/src directory (default: auto-detect)
    #[arg(long)]
    src_path: Option<PathBuf>,
}

/// Runtime health checks for deployed instances
#[derive(Args)]
pub struct HealthArgs {
    /// Skip all infrastructure checks (Grafana, Prometheus, Docker)
    #[arg(long)]
    pub skip_infra: bool,

    /// Skip Grafana health check
    #[arg(long)]
    pub skip_grafana: bool,

    /// Skip Prometheus health check
    #[arg(long)]
    pub skip_prometheus: bool,

    /// Skip Docker services check
    #[arg(long)]
    pub skip_docker: bool,

    /// Skip LLM connectivity check (requires API keys)
    #[arg(long)]
    pub skip_llm: bool,

    /// Skip Kafka connectivity check
    #[arg(long)]
    pub skip_kafka: bool,

    /// Skip documentation coverage check
    #[arg(long)]
    pub skip_docs: bool,

    /// Timeout for checks in seconds
    #[arg(long, default_value_t = 30)]
    pub timeout: u64,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Show automatic behaviors (ON by default)
#[derive(Args)]
pub struct AutomaticArgs {
    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Path to dashflow crate source (default: auto-detect)
    #[arg(long)]
    src_path: Option<PathBuf>,
}

/// Module capabilities with lint patterns (Phase 938)
#[derive(Args)]
pub struct ModulesArgs {
    /// Include lint patterns that modules replace
    #[arg(long)]
    with_patterns: bool,

    /// Filter by capability tag (e.g., "bm25", "retriever")
    #[arg(long)]
    capability: Option<String>,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Ask questions about execution traces
#[derive(Args)]
pub struct AskArgs {
    /// The question to ask (e.g., "Why did search run 3 times?")
    pub question: String,

    /// Path to execution trace file (optional - uses latest from .dashflow/traces/ by default)
    #[arg(long)]
    trace: Option<PathBuf>,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Use detailed response format with more context
    #[arg(long)]
    detailed: bool,

    /// M-606: Explicit introspection level to bypass auto-classification
    /// Useful when keyword-based routing misclassifies your question.
    /// Options: platform, application (or app), runtime, network
    #[arg(long, short = 'l', value_parser = parse_introspection_level)]
    level: Option<IntrospectionLevel>,
}

/// Search and view API documentation
#[derive(Args)]
pub struct DocsArgs {
    #[command(subcommand)]
    pub command: DocsCommand,
}

#[derive(Subcommand)]
pub enum DocsCommand {
    /// Search documentation for a keyword
    Search(DocsSearchArgs),

    /// Show full documentation for a specific item
    Show(DocsShowArgs),

    /// Show documentation coverage summary
    Coverage(DocsCoverageArgs),

    /// Manage the documentation index (build, status, rebuild)
    Index(DocsIndexArgs),
}

/// Manage documentation index
#[derive(Args)]
pub struct DocsIndexArgs {
    #[command(subcommand)]
    pub command: DocsIndexCommand,
}

#[derive(Subcommand)]
pub enum DocsIndexCommand {
    /// Build or rebuild the documentation index
    Build(DocsIndexBuildArgs),

    /// Show index status (freshness, item count, etc.)
    Status(DocsIndexStatusArgs),
}

/// Build documentation index
#[derive(Args)]
pub struct DocsIndexBuildArgs {
    /// Force rebuild even if index is up to date
    #[arg(long)]
    force: bool,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Show index status
#[derive(Args)]
pub struct DocsIndexStatusArgs {
    /// Check if rebuild is needed (exits 1 if stale)
    #[arg(long)]
    check_stale: bool,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Search documentation
#[derive(Args)]
pub struct DocsSearchArgs {
    /// Search query (matches type names, function names, descriptions)
    pub query: String,

    /// Limit number of results (default: 10)
    #[arg(long, short, default_value = "10")]
    limit: usize,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Show documentation for a specific item
#[derive(Args)]
pub struct DocsShowArgs {
    /// Item name to show (e.g., "StateGraph", "RetryPolicy", "dashflow-openai::ChatOpenAI")
    pub name: String,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Show documentation coverage summary
#[derive(Args)]
pub struct DocsCoverageArgs {
    /// Show coverage for a specific crate
    #[arg(long)]
    crate_name: Option<String>,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Show detailed breakdown per file
    #[arg(long)]
    detailed: bool,
}

/// Query optimizer selection guidance and metadata
#[derive(Args)]
pub struct OptimizersArgs {
    /// Show details for a specific optimizer (e.g., "MIPROv2")
    #[arg(long)]
    name: Option<String>,

    /// Filter by tier (1=Recommended, 2=Specialized, 3=Niche)
    #[arg(long)]
    tier: Option<u8>,

    /// Get recommendation based on number of training examples
    #[arg(long)]
    examples: Option<usize>,

    /// Include finetuning capability in recommendation (used with --examples)
    #[arg(long)]
    can_finetune: bool,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Select an optimizer based on context
#[derive(Args)]
pub struct OptimizeArgs {
    /// Number of training examples available
    #[arg(long, default_value = "0")]
    examples: usize,

    /// Task type (qa, classification, code, math, agent, reasoning, summarization, generic)
    #[arg(long, short = 't')]
    task: Option<String>,

    /// Whether the model supports finetuning
    #[arg(long)]
    can_finetune: bool,

    /// Compute budget (minimal, low, medium, high, unlimited)
    #[arg(long, default_value = "medium")]
    budget: String,

    /// Optimizer to exclude from selection
    #[arg(long)]
    exclude: Vec<String>,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// View optimization history
#[derive(Args)]
pub struct OptimizeHistoryArgs {
    /// Maximum number of outcomes to show
    #[arg(long, short = 'n', default_value = "20")]
    limit: usize,

    /// Filter by optimizer name
    #[arg(long)]
    optimizer: Option<String>,

    /// Show only successful optimizations
    #[arg(long)]
    successful: bool,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Get insights from optimization history
#[derive(Args)]
pub struct OptimizeInsightsArgs {
    /// Focus on a specific task type
    #[arg(long, short = 't')]
    task: Option<String>,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// List public types in a crate or workspace
#[derive(Args)]
pub struct TypesArgs {
    /// Crate name to search (optional - searches all crates if not specified)
    pub crate_name: Option<String>,

    /// Filter by type kind (struct, enum, trait, fn)
    #[arg(long, short = 'k')]
    kind: Option<String>,

    /// Search filter (matches name, path, description)
    #[arg(long, short = 'f')]
    filter: Option<String>,

    /// Filter by capability tag (e.g., "retriever", "bm25", "embeddings")
    #[arg(long, short = 'c')]
    capability: Option<String>,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Manage the type index (status, rebuild)
#[derive(Args)]
pub struct TypeIndexArgs {
    /// Force rebuild the index (ignore cache)
    #[arg(long)]
    rebuild: bool,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Find types by capability tag
#[derive(Args)]
pub struct FindCapabilityArgs {
    /// Capability tag to search for (e.g., "bm25", "retriever", "embeddings")
    pub capability: String,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Show capability tags for each result
    #[arg(long)]
    show_tags: bool,

    /// Limit number of results (default: all)
    #[arg(long, short = 'n')]
    limit: Option<usize>,
}

/// List all available capability tags
#[derive(Args)]
pub struct CapabilitiesArgs {
    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Show count of types for each capability
    #[arg(long)]
    with_counts: bool,
}

/// Find platform alternatives for a code snippet
#[derive(Args)]
pub struct AlternativesArgs {
    /// Code snippet or natural language description to find alternatives for
    /// Example: "fn search_keyword" or "BM25 keyword search"
    pub snippet: String,

    /// Output format (table or json)
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Maximum number of results to return (default: 10)
    #[arg(long, default_value_t = 10)]
    limit: usize,

    /// Minimum similarity score (0.0-1.0, default: 0.1)
    #[arg(long, default_value_t = 0.1)]
    min_score: f32,
}

pub async fn run(args: IntrospectArgs) -> Result<()> {
    match args.command {
        IntrospectCommand::List(list_args) => run_list(list_args).await,
        IntrospectCommand::Search(search_args) => run_search(search_args).await,
        IntrospectCommand::Show(show_args) => run_show(show_args).await,
        IntrospectCommand::Ask(ask_args) => run_ask(ask_args).await,
        IntrospectCommand::Cli(cli_args) => run_cli(cli_args).await,
        IntrospectCommand::Health(health_args) => health::run_health(health_args).await,
        IntrospectCommand::Docs(docs_args) => run_docs(docs_args).await,
        IntrospectCommand::Optimizers(opt_args) => run_optimizers(opt_args).await,
        IntrospectCommand::Optimize(opt_args) => run_optimize(opt_args).await,
        IntrospectCommand::OptimizeHistory(hist_args) => run_optimize_history(hist_args).await,
        IntrospectCommand::OptimizeInsights(ins_args) => run_optimize_insights(ins_args).await,
        IntrospectCommand::Types(types_args) => types::run_types(types_args).await,
        IntrospectCommand::Index(index_args) => types::run_type_index(index_args).await,
        IntrospectCommand::FindCapability(args) => types::run_find_capability(args).await,
        IntrospectCommand::Capabilities(args) => types::run_capabilities(args).await,
        IntrospectCommand::Alternatives(args) => types::run_alternatives(args).await,
        IntrospectCommand::Automatic(args) => run_automatic(args).await,
        IntrospectCommand::Modules(args) => run_modules(args).await,
    }
}

/// Find the workspace root directory
fn get_workspace_root() -> PathBuf {
    // Try to find workspace root relative to current directory
    let candidates = vec![
        PathBuf::from("."),
        PathBuf::from(".."),
        PathBuf::from("../.."),
    ];

    for candidate in &candidates {
        // Check for workspace Cargo.toml with [workspace] section
        let cargo_toml = candidate.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return candidate.clone();
                }
            }
        }
    }

    // Fallback
    PathBuf::from(".")
}

/// Get the source path, using the provided override or the workspace root as default.
#[cfg(test)]
fn get_src_path(src_path_override: Option<PathBuf>) -> PathBuf {
    src_path_override.unwrap_or_else(get_workspace_root)
}

/// Discover modules from all workspace crates
fn discover_all_modules_in_workspace(src_path_override: Option<PathBuf>) -> Vec<ModuleInfo> {
    if let Some(src_path) = src_path_override {
        // User provided a specific src path, only scan that
        discover_modules(&src_path)
    } else {
        // Scan ALL workspace crates for comprehensive discovery
        let workspace_root = get_workspace_root();
        let all_crates = discover_all_workspace_crates(&workspace_root);
        let mut modules = discover_workspace_modules(&workspace_root, &all_crates);

        // M-605: Also discover binaries in src/bin/ directories
        let binaries = discover_workspace_binaries(&workspace_root, &all_crates);
        modules.extend(binaries);

        modules
    }
}

async fn run_list(args: ListArgs) -> Result<()> {
    let src_path = args.src_path;
    let modules = tokio::task::spawn_blocking(move || discover_all_modules_in_workspace(src_path))
        .await
        .context("discover_all_modules_in_workspace panicked")?;

    // Filter by category if provided
    let filtered: Vec<_> = if let Some(ref cat) = args.category {
        modules
            .into_iter()
            .filter(|m| m.category.eq_ignore_ascii_case(cat))
            .collect()
    } else {
        modules
    };

    if matches!(args.format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(&filtered)?);
        return Ok(());
    }

    // Human-readable output
    println!();
    println!(
        "{} {} modules discovered",
        "DashFlow Modules:".bright_cyan().bold(),
        filtered.len().to_string().bright_green()
    );
    println!("{}", "═".repeat(80).bright_cyan());

    // Group by category
    let mut by_category: std::collections::BTreeMap<String, Vec<&ModuleInfo>> =
        std::collections::BTreeMap::new();
    for module in &filtered {
        by_category
            .entry(module.category.clone())
            .or_default()
            .push(module);
    }

    for (category, modules) in by_category {
        println!("\n{} ({})", category.bright_yellow().bold(), modules.len());
        println!("{}", "─".repeat(60));

        for module in modules {
            let status_icon = match module.status {
                ModuleStatus::Stable => "●".bright_green(),
                ModuleStatus::Experimental => "○".bright_yellow(),
                ModuleStatus::Deprecated => "◌".bright_red(),
            };

            let cli_info = if module.cli_command.is_some() {
                match module.cli_status {
                    Some(CliStatus::Wired) => " [CLI: wired]".bright_green().to_string(),
                    Some(CliStatus::Stub) => " [CLI: stub]".bright_yellow().to_string(),
                    _ => String::new(),
                }
            } else {
                String::new()
            };

            println!(
                "  {} {} {}{}",
                status_icon,
                module.name.bright_white(),
                module.path.dimmed(),
                cli_info
            );
        }
    }

    println!(
        "\n{}",
        "Legend: ● stable  ○ experimental  ◌ deprecated".dimmed()
    );

    Ok(())
}

async fn run_search(args: SearchArgs) -> Result<()> {
    let workspace_root = tokio::task::spawn_blocking(get_workspace_root)
        .await
        .context("get_workspace_root panicked")?;

    // Handle semantic search mode
    if args.semantic {
        if args.query.is_none() {
            anyhow::bail!("Semantic search requires a <query>");
        }
        return run_semantic_search(&workspace_root, &args).await;
    }

    let src_path_for_modules = args.src_path.clone();
    let modules =
        tokio::task::spawn_blocking(move || discover_all_modules_in_workspace(src_path_for_modules))
            .await
            .context("discover_all_modules_in_workspace panicked")?;

    if args.query.is_none() && args.capability.is_none() {
        anyhow::bail!("Provide a <query> and/or --capability <tag>");
    }

    let query_lower = args.query.as_deref().unwrap_or("").to_lowercase();
    // M-604: Expand query with synonyms for better search results
    let search_terms = if query_lower.is_empty() {
        vec![]
    } else {
        expand_search_query(&query_lower)
    };
    let capability_lower = args.capability.as_deref().map(|c| c.to_lowercase());
    let results: Vec<_> = modules
        .into_iter()
        .filter(|m| {
            // M-604: Check if any of the expanded search terms match
            let matches_query = search_terms.is_empty()
                || search_terms.iter().any(|term| {
                    m.name.to_lowercase().contains(term)
                        || m.path.to_lowercase().contains(term)
                        || m.description.to_lowercase().contains(term)
                        || m.capability_tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(term))
                });

            let matches_capability = match capability_lower.as_ref() {
                None => true,
                Some(cap) => m
                    .capability_tags
                    .iter()
                    .any(|tag| tag.to_lowercase().contains(cap)),
            };

            matches_query && matches_capability
        })
        .collect();

    // Also search types if --types flag is set (deduplicated by path)
    // M-604: Uses same synonym-expanded search_terms as module search
    let type_results: Vec<_> = if args.types {
        let all_types = discover_all_types(&workspace_root);
        let mut seen_paths = std::collections::HashSet::new();
        all_types
            .into_iter()
            .filter(|t| {
                // M-604: Check if any of the expanded search terms match
                let matches_query = search_terms.is_empty()
                    || search_terms.iter().any(|term| {
                        t.name.to_lowercase().contains(term)
                            || t.path.to_lowercase().contains(term)
                            || t.description.to_lowercase().contains(term)
                            || t.capability_tags
                                .iter()
                                .any(|tag| tag.to_lowercase().contains(term))
                    });

                let matches_capability = match capability_lower.as_ref() {
                    None => true,
                    Some(cap) => t
                        .capability_tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(cap)),
                };

                matches_query && matches_capability
            })
            .filter(|t| seen_paths.insert(t.path.clone())) // Deduplicate by path
            .collect()
    } else {
        Vec::new()
    };

    if matches!(args.format, OutputFormat::Json) {
        // Output combined results as JSON
        let combined = serde_json::json!({
            "modules": results,
            "types": type_results,
        });
        println!("{}", serde_json::to_string_pretty(&combined)?);
        return Ok(());
    }

    // Human-readable output
    if results.is_empty() && type_results.is_empty() {
        let what = if let Some(ref query) = args.query {
            format!("matching '{query}'")
        } else if let Some(ref cap) = args.capability {
            format!("with capability '{cap}'")
        } else {
            "matching filters".to_string()
        };
        print_info(&format!("No modules/types found {what}"));
        return Ok(());
    }

    // Show module results
    if !results.is_empty() {
        println!();
        let filter_desc = match (&args.query, &args.capability) {
            (Some(q), Some(c)) => format!("'{q}' + capability '{c}'"),
            (Some(q), None) => format!("'{q}'"),
            (None, Some(c)) => format!("capability '{c}'"),
            (None, None) => "filters".to_string(),
        };
        println!(
            "{} {} module results for {}",
            "Search:".bright_cyan().bold(),
            results.len().to_string().bright_green(),
            filter_desc.bright_white()
        );
        // M-604: Show synonym expansion hint when used
        if search_terms.len() > 1 {
            println!(
                "{} (also searched: {})",
                "Synonyms:".dimmed(),
                search_terms[1..].join(", ").dimmed()
            );
        }
        println!("{}", "═".repeat(80).bright_cyan());

        let mut table = create_table();
        table.set_header(vec!["Module", "Path", "Description", "CLI Status"]);

        for module in &results {
            let cli_status = module
                .cli_command
                .as_ref()
                .map(|cmd| {
                    let status = match module.cli_status {
                        Some(CliStatus::Wired) => "wired".bright_green().to_string(),
                        Some(CliStatus::Stub) => "stub".bright_yellow().to_string(),
                        _ => "none".dimmed().to_string(),
                    };
                    format!("{} ({})", cmd, status)
                })
                .unwrap_or_else(|| "-".dimmed().to_string());

            let desc = truncate_str(&module.description, 40);

            table.add_row(vec![
                module.name.clone(),
                module.path.clone(),
                desc,
                cli_status,
            ]);
        }

        println!("{table}");
    }

    // Show type results if --types flag was used
    if !type_results.is_empty() {
        println!();
        let filter_desc = match (&args.query, &args.capability) {
            (Some(q), Some(c)) => format!("'{q}' + capability '{c}'"),
            (Some(q), None) => format!("'{q}'"),
            (None, Some(c)) => format!("capability '{c}'"),
            (None, None) => "filters".to_string(),
        };
        println!(
            "{} {} type results for {}",
            "Types:".bright_magenta().bold(),
            type_results.len().to_string().bright_green(),
            filter_desc.bright_white()
        );
        println!("{}", "─".repeat(80).bright_magenta());

        let mut table = create_table();
        table.set_header(vec!["Type", "Kind", "Crate", "Description"]);

        for ty in &type_results {
            let kind_str = types::format_type_kind(ty.kind);
            let desc = truncate_str(&ty.description, 40);

            table.add_row(vec![ty.name.clone(), kind_str, ty.crate_name.clone(), desc]);
        }

        println!("{table}");
    }

    Ok(())
}

/// Perform semantic (TF-IDF based) similarity search
async fn run_semantic_search(workspace_root: &Path, args: &SearchArgs) -> Result<()> {
    // Build or load the type index with semantic embeddings
    let cache_path = workspace_root.join(TypeIndexCache::CACHE_PATH);
    let type_index = if let Some((index, cache)) = TypeIndex::load(&cache_path) {
        // Check staleness and auto-rebuild if needed
        let is_stale = cache.is_stale(workspace_root) == Some(true);
        let needs_semantic = !index.has_semantic_index();

        if is_stale || needs_semantic {
            let reason = if is_stale && needs_semantic {
                "stale and missing semantic index"
            } else if is_stale {
                "stale"
            } else {
                "missing semantic index (v1 cache)"
            };
            println!(
                "{} Type index is {}. Auto-rebuilding...",
                "Info:".bright_cyan().bold(),
                reason
            );
            TypeIndex::regenerate(workspace_root.to_path_buf())
        } else {
            index
        }
    } else {
        println!(
            "{} Building type index (this may take a moment)...",
            "Info:".bright_cyan().bold()
        );
        TypeIndex::regenerate(workspace_root.to_path_buf())
    };

    let query = args
        .query
        .as_deref()
        .context("Semantic search requires a <query>")?;

    // Perform semantic search
    let results: Vec<_> = type_index
        .search_semantic(query, args.limit)
        .into_iter()
        .filter(|(_, score)| *score >= args.min_score)
        .filter(|(ty, _)| match args.capability.as_deref() {
            None => true,
            Some(cap) => {
                let cap_lower = cap.to_lowercase();
                ty.capability_tags
                    .iter()
                    .any(|tag| tag.to_lowercase().contains(&cap_lower))
            }
        })
        .collect();

    if matches!(args.format, OutputFormat::Json) {
        // JSON output with similarity scores
        let json_results: Vec<_> = results
            .iter()
            .map(|(ty, score)| {
                serde_json::json!({
                    "name": ty.name,
                    "path": ty.path,
                    "crate": ty.crate_name,
                    "kind": format!("{:?}", ty.kind).to_lowercase(),
                    "description": ty.description,
                    "score": score,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_results)?);
        return Ok(());
    }

    // Human-readable output
    if results.is_empty() {
        print_info(&format!(
            "No semantically similar types found for '{}' (min-score: {:.2})",
            query, args.min_score
        ));
        println!(
            "\n{} Try lowering --min-score, adjusting your query, or use substring search (without --semantic).",
            "Tip:".bright_cyan()
        );
        return Ok(());
    }

    println!();
    println!(
        "{} {} semantically similar types for '{}'",
        "Semantic Search:".bright_magenta().bold(),
        results.len().to_string().bright_green(),
        query.bright_white()
    );

    // Show semantic index stats
    if let Some((type_count, vocab_size)) = type_index.semantic_stats() {
        println!(
            "{} ({} types indexed, {} terms in vocabulary)",
            "═".repeat(80).bright_magenta(),
            type_count.to_string().dimmed(),
            vocab_size.to_string().dimmed()
        );
    } else {
        println!("{}", "═".repeat(80).bright_magenta());
    }

    let mut table = create_table();
    table.set_header(vec!["Type", "Score", "Kind", "Crate", "Description"]);

    for (ty, score) in &results {
        let kind_str = types::format_type_kind(ty.kind);
        let desc = truncate_str(&ty.description, 35);
        let score_str = format!("{:.3}", score);

        // Color score based on similarity
        let score_colored = if *score > 0.5 {
            score_str.bright_green().to_string()
        } else if *score > 0.3 {
            score_str.bright_yellow().to_string()
        } else {
            score_str.dimmed().to_string()
        };

        table.add_row(vec![
            ty.name.clone(),
            score_colored,
            kind_str,
            ty.crate_name.clone(),
            desc,
        ]);
    }

    println!("{table}");

    Ok(())
}

async fn run_show(args: ShowArgs) -> Result<()> {
    let src_path = args.src_path;
    let modules = tokio::task::spawn_blocking(move || discover_all_modules_in_workspace(src_path))
        .await
        .context("discover_all_modules_in_workspace panicked")?;

    let module = modules.iter().find(|m| {
        m.name.eq_ignore_ascii_case(&args.name) || m.path.eq_ignore_ascii_case(&args.name)
    });

    let Some(module) = module else {
        print_error(&format!("Module '{}' not found", args.name));

        // Suggest similar names
        let name_lower = args.name.to_lowercase();
        let suggestions: Vec<_> = modules
            .iter()
            .filter(|m| {
                m.name.to_lowercase().contains(&name_lower)
                    || name_lower.contains(&m.name.to_lowercase())
            })
            .take(5)
            .collect();

        if !suggestions.is_empty() {
            println!("\n{}", "Did you mean:".bright_yellow());
            for s in suggestions {
                println!("  - {} ({})", s.name.bright_white(), s.path.dimmed());
            }
        }

        return Ok(());
    };

    if matches!(args.format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(module)?);
        return Ok(());
    }

    // Human-readable output
    println!();
    println!(
        "{} {}",
        "Module:".bright_cyan().bold(),
        module.name.bright_white().bold()
    );
    println!("{}", "═".repeat(60).bright_cyan());

    println!("  {} {}", "Path:".bright_cyan(), module.path);
    println!("  {} {}", "Category:".bright_cyan(), module.category);
    println!(
        "  {} {}",
        "Source:".bright_cyan(),
        module.source_path.display()
    );

    let status_str = match module.status {
        ModuleStatus::Stable => "stable".bright_green(),
        ModuleStatus::Experimental => "experimental".bright_yellow(),
        ModuleStatus::Deprecated => "deprecated".bright_red(),
    };
    println!("  {} {}", "Status:".bright_cyan(), status_str);

    if !module.capability_tags.is_empty() {
        println!(
            "  {} {}",
            "Capabilities:".bright_cyan(),
            module.capability_tags.join(", ")
        );
    }

    if !module.description.is_empty() {
        println!();
        println!("  {}", "Description:".bright_cyan());
        println!("  {}", module.description);
    }

    if let Some(ref cli_cmd) = module.cli_command {
        println!();
        println!("  {}", "CLI Command:".bright_cyan());
        println!("    Command: {}", cli_cmd.bright_white());

        let cli_status = match module.cli_status {
            Some(CliStatus::Wired) => "wired (implementation connected)".bright_green(),
            Some(CliStatus::Stub) => "stub (TODO: wire to library)".bright_yellow(),
            _ => "none".dimmed(),
        };
        println!("    Status:  {}", cli_status);

        if module.cli_status == Some(CliStatus::Stub) {
            println!();
            println!(
                "  {} Wire CLI command to library implementation",
                "Action needed:".bright_yellow().bold()
            );
        }
    }

    if !module.children.is_empty() {
        println!();
        println!(
            "  {} ({})",
            "Children:".bright_cyan(),
            module.children.len()
        );
        for child in &module.children {
            println!("    - {}", child);
        }
    }

    Ok(())
}

async fn run_cli(args: CliArgs) -> Result<()> {
    // Use the static CLI_COMMANDS registry as the authoritative source of all CLI commands.
    // This shows all 29 commands from the Commands enum in main.rs, not just modules with @cli annotations.
    //
    // For JSON output, we provide a structured format compatible with tooling.
    // For --stubs-only filter, we check if there's a linked library module (via @cli annotation).

    // Also load module discovery to find linked library implementations
    let src_path = args.src_path;
    let modules = tokio::task::spawn_blocking(move || discover_all_modules_in_workspace(src_path))
        .await
        .context("discover_all_modules_in_workspace panicked")?;

    // Build a map from CLI command -> module info for commands that have @cli annotations
    let module_by_cli_cmd: std::collections::HashMap<_, _> = modules
        .into_iter()
        .filter_map(|m| {
            m.cli_command
                .as_ref()
                .map(|cmd| (cmd.clone(), m.clone()))
        })
        .collect();

    // Build the display list from the static registry
    let mut display_entries: Vec<_> = CLI_COMMANDS
        .iter()
        .map(|entry| {
            // Check if this CLI command has a linked library module
            let linked_module = module_by_cli_cmd.get(&format!("dashflow {}", entry.command));

            // A command is "wired" if it exists in the Commands enum (all do).
            // The @cli annotation indicates there's a separate library implementation.
            // Commands without @cli annotations are self-contained in dashflow-cli.
            let (status, module_name) = match linked_module {
                Some(m) => match m.cli_status {
                    Some(CliStatus::Stub) => ("stub", m.name.clone()),
                    _ => ("wired", m.name.clone()),
                },
                // Commands without @cli annotation are implemented directly in dashflow-cli
                None => ("wired", format!("dashflow-cli::{}", entry.command)),
            };

            (entry.command, entry.description, entry.category, status, module_name)
        })
        .collect();

    // Apply filters
    if args.stubs_only {
        display_entries.retain(|(_, _, _, status, _)| *status == "stub");
    } else if args.wired_only {
        display_entries.retain(|(_, _, _, status, _)| *status == "wired");
    }

    if matches!(args.format, OutputFormat::Json) {
        // JSON output: provide structured data for tooling
        let json_output: Vec<_> = display_entries
            .iter()
            .map(|(cmd, desc, category, status, module)| {
                serde_json::json!({
                    "command": format!("dashflow {}", cmd),
                    "description": desc,
                    "category": category,
                    "status": status,
                    "module": module,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_output)?);
        return Ok(());
    }

    // Human-readable output
    if display_entries.is_empty() {
        let msg = if args.stubs_only {
            "No stub CLI commands found (all commands are wired!)"
        } else if args.wired_only {
            "No wired CLI commands found"
        } else {
            "No CLI commands found"
        };
        print_info(msg);
        return Ok(());
    }

    // Count by status
    let wired_count = display_entries.iter().filter(|(_, _, _, s, _)| *s == "wired").count();
    let stub_count = display_entries.iter().filter(|(_, _, _, s, _)| *s == "stub").count();

    println!();
    println!("{}", "DashFlow CLI Commands".bright_cyan().bold());
    println!("{}", "═".repeat(80).bright_cyan());
    println!(
        "  {} commands total ({} wired, {} stub)",
        display_entries.len().to_string().bright_white(),
        wired_count.to_string().bright_green(),
        stub_count.to_string().bright_yellow()
    );
    println!();

    let mut table = create_table();
    table.set_header(vec!["Command", "Category", "Status", "Description"]);

    for (cmd, desc, category, status, _module) in &display_entries {
        let status_display = match *status {
            "wired" => "wired".bright_green().to_string(),
            "stub" => "STUB".bright_yellow().bold().to_string(),
            _ => "unknown".dimmed().to_string(),
        };
        let desc_truncated = truncate_str(desc, 40);

        table.add_row(vec![
            format!("dashflow {}", cmd),
            category.to_string(),
            status_display,
            desc_truncated,
        ]);
    }

    println!("{table}");

    if stub_count > 0 && !args.wired_only {
        println!();
        println!(
            "{} {} CLI commands have stub library implementations",
            "Note:".bright_yellow().bold(),
            stub_count
        );
        println!(
            "{}",
            "Use 'dashflow introspect search <command>' to find related modules.".dimmed()
        );
    }

    Ok(())
}

/// Load an execution trace from a file path
fn load_trace_from_path(path: &std::path::Path) -> Result<ExecutionTrace> {
    let contents = std::fs::read(path)
        .with_context(|| format!("Failed to read trace file: {}", path.display()))?;

    // Try JSON first (human readable, common format)
    if let Ok(trace) = serde_json::from_slice::<ExecutionTrace>(&contents) {
        return Ok(trace);
    }

    // Try bincode (binary format, faster)
    bincode::deserialize(&contents).with_context(|| {
        format!(
            "Failed to parse trace file as JSON or bincode: {}",
            path.display()
        )
    })
}

/// Ask a natural language question using the unified four-level introspection API
///
/// Routes questions automatically to the correct level:
/// - Platform: "Is distillation implemented?" - framework capabilities
/// - Application: "What graphs do I have?" - project configuration
/// - Runtime: "Why did search run 3 times?" - execution traces
/// - Network: "What RAG packages exist?" - ecosystem packages
async fn run_ask(args: AskArgs) -> Result<()> {
    let json_output = matches!(args.format, OutputFormat::Json);

    // Use the unified introspection API
    let introspection = DashFlowIntrospection::for_cwd();

    // M-606: Use explicit level if provided, otherwise auto-classify
    let level = args
        .level
        .unwrap_or_else(|| introspection.classify_question(&args.question));

    // For Runtime level questions, we may need trace context
    // For other levels, we can answer directly
    let response = if level == IntrospectionLevel::Runtime && args.trace.is_some() {
        // User provided a specific trace - use the old interface for detailed trace analysis
        let trace_path = args.trace.as_ref().expect("guarded by is_some() check").clone();
        let trace = tokio::task::spawn_blocking(move || load_trace_from_path(&trace_path))
            .await
            .context("load_trace_from_path panicked")??;
        let interface = if args.detailed {
            IntrospectionInterface::with_config(InterfaceConfig::detailed())
        } else {
            IntrospectionInterface::new()
        };
        let old_response = interface.ask(&trace, &args.question);

        // Convert old response format to unified format
        if json_output {
            println!("{}", serde_json::to_string_pretty(&old_response)?);
            return Ok(());
        } else {
            // Header with trace info
            println!();
            println!(
                "{} {}",
                "Introspection Query".bright_cyan().bold(),
                format!(
                    "(trace: {})",
                    trace.execution_id.as_deref().unwrap_or("unknown")
                )
                .dimmed()
            );
            println!("{}", "═".repeat(60).bright_cyan());
            println!();
            println!("{}", old_response.report());
            return Ok(());
        }
    } else {
        // M-606: Use ask_at_level to respect explicit level override
        introspection.ask_at_level(level, &args.question)
    };

    // M-284: For Platform level questions, also search the docs index (if available)
    let doc_results = if level == IntrospectionLevel::Platform {
        let workspace_root = tokio::task::spawn_blocking(get_workspace_root)
            .await
            .context("get_workspace_root panicked")?;
        let question = args.question.clone();
        tokio::task::spawn_blocking(move || {
            super::docs_index::search_index(&workspace_root, &question, 3).ok()
        })
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    // Output
    if json_output {
        // For JSON output, include doc results in the response
        let mut json_output = serde_json::to_value(&response)?;
        if let Some(ref docs) = doc_results {
            if !docs.is_empty() {
                let doc_entries: Vec<_> = docs
                    .iter()
                    .map(|d| {
                        serde_json::json!({
                            "name": d.name,
                            "crate": d.crate_name,
                            "type": d.item_type,
                            "summary": d.summary,
                        })
                    })
                    .collect();
                json_output["related_docs"] = serde_json::json!(doc_entries);
            }
        }
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else {
        // Header
        println!();
        println!(
            "{} (level: {})",
            "Introspection Query".bright_cyan().bold(),
            response.level.to_string().bright_yellow()
        );
        println!("{}", "═".repeat(60).bright_cyan());
        println!();

        // Answer
        println!("{}", response.answer);

        // Details
        if !response.details.is_empty() {
            println!();
            println!("{}", "Details:".bright_cyan());
            for detail in &response.details {
                println!("  - {}", detail);
            }
        }

        // M-284: Show related documentation if found
        if let Some(docs) = doc_results {
            if !docs.is_empty() {
                println!();
                println!("{}", "Related Documentation:".bright_cyan());
                for doc in docs.iter().take(3) {
                    println!(
                        "  {} {} ({})",
                        "•".bright_blue(),
                        doc.name.bright_green(),
                        doc.crate_name.bright_black()
                    );
                    if !doc.summary.is_empty() {
                        let summary = truncate_str(&doc.summary, 70);
                        println!("    {}", summary.dimmed());
                    }
                }
                println!(
                    "  {}",
                    "Use 'dashflow introspect docs show <name>' for full docs".bright_black()
                );
            }
        }

        // Follow-ups
        if !response.follow_ups.is_empty() {
            println!();
            println!("{}", "You might also ask:".dimmed());
            for q in &response.follow_ups {
                println!("  - {}", q);
            }
        }

        println!();
        println!(
            "{}",
            format!("Confidence: {:.0}%", response.confidence * 100.0).dimmed()
        );
    }

    Ok(())
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // M-497: Use char_indices to find safe UTF-8 boundary
        // Avoid panics from slicing in the middle of multi-byte characters
        let target_len = max_len.saturating_sub(3);
        let truncate_at = s
            .char_indices()
            .take_while(|(i, _)| *i < target_len)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        format!("{}...", &s[..truncate_at])
    }
}

// ============================================================================
// Documentation Search and View Commands
// ============================================================================

async fn run_docs(args: DocsArgs) -> Result<()> {
    match args.command {
        DocsCommand::Search(search_args) => run_docs_search(search_args).await,
        DocsCommand::Show(show_args) => run_docs_show(show_args).await,
        DocsCommand::Coverage(coverage_args) => run_docs_coverage(coverage_args).await,
        DocsCommand::Index(index_args) => run_docs_index(index_args).await,
    }
}

/// Handle documentation index commands
async fn run_docs_index(args: DocsIndexArgs) -> Result<()> {
    use super::docs_index;

    let workspace_root = tokio::task::spawn_blocking(get_workspace_root)
        .await
        .context("get_workspace_root panicked")?;

    match args.command {
        DocsIndexCommand::Build(build_args) => {
            let json_output = matches!(build_args.format, OutputFormat::Json);

            // Check if rebuild needed
            if !build_args.force {
                if let Ok(false) = docs_index::index_needs_rebuild(&workspace_root) {
                    if json_output {
                        println!(
                            r#"{{"status": "up_to_date", "message": "Index is already up to date"}}"#
                        );
                    } else {
                        println!("{}: Index is already up to date", "Info".bright_cyan());
                        println!("Use --force to rebuild anyway");
                    }
                    return Ok(());
                }
            }

            if !json_output {
                println!("{}: Building documentation index...", "Info".bright_cyan());
            }

            let start = std::time::Instant::now();
            let metadata = docs_index::build_index(&workspace_root)?;
            let duration = start.elapsed();

            if json_output {
                println!("{}", serde_json::to_string_pretty(&metadata)?);
            } else {
                println!("\n{}", "Index built successfully!".bright_green().bold());
                println!(
                    "  Items indexed: {}",
                    metadata.item_count.to_string().bright_cyan()
                );
                println!(
                    "  Crates: {}",
                    metadata.crates.len().to_string().bright_cyan()
                );
                println!(
                    "  Files scanned: {}",
                    metadata.file_count.to_string().bright_cyan()
                );
                println!(
                    "  Documentation: {} KB",
                    (metadata.doc_bytes / 1024).to_string().bright_cyan()
                );
                println!("  Build time: {:?}", duration);
                println!(
                    "\n{}: dashflow introspect docs search <query>",
                    "Usage".bright_yellow()
                );
            }
        }
        DocsIndexCommand::Status(status_args) => {
            let json_output = matches!(status_args.format, OutputFormat::Json);

            match docs_index::check_index_status(&workspace_root)? {
                Some(metadata) => {
                    let needs_rebuild =
                        docs_index::index_needs_rebuild(&workspace_root).unwrap_or(true);

                    if json_output {
                        let status = serde_json::json!({
                            "exists": true,
                            "needs_rebuild": needs_rebuild,
                            "built_at": metadata.built_at,
                            "item_count": metadata.item_count,
                            "crate_count": metadata.crates.len(),
                            "file_count": metadata.file_count,
                            "doc_bytes": metadata.doc_bytes,
                        });
                        println!("{}", serde_json::to_string_pretty(&status)?);
                    } else {
                        println!("\n{}", "Documentation Index Status".bright_cyan().bold());
                        println!("{}", "─".repeat(40));
                        println!("  Built at: {}", metadata.built_at);
                        println!("  Items: {}", metadata.item_count);
                        println!("  Crates: {}", metadata.crates.len());
                        println!("  Files: {}", metadata.file_count);
                        println!("  Size: {} KB documentation", metadata.doc_bytes / 1024);
                        println!();
                        if needs_rebuild {
                            println!("  Status: {} (files changed)", "STALE".bright_yellow());
                            println!("  Run: dashflow introspect docs index build");
                        } else {
                            println!("  Status: {}", "UP TO DATE".bright_green());
                        }
                    }

                    if status_args.check_stale && needs_rebuild {
                        std::process::exit(1);
                    }
                }
                None => {
                    if json_output {
                        println!(r#"{{"exists": false, "message": "No index found"}}"#);
                    } else {
                        println!(
                            "{}: No documentation index found",
                            "Warning".bright_yellow()
                        );
                        println!("Run: dashflow introspect docs index build");
                    }

                    if status_args.check_stale {
                        std::process::exit(1);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Search documentation for a keyword
///
/// Uses the Tantivy index for fast search if available, otherwise falls back to grep.
async fn run_docs_search(args: DocsSearchArgs) -> Result<()> {
    use super::docs_index;

    let json_output = matches!(args.format, OutputFormat::Json);

    let workspace_root = tokio::task::spawn_blocking(get_workspace_root)
        .await
        .context("get_workspace_root panicked")?;

    // Try to use the index first
    let index_path = workspace_root.join(".dashflow/docs_index/tantivy");
    let using_index = index_path.exists();

    let results: Vec<DocSearchResult> = if using_index {
        // Use fast indexed search (blocking - wrap in spawn_blocking)
        let start = std::time::Instant::now();
        let workspace_root_clone = workspace_root.clone();
        let query_clone = args.query.clone();
        let limit = args.limit;
        let indexed_results = tokio::task::spawn_blocking(move || {
            docs_index::search_index(&workspace_root_clone, &query_clone, limit)
        })
        .await
        .context("spawn_blocking panicked")??;
        let search_time = start.elapsed();

        if !json_output {
            print!(
                "{}",
                format!("(indexed, {:?}) ", search_time).bright_black()
            );
        }

        indexed_results
            .into_iter()
            .map(|r| DocSearchResult {
                name: r.name,
                crate_name: r.crate_name,
                summary: r.summary,
                file_path: r.file_path,
                score: (r.score * 100.0) as u32,
            })
            .collect()
    } else {
        // Fall back to grep-based search (blocking - wrap in spawn_blocking)
        if !json_output {
            print!("{}", "(scanning files...) ".bright_black());
        }
        let workspace_root_clone = workspace_root.clone();
        let query_clone = args.query.clone();
        let limit = args.limit;
        tokio::task::spawn_blocking(move || {
            grep_based_doc_search(&workspace_root_clone, &query_clone, limit)
        })
        .await
        .context("spawn_blocking panicked")??
    };

    if json_output {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        if results.is_empty() {
            println!("No results found for '{}'", args.query);
            if !using_index {
                println!(
                    "\n{}: Build the index for faster, more accurate search:",
                    "Tip".bright_yellow()
                );
                println!("  dashflow introspect docs index build");
            }
            return Ok(());
        }

        println!(
            "{} results for '{}':\n",
            results.len().to_string().bright_cyan(),
            args.query.bright_yellow()
        );

        for (i, result) in results.iter().enumerate() {
            println!(
                "{}. {} ({})",
                (i + 1).to_string().bright_white(),
                result.name.bright_green(),
                result.crate_name.bright_blue()
            );
            if !result.summary.is_empty() {
                println!("   {}", truncate_str(&result.summary, 70));
            }
        }

        println!(
            "\n{}: dashflow introspect docs show <name>",
            "Tip".bright_yellow()
        );

        if !using_index {
            println!(
                "{}: Build index for faster search: dashflow introspect docs index build",
                "Tip".bright_yellow()
            );
        }
    }

    Ok(())
}

/// Fallback grep-based documentation search (slower, no index required)
fn grep_based_doc_search(
    workspace_root: &std::path::Path,
    query: &str,
    limit: usize,
) -> Result<Vec<DocSearchResult>> {
    use std::fs;
    use walkdir::WalkDir;

    let crates_dir = workspace_root.join("crates");
    if !crates_dir.exists() {
        anyhow::bail!("crates/ directory not found. Run from workspace root.");
    }

    let query_lower = query.to_lowercase();
    let mut results: Vec<DocSearchResult> = Vec::new();

    let pub_pattern = regex::Regex::new(
        r"(?ms)((?:///.+\n)+)\s*pub(?:\([^)]*\))?\s+(?:async\s+)?(?:unsafe\s+)?(?:fn|struct|enum|trait|type|const|static)\s+(\w+)"
    ).expect("valid regex");

    for entry in WalkDir::new(&crates_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let path = entry.path();
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let crate_name = path
            .strip_prefix(&crates_dir)
            .ok()
            .and_then(|p| p.components().next())
            .and_then(|c| c.as_os_str().to_str())
            .unwrap_or("unknown");

        for cap in pub_pattern.captures_iter(&content) {
            let doc_comment = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let item_name = cap.get(2).map(|m| m.as_str()).unwrap_or("");

            let doc_lower = doc_comment.to_lowercase();
            let name_lower = item_name.to_lowercase();

            if name_lower.contains(&query_lower) || doc_lower.contains(&query_lower) {
                let summary = doc_comment
                    .lines()
                    .find(|l| l.trim().starts_with("///"))
                    .map(|l| l.trim().trim_start_matches("///").trim())
                    .unwrap_or("")
                    .to_string();

                let score = if name_lower == query_lower {
                    100
                } else if name_lower.starts_with(&query_lower) {
                    80
                } else if name_lower.contains(&query_lower) {
                    60
                } else {
                    30
                };

                results.push(DocSearchResult {
                    name: item_name.to_string(),
                    crate_name: crate_name.to_string(),
                    summary,
                    file_path: path.to_string_lossy().to_string(),
                    score,
                });
            }
        }
    }

    results.sort_by(|a, b| b.score.cmp(&a.score));
    results.truncate(limit);
    Ok(results)
}

#[derive(serde::Serialize)]
struct DocSearchResult {
    name: String,
    crate_name: String,
    summary: String,
    file_path: String,
    score: u32,
}

/// Synchronous helper for docs show - performs blocking file I/O
fn docs_show_sync(crates_dir: PathBuf, name: String) -> Result<Option<DocShowResult>> {
    use std::fs;
    use walkdir::WalkDir;

    // Regex to match documented pub items with full doc block
    let pub_pattern = regex::Regex::new(
        r"(?ms)((?:///.+\n)+)\s*pub(?:\([^)]*\))?\s+(?:async\s+)?(?:unsafe\s+)?(fn|struct|enum|trait|type|const|static)\s+(\w+)[^{;]*[{;]"
    ).expect("valid regex");

    // Check for crate-qualified search (e.g., "dashflow-openai::ChatOpenAI")
    let target_crate = if name.contains("::") {
        name.split("::").next()
    } else {
        None
    };
    let target_name = name.split("::").last().unwrap_or(&name);
    let target_lower = target_name.to_lowercase();

    for entry in WalkDir::new(&crates_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let path = entry.path();
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Extract crate name from path
        let crate_name = path
            .strip_prefix(&crates_dir)
            .ok()
            .and_then(|p| p.components().next())
            .and_then(|c| c.as_os_str().to_str())
            .unwrap_or("unknown");

        // Skip if searching for specific crate and this isn't it
        if let Some(tc) = target_crate {
            if crate_name != tc {
                continue;
            }
        }

        for cap in pub_pattern.captures_iter(&content) {
            let doc_comment = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let item_type = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let item_name = cap.get(3).map(|m| m.as_str()).unwrap_or("");

            if item_name.to_lowercase() == target_lower {
                // Clean up doc comment (remove /// prefix)
                let documentation = doc_comment
                    .lines()
                    .map(|l| l.trim().trim_start_matches("///").trim_start_matches("//!"))
                    .collect::<Vec<_>>()
                    .join("\n")
                    .trim()
                    .to_string();

                return Ok(Some(DocShowResult {
                    name: item_name.to_string(),
                    item_type: item_type.to_string(),
                    crate_name: crate_name.to_string(),
                    file_path: path.to_string_lossy().to_string(),
                    documentation,
                }));
            }
        }
    }

    Ok(None)
}

/// Show full documentation for a specific item
async fn run_docs_show(args: DocsShowArgs) -> Result<()> {
    let workspace_root = tokio::task::spawn_blocking(get_workspace_root)
        .await
        .context("get_workspace_root panicked")?;
    let crates_dir = workspace_root.join("crates");

    if !crates_dir.exists() {
        anyhow::bail!("crates/ directory not found. Run from workspace root.");
    }

    // Offload blocking file I/O to spawn_blocking
    let name = args.name.clone();
    let found = tokio::task::spawn_blocking(move || docs_show_sync(crates_dir, name))
        .await
        .context("spawn_blocking panicked")??;

    match found {
        Some(result) => {
            if matches!(args.format, OutputFormat::Json) {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!();
                println!(
                    "{} {} {}",
                    result.item_type.bright_magenta(),
                    result.name.bright_green().bold(),
                    format!("({})", result.crate_name).bright_blue()
                );
                println!("{}", "─".repeat(60).bright_cyan());
                println!();

                // Format documentation sections
                let doc = &result.documentation;
                for line in doc.lines() {
                    // Highlight section headers
                    if line.starts_with("# ") {
                        println!("\n{}", line.trim_start_matches("# ").bright_yellow().bold());
                    } else if line.starts_with("## ") {
                        println!("\n{}", line.trim_start_matches("## ").bright_yellow());
                    } else if line.starts_with("```") {
                        println!("{}", line.bright_black());
                    } else {
                        println!("{}", line);
                    }
                }

                println!();
                println!(
                    "{}: {}",
                    "Source".bright_cyan(),
                    result.file_path.bright_black()
                );
            }
        }
        None => {
            if matches!(args.format, OutputFormat::Json) {
                println!(r#"{{"error": "Item not found: {}"}}"#, args.name);
            } else {
                println!(
                    "{}: No documentation found for '{}'",
                    "Error".bright_red(),
                    args.name
                );
                println!(
                    "\n{}: Use 'dashflow introspect docs search {}' to find similar items",
                    "Tip".bright_yellow(),
                    args.name
                );
            }
        }
    }

    Ok(())
}

#[derive(serde::Serialize)]
struct DocShowResult {
    name: String,
    item_type: String,
    crate_name: String,
    file_path: String,
    documentation: String,
}

/// Synchronous helper for docs coverage - performs blocking file I/O
fn docs_coverage_sync(
    crates_dir: PathBuf,
    filter_crate: Option<String>,
    detailed: bool,
) -> Vec<CrateCoverage> {
    use std::fs;
    use walkdir::WalkDir;

    let pub_pattern = regex::Regex::new(
        r"(?m)^[ \t]*((?:///[^\n]*\n(?:[ \t]*///[^\n]*\n)*)?)?[ \t]*pub(?:\([^)]*\))?\s+(?:async\s+)?(?:unsafe\s+)?(?:fn|struct|enum|trait|type|const|static)\s+(\w+)"
    ).expect("valid regex");

    let mut crate_stats: std::collections::HashMap<String, CrateCoverage> =
        std::collections::HashMap::new();

    for entry in WalkDir::new(&crates_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let path = entry.path();

        // Extract crate name from path
        let crate_name = path
            .strip_prefix(&crates_dir)
            .ok()
            .and_then(|p| p.components().next())
            .and_then(|c| c.as_os_str().to_str())
            .unwrap_or("unknown")
            .to_string();

        // Filter by crate if specified
        if let Some(ref fc) = filter_crate {
            if &crate_name != fc {
                continue;
            }
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let stats = crate_stats.entry(crate_name).or_insert(CrateCoverage {
            name: String::new(),
            total: 0,
            documented: 0,
            undocumented_items: Vec::new(),
        });

        for cap in pub_pattern.captures_iter(&content) {
            let doc_comment = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let item_name = cap.get(2).map(|m| m.as_str()).unwrap_or("");

            stats.total += 1;

            // Check if there's actual doc content (not just whitespace)
            let has_doc = doc_comment.lines().any(|l| {
                let trimmed = l.trim();
                trimmed.starts_with("///") && trimmed.len() > 3
            });

            if has_doc {
                stats.documented += 1;
            } else if detailed {
                stats
                    .undocumented_items
                    .push(format!("{}:{}", path.display(), item_name));
            }
        }
    }

    // Convert to sorted vec
    let mut crates: Vec<CrateCoverage> = crate_stats
        .into_iter()
        .map(|(name, mut stats)| {
            stats.name = name;
            stats
        })
        .collect();
    crates.sort_by(|a, b| {
        let a_pct = if a.total > 0 {
            a.documented as f64 / a.total as f64
        } else {
            1.0
        };
        let b_pct = if b.total > 0 {
            b.documented as f64 / b.total as f64
        } else {
            1.0
        };
        b_pct.partial_cmp(&a_pct).unwrap()
    });

    crates
}

/// Show documentation coverage summary
async fn run_docs_coverage(args: DocsCoverageArgs) -> Result<()> {
    let workspace_root = tokio::task::spawn_blocking(get_workspace_root)
        .await
        .context("get_workspace_root panicked")?;
    let crates_dir = workspace_root.join("crates");

    if !crates_dir.exists() {
        anyhow::bail!("crates/ directory not found. Run from workspace root.");
    }

    // Offload blocking file I/O to spawn_blocking
    let filter_crate = args.crate_name.clone();
    let detailed = args.detailed;
    let crates = tokio::task::spawn_blocking(move || {
        docs_coverage_sync(crates_dir, filter_crate, detailed)
    })
    .await
    .context("spawn_blocking panicked")?;

    // Calculate totals (crates already sorted by sync helper)
    let total_items: usize = crates.iter().map(|c| c.total).sum();
    let total_documented: usize = crates.iter().map(|c| c.documented).sum();
    let overall_pct = if total_items > 0 {
        (total_documented as f64 / total_items as f64) * 100.0
    } else {
        100.0
    };

    if matches!(args.format, OutputFormat::Json) {
        #[derive(serde::Serialize)]
        struct CoverageOutput {
            overall_percentage: f64,
            total_items: usize,
            documented_items: usize,
            undocumented_items: usize,
            crates: Vec<CrateCoverage>,
        }
        let output = CoverageOutput {
            overall_percentage: overall_pct,
            total_items,
            documented_items: total_documented,
            undocumented_items: total_items - total_documented,
            crates,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!();
        println!("{}", "Documentation Coverage Summary".bright_cyan().bold());
        println!("{}", "─".repeat(60).bright_cyan());
        println!();

        // Overall stats
        let status_color = if overall_pct >= 90.0 {
            "green"
        } else if overall_pct >= 70.0 {
            "yellow"
        } else {
            "red"
        };
        let status_str = format!("{:.1}%", overall_pct);
        let colored_status = match status_color {
            "green" => status_str.bright_green(),
            "yellow" => status_str.bright_yellow(),
            _ => status_str.bright_red(),
        };

        println!(
            "  Overall: {} ({}/{} items)",
            colored_status, total_documented, total_items
        );
        println!("  Crates analyzed: {}", crates.len());
        println!();

        // Top/bottom crates
        let show_count = if args.detailed { crates.len() } else { 10 };

        if !args.detailed && crates.len() > 10 {
            println!("{}", "Top 10 by coverage:".bright_green());
            for crate_info in crates.iter().take(5) {
                let pct = if crate_info.total > 0 {
                    (crate_info.documented as f64 / crate_info.total as f64) * 100.0
                } else {
                    100.0
                };
                println!(
                    "  {:>5.1}% {} ({}/{})",
                    pct, crate_info.name, crate_info.documented, crate_info.total
                );
            }

            println!();
            println!("{}", "Bottom 5 by coverage:".bright_red());
            let bottom_start = crates.len().saturating_sub(5);
            for crate_info in &crates[bottom_start..] {
                let pct = if crate_info.total > 0 {
                    (crate_info.documented as f64 / crate_info.total as f64) * 100.0
                } else {
                    100.0
                };
                println!(
                    "  {:>5.1}% {} ({}/{})",
                    pct, crate_info.name, crate_info.documented, crate_info.total
                );
            }
        } else {
            println!("{}", "All crates by coverage:".bright_cyan());
            for crate_info in crates.iter().take(show_count) {
                let pct = if crate_info.total > 0 {
                    (crate_info.documented as f64 / crate_info.total as f64) * 100.0
                } else {
                    100.0
                };
                let pct_str = format!("{:>5.1}%", pct);
                let colored = if pct >= 90.0 {
                    pct_str.bright_green()
                } else if pct >= 70.0 {
                    pct_str.bright_yellow()
                } else {
                    pct_str.bright_red()
                };
                println!(
                    "  {} {} ({}/{})",
                    colored, crate_info.name, crate_info.documented, crate_info.total
                );

                if args.detailed && !crate_info.undocumented_items.is_empty() {
                    for item in crate_info.undocumented_items.iter().take(5) {
                        println!("      - {}", item.bright_black());
                    }
                    if crate_info.undocumented_items.len() > 5 {
                        println!(
                            "      ... and {} more",
                            crate_info.undocumented_items.len() - 5
                        );
                    }
                }
            }
        }

        println!();
        println!(
            "{}: Run with --detailed for full breakdown",
            "Tip".bright_yellow()
        );
    }

    Ok(())
}

/// Run the optimizers command
async fn run_optimizers(args: OptimizersArgs) -> Result<()> {
    use dashflow::optimize::optimizers::registry::{
        all_optimizers, get_by_tier, get_optimizer, recommend_optimizer, OptimizerTier,
    };

    let json_output = matches!(args.format, OutputFormat::Json);

    // Handle specific optimizer lookup
    if let Some(name) = &args.name {
        if let Some(opt) = get_optimizer(name) {
            if json_output {
                println!("{}", serde_json::to_string_pretty(&opt)?);
            } else {
                println!("{}", format!("# {}", opt.name).bright_cyan().bold());
                println!("{}: {}", "Tier".bright_yellow(), opt.tier);
                println!("{}", opt.description);
                println!();
                println!("{}: {}", "Use when".bright_green(), opt.use_when);
                println!(
                    "{}: {}",
                    "Cannot use when".bright_red(),
                    opt.cannot_use_when
                );
                println!("{}: {}", "Min examples".bright_yellow(), opt.min_examples);
                println!("{}: {}", "Citation".bright_blue(), opt.citation);
                if let Some(bench) = opt.benchmark {
                    println!("{}: {}", "Benchmark".bright_magenta(), bench);
                }
                if !opt.requirements.is_empty() {
                    println!(
                        "{}: {}",
                        "Requirements".bright_yellow(),
                        opt.requirements.join(", ")
                    );
                }
            }
        } else {
            print_error(&format!("Optimizer '{}' not found", name));
            println!();
            println!("Available optimizers:");
            for opt in all_optimizers() {
                println!("  - {}", opt.name);
            }
        }
        return Ok(());
    }

    // Handle recommendation request
    if let Some(num_examples) = args.examples {
        let rec = recommend_optimizer(num_examples, args.can_finetune);
        if json_output {
            println!(
                "{}",
                serde_json::json!({
                    "examples": num_examples,
                    "can_finetune": args.can_finetune,
                    "recommended": rec
                })
            );
        } else {
            println!(
                "{}: {} examples{}",
                "Query".bright_cyan(),
                num_examples,
                if args.can_finetune {
                    " (can finetune)"
                } else {
                    ""
                }
            );
            println!("{}: {}", "Recommended optimizer".bright_green().bold(), rec);

            // Show the optimizer details if it's a real recommendation
            if let Some(opt) = get_optimizer(rec) {
                println!();
                println!("  {} - {}", opt.name.bright_yellow(), opt.description);
                if let Some(bench) = opt.benchmark {
                    println!("  {}: {}", "Benchmark".bright_blue(), bench);
                }
            }
        }
        return Ok(());
    }

    // Handle tier filter
    let optimizers = if let Some(tier_num) = args.tier {
        let tier = match tier_num {
            1 => OptimizerTier::Tier1Recommended,
            2 => OptimizerTier::Tier2Specialized,
            3 => OptimizerTier::Tier3Niche,
            _ => {
                print_error("Tier must be 1, 2, or 3");
                return Ok(());
            }
        };
        get_by_tier(tier)
    } else {
        all_optimizers()
    };

    // Output all/filtered optimizers
    if json_output {
        println!("{}", serde_json::to_string_pretty(&optimizers)?);
    } else {
        println!(
            "{}",
            "DashOptimize: Optimizer Selection Guide"
                .bright_cyan()
                .bold()
        );
        println!();

        // Group by tier
        let tier1: Vec<_> = optimizers
            .iter()
            .filter(|o| o.tier == OptimizerTier::Tier1Recommended)
            .collect();
        let tier2: Vec<_> = optimizers
            .iter()
            .filter(|o| o.tier == OptimizerTier::Tier2Specialized)
            .collect();
        let tier3: Vec<_> = optimizers
            .iter()
            .filter(|o| o.tier == OptimizerTier::Tier3Niche)
            .collect();

        if !tier1.is_empty() {
            println!(
                "{}",
                "Tier 1: Recommended (best defaults)".bright_green().bold()
            );
            for opt in tier1 {
                println!("  {} - {}", opt.name.bright_yellow(), opt.description);
                println!("    Use when: {}", opt.use_when);
                println!("    Citation: {}", opt.citation.bright_blue());
            }
            println!();
        }

        if !tier2.is_empty() {
            println!("{}", "Tier 2: Specialized".bright_yellow().bold());
            for opt in tier2 {
                println!("  {} - {}", opt.name.bright_yellow(), opt.description);
                println!("    Use when: {}", opt.use_when);
            }
            println!();
        }

        if !tier3.is_empty() {
            println!("{}", "Tier 3: Niche".bright_magenta().bold());
            for opt in tier3 {
                println!("  {} - {}", opt.name, opt.description);
            }
            println!();
        }

        println!("{}", "Usage examples:".bright_cyan());
        println!("  dashflow introspect optimizers --name MIPROv2     # Show MIPROv2 details");
        println!("  dashflow introspect optimizers --examples 50      # Get recommendation for 50 examples");
        println!(
            "  dashflow introspect optimizers --tier 1           # Show only Tier 1 optimizers"
        );
        println!(
            "  dashflow introspect optimizers --format json      # JSON output for automation"
        );
    }

    Ok(())
}

/// Run the optimize command - select an optimizer based on context
async fn run_optimize(args: OptimizeArgs) -> Result<()> {
    use dashflow::optimize::auto_optimizer::{
        AutoOptimizer, ComputeBudget, OptimizationContext, TaskType,
    };

    // Parse task type
    let task_type = match args.task.as_deref() {
        Some("qa") | Some("question_answering") => TaskType::QuestionAnswering,
        Some("classification") => TaskType::Classification,
        Some("code") | Some("code_generation") => TaskType::CodeGeneration,
        Some("math") | Some("math_reasoning") => TaskType::MathReasoning,
        Some("agent") => TaskType::Agent,
        Some("reasoning") => TaskType::Reasoning,
        Some("summarization") => TaskType::Summarization,
        Some("generic") | None => TaskType::Generic,
        Some(other) => {
            print_error(&format!(
                "Unknown task type: '{}'. Use one of: qa, classification, code, math, agent, reasoning, summarization, generic",
                other
            ));
            return Ok(());
        }
    };

    // Parse compute budget
    let budget = match args.budget.to_lowercase().as_str() {
        "minimal" => ComputeBudget::Minimal,
        "low" => ComputeBudget::Low,
        "medium" => ComputeBudget::Medium,
        "high" => ComputeBudget::High,
        "unlimited" => ComputeBudget::Unlimited,
        other => {
            print_error(&format!(
                "Unknown budget: '{}'. Use one of: minimal, low, medium, high, unlimited",
                other
            ));
            return Ok(());
        }
    };

    // Build context
    let mut builder = OptimizationContext::builder()
        .num_examples(args.examples)
        .can_finetune(args.can_finetune)
        .task_type(task_type)
        .compute_budget(budget);

    for excluded in &args.exclude {
        builder = builder.exclude_optimizer(excluded);
    }

    let context = builder.build();
    let selection = AutoOptimizer::select(&context);

    if matches!(args.format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(&selection)?);
        return Ok(());
    }

    // Human-readable output
    println!("{}", "Optimizer Selection".bright_cyan().bold());
    println!();
    println!(
        "Context: {} examples, task={}, budget={}, finetune={}",
        args.examples, task_type, args.budget, args.can_finetune
    );
    println!();

    println!(
        "{}: {} (confidence: {:.0}%)",
        "Selected".bright_green().bold(),
        selection.optimizer_name.bright_cyan().bold(),
        selection.confidence * 100.0
    );
    println!();
    println!("{}: {}", "Reason".bright_white(), selection.reason);

    if let Some(tier) = selection.tier {
        println!(
            "{}: {} (1=recommended, 2=specialized, 3=niche)",
            "Tier".bright_white(),
            tier
        );
    }

    if let Some(citation) = &selection.citation {
        println!("{}: {}", "Citation".bright_white(), citation);
    }

    if !selection.alternatives.is_empty() {
        println!();
        println!("{}", "Alternatives:".bright_yellow());
        for alt in &selection.alternatives {
            println!(
                "  {} - {} ({:.0}%)",
                alt.name.bright_yellow(),
                alt.reason,
                alt.confidence * 100.0
            );
        }
    }

    Ok(())
}

/// Run the optimize-history command - view past optimization outcomes
async fn run_optimize_history(args: OptimizeHistoryArgs) -> Result<()> {
    use dashflow::optimize::auto_optimizer::AutoOptimizer;

    let optimizer = AutoOptimizer::new();
    let mut outcomes = optimizer.load_outcomes().await?;

    // Filter by optimizer name if specified
    if let Some(ref name) = args.optimizer {
        outcomes.retain(|o| o.optimizer_name.eq_ignore_ascii_case(name));
    }

    // Filter by success if requested
    if args.successful {
        outcomes.retain(|o| o.success);
    }

    // Limit results
    outcomes.reverse(); // Most recent first
    outcomes.truncate(args.limit);

    if outcomes.is_empty() {
        print_info("No optimization history found. Run optimizations to build history.");
        return Ok(());
    }

    if matches!(args.format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(&outcomes)?);
        return Ok(());
    }

    // Human-readable output
    println!("{}", "Optimization History".bright_cyan().bold());
    println!("Showing {} most recent outcomes", outcomes.len());
    println!();

    for outcome in &outcomes {
        let status = if outcome.success {
            "✓".bright_green()
        } else {
            "✗".bright_red()
        };

        println!(
            "{} {} - {} ({:.1}% improvement, {:.1}s)",
            status,
            outcome.timestamp.format("%Y-%m-%d %H:%M"),
            outcome.optimizer_name.bright_cyan(),
            outcome.improvement_percent(),
            outcome.duration_secs
        );
        println!(
            "   Task: {}, Examples: {}, Score: {:.2} → {:.2}",
            outcome.context.task_type,
            outcome.context.num_examples,
            outcome.initial_score,
            outcome.final_score
        );
        if let Some(ref notes) = outcome.notes {
            println!("   Notes: {}", notes);
        }
        println!();
    }

    Ok(())
}

/// Run the optimize-insights command - get learned patterns from history
async fn run_optimize_insights(args: OptimizeInsightsArgs) -> Result<()> {
    use dashflow::optimize::auto_optimizer::{AutoOptimizer, TaskType};

    let optimizer = AutoOptimizer::new();
    let stats = optimizer.historical_stats().await?;

    if stats.is_empty() {
        print_info("No optimization history found. Run optimizations to build insights.");
        return Ok(());
    }

    // Filter by task type if specified
    let target_task = args.task.as_ref().map(|t| match t.to_lowercase().as_str() {
        "qa" | "question_answering" => TaskType::QuestionAnswering,
        "classification" => TaskType::Classification,
        "code" | "code_generation" => TaskType::CodeGeneration,
        "math" | "math_reasoning" => TaskType::MathReasoning,
        "agent" => TaskType::Agent,
        "reasoning" => TaskType::Reasoning,
        "summarization" => TaskType::Summarization,
        _ => TaskType::Generic,
    });

    let filtered_stats: Vec<_> = if let Some(task) = target_task {
        stats
            .into_iter()
            .filter(|s| s.best_task_types.contains(&task))
            .collect()
    } else {
        stats
    };

    if matches!(args.format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(&filtered_stats)?);
        return Ok(());
    }

    // Human-readable output
    println!("{}", "Optimization Insights".bright_cyan().bold());
    println!();

    if filtered_stats.is_empty() {
        print_info("No insights available for the specified task type.");
        return Ok(());
    }

    // Summary statistics
    let total_runs: usize = filtered_stats.iter().map(|s| s.usage_count).sum();
    let avg_improvement: f64 = if total_runs > 0 {
        filtered_stats
            .iter()
            .map(|s| s.avg_improvement * s.usage_count as f64)
            .sum::<f64>()
            / total_runs as f64
    } else {
        0.0
    };

    println!("{}", "Summary:".bright_white().bold());
    println!(
        "  Total optimization runs: {}",
        total_runs.to_string().bright_cyan()
    );
    println!("  Average improvement: {:.1}%", avg_improvement * 100.0);
    println!();

    println!("{}", "Optimizer Performance:".bright_white().bold());
    for stat in &filtered_stats {
        println!(
            "  {} - {} uses, {:.1}% avg improvement, {:.0}% success rate",
            stat.optimizer_name.bright_cyan(),
            stat.usage_count,
            stat.avg_improvement * 100.0,
            stat.success_rate * 100.0
        );
        if !stat.best_task_types.is_empty() {
            let tasks: Vec<_> = stat.best_task_types.iter().map(|t| t.to_string()).collect();
            println!("    Best for: {}", tasks.join(", "));
        }
    }

    println!();
    println!(
        "{}",
        "Recommendations based on history:".bright_white().bold()
    );

    // Find best performers
    let best_by_improvement = filtered_stats.iter().max_by(|a, b| {
        a.avg_improvement
            .partial_cmp(&b.avg_improvement)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let best_by_success = filtered_stats.iter().max_by(|a, b| {
        a.success_rate
            .partial_cmp(&b.success_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let most_used = filtered_stats.iter().max_by_key(|s| s.usage_count);

    if let Some(best) = best_by_improvement {
        println!(
            "  Highest improvement: {} ({:.1}% avg)",
            best.optimizer_name.bright_green(),
            best.avg_improvement * 100.0
        );
    }
    if let Some(best) = best_by_success {
        println!(
            "  Most reliable: {} ({:.0}% success rate)",
            best.optimizer_name.bright_green(),
            best.success_rate * 100.0
        );
    }
    if let Some(most) = most_used {
        println!(
            "  Most used: {} ({} runs)",
            most.optimizer_name.bright_yellow(),
            most.usage_count
        );
    }

    Ok(())
}

// ============================================================================
// Automatic Behaviors Command
// ============================================================================

/// Automatic behavior entry for the report
#[derive(serde::Serialize)]
struct AutomaticBehavior {
    name: String,
    description: String,
    file_location: String,
    line_number: Option<usize>,
    env_var_opt_out: Option<String>,
    default_value: String,
}

/// Run the automatic behaviors command - generates markdown report to stdout
async fn run_automatic(args: AutomaticArgs) -> Result<()> {
    // Get the dashflow crate source path
    let src_path = args.src_path.unwrap_or_else(|| {
        let workspace_root = get_workspace_root();
        workspace_root.join("crates/dashflow/src")
    });

    // Scan for automatic behaviors by looking for known patterns
    let behaviors = scan_automatic_behaviors(&src_path).await?;

    if matches!(args.format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(&behaviors)?);
    } else {
        // Generate markdown report to stdout
        print_automatic_markdown(&behaviors);
    }

    Ok(())
}

/// Scan the codebase for automatic behaviors
async fn scan_automatic_behaviors(src_path: &Path) -> Result<Vec<AutomaticBehavior>> {
    let mut behaviors = Vec::new();

    // Known automatic behaviors from DESIGN_INVARIANTS.md Invariant 6
    // These are discovered by scanning specific files for env var patterns

    // 1. Trace Persistence
    let trace_rs = src_path.join("executor/trace.rs");
    if trace_rs.exists() {
        if let Ok(content) = std::fs::read_to_string(&trace_rs) {
            // Check for DASHFLOW_TRACE
            if content.contains("DASHFLOW_TRACE") {
                let line_num = content.lines()
                    .enumerate()
                    .find(|(_, line)| line.contains("fn is_trace_persistence_enabled"))
                    .map(|(i, _)| i + 1);

                behaviors.push(AutomaticBehavior {
                    name: "Trace Persistence".to_string(),
                    description: "Automatically saves ExecutionTrace to .dashflow/traces/ after every graph execution".to_string(),
                    file_location: "crates/dashflow/src/executor/trace.rs".to_string(),
                    line_number: line_num,
                    env_var_opt_out: Some("DASHFLOW_TRACE=false".to_string()),
                    default_value: "ON (enabled)".to_string(),
                });
            }

            // Check for DASHFLOW_TRACE_REDACT
            if content.contains("DASHFLOW_TRACE_REDACT") {
                let line_num = content.lines()
                    .enumerate()
                    .find(|(_, line)| line.contains("fn is_trace_redaction_enabled"))
                    .map(|(i, _)| i + 1);

                behaviors.push(AutomaticBehavior {
                    name: "PII Redaction".to_string(),
                    description: "Automatically redacts sensitive data (API keys, emails, SSNs, credit cards, JWTs) from traces before persistence".to_string(),
                    file_location: "crates/dashflow/src/executor/trace.rs".to_string(),
                    line_number: line_num,
                    env_var_opt_out: Some("DASHFLOW_TRACE_REDACT=false".to_string()),
                    default_value: "ON (enabled)".to_string(),
                });
            }

            // Check for DASHFLOW_LIVE_INTROSPECTION
            if content.contains("DASHFLOW_LIVE_INTROSPECTION") {
                let line_num = content.lines()
                    .enumerate()
                    .find(|(_, line)| line.contains("fn is_live_introspection_enabled"))
                    .map(|(i, _)| i + 1);

                behaviors.push(AutomaticBehavior {
                    name: "Live Introspection".to_string(),
                    description: "Automatically tracks live execution state for real-time debugging and monitoring".to_string(),
                    file_location: "crates/dashflow/src/executor/trace.rs".to_string(),
                    line_number: line_num,
                    env_var_opt_out: Some("DASHFLOW_LIVE_INTROSPECTION=false".to_string()),
                    default_value: "ON (enabled)".to_string(),
                });
            }
        }
    }

    // 2. Automatic Event Emission
    let execution_rs = src_path.join("executor/execution.rs");
    if execution_rs.exists() {
        if let Ok(content) = std::fs::read_to_string(&execution_rs) {
            if content.contains("emit_event") {
                let line_num = content.lines()
                    .enumerate()
                    .find(|(_, line)| line.contains("GraphEvent::GraphStart"))
                    .map(|(i, _)| i + 1);

                behaviors.push(AutomaticBehavior {
                    name: "Automatic Event Emission".to_string(),
                    description: "Executor automatically emits GraphEvent on every node start/end/error, edge traversal, and parallel execution".to_string(),
                    file_location: "crates/dashflow/src/executor/execution.rs".to_string(),
                    line_number: line_num,
                    env_var_opt_out: None, // Always on, no opt-out
                    default_value: "ALWAYS ON (no opt-out)".to_string(),
                });
            }
        }
    }

    // 3. State Diffs (in dashflow-streaming)
    let streaming_diff = src_path.parent()
        .map(|p| p.join("dashflow-streaming/src/diff.rs"));
    if let Some(diff_path) = streaming_diff {
        if diff_path.exists() {
            behaviors.push(AutomaticBehavior {
                name: "State Diffs (RFC 6902 JSON Patch)".to_string(),
                description: "DashStream provides diff_states() and apply_patch() for efficient state synchronization".to_string(),
                file_location: "crates/dashflow-streaming/src/diff.rs".to_string(),
                line_number: None,
                env_var_opt_out: None, // Library feature, always available
                default_value: "Available (use DashStreamCallback)".to_string(),
            });
        }
    }

    // 4. DashStreamCallback for Kafka
    let dashstream_callback = src_path.join("dashstream_callback/mod.rs");
    if dashstream_callback.exists() {
        behaviors.push(AutomaticBehavior {
            name: "Kafka Streaming (DashStreamCallback)".to_string(),
            description: "Ready-to-use EventCallback for streaming events to Kafka with Prometheus metrics, backpressure handling, and PII redaction".to_string(),
            file_location: "crates/dashflow/src/dashstream_callback/mod.rs".to_string(),
            line_number: None,
            env_var_opt_out: None, // Opt-in by adding callback
            default_value: "Available (add via .with_callback())".to_string(),
        });
    }

    Ok(behaviors)
}

/// Print the automatic behaviors as markdown to stdout
fn print_automatic_markdown(behaviors: &[AutomaticBehavior]) {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");

    println!("# DashFlow Current Capabilities (Auto-Generated)");
    println!();
    println!("**Generated:** {}", now);
    println!("**Source:** `dashflow introspect automatic`");
    println!();
    println!("Per DESIGN_INVARIANTS.md Invariant 6: \"All features are ON by default. Users disable what they don't want.\"");
    println!();
    println!("---");
    println!();
    println!("## Automatic Behaviors (ON by Default)");
    println!();
    println!("| Feature | Default | Opt-Out | Location |");
    println!("|---------|---------|---------|----------|");

    for b in behaviors {
        let opt_out = b.env_var_opt_out.as_deref().unwrap_or("N/A");
        let location = if let Some(line) = b.line_number {
            format!("{}:{}", b.file_location, line)
        } else {
            b.file_location.clone()
        };
        println!("| {} | {} | `{}` | `{}` |", b.name, b.default_value, opt_out, location);
    }

    println!();
    println!("## Details");
    println!();

    for (i, b) in behaviors.iter().enumerate() {
        println!("### {}. {}", i + 1, b.name);
        println!();
        println!("{}", b.description);
        println!();
        if let Some(line) = b.line_number {
            println!("- **Location:** `{}:{}`", b.file_location, line);
        } else {
            println!("- **Location:** `{}`", b.file_location);
        }
        println!("- **Default:** {}", b.default_value);
        if let Some(ref opt_out) = b.env_var_opt_out {
            println!("- **Opt-out:** `{}`", opt_out);
        }
        println!();
    }

    println!("---");
    println!();
    println!("## Environment Variables Summary");
    println!();
    println!("```bash");
    println!("# Opt-out of automatic behaviors:");
    for b in behaviors {
        if let Some(ref opt_out) = b.env_var_opt_out {
            println!("export {}  # Disable {}", opt_out, b.name);
        }
    }
    println!("```");
    println!();
    println!("---");
    println!();
    println!("*This report was auto-generated by `dashflow introspect automatic`.*");
    println!("*Run this command before claiming any feature is missing.*");
}

/// Phase 938: Run the `dashflow introspect modules` command
async fn run_modules(args: ModulesArgs) -> Result<()> {
    let registry = ModulePatternRegistry::with_defaults();

    // Filter by capability if provided
    let entries: Vec<_> = if let Some(ref cap) = args.capability {
        registry.find_by_capability(cap)
    } else {
        registry.entries().collect()
    };

    if matches!(args.format, OutputFormat::Json) {
        // JSON output
        let output: Vec<_> = entries
            .iter()
            .map(|e| {
                let mut map = serde_json::Map::new();
                map.insert(
                    "module_path".to_string(),
                    serde_json::Value::String(e.module_path.clone()),
                );
                map.insert(
                    "capability_tags".to_string(),
                    serde_json::json!(e.capability_tags),
                );
                if args.with_patterns && !e.replaces_patterns.is_empty() {
                    let patterns: Vec<_> = e
                        .replaces_patterns
                        .iter()
                        .map(|p| {
                            serde_json::json!({
                                "triggers": p.triggers,
                                "severity": p.severity.to_string(),
                                "message": p.message
                            })
                        })
                        .collect();
                    map.insert("patterns".to_string(), serde_json::json!(patterns));
                }
                if !e.example_usage.is_empty() {
                    map.insert(
                        "example_usage".to_string(),
                        serde_json::Value::String(e.example_usage.clone()),
                    );
                }
                if let Some(ref url) = e.docs_url {
                    map.insert(
                        "docs_url".to_string(),
                        serde_json::Value::String(url.clone()),
                    );
                }
                serde_json::Value::Object(map)
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        // Human-readable output
        if entries.is_empty() {
            if let Some(ref cap) = args.capability {
                println!("No modules found with capability: {}", cap);
            } else {
                println!("No modules registered in the pattern registry.");
            }
            return Ok(());
        }

        println!(
            "{}",
            format!("Module Capabilities ({} modules)", entries.len())
                .bold()
                .cyan()
        );
        println!();

        for entry in entries {
            println!("{}", entry.module_path.bold().green());
            if !entry.capability_tags.is_empty() {
                println!(
                    "  {} {}",
                    "Tags:".dimmed(),
                    entry.capability_tags.join(", ")
                );
            }
            if !entry.example_usage.is_empty() {
                println!("  {} {}", "Example:".dimmed(), entry.example_usage);
            }
            if let Some(ref url) = entry.docs_url {
                println!("  {} {}", "Docs:".dimmed(), url);
            }

            if args.with_patterns && !entry.replaces_patterns.is_empty() {
                println!("  {}:", "Lint Patterns".yellow());
                for pattern in &entry.replaces_patterns {
                    println!(
                        "    {} [{}] {}",
                        "→".dimmed(),
                        pattern.severity,
                        pattern.message
                    );
                    for trigger in &pattern.triggers {
                        println!("      {} {}", "regex:".dimmed(), trigger.cyan());
                    }
                }
            }
            println!();
        }
    }

    Ok(())
}

#[derive(serde::Serialize)]
struct CrateCoverage {
    name: String,
    total: usize,
    documented: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    undocumented_items: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        command: TestCommands,
    }

    #[derive(Subcommand)]
    enum TestCommands {
        Introspect(IntrospectArgs),
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 8), "hello...");
        assert_eq!(truncate_str("", 10), "");
    }

    #[test]
    fn test_get_src_path_default() {
        // When no path is provided, it should return a PathBuf
        let path = get_src_path(None);
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_get_src_path_provided() {
        let custom = PathBuf::from("/custom/path");
        let path = get_src_path(Some(custom.clone()));
        assert_eq!(path, custom);
    }

    #[test]
    fn test_ask_args_default_format_table() {
        let cli = Cli::parse_from(["test", "introspect", "ask", "why did this happen?"]);
        match cli.command {
            TestCommands::Introspect(IntrospectArgs {
                command: IntrospectCommand::Ask(args),
            }) => {
                assert_eq!(args.question, "why did this happen?");
                assert!(matches!(args.format, OutputFormat::Table));
            }
            _ => panic!("Expected introspect ask command"),
        }
    }

    #[test]
    fn test_ask_args_format_json() {
        let cli = Cli::parse_from([
            "test",
            "introspect",
            "ask",
            "why did this happen?",
            "--format",
            "json",
        ]);
        match cli.command {
            TestCommands::Introspect(IntrospectArgs {
                command: IntrospectCommand::Ask(args),
            }) => {
                assert!(matches!(args.format, OutputFormat::Json));
            }
            _ => panic!("Expected introspect ask command"),
        }
    }

    #[test]
    fn test_ask_args_rejects_legacy_json_flag() {
        let result = Cli::try_parse_from(["test", "introspect", "ask", "q", "--json"]);
        assert!(result.is_err());
    }
}
