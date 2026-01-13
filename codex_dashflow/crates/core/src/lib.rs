//! Codex DashFlow Core
//!
//! Core agent logic using DashFlow StateGraph for workflow orchestration.
//! This crate provides the agent loop, state management, and tool execution.
//!
//! ## Features
//!
//! - **StateGraph**: Agent workflow orchestrated via DashFlow's StateGraph
//! - **Streaming**: Real-time visibility into agent actions via streaming callbacks
//! - **Checkpointing**: Session persistence via DashFlow checkpointers
//! - **Tool Execution**: Shell and file operations via DashFlow tools
//! - **OAuth Auth**: ChatGPT account authentication via OAuth flow

pub mod approval_presets;
pub mod auth;
pub mod bash;
pub mod codex;
pub mod codex_delegate;
pub mod compact;
pub mod config;
pub mod config_override;
pub mod config_summary;
pub mod context;
pub mod custom_prompts;
pub mod elapsed;
pub mod error;
pub mod exec;
pub mod exec_env;
pub mod execpolicy;
pub mod exit_status;
pub mod features;
pub mod flags;
pub mod format_env;
pub mod fuzzy_match;
pub mod ghost_commit;
pub mod git_info;
pub mod graph;
pub mod list_dir;
pub mod llm;
pub mod message_history;
pub mod model_family;
pub mod model_provider_info;
pub mod openai_model_info;
pub mod optimize;
pub mod parse_command;
pub mod powershell;
pub mod project_doc;
pub mod review;
pub mod runner;
pub mod safe_commands;
pub mod safety;
pub mod sandbox_summary;
pub mod shell;
pub mod skills;
pub mod spawn;
pub mod state;
pub mod streaming;
pub mod terminal;
pub mod text_encoding;
pub mod turn_diff_tracker;
pub mod user_instructions;
pub mod user_notification;
pub mod user_shell_command;
pub mod util;
pub mod windows_dangerous_commands;
pub mod windows_safe_commands;
pub mod wsl_paths;

pub mod nodes {
    pub mod reasoning;
    pub mod result_analysis;
    pub mod tool_execution;
    pub mod tool_selection;
    pub mod user_input;
}

pub mod tools;

// Re-exports
pub use auth::{
    parse_id_token, AuthCredentialsStoreMode, AuthDotJson, AuthError, AuthManager, AuthResult,
    AuthStatus, IdTokenError, IdTokenInfo, TokenData,
};
pub use codex::{
    AbortReason, ApprovalDecision, ApprovalPolicy, Codex, CodexSpawnOk, CommandAssessment,
    ContextItem, Event, ModelInfo, Op, ReviewRequest, ReviewType, RiskLevel, SandboxPolicy,
    Submission, SUBMISSION_CHANNEL_CAPACITY,
};
pub use codex_delegate::{
    run_codex_conversation_interactive, run_codex_conversation_one_shot, DelegateResult,
    ParentContext, SubAgentSource,
};
pub use compact::{
    build_compacted_history, collect_user_messages, is_summary_message, should_compact,
    CompactConfig, CompactionResult, SUMMARIZATION_PROMPT, SUMMARY_PREFIX,
};
pub use config::{
    validate_config_toml, validate_toml_syntax, Config, ConfigError, ConfigIssue,
    ConfigIssueSeverity, ConfigValidationResult, DashFlowConfig, DoctorConfig, PolicyConfig,
    TomlParseErrorDetail,
};
pub use elapsed::{format_duration, format_elapsed, format_elapsed_millis};
pub use error::{Error, FunctionCallError, Result};
pub use exec::{
    execute, execute_with_stream, is_likely_sandbox_denied, ExecExpiration, ExecOutput, ExecParams,
    OutputChunk, OutputStream, OutputStreamSender, SandboxType, StreamOutput,
    DEFAULT_EXEC_COMMAND_TIMEOUT_MS, EXEC_TIMEOUT_EXIT_CODE, MAX_EXEC_OUTPUT_DELTAS_PER_CALL,
};
pub use exec_env::{
    create_env, EnvironmentVariablePattern, ShellEnvironmentPolicy, ShellEnvironmentPolicyInherit,
};
pub use execpolicy::{ApprovalMode, ApprovalRequirement, Decision, ExecPolicy, PolicyRule};
pub use features::{
    feature_for_key, is_known_feature_key, Feature, FeatureSpec, Features, FeaturesToml, Stage,
    FEATURES,
};
pub use flags::{get_sse_fixture, is_sse_fixture_enabled};
pub use format_env::{format_env_display, format_env_map_display, format_env_var_masked};
pub use fuzzy_match::{fuzzy_indices, fuzzy_match, fuzzy_matches, fuzzy_score};
pub use graph::{
    build_agent_graph, build_agent_graph_manifest, get_agent_graph_mermaid,
    get_agent_graph_metadata, get_graph_registry, AGENT_GRAPH_NAME, AGENT_GRAPH_VERSION,
};
pub use llm::{
    AuthMode, ChatGptConfig, LlmClient, LlmConfig, LlmProvider, LlmResponse, LlmRetryConfig,
};
pub use nodes::tool_execution::ToolExecutor;
pub use runner::{
    can_resume_session, cleanup_checkpoints, delete_all_sessions, delete_session,
    get_latest_session, get_latest_session_with_max_age, get_session_info, list_sessions,
    resume_session, run_agent, run_turn, AgentResult, CheckpointMetadata,
    CheckpointRetentionPolicy, RunnerConfig, SessionDetails, ThreadInfo,
};
pub use spawn::{
    spawn_child, spawn_simple, SpawnOptions, StdioPolicy, CODEX_SANDBOX_ENV_VAR,
    CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR,
};
pub use state::{
    AgentState, ApprovalCallback, AutoApproveCallback, AutoRejectCallback, Message, MessageRole,
    ToolCall, ToolResult,
};
pub use streaming::{
    AgentEvent, ConsoleStreamCallback, DashFlowStreamConfig, MetricsCallback, NullStreamCallback,
    StreamCallback, StreamCallbackBuilder,
};

// Conditional re-export of DashFlowStreamAdapter (requires "dashstream" feature)
#[cfg(feature = "dashstream")]
pub use streaming::DashFlowStreamAdapter;

// DashFlow Streaming observability re-exports (requires "dashstream" feature)
// These types enable streaming telemetry monitoring and quality observability
#[cfg(feature = "dashstream")]
pub use dashflow_streaming::metrics_monitor::{
    calculate_loss_rate, check_for_high_loss, get_metrics_text, MetricsSnapshot,
};
#[cfg(feature = "dashstream")]
pub use dashflow_streaming::quality::{
    QualityIssue, QualityJudge, QualityMonitor, QualityScore as StreamingQualityScore,
};
#[cfg(feature = "dashstream")]
pub use dashflow_streaming::quality_gate::{
    QualityConfig as StreamingQualityConfig, QualityGate as StreamingQualityGate, QualityGateError,
};

// DashFlow DashStreamCallback re-exports (requires "dashstream" feature)
// These types provide the native DashFlow streaming callback implementation
// Note: DashStreamConfig is renamed to DashFlowDashStreamConfig to avoid conflict
// with the simpler DashStreamConfig used in the CLI crate
#[cfg(feature = "dashstream")]
pub use dashflow::dashstream_callback::{
    DashStreamCallback, DashStreamConfig as DashFlowDashStreamConfig,
    DEFAULT_MAX_CONCURRENT_TELEMETRY_SENDS, DEFAULT_MAX_STATE_DIFF_SIZE,
    DEFAULT_TELEMETRY_BATCH_SIZE, DEFAULT_TELEMETRY_BATCH_TIMEOUT_MS,
};

pub use tools::{get_tool_definitions, get_tool_definitions_with_mcp, ToolDefinition, ToolType};

// AI Introspection re-exports from DashFlow
pub use dashflow::introspection::{
    // Alert types
    AlertSeverity,
    AlertType,
    // Bottleneck analysis types
    Bottleneck,
    BottleneckAnalysis,
    BottleneckBuilder,
    BottleneckMetric,
    BottleneckSeverity,
    BottleneckThresholds,
    BudgetAlert,
    BudgetAlertSeverity,
    BudgetAlertType,
    // Capability manifest types
    CapabilityManifest,
    CapabilityManifestBuilder,
    // Configuration types
    ConfigurationRecommendations,
    // Decision log types
    DecisionHistory,
    DecisionLog,
    DecisionLogBuilder,
    // Graph manifest types
    EdgeManifest,
    // Execution types
    ErrorTrace,
    ExecutionContext,
    ExecutionContextBuilder,
    ExecutionTrace,
    ExecutionTraceBuilder,
    FieldSchema,
    GraphManifest,
    GraphManifestBuilder,
    GraphMetadata,
    // Graph reconfiguration types
    GraphReconfiguration,
    GraphReconfigurationBuilder,
    ModelCapability,
    ModelFeature,
    NodeExecution,
    NodeManifest,
    NodeType,
    // Optimization types
    OptimizationAnalysis,
    OptimizationCategory,
    OptimizationPriority,
    OptimizationSuggestion,
    OptimizationSuggestionBuilder,
    // Pattern types
    Pattern,
    PatternAnalysis,
    PatternBuilder,
    PatternCondition,
    PatternOperator,
    PatternThresholds,
    PatternType,
    PatternValue,
    PerformanceAlert,
    // Performance monitoring types
    PerformanceHistory,
    PerformanceMetrics,
    PerformanceMetricsBuilder,
    PerformanceThresholds,
    RecommendationConfig,
    ReconfigurationPriority,
    ReconfigurationType,
    ResourceUsage,
    ResourceUsageBuilder,
    ResourceUsageHistory,
    StateIntrospection,
    StateSchema,
    StorageBackend,
    StorageFeature,
    StorageType,
    ToolManifest,
    ToolParameter,
};
// Platform Registry re-exports from DashFlow for AI platform awareness
pub use dashflow::platform_registry::{
    generate_flow_description, parse_cargo_toml, ApiDocs, ApiInfo, AppArchitecture,
    AppArchitectureBuilder, ArchitectureGraphInfo, ArchitectureMetadata, CodeModule, ConfigOption,
    CrateCategory, CrateDependency, CrateInfo, DecisionPath, DecisionPoint, DecisionType,
    Dependency, DependencyAnalysis, DependencyAnalysisBuilder, DependencyCategory,
    DependencyMetadata, DocResult, DocumentationQuery, ExecutionFlow, ExecutionFlowBuilder,
    ExecutionFlowMetadata, ExecutionPath, FeatureDetails, FeatureDetailsBuilder, FeatureInfo,
    FeatureUsage, LoopStructure, LoopType, ModuleInfo, ModuleInfoBuilder, ParamDoc,
    PlatformMetadata, PlatformRegistry, PlatformRegistryBuilder,
};

// Graph Registry & Versioning re-exports from DashFlow
pub use dashflow::graph_registry::{
    state_diff, AISelfKnowledge, ExecutionRecord, ExecutionRegistry, ExecutionStatus, FieldDiff,
    GraphDiff, GraphRegistry, GraphVersion, NodeVersion, RegistryEntry, RegistryMetadata,
    StateDiff, StateRegistry, StateSnapshot, VersionStore,
};

// Execution Metrics re-exports from DashFlow
pub use dashflow::metrics::{ExecutionMetrics, LocalMetricsBatch};

// Quality assurance re-exports from DashFlow
pub use dashflow::quality::{
    QualityGate, QualityGateConfig, QualityGateResult, QualityScore, ResponseValidator,
    RetryStrategy, ToolResultValidator, ToolValidationAction, ToolValidationResult,
    ToolValidatorConfig, ValidationAction, ValidationResult,
};

// Graph Templates re-exports from DashFlow
pub use dashflow::templates::{GraphTemplate, MapReduceBuilder, SupervisorBuilder};

// Scheduler re-exports from DashFlow
pub use dashflow::scheduler::{
    SchedulerConfig, SchedulerMetrics, SelectionStrategy, WorkStealingScheduler,
};

// Retention Policy re-exports from DashFlow
pub use dashflow::retention::{RetentionPolicy, RetentionPolicyBuilder};

// Cost monitoring re-exports from dashflow-observability
// Note: CostMonitor was renamed to CostTracker, UsageRecord was renamed to CostRecord
pub use dashflow_observability::cost::{
    AlertLevel, BudgetConfig, BudgetEnforcer, CostReport, CostTracker, ModelPrice, ModelPricing,
    Pricing, TokenUsage, CostRecord,
};

// DashOptimize A/B testing re-exports from DashFlow
pub use dashflow::optimize::ab_testing::{
    ABTest, ConfidenceInterval, ResultsReport, StatisticalAnalysis, TTestResult, TrafficSplitter,
    Variant, VariantReport,
};

// DashOptimize distillation re-exports from DashFlow
pub use dashflow::optimize::distillation::{
    CostAnalysis, DistillationConfig, DistillationReport, ModelDistillation, QualityGap,
    ROIMetrics, SyntheticDataGenerator,
};

// DashOptimize optimizers re-exports from DashFlow
pub use dashflow::optimize::optimizers::{
    AutoPrompt, AutoPromptBuilder, BootstrapFewShot, BootstrapOptuna, CandidateProgram, GEPAConfig,
    GEPAResult, KNNFewShot, LabeledFewShot, OptimizerConfig, RandomSearch, SimbaOutput,
    SimbaStrategy, StrategyContext, GEPA, SIMBA,
};

// DashOptimize graph optimizer re-exports from DashFlow
pub use dashflow::optimize::graph_optimizer::{GraphOptimizer, OptimizationStrategy};

// DashOptimize evaluation metrics re-exports from DashFlow
pub use dashflow::optimize::metrics::{
    compute_all_json_metrics, exact_match, exact_match_any, f1_score, json_exact_match,
    json_f1_score, json_precision_score, json_recall_score, max_f1, normalize_text,
    precision_score, recall_score, JsonMetricConfig, MetricFn, SemanticF1, SemanticF1Config,
    SemanticF1Result,
};

// DashOptimize data collection re-exports from DashFlow
pub use dashflow::optimize::data_collection::{
    DataCollector, DataFormat, DataSource, DataStore, DistributionAnalysis, DistributionAnalyzer,
    TrainingExample as DashFlowTrainingExample,
};

// DashOptimize modules re-exports from DashFlow (optimizable node patterns)
pub use dashflow::optimize::modules::{
    Action, ActionOutput, AggregationStrategy, AvatarNode, AvatarTool, BestOfNNode,
    ChainOfThoughtNode, EnsembleNode, FeedbackFn, MultiChainComparisonNode, ReActNode, RefineNode,
    RefineableState, RewardFn, SimpleTool, Tool,
};

// DashOptimize multi-objective optimization re-exports from DashFlow
pub use dashflow::optimize::multi_objective::{
    Candidate, MultiObjectiveConfig, MultiObjectiveError, MultiObjectiveOptimizer, Objective,
    ObjectiveType, ObjectiveValue, ParetoError, ParetoFrontier, ParetoSolution,
};

// DashOptimize aggregation utilities re-exports from DashFlow
pub use dashflow::optimize::aggregation::{default_normalize, majority};

// DashOptimize KNN and Example re-exports from DashFlow
pub use dashflow::optimize::example::Example as DashFlowExample;
pub use dashflow::optimize::knn::KNN;

// DashOptimize signature types re-exports from DashFlow
pub use dashflow::optimize::signature::{make_signature, Field, FieldKind, Signature};

// DashOptimize content types re-exports from DashFlow
// Note: Message, ToolCall, ToolResult are renamed to avoid conflicts with our own types
pub use dashflow::optimize::types::{
    Audio, AudioFormat, Citation, Code, Document, File, FileType, History, Image, ImageFormat,
    Language, LlmContent, Message as DashFlowOptMessage, Reasoning, ReasoningEffort,
    ReasoningOutput, ReasoningStep, Role as DashFlowOptRole, ToLlmContent,
    ToolCall as DashFlowOptToolCall, ToolCalls, ToolResult as DashFlowOptToolResult,
};

// DashOptimize extension trait re-export from DashFlow
pub use dashflow::optimize::ext::DspyGraphExt;

// DashOptimize additional optimizer types re-exports from DashFlow
// Note: SelectionStrategy is renamed to avoid conflict with scheduler::SelectionStrategy
pub use dashflow::optimize::optimizers::{
    AutoPromptMetricFn, COPROv2, COPROv2Builder, COPROv2MetricFn, GEPAMetricFn,
    OptimizationResult as DashFlowOptimizationResult, ScoreWithFeedback,
    SelectionStrategy as OptSelectStrategy, TraceStep as OptTraceStep,
};

// DashOptimize additional distillation types re-exports from DashFlow
pub use dashflow::optimize::distillation::{
    DistillationConfigBuilder, DistillationResult, SyntheticDataConfig,
};

// Note: CostMonitorError no longer exists. Cost operations now use dashflow_observability::Error
// which is re-exported as ObservabilityError below
pub use dashflow_observability::Error as ObservabilityError;

// DashOptimize Optimizable trait and types re-exports from DashFlow
pub use dashflow::optimize::{
    FewShotExample as DashFlowFewShotExample, Optimizable, OptimizationState,
};

// DashFlow Debug module re-exports for graph visualization
// Note: ExecutionTracer and TracingCallback were removed. Use introspection module for tracing.
pub use dashflow::debug::{
    EdgeTaken, GraphStructure, MermaidConfig, MermaidDirection, MermaidExport,
    MermaidNodeShape, TraceStep, TracedEdgeType,
};

// Note: ExecutionTrace, ExecutionTraceBuilder, NodeExecution are already exported
// from the introspection module re-exports above (lines 209-220)

// DashFlow Approval module re-exports for human-in-the-loop patterns
// Note: These complement our own approval_presets module with DashFlow's built-in approval flow
pub use dashflow::approval::{
    auto_approval_handler, ApprovalChannel, ApprovalNode, ApprovalReceiver, ApprovalRequest,
    ApprovalResponse, AutoApprovalPolicy, PendingApproval, RiskLevel as DashFlowRiskLevel,
};

// DashFlow Checkpoint module re-exports for advanced checkpointing features
// Note: These complement the basic checkpointing already in runner.rs
// CheckpointMetadata and ThreadInfo are renamed to avoid conflicts with our own types
pub use dashflow::checkpoint::{
    Checkpoint, CheckpointId, CheckpointMetadata as DashFlowCheckpointMetadata, Checkpointer,
    CompressedFileCheckpointer, CompressionAlgorithm, DistributedCheckpointCoordinator,
    FileCheckpointer, MemoryCheckpointer, MigrationChain, MultiTierCheckpointer, ResumeEnvironment,
    ResumeError, ResumeOutcome, ResumeRunner, ResumeValidator, SqliteCheckpointer, StateMigration,
    ThreadId, ThreadInfo as DashFlowThreadInfo, Version, VersionedCheckpoint,
    VersionedFileCheckpointer, WritePolicy,
};

// Optimize re-exports
pub use optimize::{
    optimize_prompts, FewShotExample, OptimizationMetadata, OptimizationResult, OptimizeConfig,
    OptimizeError, PromptConfig, PromptRegistry, TrainingData, TrainingExample,
    DEFAULT_SYSTEM_PROMPT,
};

// Context re-exports (application-specific utilities complementing DashFlow context)
pub use context::{
    approx_bytes_for_tokens, approx_token_count, formatted_truncate_text, messages_token_count,
    truncate_text, ContextManager, TruncationConfig, TruncationPolicy, DEFAULT_CONTEXT_BUDGET,
};

// Model family re-exports
pub use model_family::{
    default_model_family, find_family_for_model, provider_for_model, ModelFamily, ModelProvider,
};

// OpenAI model info re-exports
pub use openai_model_info::{
    get_auto_compact_limit, get_context_window, get_model_info, OpenAiModelInfo,
    CONTEXT_WINDOW_272K,
};

// List directory re-exports
pub use list_dir::{
    format_result as format_list_dir_result, list_dir, ListDirError, ListDirResult, DEFAULT_DEPTH,
    DEFAULT_LIMIT, DEFAULT_OFFSET,
};

// Git info re-exports
pub use git_info::{
    collect_git_info, current_branch_name, format_commits_for_context, get_git_diff,
    get_git_repo_root, git_diff_range, git_status_short, is_in_git_repo, merge_base_with_head,
    recent_commits, uncommitted_change_count, CommitLogEntry, GitInfo,
};

// Ghost commit re-exports
pub use ghost_commit::{
    capture_ghost_snapshot_report, create_ghost_commit, create_ghost_commit_with_report,
    restore_ghost_commit, restore_to_commit, CreateGhostCommitOptions, GhostCommit,
    GhostCommitError, GhostSnapshotReport, LargeUntrackedDir,
};

// Safe commands re-exports
pub use safe_commands::is_known_safe_command;

// Windows safety re-exports
pub use windows_dangerous_commands::is_dangerous_command_windows;
pub use windows_safe_commands::is_safe_command_windows;

// Safety re-exports
pub use safety::{
    analyze_command, contains_sensitive_content, get_danger_reasons, is_dangerous,
    sanitize_for_logging, sanitize_tool_output, SafetyCheck, Severity,
};

// Shell re-exports
pub use shell::{
    default_user_shell, detect_shell_type, get_shell, get_shell_by_model_provided_path, Shell,
    ShellType,
};

// Bash parsing re-exports
pub use bash::{
    extract_bash_command, parse_shell_lc_plain_commands, try_parse_shell,
    try_parse_word_only_commands_sequence,
};

// Parse command re-exports
pub use parse_command::{extract_shell_command, parse_command, shlex_join, ParsedCommand};

// PowerShell re-exports
pub use powershell::{extract_powershell_command, is_powershell_command};

// User instructions re-exports
pub use user_instructions::{
    DeveloperInstructions, DeveloperInstructionsMessage, UserInstructions, UserInstructionsMessage,
    USER_INSTRUCTIONS_OPEN_TAG_LEGACY, USER_INSTRUCTIONS_PREFIX,
};

// User notification re-exports
pub use user_notification::{UserNotification, UserNotifier};

// Message history re-exports
pub use message_history::{
    append_entry as append_history_entry, history_metadata, lookup as lookup_history_entry,
    HistoryConfig, HistoryEntry, HISTORY_FILENAME,
};

// Project doc re-exports
pub use project_doc::{
    discover_project_doc_paths, get_user_instructions, read_project_docs, ProjectDocOptions,
    DEFAULT_PROJECT_DOC_FILENAME, DEFAULT_PROJECT_DOC_MAX_BYTES, LOCAL_PROJECT_DOC_FILENAME,
};

// Model provider re-exports
pub use model_provider_info::{
    built_in_model_providers, create_oss_provider, create_oss_provider_with_base_url,
    ModelProviderInfo, ProviderRegistry, RetryConfig, WireApi, ANTHROPIC_PROVIDER_ID,
    DEFAULT_LMSTUDIO_PORT, DEFAULT_OLLAMA_PORT, LMSTUDIO_OSS_PROVIDER_ID, OLLAMA_OSS_PROVIDER_ID,
    OPENAI_PROVIDER_ID,
};

// Custom prompts re-exports
pub use custom_prompts::{
    default_prompts_dir, discover_default_prompts, discover_default_prompts_excluding,
    discover_prompts_in, discover_prompts_in_excluding, substitute_arguments, CustomPrompt,
    PROMPTS_CMD_PREFIX,
};

// Text encoding re-exports
pub use text_encoding::bytes_to_string_smart;

// Terminal detection re-exports
pub use terminal::user_agent;

// Skills re-exports
pub use skills::{
    default_skills_dir, load_skills, load_skills_from, render_skills_section, SkillError,
    SkillLoadOutcome, SkillMetadata,
};

// Turn diff tracker re-exports
pub use turn_diff_tracker::{FileChange, FileMode, TurnDiffTracker};

// User shell command re-exports
pub use user_shell_command::{
    format_user_shell_command_record, is_user_shell_command_text, UserShellCommandRecord,
    USER_SHELL_COMMAND_CLOSE, USER_SHELL_COMMAND_OPEN,
};

// Util re-exports
pub use util::{
    backoff, error_or_panic, is_json_like, resolve_path, truncate_string, try_parse_error_message,
};

// Review re-exports
pub use review::{
    format_review_findings_block, generate_review_prompt, render_review_output_text,
    review_target_hint, ResolvedReviewRequest, ReviewCodeLocation, ReviewFinding, ReviewLineRange,
    ReviewOutputEvent, ReviewTarget,
};

// Exit status re-exports
pub use exit_status::{exit_code_from_status, handle_exit_status};

// WSL path re-exports
pub use wsl_paths::{is_wsl, normalize_for_wsl, win_path_to_wsl};

// Approval presets re-exports
pub use approval_presets::{
    approval_policy_to_mode, builtin_approval_presets, default_preset_id, exec_policy_from_preset,
    find_preset, ApprovalPreset,
};

// Sandbox summary re-exports
pub use sandbox_summary::{
    sandbox_security_level, summarize_sandbox_policy, summarize_sandbox_policy_short,
};

// Config summary re-exports
pub use config_summary::{
    create_config_summary_entries, format_aligned, format_config_compact, format_config_summary,
    max_key_width,
};

// Config override re-exports
pub use config_override::{
    apply_overrides, apply_single_override, parse_and_apply_overrides, parse_override,
    parse_overrides,
};

// DashFlow Core StateGraph re-exports
// These are the fundamental building blocks for graph-based agent workflows

// Edge types for defining graph transitions
pub use dashflow::edge::{ConditionalEdge, Edge, ParallelEdge, END, START};

// Event types for tracing and callbacks during graph execution
pub use dashflow::event::{
    CollectingCallback, EdgeType, EventCallback, FnTracer, GraphEvent, PrintCallback, TracerEvent,
};

// Executor types for compiling and running graphs
pub use dashflow::executor::{
    CompiledGraph, ExecutionResult, GraphIntrospection, GraphValidationResult,
    GraphValidationWarning, DEFAULT_GRAPH_TIMEOUT, DEFAULT_MAX_STATE_SIZE, DEFAULT_NODE_TIMEOUT,
};

// Graph builders for constructing StateGraphs
pub use dashflow::graph::{GraphBuilder, StateGraph};

// Integration types for tool execution patterns
pub use dashflow::integration::{
    auto_tool_executor, tools_condition, AgentNode, RunnableNode, ToolNode,
};

// Node type for defining graph nodes
pub use dashflow::node::Node;

// Prebuilt agent patterns (renamed to avoid conflict with our AgentState)
pub use dashflow::prebuilt::{create_react_agent, AgentState as DashFlowAgentState};

// Reducer types for state management
pub use dashflow::reducer::{add_messages, AddMessagesReducer, MessageExt, Reducer};

// State types for graph state management
pub use dashflow::state::{GraphState, JsonState, JsonStateIter, MergeableState};

// Stream types for streaming execution results
pub use dashflow::stream::{
    reset_stream_dropped_count, stream_dropped_count, StreamEvent, StreamMode,
    DEFAULT_STREAM_CHANNEL_CAPACITY,
};

// Subgraph node for composing graphs
pub use dashflow::subgraph::SubgraphNode;

// DashFlow derive macros for state types
pub use dashflow::{DeriveGraphState, DeriveMergeableState, GraphStateDerive};

// DashFlow error types
pub use dashflow::error::{CheckpointError, Error as DashFlowError};

// MCP re-exports for convenience
pub mod mcp {
    pub use codex_dashflow_mcp::*;
}

// Sandbox re-exports for convenience
pub mod sandbox {
    pub use codex_dashflow_sandbox::*;
}
