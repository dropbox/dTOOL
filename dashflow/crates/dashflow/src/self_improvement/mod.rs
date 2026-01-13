// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @dashflow-module
//! @name self_improvement
//! @category runtime
//! @status stable
//!
//! # Self-Improving Introspection System
//!
//! A structured system for AI self-improvement through introspection, multi-model consensus,
//! hypothesis tracking, and meta-analysis. The AI examines its own execution, identifies
//! improvements, validates with external models, generates execution plans, and learns
//! from the history of its own improvement attempts.
//!
//! ## Design Principle: Full Opt-In by Default, Opt-Out Only
//!
//! Everything is ON by default. Users disable what they don't want.
//! DashFlow is maximally helpful out of the box.
//!
//! ## Core Components
//!
//! - **Types** (`types.rs`): Core data structures for introspection reports,
//!   capability gaps, deprecation recommendations, execution plans, and hypotheses.
//!
//! - **Storage** (`storage.rs`): File-based storage in `.dashflow/introspection/`
//!   with JSON and markdown formats for both machine and human consumption.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use dashflow::self_improvement::{
//!     IntrospectionReport, IntrospectionScope, IntrospectionStorage,
//!     CapabilityGap, GapCategory, GapManifestation, Impact,
//!     ExecutionPlan, PlanCategory, ImplementationStep,
//! };
//!
//! // Create a new introspection report
//! let mut report = IntrospectionReport::new(IntrospectionScope::System);
//!
//! // Add a capability gap
//! report.add_capability_gap(
//!     CapabilityGap::new(
//!         "Missing sentiment analysis tool",
//!         GapCategory::MissingTool {
//!             tool_description: "Analyze customer sentiment from text".to_string(),
//!         },
//!         GapManifestation::PromptWorkarounds {
//!             patterns: vec!["Based on word choice...".to_string()],
//!         },
//!     )
//!     .with_solution("Add SentimentAnalysisTool node")
//!     .with_impact(Impact::high("Improve accuracy and reduce latency"))
//!     .with_confidence(0.85),
//! );
//!
//! // Generate markdown report
//! let markdown = report.to_markdown();
//!
//! // Save to storage
//! let storage = IntrospectionStorage::default();
//! storage.initialize()?;
//! storage.save_report(&report)?;
//!
//! // Create an execution plan
//! let plan = ExecutionPlan::new("Add sentiment analysis", PlanCategory::ApplicationImprovement)
//!     .with_description("Add a sentiment analysis tool to the graph")
//!     .with_priority(1)
//!     .with_steps(vec![
//!         ImplementationStep::new(1, "Create sentiment module")
//!             .with_files(vec!["src/tools/sentiment.rs".to_string()])
//!             .with_verification("cargo test"),
//!     ])
//!     .with_success_criteria(vec!["Accuracy >= 90%".to_string()])
//!     .validated(0.85);
//!
//! storage.save_plan(&plan)?;
//! ```
//!
//! ## Storage Structure
//!
//! ```text
//! .dashflow/
//! └── introspection/
//!     ├── reports/           # Introspection reports (JSON + markdown)
//!     ├── plans/
//!     │   ├── pending/       # Plans awaiting approval
//!     │   ├── approved/      # Plans approved for implementation
//!     │   ├── implemented/   # Successfully implemented plans
//!     │   └── failed/        # Failed or superseded plans
//!     ├── hypotheses/
//!     │   ├── active/        # Active hypotheses being tracked
//!     │   └── evaluated/     # Evaluated hypotheses
//!     └── meta/              # Meta-analysis data
//! ```
//!
//! ## Reference
//!
//! See `archive/roadmaps/ROADMAP_SELF_IMPROVEMENT.md` for design documentation (archived).
//! See `DESIGN_INVARIANTS.md` Invariant 7 for architectural constraints.

mod analyzers;
mod audit;
mod config;
mod consensus;
mod daemon;
mod error;
mod export_import;
mod health;
mod integration;
mod meta_analysis;
pub mod metrics;
pub mod parallel_analysis;
mod planners;
mod plugins;
mod redaction;
mod storage;
mod streaming_consumer;
mod test_generation;
pub mod testing;
mod trace_retention;
mod traits;
mod types;

// Consolidated modules
pub mod observability; // alerts + events + logging
pub mod performance; // cache + lazy_loading
pub mod resilience; // circuit_breaker + rate_limiter

// Re-export submodules for backwards compatibility
pub mod logging {
    //! Re-export from observability::logging for backwards compatibility.
    pub use super::observability::logging::*;
}
pub mod lazy_loading {
    //! Re-export from performance::lazy_loading for backwards compatibility.
    pub use super::performance::lazy_loading::*;
}

// Re-export all public types
pub use analyzers::{
    // Capability gap analysis
    CapabilityGapAnalyzer,
    CapabilityGapConfig,
    // Deprecation analysis
    DeprecationAnalyzer,
    DeprecationConfig,
    DetectedPattern,
    PatternConfig,
    // Pattern detection
    PatternDetector,
    PatternType,
    // Retrospective analysis
    RetrospectiveAnalyzer,
    RetrospectiveConfig,
};
pub use audit::{
    AuditAction,
    AuditEvent,
    AuditLog,
    AuditQueryBuilder,
    AuditSeverity,
    AuditStats,
    // Audit logging
    Auditable,
};
pub use config::{
    // Unified configuration
    ConfigValidationError,
    SelfImprovementConfig,
    SelfImprovementConfigBuilder,
};
pub use consensus::{
    // Connection pooling
    create_http_client,
    create_shared_reviewer_client,
    AnthropicReviewer,
    // Consensus builder
    ConsensusBuilder,
    ExecutionContext,
    GoogleReviewer,
    MockReviewer,
    // ModelReviewer trait and implementations
    ModelReviewer,
    OpenAIReviewer,
    ReviewFocus,
    // Review request types
    ReviewRequest,
};
pub use daemon::{
    // CLI
    run_daemon_cli,
    // Daemon
    AnalysisDaemon,
    AnalysisTriggerType,
    DaemonConfig,
    DaemonCycleResult,
    FiredTrigger,
    // Triggers
    HighErrorRateConfig,
    HighErrorRateTrigger,
    // Metrics source
    MetricsSource,
    RepeatedRetryConfig,
    RepeatedRetryTrigger,
    SlowNodeConfig,
    SlowNodeTrigger,
    UnusedCapabilityConfig,
    UnusedCapabilityTrigger,
};
pub use error::{
    // Unified error type
    Result as SelfImprovementResult,
    SelfImprovementError,
};
pub use export_import::{
    export_introspection,
    import_introspection,
    // Export/Import API
    ArchiveMetadata,
    ConflictResolution,
    ExportConfig,
    ImportConfig,
    ImportResult,
    IntrospectionArchive,
    ARCHIVE_VERSION,
};
pub use health::{
    // Health checks
    ComponentHealth,
    HealthCheck,
    HealthCheckConfig,
    HealthChecker,
    HealthLevel,
    SystemHealth,
};
pub use integration::{
    approve_plan_cli,
    list_plans_cli,
    // CLI support
    run_cli_introspection,
    // Dasher integration
    DasherIntegration,
    ExecutionStats,
    ImplementationStatus,
    ImplementationSummary,
    // Orchestrator
    IntrospectionOrchestrator,
    IntrospectionResult,
    OrchestratorConfig,
    TriggerConfig,
    TriggerReason,
    // Trigger system
    TriggerSystem,
};
pub use meta_analysis::{
    DeadEnd,
    DesignNote,
    // Design notes
    DesignNoteGenerator,
    DesignNoteGeneratorConfig,
    HypothesisAccuracy,
    // Hypothesis tracking
    HypothesisTracker,
    HypothesisTrackerConfig,
    ImprovementMomentum,
    MetaAnalysisResult,
    // Meta-analysis
    MetaAnalyzer,
    MetaAnalyzerConfig,
    NoteCategory,
    PatternCategory,
    RecurringPattern,
    SourceAccuracy,
    SuccessPattern,
};
pub use metrics::{
    // Self-improvement metrics
    record_analysis_duration,
    record_cache_hit,
    record_cache_miss,
    record_cycle_complete,
    record_error,
    record_plan_approved,
    record_plan_failed,
    record_plan_generated,
    record_plan_implemented,
    record_storage_operation,
    record_trigger_fired,
    update_cache_size,
    update_plan_counts,
    update_storage_items,
    update_storage_size,
    SelfImprovementMetrics,
};
pub use observability::alerts::{
    // CLI
    run_alerts_daemon_cli,
    // Alert types
    Alert,
    AlertDispatcher,
    AlertError,
    AlertHandler,
    AlertSeverity,
    // Built-in handlers
    ConsoleAlertHandler,
    FileAlertHandler,
    WebhookAlertHandler,
};
pub use observability::events::{
    global_event_bus,
    publish as publish_event,
    subscribe as subscribe_event,
    subscribe_all as subscribe_all_events,
    // Event pub/sub system
    Event,
    EventBus,
    EventData,
    EventHandler,
    EventType,
};
pub use observability::logging::{
    create_span,
    // Debug mode
    debug_data,
    debug_decision,
    debug_log,
    debug_plan_processing,
    debug_storage_details,
    debug_timing,
    debug_trigger_evaluation,
    is_debug_mode,
    log_alert_dispatch_failure,
    log_cache_access,
    log_consensus_review,
    log_daemon_cycle,
    log_operation_failure,
    log_operation_start,
    log_operation_success,
    log_plan_status_change,
    log_storage_operation,
    log_trace_load_warning,
    log_trace_parse_warning,
    log_trigger_fired,
    print_debug_banner,
    recommended_log_level,
    // Structured logging
    Component as LoggingComponent,
    DEBUG_ENV_VAR,
};
pub use performance::cache::{MetricsCache, DEFAULT_CACHE_CAPACITY, DEFAULT_RECENT_WINDOW};
pub use planners::{
    // Plan generation
    PlanGenerator,
    PlanGeneratorConfig,
    PlanSummary,
    // Plan tracking
    PlanTracker,
    PlanTrackerConfig,
    // Plan validation
    PlanValidator,
    PlanValidatorConfig,
    SeverityWeights,
    ValidationResult,
};
pub use plugins::{
    // Plugin architecture
    AnalyzerResult,
    PluginCategory,
    PluginInfo,
    PluginManager,
};
pub use redaction::{
    // Sensitive data redaction
    CustomPattern,
    RedactionConfig,
    RedactionStats,
    SensitiveDataRedactor,
};
pub use resilience::circuit_breaker::{
    // Circuit breaker pattern
    CircuitBreaker,
    CircuitBreakerConfig,
    CircuitBreakerError,
    CircuitBreakerRegistry,
    CircuitBreakerStats,
    CircuitOpenError,
    CircuitState,
    // Default configurations
    API_CIRCUIT_CONFIG,
    PROMETHEUS_CIRCUIT_CONFIG,
    WEBHOOK_CIRCUIT_CONFIG,
};
pub use resilience::rate_limiter::{
    // Rate limiting
    RateLimiter,
    RateLimiterConfig,
    RateLimiterStats,
};
pub use storage::{
    DegradationFailure,
    // Graceful degradation
    DegradedComponent,
    DegradedMode,
    DegradedResult,
    // Storage
    IntrospectionStorage,
    // Schema versioning
    MigrationError,
    MigrationResult,
    MigrationStep,
    // Plan index
    PlanIndex,
    // JSON schema validation
    SchemaGenerator,
    SchemaMigrator,
    SchemaValidationError,
    SchemaValidationResult,
    StorageCleanupStats,
    // Health monitoring
    StorageHealthLevel,
    StorageHealthStatus,
    StoragePolicy,
    StorageStats,
    VersionedData,
    DEFAULT_HYPOTHESIS_ARCHIVE_AGE_DAYS,
    DEFAULT_INTROSPECTION_DIR,
    DEFAULT_MAX_PLANS_PER_STATUS,
    DEFAULT_MAX_REPORTS,
    DEFAULT_PLAN_ARCHIVE_AGE_DAYS,
    DEFAULT_PLAN_WARNING_COUNT,
    DEFAULT_REPORT_WARNING_COUNT,
    DEFAULT_STORAGE_CRITICAL_SIZE_BYTES,
    DEFAULT_STORAGE_WARNING_SIZE_BYTES,
    MIN_SUPPORTED_SCHEMA_VERSION,
    SCHEMA_VERSION,
};
#[cfg(feature = "dashstream")]
pub use streaming_consumer::{convert_dashstream_message, start_streaming_consumer};
pub use streaming_consumer::{
    // Streaming consumer for real-time self-improvement
    SelfImprovementConsumer,
    StreamingConsumerConfig,
    StreamingMessage,
    StreamingMetricsWindow,
};
pub use test_generation::{
    // CLI
    run_test_generation_cli,
    // Test generation
    GeneratedTest,
    OutputFormat,
    TestExpectations,
    TestGenerationConfig,
    TestGenerationResult,
    TestGenerator,
    TestInput,
};
pub use trace_retention::{
    // Trace retention
    cleanup_default_traces,
    cleanup_traces,
    // Compression
    decompress_trace,
    read_trace_file,
    CleanupStats,
    RetentionPolicy,
    TraceDirectoryStats,
    TraceRetentionManager,
    DEFAULT_COMPRESS_AGE_DAYS,
    DEFAULT_MAX_AGE_DAYS,
    DEFAULT_MAX_SIZE_BYTES,
    DEFAULT_MAX_TRACES,
};
pub use traits::{
    AnalysisOutput,
    // Extensibility traits
    Analyzer,
    AnalyzerContext,
    AnalyzerOutputType,
    AnalyzerRegistry,
    Planner,
    PlannerInput,
    PlannerInputType,
    PlannerOutput,
    PlannerRegistry,
    // Generic storage trait
    Storable,
    StorageBackend,
    StorageCleanupResult,
    StorageStatistics,
};
pub use types::{
    // Auto-apply types
    ActionType,
    // Analysis depth
    AnalysisDepth,
    ApplyResult,

    Assessment,
    // Capability gaps
    CapabilityGap,
    // Citations
    Citation,
    CitationRetrieval,

    CitationSource,
    ComparisonMetrics,

    ConfigChange,
    ConfigChangeType,
    // Consensus
    ConsensusResult,
    Counterfactual,
    Critique,
    CritiqueSeverity,
    // Deprecation
    DeprecationRecommendation,
    DeprecationTarget,

    Disagreement,

    EvaluationTrigger,
    // Execution plans
    ExecutionPlan,
    ExpectedEvidence,
    GapCategory,
    GapManifestation,
    // Hypotheses
    Hypothesis,
    HypothesisOutcome,
    HypothesisSource,
    HypothesisStatus,
    Impact,
    ImplementationStep,

    // Proposals
    ImprovementProposal,
    // Core report
    IntrospectionReport,
    IntrospectionScope,
    MissingToolAnalysis,

    ModelIdentifier,
    ModelReview,
    ObservedEvidence,

    PlanAction,
    PlanCategory,
    PlanStatus,
    Priority,

    ProposalSource,

    ReportExecutionSummary,
    // Retrospective
    RetrospectiveAnalysis,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all expected types are accessible
        let _report = IntrospectionReport::new(IntrospectionScope::System);
        let _gap = CapabilityGap::new(
            "test",
            GapCategory::MissingTool {
                tool_description: "test".to_string(),
            },
            GapManifestation::Errors {
                count: 0,
                sample_messages: vec![],
            },
        );
        let _plan = ExecutionPlan::new("test", PlanCategory::ApplicationImprovement);
        let _hypothesis = Hypothesis::new("test", "test");
        let _citation = Citation::trace("test-thread");
        let _storage = IntrospectionStorage::default();

        // Verify analyzer types are accessible
        let _gap_analyzer = CapabilityGapAnalyzer::default();
        let _gap_config = CapabilityGapConfig::default();
        let _dep_analyzer = DeprecationAnalyzer::default();
        let _dep_config = DeprecationConfig::default();
        let _retro_analyzer = RetrospectiveAnalyzer::default();
        let _retro_config = RetrospectiveConfig::default();
        let _pattern_detector = PatternDetector::default();
        let _pattern_config = PatternConfig::default();

        // Verify consensus types are accessible
        let _mock_reviewer = MockReviewer::agreeing("test-model");
        let _consensus_builder = ConsensusBuilder::new();
        let _review_request = ReviewRequest::new(vec![]);
        let _review_focus = ReviewFocus::Feasibility;

        // Verify planner types are accessible
        let _plan_generator = PlanGenerator::default();
        let _plan_gen_config = PlanGeneratorConfig::default();
        let _plan_validator = PlanValidator::default();
        let _plan_val_config = PlanValidatorConfig::default();
        let _severity_weights = SeverityWeights::default();
        let _plan_summary = PlanSummary::default();
        let _plan_tracker_config = PlanTrackerConfig::default();

        // Verify meta-analysis types are accessible
        let _hyp_tracker_config = HypothesisTrackerConfig::default();
        let _hyp_accuracy = HypothesisAccuracy::default();
        let _meta_analyzer_config = MetaAnalyzerConfig::default();
        let _momentum = ImprovementMomentum::default();
        let _design_note_config = DesignNoteGeneratorConfig::default();

        // Verify integration types are accessible
        let _trigger_config = TriggerConfig::default();
        let _trigger_system = TriggerSystem::default();
        let _orch_config = OrchestratorConfig::default();
        let _impl_summary = ImplementationSummary::default();
    }

    #[test]
    fn test_integration_workflow() {
        // Simulate a complete introspection workflow
        let mut report = IntrospectionReport::new(IntrospectionScope::GraphAggregate {
            graph_id: "customer_service_bot".to_string(),
            execution_count: 47,
        });

        // Set execution summary
        report.execution_summary.total_executions = 47;
        report.execution_summary.successful_executions = 42;
        report.execution_summary.success_rate = 0.894;
        report.execution_summary.retry_rate = 0.085;

        // Add capability gap
        let gap = CapabilityGap::new(
            "Missing sentiment analysis tool",
            GapCategory::MissingTool {
                tool_description: "Analyze customer sentiment".to_string(),
            },
            GapManifestation::PromptWorkarounds {
                patterns: vec!["Based on the customer's word choice...".to_string()],
            },
        )
        .with_evidence(vec![
            Citation::trace("trace-001"),
            Citation::trace("trace-017"),
        ])
        .with_solution("Add SentimentAnalysisTool node with pre-trained model")
        .with_impact(Impact::high("Reduce retry rate by ~3%, improve accuracy"))
        .with_confidence(0.85);

        report.add_capability_gap(gap);

        // Create execution plan from the gap
        let plan = ExecutionPlan::new(
            "Add Sentiment Analysis Tool",
            PlanCategory::ApplicationImprovement,
        )
        .with_description("Add a sentiment analysis tool to reduce prompt workarounds")
        .with_priority(1)
        .with_estimated_commits(2)
        .with_steps(vec![
            ImplementationStep::new(1, "Create SentimentAnalysisTool in src/tools/sentiment.rs")
                .with_files(vec!["src/tools/sentiment.rs".to_string()])
                .with_verification("cargo test sentiment"),
            ImplementationStep::new(2, "Add node to graph after message_parser")
                .with_files(vec!["src/graph.rs".to_string()])
                .with_verification("integration tests"),
        ])
        .with_success_criteria(vec![
            "Sentiment accuracy >= 90% on test set".to_string(),
            "Latency reduction >= 150ms".to_string(),
            "Retry rate reduction >= 2%".to_string(),
        ])
        .with_rollback_plan("Remove sentiment node, revert to prompt heuristics")
        .validated(0.85);

        report.add_execution_plan(plan);

        // Create hypothesis
        let hypothesis = Hypothesis::new(
            "Retry rate will drop below 6%",
            "34% of retries are sentiment-related. Removing prompt heuristics should eliminate most.",
        )
        .with_expected_evidence(vec![
            ExpectedEvidence::new("retry_rate", "< 6%", "Measure over 50 executions"),
            ExpectedEvidence::new("sentiment_retries", "< 1%", "Count sentiment-related retries"),
        ])
        .with_trigger(EvaluationTrigger::AfterExecutions(50));

        report.add_hypothesis(hypothesis);

        // Verify report
        assert_eq!(report.capability_gaps.len(), 1);
        assert_eq!(report.execution_plans.len(), 1);
        assert_eq!(report.hypotheses.len(), 1);
        assert!(!report.citations.is_empty());

        // Generate markdown
        let md = report.to_markdown();
        assert!(md.contains("Introspection Report"));
        assert!(md.contains("sentiment analysis"));
        assert!(md.contains("Retry rate will drop below 6%"));

        // Generate JSON
        let json = report.to_json().unwrap();
        let parsed = IntrospectionReport::from_json(&json).unwrap();
        assert_eq!(parsed.id, report.id);
    }
}
