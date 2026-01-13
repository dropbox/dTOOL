// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// M-132: Enforce documentation on new public items.
// Phase 1: warn (not deny) to establish baseline without breaking build.
// Phase 2: Fix existing warnings incrementally.
// Phase 3: Extend to other high-traffic crates.
#![warn(missing_docs)]
// M-2142: Many unit tests compare known float constants (thresholds, ratios).
// Prefer approximate comparisons for computed floats, but allow exact comparisons in tests.
#![cfg_attr(test, allow(clippy::float_cmp))]

//! # `DashFlow` - Graph-based Multi-Agent Workflows
//!
//! `DashFlow` is a framework for building stateful, multi-agent applications with LLMs.
//! It enables complex workflows using directed graphs with cycles, conditional routing,
//! and state management.
//!
//! ## Key Concepts
//!
//! - **`StateGraph`** / **`GraphBuilder`**: Define your application state and workflow as a graph
//! - **Nodes**: Functions or agents that transform state
//! - **Edges**: Transitions between nodes (simple, conditional, or parallel)
//! - **Execution**: Run graphs with streaming, checkpointing, and human-in-the-loop
//!
//! ## Example: Basic Usage
//!
//! ```rust,ignore
//! use dashflow::{StateGraph, Node};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Serialize, Deserialize)]
//! struct AgentState {
//!     messages: Vec<String>,
//!     next: String,
//! }
//!
//! // Define a simple node
//! async fn research_node(state: AgentState) -> Result<AgentState, Box<dyn std::error::Error>> {
//!     let mut state = state;
//!     state.messages.push("Research complete".to_string());
//!     state.next = "writer".to_string();
//!     Ok(state)
//! }
//!
//! // Build the graph
//! let mut graph = StateGraph::new();
//! graph.add_node("researcher", research_node);
//! graph.add_edge("researcher", "writer");
//! graph.set_entry_point("researcher");
//!
//! // Compile and run
//! let app = graph.compile()?;
//! let result = app.invoke(initial_state).await?;
//! ```
//!
//! ## Example: Fluent Builder (Recommended)
//!
//! ```rust,ignore
//! use dashflow::{GraphBuilder, END};
//!
//! let mut graph = GraphBuilder::new();
//! graph
//!     .add_node("researcher", research_node)
//!     .add_node("writer", writer_node)
//!     .add_edge("researcher", "writer")
//!     .add_edge("writer", END)
//!     .set_entry_point("researcher");
//!
//! let app = graph.compile()?;
//! let result = app.invoke(initial_state).await?;
//! ```

// Core framework (merged from dashflow::core)
pub mod core;

// Centralized constants for time, retry, and size values
pub mod constants;

// Graph-based workflow modules
pub mod ab_testing; // Automatic A/B testing - AI experiments with variations
pub mod adaptive_timeout; // Adaptive timeout adjustment - AI learns optimal timeouts
pub mod ai_explanation; // AI explanation of decisions
pub mod anomaly_detection; // Anomaly detection - AI detects unusual behavior
pub mod api; // The One Correct Path - graph-based convenience API (use this!)
pub mod approval; // Built-in approval flow for human-in-the-loop patterns
pub mod causal_analysis; // Causal analysis - AI understands WHY things happened
pub mod checkpoint;
pub mod checkpointer_helpers; // Common utilities for checkpointer backends
pub mod colony; // Colony expansion - system introspection and organic spawning
pub mod counterfactual_analysis; // Counterfactual analysis - AI simulates "what if" scenarios
pub mod cross_agent_learning; // Cross-agent learning - AI learns from other agents
pub mod debug; // StateGraph debugging (Mermaid export, execution tracing)
pub mod decision_tracking; // Decision tracking - agent helper for DecisionMade/OutcomeObserved events
pub mod edge;
pub mod error;
pub mod event;
pub mod execution_prediction; // Execution prediction - AI predicts its own execution
pub mod executor;
pub mod factory_trait; // Base factory trait hierarchy
pub mod func;
pub mod graph;
pub mod graph_manifest_import; // Graph manifest import - dynamic graph construction from JSON
pub mod graph_reconfiguration; // Dynamic graph reconfiguration - AI self-modification
pub mod graph_registry; // Graph registry & versioning - multi-graph management
pub mod integration;
pub mod introspection; // AI self-awareness - graph manifest, execution context
                       // introspection_interface moved to introspection::interface (2025-12-21)
pub mod lint; // Platform usage linter - detects reimplementations of platform features
pub mod live_introspection; // Live execution introspection - runtime state tracking
pub mod mcp_self_doc; // MCP Self-Documentation Protocol - AI-to-AI understanding
pub mod metrics;
pub mod network; // Network coordination - peer discovery and messaging
pub mod node;
pub mod node_registry; // Node registry - dynamic node discovery and factories
pub mod optimize; // DashOptimize - native prompt optimization
pub mod packages; // Package ecosystem - registry, discovery, trust, contributions
pub mod parallel; // Parallel AI development coordination - lock system for multiple workers
pub mod pattern_engine; // Unified pattern detection - consolidates all pattern detection systems
pub mod pattern_recognition; // Pattern recognition - AI learns from multiple executions
pub mod platform_introspection; // Platform-level introspection - framework capabilities
pub mod platform_registry; // AI platform awareness - DashFlow self-knowledge
pub mod prebuilt;
pub mod prometheus_client; // Prometheus client for querying metrics
pub mod prompt_evolution; // Prompt self-evolution - AI improves its own prompts
pub mod quality;
pub mod reducer;
pub mod registry_trait; // Base registry trait hierarchy
pub mod retention;
pub mod scheduler;
pub mod schema; // Graph schema export for visualization
pub mod self_improvement; // Self-improving introspection system - AI self-analysis & planning
pub mod state;
pub mod stream;
pub mod subgraph;
pub mod telemetry; // Unified telemetry primitives (FIX-009: GraphContext, TelemetrySink)
pub mod templates;
pub mod trace_analysis; // Shared trace analysis primitives
pub mod unified_introspection; // Unified four-level introspection API
pub mod wal; // Write-Ahead Log for persistent observability telemetry

#[cfg(test)]
mod serialization_proptest; // Property-based tests for serialization roundtrips
#[cfg(test)]
mod scheduler_proptest; // Property-based tests for scheduler invariants

#[cfg(feature = "dashstream")]
pub mod dashstream_callback;

// Kani proof harnesses - only compiled when running `cargo kani`
#[cfg(kani)]
mod kani_harnesses;

pub use checkpoint::{
    Checkpoint, CheckpointDiff, CheckpointId, CheckpointMetadata, CheckpointPolicy, Checkpointer,
    CompressedFileCheckpointer, CompressionAlgorithm, DifferentialCheckpointer, DifferentialConfig,
    DistributedCheckpointCoordinator, FileCheckpointer, MemoryCheckpointer, MigrationChain,
    MultiTierCheckpointer, ResumeEnvironment, ResumeError, ResumeOutcome, ResumeRunner,
    ResumeValidator, SqliteCheckpointer, StateMigration, ThreadId, ThreadInfo, Version,
    VersionedCheckpoint, VersionedFileCheckpointer, WritePolicy,
};
pub use constants::{
    DEFAULT_BACKOFF_MULTIPLIER,
    DEFAULT_BATCH_SIZE,
    DEFAULT_CACHE_SIZE,
    DEFAULT_INITIAL_DELAY_MS,
    DEFAULT_JITTER_MS,
    DEFAULT_MAX_DELAY_MS,
    // Retry constants
    DEFAULT_MAX_RETRIES,
    DEFAULT_QUEUE_CAPACITY,
    DEFAULT_TIMEOUT_MS,
    HIGH_TOKEN_THRESHOLD,
    LONG_TIMEOUT_MS,
    LONG_TIMEOUT,
    // Size constants
    MAX_BYTES_ERROR,
    MAX_CONCURRENT_EXECUTIONS,
    MAX_RETRIES_LIMIT,
    MAX_TELEMETRY_BATCH_SIZE,
    // Monitoring constants
    MAX_TRACE_COUNT,
    MILLION,
    // Time constants
    SECONDS_PER_DAY,
    SECONDS_PER_WEEK,
    SLOW_THRESHOLD_MS,
    THOUSAND,
    VERY_LONG_TIMEOUT_MS,
    // HTTP client Duration constants (M-146)
    DEFAULT_HTTP_CONNECT_TIMEOUT,
    DEFAULT_HTTP_REQUEST_TIMEOUT,
    DEFAULT_POOL_IDLE_TIMEOUT,
    DEFAULT_TCP_KEEPALIVE,
    MEDIUM_POLL_INTERVAL,
    SHORT_POLL_INTERVAL,
    SHORT_TIMEOUT,
    // Lock/coordination constants (M-147)
    LOCK_RETRY_INTERVAL,
    // Channel capacity constants (M-147)
    DEFAULT_BROADCAST_CHANNEL_CAPACITY,
    DEFAULT_MPSC_CHANNEL_CAPACITY,
    DEFAULT_WS_CHANNEL_CAPACITY,
    // HTTP pool constants (M-147)
    DEFAULT_POOL_MAX_IDLE_PER_HOST,
    DEFAULT_LLM_REQUEST_TIMEOUT,
    // Regex engine constants (M-147)
    REGEX_SIZE_LIMIT,
    REGEX_DFA_SIZE_LIMIT,
    // Streaming & network constants (M-147)
    DEFAULT_STREAM_CHANNEL_CAPACITY,
    DEFAULT_MDNS_TTL_SECS,
    DEFAULT_RESOURCE_BROADCAST_INTERVAL_SECS,
    // Self-improvement daemon constants (M-147)
    DEFAULT_FILE_WATCHER_CHANNEL_CAPACITY,
    DEFAULT_TRIGGER_CHANNEL_CAPACITY,
    // Live introspection constants (M-147)
    DEFAULT_COMPLETED_TTL_SECS,
    DEFAULT_MAX_HISTORY_STEPS,
    DEFAULT_FLUSH_TIMEOUT_SECS,
    DEFAULT_TELEMETRY_BATCH_TIMEOUT_MS,
    // Channel validation constants (M-147)
    DEFAULT_MAX_CHANNEL_CAPACITY,
    // Self-improvement metrics buffer constants (M-147)
    DEFAULT_MAX_NODE_DURATION_SAMPLES,
    DEFAULT_MAX_QUALITY_SCORE_SAMPLES,
};
pub use edge::{ConditionalEdge, Edge, ParallelEdge, END, START};
pub use error::{ActionableError, ActionableSuggestion, CheckpointError, Error, Result};
pub use event::{
    CollectingCallback, DecisionAlternative, EdgeAlternative, EdgeType, EventCallback, FnTracer,
    GraphEvent, PrintCallback, TracerEvent,
};
pub use decision_tracking::{AlternativeBuilder, DecisionTracker};
pub use executor::{
    current_graph_context, CompiledGraph, ExecutionResult, GraphIntrospection,
    GraphValidationResult, GraphValidationWarning, UnifiedIntrospection, DEFAULT_GRAPH_TIMEOUT,
    DEFAULT_MAX_PARALLEL_TASKS, DEFAULT_MAX_STATE_SIZE, DEFAULT_NODE_TIMEOUT,
};
pub use graph::{GraphBuilder, StateGraph};
pub use integration::{auto_tool_executor, tools_condition, AgentNode, RunnableNode, ToolNode};
pub use metrics::{ExecutionMetrics, LocalMetricsBatch};
pub use node::Node;
pub use prebuilt::{create_react_agent, AgentState};

// Graph-based API - The One Correct Path (replaces direct trait method calls)
pub use api::{
    build_generate_graph_for_streaming, call_tool, call_tool_structured, embed, embed_query,
    generate, generate_with_options, retrieve, vector_search, vector_search_with_filter,
    EmbedState, GenerateState, RetrieveState, ToolCallState, VectorSearchState,
};
pub use quality::{
    QualityGate, QualityGateConfig, QualityGateResult, QualityScore, ResponseValidator,
    RetryStrategy, ToolResultValidator, ToolValidationAction, ToolValidationResult,
    ToolValidatorConfig, ValidationAction, ValidationResult,
};
pub use reducer::{add_messages, AddMessagesReducer, MessageExt, Reducer};
pub use retention::{RetentionPolicy, RetentionPolicyBuilder};
pub use scheduler::{SchedulerConfig, SchedulerMetrics, SelectionStrategy, WorkStealingScheduler};
pub use state::{GraphState, JsonState, JsonStateIter, MergeableState};
pub use stream::{reset_stream_dropped_count, stream_dropped_count, StreamEvent, StreamMode};
pub use subgraph::SubgraphNode;
pub use templates::{GraphTemplate, MapReduceBuilder, SupervisorBuilder};

// AI Introspection exports
pub use introspection::{
    AlertSeverity, AlertType, Bottleneck, BottleneckAnalysis, BottleneckBuilder, BottleneckMetric,
    BottleneckSeverity, BottleneckThresholds, BudgetAlert, BudgetAlertSeverity, BudgetAlertType,
    CapabilityManifest, CapabilityManifestBuilder, ConfigurationRecommendations, DecisionHistory,
    DecisionLog, DecisionLogBuilder, EdgeManifest, ErrorTrace, ExecutionContext,
    ExecutionContextBuilder, ExecutionTrace, ExecutionTraceBuilder, FieldSchema, GraphManifest,
    GraphManifestBuilder, GraphMetadata, GraphReconfiguration, GraphReconfigurationBuilder,
    ModelCapability, ModelFeature, NodeConfig, NodeExecution, NodeManifest, NodeType,
    OptimizationAnalysis, OptimizationCategory, OptimizationPriority, OptimizationSuggestion,
    OptimizationSuggestionBuilder, OptimizationTrace, Pattern, PatternAnalysis, PatternBuilder,
    PatternCondition, PatternOperator, PatternThresholds, PatternType, PatternValue,
    PerformanceAlert, PerformanceHistory, PerformanceMetrics, PerformanceMetricsBuilder,
    PerformanceThresholds, RecommendationConfig, ReconfigurationPriority, ReconfigurationType,
    ResourceUsage, ResourceUsageBuilder, ResourceUsageHistory, StateIntrospection, StateSchema,
    StorageBackend, StorageFeature, StorageType, TerminationReason, ToolManifest, ToolParameter,
    VariantResult,
};

// Approval flow exports
pub use approval::{
    auto_approval_handler, ApprovalChannel, ApprovalNode, ApprovalReceiver, ApprovalRequest,
    ApprovalResponse, AutoApprovalPolicy, PendingApproval, RiskLevel,
};

// Graph Registry & Versioning exports
pub use graph_registry::{
    state_diff, AISelfKnowledge, ExecutionRecord, ExecutionRegistry, ExecutionStatus, FieldDiff,
    GraphDiff, GraphRegistry, GraphVersion, NodeVersion, RegistryEntry, RegistryMetadata,
    StateDiff, StateRegistry, StateSnapshot, VersionStore,
};

// Platform Introspection exports
// Note: FeatureInfo is unified and exported from platform_registry (below)
pub use platform_introspection::{
    CapabilityInfo, EdgeTypeInfo, NodeTypeInfo, PlatformIntrospection, StateTypeInfo, TemplateInfo,
    VersionInfo,
};

// Unified Introspection exports: Four-level introspection
// Added MetricsSnapshot for Prometheus data parity
pub use unified_introspection::{
    DashFlowIntrospection,
    GraphFileInfo,
    IntrospectionLevel,
    IntrospectionResponse,
    LevelSearchResult,
    MetricsSnapshot,
    ProjectInfo,
    SearchResults,
    TraceStore,
    // Note: InstalledPackage not re-exported to avoid conflict with packages::InstalledPackage
};

// Graph Reconfiguration exports
pub use graph_reconfiguration::{
    GraphMutation, MutationResult, MutationType, NodeTimeouts, OptimizationSuggestions,
};

// Node Registry exports - dynamic node discovery and factories
pub use node_registry::{
    FactoryTypeInfo, FunctionNodeFactory, IdentityNodeFactory, NodeFactory, NodeFactoryError,
    NodeRegistry,
};

// Graph Manifest Import exports - dynamic graph construction from JSON
pub use graph_manifest_import::{
    ConditionFactory, ConditionFn, ConditionRegistry, ConditionalEdgeConfig, ManifestImportError,
    ManifestImporter,
};

// Prompt Evolution exports
pub use prompt_evolution::{
    EvolutionOutcome, PromptAnalysis, PromptEvolution, PromptEvolutionHistory,
    PromptEvolutionPriority, PromptEvolutionRecord, PromptEvolutionResult, PromptImprovementType,
    PromptIssue, PromptIssueType, PromptThresholds,
};

// Adaptive Timeout exports
pub use adaptive_timeout::{
    LatencyStats, TimeoutAdjustmentOutcome, TimeoutAdjustmentRecord, TimeoutConfig, TimeoutHistory,
    TimeoutLearner, TimeoutPercentile, TimeoutRecommendation, TimeoutRecommendations,
};

// Trace Analysis exports - shared primitives
pub use trace_analysis::{percentile, percentile_u64, NodeMetrics, TraceStats, TraceVisitor};

// Registry Trait exports - base registry hierarchy
pub use registry_trait::{
    ConcurrentRegistry, GenericRegistry, GenericRegistryMut, Registry, RegistryIter, RegistryMut,
    RegistryStats, SimpleRegistry,
};

// Causal Analysis exports
pub use causal_analysis::{
    CausalAnalysisConfig, CausalAnalyzer, CausalChain, CausalFactor, CausalThresholds, Cause,
    Effect,
};

// Counterfactual Analysis exports
pub use counterfactual_analysis::{
    Alternative, CounterfactualAnalyzer, CounterfactualConfig, CounterfactualResult, Improvement,
    OutcomeMetrics, Recommendation,
};

// Pattern Recognition exports (consider using pattern_engine::UnifiedPatternEngine for new code)
pub use pattern_recognition::{
    ExecutionPattern, PatternCondition as RecognitionCondition, PatternOutcome,
    PatternRecognitionConfig, PatternRecognizer, PatternThresholds as RecognitionThresholds,
};

// Unified Pattern Engine exports (consolidates PatternRecognizer, PatternDetector, CrossAgentLearner)
pub use pattern_engine::{
    CrossAgentPatternAdapter, ExecutionPatternAdapter, PatternEngine, PatternSource,
    SelfImprovementPatternAdapter, UnifiedPattern, UnifiedPatternEngine,
    UnifiedPatternEngineBuilder, UnifiedPatternEngineConfig, UnifiedPatternType,
};

// Execution Prediction exports
pub use execution_prediction::{
    ConfidenceBreakdown, ExecutionPrediction, ExecutionPredictor, InputFeatures, NodeStats,
    PredictionAccuracy, PredictionConfig, PredictionIntervals, PredictionWarning, WarningSeverity,
};

// Anomaly Detection exports
pub use anomaly_detection::{
    Anomaly, AnomalyDetectionConfig, AnomalyDetector, AnomalyMetric, AnomalySeverity,
    ExecutionStats, NodeExecutionStats,
};

// Cross-Agent Learning exports
pub use cross_agent_learning::{
    AgentCorrelation, AgentSummary, ComparisonOperator, CorrelationType, CrossAgentConfig,
    CrossAgentInsights, CrossAgentLearner, OptimizationStrategy,
    PatternCondition as LearningCondition, PatternType as LearningPatternType, Pitfall,
    PitfallCategory, PitfallSeverity, StrategyRisk, StrategyType, SuccessPattern,
};

// A/B Testing exports
pub use ab_testing::{
    ABTest, ABTestConfig, ABTestHistory, ABTestRecord, ABTestResult, ABTestRunner, InsightType,
    Recommendation as ABRecommendation, TestInsight, TestStatus, Variant, VariantResults, Winner,
};

// AI Explanation exports
pub use ai_explanation::{
    Decision, DecisionExplainer, DecisionExplanation, DecisionReason,
    DecisionType as ExplanationDecisionType, ExplainerConfig, LoopAction,
};

// Introspection Interface exports (moved to introspection::interface)
pub use introspection::interface::{
    InterfaceConfig, IntrospectionInterface, ParsedQuery, QueryResponse, QueryType,
};

// Parallel AI Development exports
pub use parallel::{
    Lock, LockError, LockManager, LockResult, LockScope, LockStatus, DEFAULT_LOCKS_DIR,
    DEFAULT_LOCK_DURATION_SECS,
};

// Colony Expansion exports (see DESIGN_ORGANIC_SPAWNING.md)
pub use colony::{
    // Topology
    ContainerRuntime,
    // Resource types
    CostPer1k,
    DeploymentOption,
    DiskInfo,
    GpuInfo,
    LlmServiceStats,
    MemoryInfo,
    NetworkInterface,
    NumaNode,
    ResourceSnapshot,
    SpawnOption,
    SpawnRequirements,
    // System monitoring
    SystemMonitor,
    SystemMonitorConfig,
    SystemMonitorError,
    SystemTopology,
    TotalLlmCapacity,
};

// Telemetry exports (FIX-009: Design doc promises)
pub use telemetry::{
    AgentObservability, CompositeTelemetrySink, GraphContext, LogTelemetrySink, NullTelemetrySink,
    ObservabilityError, TelemetryEvent, TelemetrySink,
};

// Self-Improvement System exports (see archive/roadmaps/ROADMAP_SELF_IMPROVEMENT.md)
pub use self_improvement::{
    // Analysis depth
    AnalysisDepth,
    Assessment,
    // Capability gap analysis
    CapabilityGap,
    // Analysis engines
    CapabilityGapAnalyzer,
    CapabilityGapConfig,
    // Citations
    Citation,
    CitationRetrieval,
    CitationSource,
    ConsensusResult,
    Counterfactual,
    Critique,
    CritiqueSeverity,
    DeprecationAnalyzer,
    DeprecationConfig,
    // Deprecation analysis
    DeprecationRecommendation,
    DeprecationTarget,
    DetectedPattern,
    Disagreement,
    EvaluationTrigger,
    // Execution plans
    ExecutionPlan,
    ExpectedEvidence,
    GapCategory,
    GapManifestation,
    // Hypothesis tracking
    Hypothesis,
    HypothesisOutcome,
    HypothesisStatus,
    Impact,
    ImplementationStep,
    // Proposals and consensus
    ImprovementProposal,
    // Core report types
    IntrospectionReport,
    IntrospectionScope,
    // Storage
    IntrospectionStorage,
    MissingToolAnalysis,
    ModelIdentifier,
    ModelReview,
    ObservedEvidence,
    PatternConfig,
    PatternDetector,
    PatternType as AnalyzerPatternType,
    PlanCategory,
    PlanStatus,
    Priority as SelfImprovementPriority,
    ProposalSource,
    ReportExecutionSummary,
    // Retrospective analysis
    RetrospectiveAnalysis,
    RetrospectiveAnalyzer,
    RetrospectiveConfig,
    DEFAULT_INTROSPECTION_DIR,
};

// Package Ecosystem exports (see DESIGN_PACKAGE_ECOSYSTEM.md)
pub use packages::{
    compute_key_fingerprint,
    generate_ecdsa_p256_keypair,
    generate_ed25519_keypair,
    AdvisorySeverity,
    AuditStatus,
    Author,
    BugSeverity,
    CacheConfig,
    Capability,
    CapabilityGapCategory,
    CapabilityGapInfo,
    ClientError,
    ClientResult,
    ColonyPackageEntry,
    // Sharing system
    ColonyPackageRegistry,
    ColonyPackageSource,
    ColonyPackageStats,
    ConfigError,
    Contribution,
    ContributionClient,
    ContributionClientConfig,
    // Contribution system
    ContributionError,
    ContributionPackageRef,
    ContributionResult,
    ContributionState,
    ContributionStatus,
    Contributor,
    Dependency as PackageDependency,
    DerivationStep,
    DiscoveryConfig,
    DiscoveryError,
    DiscoveryMethod,
    DiscoveryResult,
    EnhancementPoint,
    EnhancementType,
    Evidence,
    ExpectedImpact,
    GapCategoryRef,
    GitAuth,
    GraphAnalysis,
    GraphPattern,
    HashAlgorithm,
    Hasher,
    HttpAuth,
    ImpactLevel,
    ImpactMetric,
    Implementation as ContributionImplementation,
    ImprovementPriority,
    ImprovementType,
    InstalledPackage,
    IntoCapabilityGapInfo,
    KeyStore,
    Lineage,
    // Local registry
    LocalRegistry,
    Maintainer,
    NewFile as ContributionNewFile,
    NewPackageRequest,
    OptionalDependency,
    OutdatedPackage,
    PackageAdvertisement,
    PackageBugReport,
    // Discovery system
    PackageDiscovery,
    PackageDownload,
    PackageEntry,
    PackageFix,
    // Core types
    PackageId,
    PackageImprovement,
    PackageIndex,
    PackageInfo,
    // Manifest
    PackageManifest,
    PackageManifestBuilder,
    PackageMessage as ColonyPackageMessage,
    PackageRecommendation,
    PackageRecommendationExt,
    PackageRef,
    PackageRequest as ColonyPackageRequest,
    PackageResponse as ColonyPackageResponse,
    PackageSearchResult,
    PackageSharingPolicy,
    PackageSigner,
    PackageSuggestion,
    PackageType,
    PackageVerifier,
    PackageVersionInfo,
    Permission,
    RecommendationPriority,
    RecommendedImpact,
    // HTTP client
    RegistryClient,
    RegistryClientConfig,
    // Configuration
    RegistryConfig,
    RegistryError,
    RegistryResult,
    RegistrySource,
    ReporterIdentity,
    ReproductionSteps,
    RequestPriority,
    RequiredSignatures,
    ReviewerComment,
    SearchOptions,
    SecurityAdvisory,
    SemanticSearchResult,
    SharedPackage,
    SharingError,
    SharingResult,
    Signature,
    SignatureAlgorithm,
    SignedContent,
    SimilarPackage,
    SortOrder,
    SuggestedFix,
    SuggestedPackage,
    SuggestionReason,
    SuggestionSource,
    TestCase as ContributionTestCase,
    TransferMethod,
    TrustConfig,
    TrustError,
    TrustLevel,
    TrustResult,
    // Trust system
    TrustedKey,
    VerificationResult,
    Version as PackageVersion,
    VersionInfo as PackageVersionListInfo,
    VersionOp,
    VersionReq,
    PACKAGES_BROADCAST_INTERVAL,
    PACKAGES_CHANNEL,
};

// Live Introspection exports
// Note: DEFAULT_COMPLETED_TTL_SECS and DEFAULT_MAX_HISTORY_STEPS are exported from constants module
pub use live_introspection::{
    CheckpointStatusInfo, ExecutionEvent, ExecutionEventStream, ExecutionState, ExecutionStep,
    ExecutionSummary, ExecutionTracker, ExecutionTrackerConfig, LiveExecutionMetrics,
    LiveExecutionStatus, StepOutcome, DEFAULT_EVENT_CHANNEL_CAPACITY,
    DEFAULT_MAX_CONCURRENT_EXECUTIONS,
};

// AI Platform Awareness exports
pub use platform_registry::{
    generate_flow_description, parse_cargo_toml, ApiDocs, ApiInfo, AppArchitecture,
    AppArchitectureBuilder, ArchitectureGraphInfo, ArchitectureMetadata, CodeModule, ConfigOption,
    CrateCategory, CrateDependency, CrateInfo, DecisionPath, DecisionPoint, DecisionType,
    Dependency, DependencyAnalysis, DependencyAnalysisBuilder, DependencyCategory,
    DependencyMetadata, DocResult, DocumentationQuery, ExecutionFlow, ExecutionFlowBuilder,
    ExecutionFlowMetadata, ExecutionPath, FeatureDetails, FeatureDetailsBuilder, FeatureInfo,
    FeatureUsage, LoopStructure, LoopType, ModuleInfo, ModuleInfoBuilder, ParamDoc,
    PlatformMetadata, PlatformRegistry, PlatformRegistryBuilder,
};

// Re-export derive macros from dashflow-derive
// Note: These have different names to avoid conflicts with trait names
pub use dashflow_derive::{GraphState as DeriveGraphState, MergeableState as DeriveMergeableState};

// Re-export the old derive macro for backwards compatibility
pub use dashflow_macros::GraphState as GraphStateDerive;

// Static analysis annotations (used by module discovery / linting)
pub use dashflow_macros::capability;

// Hidden module for macro usage - DO NOT USE DIRECTLY
#[doc(hidden)]
pub mod __private {
    pub use crate::reducer;
}

#[cfg(feature = "dashstream")]
pub use dashstream_callback::{DashStreamCallback, DashStreamConfig, DEFAULT_MAX_STATE_DIFF_SIZE};

// Prelude for common imports in production code
pub mod prelude;

// Test prelude for common imports in test modules
#[cfg(test)]
pub mod test_prelude;

#[cfg(test)]
pub(crate) mod test_support;
