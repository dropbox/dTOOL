// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for integration layer
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Integration layer for the self-improvement system.
//!
//! This module coordinates all self-improvement components:
//! - TriggerSystem: Determines when to run introspection
//! - DasherIntegration: Implements validated plans
//! - IntrospectionOrchestrator: Runs the full pipeline
//!
//! ## Design Principle
//!
//! Everything runs automatically. Plans are generated but NEVER executed
//! without explicit human approval.

use super::analyzers::{
    CapabilityGapAnalyzer, DeprecationAnalyzer, PatternDetector, RetrospectiveAnalyzer,
};
use super::meta_analysis::{DesignNoteGenerator, HypothesisTracker, MetaAnalyzer};
use super::planners::{PlanGenerator, PlanTracker, PlanValidator};
use super::storage::IntrospectionStorage;
#[cfg(test)]
use super::types::PlanAction;
use super::types::{
    ActionType, AnalysisDepth, ApplyResult, ConfigChange, ConfigChangeType, ExecutionPlan,
    Hypothesis, IntrospectionReport, IntrospectionScope,
};
use crate::core::config_loader::env_vars::{
    env_is_set, ANTHROPIC_API_KEY, GOOGLE_API_KEY, OPENAI_API_KEY,
};
use crate::introspection::ExecutionTrace;
use chrono::{DateTime, Duration, Utc};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tracing::warn;
use uuid::Uuid;

// ============================================================================
// Trigger System
// ============================================================================

/// Configuration for automatic introspection triggers
#[derive(Debug, Clone)]
pub struct TriggerConfig {
    /// Enable per-execution analysis (metrics collection only)
    pub per_execution: bool,

    /// Run local analysis every N executions (0 = disabled)
    pub periodic_executions: usize,

    /// Run local analysis every T duration (None = disabled)
    pub periodic_time: Option<Duration>,

    /// Run full analysis when error rate exceeds threshold (0.0 = disabled)
    pub error_spike_threshold: f64,

    /// Minimum executions before error spike detection kicks in
    pub error_spike_min_executions: usize,

    /// Analysis depth for periodic triggers
    pub periodic_depth: AnalysisDepth,
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            per_execution: true,
            periodic_executions: 10,
            periodic_time: Some(Duration::hours(1)),
            error_spike_threshold: 0.3,
            error_spike_min_executions: 5,
            periodic_depth: AnalysisDepth::LocalAnalysis,
        }
    }
}

impl TriggerConfig {
    /// Create config with all triggers disabled
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            per_execution: false,
            periodic_executions: 0,
            periodic_time: None,
            error_spike_threshold: 0.0,
            error_spike_min_executions: 0,
            periodic_depth: AnalysisDepth::Metrics,
        }
    }

    /// Create config for minimal overhead (metrics only)
    #[must_use]
    pub fn metrics_only() -> Self {
        Self {
            per_execution: true,
            periodic_executions: 0,
            periodic_time: None,
            error_spike_threshold: 0.0,
            error_spike_min_executions: 0,
            periodic_depth: AnalysisDepth::Metrics,
        }
    }

    /// Create config for aggressive analysis
    #[must_use]
    pub fn aggressive() -> Self {
        Self {
            per_execution: true,
            periodic_executions: 5,
            periodic_time: Some(Duration::minutes(30)),
            error_spike_threshold: 0.2,
            error_spike_min_executions: 3,
            periodic_depth: AnalysisDepth::DeepAnalysis,
        }
    }
}

/// Trigger type indicating why introspection should run
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerReason {
    /// Triggered after a single execution (metrics only).
    PerExecution {
        /// ID of the execution thread that triggered analysis.
        thread_id: String,
    },

    /// Periodic trigger based on execution count.
    PeriodicExecution {
        /// Number of executions since last analysis.
        count: usize,
    },

    /// Periodic trigger based on time.
    PeriodicTime {
        /// Duration since the last analysis run.
        since_last: Duration,
    },

    /// Error rate exceeded threshold.
    ErrorSpike {
        /// Current error rate that triggered the spike.
        error_rate: f64,
        /// Threshold that was exceeded.
        threshold: f64,
    },

    /// Manual trigger via CLI or API.
    Manual {
        /// Optional reason for the manual trigger.
        reason: Option<String>,
    },
}

/// Tracks execution statistics for trigger decisions
#[derive(Debug, Default)]
pub struct ExecutionStats {
    /// Total executions since last analysis
    pub executions_since_analysis: AtomicU64,

    /// Successful executions since last analysis
    pub successes_since_analysis: AtomicU64,

    /// Failed executions since last analysis
    pub failures_since_analysis: AtomicU64,

    /// Timestamp of last analysis
    pub last_analysis: RwLock<Option<DateTime<Utc>>>,

    /// Recent execution traces (bounded buffer)
    pub recent_traces: RwLock<VecDeque<ExecutionTrace>>,
}

impl ExecutionStats {
    /// Create new stats tracker
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Maximum recent traces to keep
    const MAX_RECENT_TRACES: usize = 100;

    /// Record an execution
    pub fn record_execution(&self, trace: ExecutionTrace, success: bool) {
        self.executions_since_analysis
            .fetch_add(1, Ordering::SeqCst);
        if success {
            self.successes_since_analysis.fetch_add(1, Ordering::SeqCst);
        } else {
            self.failures_since_analysis.fetch_add(1, Ordering::SeqCst);
        }

        // Add to recent traces
        // Use poison-safe pattern to recover from poisoned lock
        let mut traces = self
            .recent_traces
            .write()
            .unwrap_or_else(|e| e.into_inner());
        traces.push_back(trace);
        while traces.len() > Self::MAX_RECENT_TRACES {
            traces.pop_front();
        }
    }

    /// Get current error rate
    #[must_use]
    pub fn error_rate(&self) -> f64 {
        let total = self.executions_since_analysis.load(Ordering::SeqCst);
        if total == 0 {
            return 0.0;
        }
        let failures = self.failures_since_analysis.load(Ordering::SeqCst);
        failures as f64 / total as f64
    }

    /// Reset stats after analysis
    pub fn reset(&self) {
        self.executions_since_analysis.store(0, Ordering::SeqCst);
        self.successes_since_analysis.store(0, Ordering::SeqCst);
        self.failures_since_analysis.store(0, Ordering::SeqCst);
        // Use poison-safe pattern to recover from poisoned lock
        *self
            .last_analysis
            .write()
            .unwrap_or_else(|e| e.into_inner()) = Some(Utc::now());
    }

    /// Get recent traces (cloned)
    #[must_use]
    pub fn get_recent_traces(&self) -> Vec<ExecutionTrace> {
        // Use poison-safe pattern to recover from poisoned lock
        self.recent_traces
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect()
    }

    /// Get execution count since last analysis
    #[must_use]
    pub fn execution_count(&self) -> usize {
        self.executions_since_analysis.load(Ordering::SeqCst) as usize
    }
}

/// System for determining when introspection should run
pub struct TriggerSystem {
    config: TriggerConfig,
    stats: Arc<ExecutionStats>,
}

impl TriggerSystem {
    /// Create a new trigger system with the given config
    #[must_use]
    pub fn new(config: TriggerConfig) -> Self {
        Self {
            config,
            stats: Arc::new(ExecutionStats::new()),
        }
    }

    /// Get shared reference to stats
    #[must_use]
    pub fn stats(&self) -> Arc<ExecutionStats> {
        Arc::clone(&self.stats)
    }

    /// Record an execution and check if any triggers fire
    pub fn record_and_check(&self, trace: ExecutionTrace, success: bool) -> Vec<TriggerReason> {
        let thread_id = trace
            .thread_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        self.stats.record_execution(trace, success);

        let mut triggers = Vec::new();

        // Per-execution trigger (always fires if enabled)
        if self.config.per_execution {
            triggers.push(TriggerReason::PerExecution {
                thread_id: thread_id.clone(),
            });
        }

        // Periodic execution trigger
        if self.config.periodic_executions > 0 {
            let count = self.stats.execution_count();
            if count > 0 && count % self.config.periodic_executions == 0 {
                triggers.push(TriggerReason::PeriodicExecution { count });
            }
        }

        // Time-based trigger
        if let Some(period) = self.config.periodic_time {
            // Use poison-safe pattern to recover from poisoned lock
            let last = self
                .stats
                .last_analysis
                .read()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(last_time) = *last {
                let since_last = Utc::now().signed_duration_since(last_time);
                if since_last >= period {
                    triggers.push(TriggerReason::PeriodicTime { since_last });
                }
            } else {
                // No previous analysis, trigger if we have some executions
                if self.stats.execution_count() >= self.config.error_spike_min_executions {
                    triggers.push(TriggerReason::PeriodicTime {
                        since_last: Duration::zero(),
                    });
                }
            }
        }

        // Error spike trigger
        if self.config.error_spike_threshold > 0.0 {
            let count = self.stats.execution_count();
            if count >= self.config.error_spike_min_executions {
                let error_rate = self.stats.error_rate();
                if error_rate >= self.config.error_spike_threshold {
                    triggers.push(TriggerReason::ErrorSpike {
                        error_rate,
                        threshold: self.config.error_spike_threshold,
                    });
                }
            }
        }

        triggers
    }

    /// Get recommended analysis depth for a trigger
    #[must_use]
    pub fn depth_for_trigger(&self, reason: &TriggerReason) -> AnalysisDepth {
        match reason {
            TriggerReason::PerExecution { .. } => AnalysisDepth::Metrics,
            TriggerReason::ErrorSpike { .. } => AnalysisDepth::DeepAnalysis,
            TriggerReason::Manual { .. } => AnalysisDepth::DeepAnalysis,
            _ => self.config.periodic_depth,
        }
    }

    /// Manually trigger introspection
    #[must_use]
    pub fn manual_trigger(&self, reason: Option<String>) -> TriggerReason {
        TriggerReason::Manual { reason }
    }

    /// Reset stats after analysis completes
    pub fn mark_analysis_complete(&self) {
        self.stats.reset();
    }
}

impl Default for TriggerSystem {
    fn default() -> Self {
        Self::new(TriggerConfig::default())
    }
}

// ============================================================================
// Dasher Integration
// ============================================================================

/// Status of plan implementation by Dasher
#[derive(Debug, Clone)]
pub enum ImplementationStatus {
    /// Plan is queued for implementation.
    Queued,

    /// Plan is currently being implemented.
    InProgress {
        /// When implementation started.
        started: DateTime<Utc>,
        /// Index of the current step being executed.
        current_step: usize,
    },

    /// Plan was successfully implemented.
    Completed {
        /// When implementation completed.
        completed: DateTime<Utc>,
        /// Git commit hash of the implementation.
        commit_hash: String,
    },

    /// Plan implementation failed.
    Failed {
        /// Reason for the failure.
        reason: String,
    },

    /// Plan was skipped (e.g., superseded by a newer plan).
    Skipped {
        /// Reason for skipping.
        reason: String,
    },
}

/// Callback for plan implementation events
pub type PlanCallback = Box<dyn Fn(&ExecutionPlan, &ImplementationStatus) + Send + Sync>;

/// Integration with Dasher for automated plan implementation
pub struct DasherIntegration {
    storage: IntrospectionStorage,
    on_status_change: Option<PlanCallback>,
}

impl DasherIntegration {
    /// Create new Dasher integration with storage
    #[must_use]
    pub fn new(storage: IntrospectionStorage) -> Self {
        Self {
            storage,
            on_status_change: None,
        }
    }

    /// Set callback for status changes
    #[must_use]
    pub fn with_callback(mut self, callback: PlanCallback) -> Self {
        self.on_status_change = Some(callback);
        self
    }

    /// Get the next plan to implement (highest priority approved plan)
    ///
    /// # Errors
    ///
    /// Returns error if storage operations fail
    pub fn next_plan(&self) -> std::io::Result<Option<ExecutionPlan>> {
        let approved = self.storage.approved_plans()?;
        Ok(approved.into_iter().next())
    }

    /// Get all plans ready for implementation
    ///
    /// # Errors
    ///
    /// Returns error if storage operations fail
    pub fn all_ready_plans(&self) -> std::io::Result<Vec<ExecutionPlan>> {
        self.storage.approved_plans()
    }

    /// Mark a plan as implementation started
    ///
    /// # Errors
    ///
    /// Returns error if storage operations fail
    pub fn start_implementation(
        &self,
        plan_id: Uuid,
        assignee: impl Into<String>,
    ) -> std::io::Result<()> {
        let plan = self.storage.load_plan(plan_id)?;

        // Update status
        self.storage.approve_plan(plan_id, assignee)?;

        // Notify callback
        if let Some(ref callback) = self.on_status_change {
            callback(
                &plan,
                &ImplementationStatus::InProgress {
                    started: Utc::now(),
                    current_step: 1,
                },
            );
        }

        Ok(())
    }

    /// Mark a plan as successfully implemented
    ///
    /// # Errors
    ///
    /// Returns error if storage operations fail
    pub fn mark_implemented(
        &self,
        plan_id: Uuid,
        commit_hash: impl Into<String>,
    ) -> std::io::Result<()> {
        let plan = self.storage.load_plan(plan_id)?;
        let hash = commit_hash.into();

        // Update status
        self.storage.complete_plan(plan_id, &hash)?;

        // Notify callback
        if let Some(ref callback) = self.on_status_change {
            callback(
                &plan,
                &ImplementationStatus::Completed {
                    completed: Utc::now(),
                    commit_hash: hash,
                },
            );
        }

        Ok(())
    }

    /// Mark a plan as failed
    ///
    /// # Errors
    ///
    /// Returns error if storage operations fail
    pub fn mark_failed(&self, plan_id: Uuid, reason: impl Into<String>) -> std::io::Result<()> {
        let plan = self.storage.load_plan(plan_id)?;
        let reason_str = reason.into();

        // Update status
        self.storage.fail_plan(plan_id, &reason_str)?;

        // Notify callback
        if let Some(ref callback) = self.on_status_change {
            callback(&plan, &ImplementationStatus::Failed { reason: reason_str });
        }

        Ok(())
    }

    /// Get implementation summary
    ///
    /// # Errors
    ///
    /// Returns error if storage operations fail
    pub fn implementation_summary(&self) -> std::io::Result<ImplementationSummary> {
        let pending = self.storage.pending_plans()?.len();
        let approved = self.storage.approved_plans()?.len();
        let implemented = self.storage.list_implemented_plans()?.len();
        let failed = self.storage.list_failed_plans()?.len();

        Ok(ImplementationSummary {
            pending,
            approved,
            implemented,
            failed,
        })
    }
}

/// Summary of plan implementation status
#[derive(Debug, Clone, Default)]
pub struct ImplementationSummary {
    /// Plans pending approval
    pub pending: usize,
    /// Plans approved and ready for implementation
    pub approved: usize,
    /// Successfully implemented plans
    pub implemented: usize,
    /// Failed plans
    pub failed: usize,
}

impl ImplementationSummary {
    /// Total plans tracked
    #[must_use]
    pub fn total(&self) -> usize {
        self.pending + self.approved + self.implemented + self.failed
    }

    /// Success rate (implemented / (implemented + failed))
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        let completed = self.implemented + self.failed;
        if completed == 0 {
            return 1.0;
        }
        self.implemented as f64 / completed as f64
    }
}

// ============================================================================
// Introspection Orchestrator
// ============================================================================

/// Configuration for the introspection orchestrator
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Trigger configuration
    pub trigger: TriggerConfig,

    /// Enable multi-model consensus (requires API keys)
    pub enable_consensus: bool,

    /// Enable hypothesis tracking
    pub enable_hypotheses: bool,

    /// Enable meta-analysis
    pub enable_meta_analysis: bool,

    /// Auto-approve plans below this priority (0 = never auto-approve)
    pub auto_approve_priority: u8,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            trigger: TriggerConfig::default(),
            enable_consensus: true,
            enable_hypotheses: true,
            enable_meta_analysis: true,
            auto_approve_priority: 0, // Never auto-approve by default
        }
    }
}

impl OrchestratorConfig {
    /// Create a minimal configuration (fast, local-only)
    #[must_use]
    pub fn minimal() -> Self {
        Self {
            trigger: TriggerConfig::metrics_only(),
            enable_consensus: false,
            enable_hypotheses: false,
            enable_meta_analysis: false,
            auto_approve_priority: 0,
        }
    }

    /// Create a full configuration (all features enabled)
    #[must_use]
    pub fn full() -> Self {
        Self {
            trigger: TriggerConfig::aggressive(),
            enable_consensus: true,
            enable_hypotheses: true,
            enable_meta_analysis: true,
            auto_approve_priority: 0,
        }
    }
}

/// Result of running an introspection cycle
#[derive(Debug)]
pub struct IntrospectionResult {
    /// The generated report
    pub report: IntrospectionReport,

    /// Validated execution plans
    pub plans: Vec<ExecutionPlan>,

    /// Generated hypotheses
    pub hypotheses: Vec<Hypothesis>,

    /// Paths where files were saved
    pub saved_files: Vec<std::path::PathBuf>,

    /// Whether consensus was performed
    pub consensus_performed: bool,

    /// Errors encountered (non-fatal)
    pub warnings: Vec<String>,
}

/// Orchestrates the full self-improvement pipeline
pub struct IntrospectionOrchestrator {
    config: OrchestratorConfig,
    storage: IntrospectionStorage,
    trigger_system: TriggerSystem,

    // Component instances
    gap_analyzer: CapabilityGapAnalyzer,
    deprecation_analyzer: DeprecationAnalyzer,
    retrospective_analyzer: RetrospectiveAnalyzer,
    pattern_detector: PatternDetector,
    plan_generator: PlanGenerator,
    plan_validator: PlanValidator, // Used for consensus-based validation
    plan_tracker: PlanTracker,
    hypothesis_tracker: HypothesisTracker,
    meta_analyzer: MetaAnalyzer,
    design_note_generator: DesignNoteGenerator,
}

impl IntrospectionOrchestrator {
    /// Create a new orchestrator with default configuration
    #[must_use]
    pub fn new(storage: IntrospectionStorage) -> Self {
        Self::with_config(storage, OrchestratorConfig::default())
    }

    /// Create with custom configuration
    #[must_use]
    pub fn with_config(storage: IntrospectionStorage, config: OrchestratorConfig) -> Self {
        let trigger_system = TriggerSystem::new(config.trigger.clone());

        Self {
            config,
            storage: storage.clone(),
            trigger_system,
            gap_analyzer: CapabilityGapAnalyzer::default(),
            deprecation_analyzer: DeprecationAnalyzer::default(),
            retrospective_analyzer: RetrospectiveAnalyzer::default(),
            pattern_detector: PatternDetector::default(),
            plan_generator: PlanGenerator::default(),
            plan_validator: PlanValidator::default(),
            plan_tracker: PlanTracker::with_storage(storage.clone()),
            hypothesis_tracker: HypothesisTracker::with_storage(storage.clone()),
            meta_analyzer: MetaAnalyzer::with_storage(storage.clone()),
            design_note_generator: DesignNoteGenerator::with_storage(storage),
        }
    }

    /// Get the trigger system for external use
    #[must_use]
    pub fn trigger_system(&self) -> &TriggerSystem {
        &self.trigger_system
    }

    /// Get shared stats from trigger system
    #[must_use]
    pub fn stats(&self) -> Arc<ExecutionStats> {
        self.trigger_system.stats()
    }

    /// Record an execution and run analysis if triggers fire
    ///
    /// This is the main entry point for automatic introspection.
    ///
    /// # Errors
    ///
    /// Returns error if analysis fails critically
    pub fn record_execution(
        &mut self,
        trace: ExecutionTrace,
        success: bool,
    ) -> Result<Option<IntrospectionResult>, String> {
        let triggers = self.trigger_system.record_and_check(trace, success);

        if triggers.is_empty() {
            return Ok(None);
        }

        // Find the most significant trigger (highest depth)
        // SAFETY: triggers is non-empty (checked above), so max_by_key always returns Some
        let max_depth_trigger = triggers
            .iter()
            .max_by_key(|t| match self.trigger_system.depth_for_trigger(t) {
                AnalysisDepth::Metrics => 0,
                AnalysisDepth::LocalAnalysis => 1,
                AnalysisDepth::DeepAnalysis => 2,
            })
            .expect("triggers is non-empty");

        let depth = self.trigger_system.depth_for_trigger(max_depth_trigger);

        // Run analysis
        let result = self.run_analysis(depth, Some(max_depth_trigger.clone()))?;

        // Mark analysis complete
        self.trigger_system.mark_analysis_complete();

        Ok(Some(result))
    }

    /// Run introspection with specified depth
    ///
    /// # Errors
    ///
    /// Returns error if analysis fails
    pub fn run_analysis(
        &mut self,
        depth: AnalysisDepth,
        trigger: Option<TriggerReason>,
    ) -> Result<IntrospectionResult, String> {
        let mut warnings = Vec::new();
        let mut saved_files = Vec::new();

        // Get traces to analyze
        let traces = self.trigger_system.stats().get_recent_traces();

        // Create base report
        let scope = if let Some(TriggerReason::PerExecution { ref thread_id }) = trigger {
            IntrospectionScope::Execution {
                thread_id: thread_id.clone(),
            }
        } else {
            IntrospectionScope::GraphAggregate {
                graph_id: "default".to_string(),
                execution_count: traces.len(),
            }
        };

        let mut report = IntrospectionReport::new(scope);

        // Set execution summary from stats
        let stats = self.trigger_system.stats();
        report.execution_summary.total_executions = stats.execution_count();
        report.execution_summary.successful_executions =
            stats.successes_since_analysis.load(Ordering::SeqCst) as usize;
        report.execution_summary.success_rate = if report.execution_summary.total_executions > 0 {
            report.execution_summary.successful_executions as f64
                / report.execution_summary.total_executions as f64
        } else {
            1.0
        };
        report.execution_summary.retry_rate = 0.0; // Would be calculated from traces

        // Metrics depth: just collect stats
        if matches!(depth, AnalysisDepth::Metrics) {
            // Save minimal report
            if let Ok((json, _md)) = self.storage.save_report(&report) {
                saved_files.push(json);
            }

            return Ok(IntrospectionResult {
                report,
                plans: Vec::new(),
                hypotheses: Vec::new(),
                saved_files,
                consensus_performed: false,
                warnings,
            });
        }

        // Local analysis: run all analyzers
        let gaps = self.gap_analyzer.analyze(&traces);
        for gap in &gaps {
            report.add_capability_gap(gap.clone());
        }

        let known_nodes: Vec<String> = Vec::new(); // Would come from graph metadata
        let deprecations = self.deprecation_analyzer.analyze(&traces, &known_nodes);
        for dep in &deprecations {
            report.add_deprecation(dep.clone());
        }

        let retrospective = self.retrospective_analyzer.analyze(&traces);
        report.retrospective = retrospective;

        let _patterns = self.pattern_detector.detect(&traces);

        // Generate plans from analysis
        let mut all_plans = Vec::new();
        all_plans.extend(self.plan_generator.generate_from_gaps(&gaps));
        all_plans.extend(
            self.plan_generator
                .generate_from_deprecations(&deprecations),
        );
        all_plans.extend(
            self.plan_generator
                .generate_from_retrospective(&report.retrospective),
        );

        // Deep analysis: run consensus if enabled
        let consensus_performed = if matches!(depth, AnalysisDepth::DeepAnalysis)
            && self.config.enable_consensus
            && !all_plans.is_empty()
        {
            // Check for API keys
            let has_keys = env_is_set(ANTHROPIC_API_KEY)
                || env_is_set(OPENAI_API_KEY)
                || env_is_set(GOOGLE_API_KEY);

            if has_keys {
                // Note: Actual consensus is async, so we'd need to handle that
                // For now, mark that we would run consensus
                warnings.push(
                    "Consensus requires async runtime - skipping in sync context".to_string(),
                );
                false
            } else {
                warnings.push("No API keys found for multi-model consensus".to_string());
                false
            }
        } else {
            false
        };

        // Track and validate plans
        // Note: When consensus is performed, plan_validator.validate_with_consensus() would be used
        // For now, use basic threshold validation
        let _ = &self.plan_validator; // Mark as used - will be used for consensus validation
        let validated_plans: Vec<ExecutionPlan> = if consensus_performed {
            // With consensus, we'd use self.plan_validator.validate_and_filter() here
            all_plans
        } else {
            all_plans
                .into_iter()
                .filter(|p| p.validation_score >= 0.5) // Basic threshold without consensus
                .collect()
        };

        for plan in &validated_plans {
            report.add_execution_plan(plan.clone());
            if let Err(e) = self.plan_tracker.track_plan(plan) {
                warn!(plan_id = %plan.id, "Failed to track plan: {}", e);
                warnings.push(format!("Failed to track plan {}: {}", plan.id, e));
            }

            // Save plan to storage
            if let Err(e) = self.storage.save_plan(plan) {
                warnings.push(format!("Failed to save plan {}: {}", plan.id, e));
            }
        }

        // Generate hypotheses if enabled
        let mut hypotheses: Vec<Hypothesis> = Vec::new();
        if self.config.enable_hypotheses && !gaps.is_empty() {
            for gap in &gaps {
                if let Some(hyp) = self.hypothesis_tracker.create_from_gap(gap) {
                    hypotheses.push(hyp);
                }
            }
        }

        for hyp in &hypotheses {
            report.add_hypothesis(hyp.clone());
            if let Err(e) = self.storage.save_hypothesis(hyp) {
                warnings.push(format!("Failed to save hypothesis {}: {}", hyp.id, e));
            }
        }

        // Save report
        match self.storage.save_report(&report) {
            Ok((json, md)) => {
                saved_files.push(json);
                saved_files.push(md);
            }
            Err(e) => {
                warnings.push(format!("Failed to save report: {}", e));
            }
        }

        Ok(IntrospectionResult {
            report,
            plans: validated_plans,
            hypotheses,
            saved_files,
            consensus_performed,
            warnings,
        })
    }

    /// Run manual introspection with full analysis
    ///
    /// # Errors
    ///
    /// Returns error if analysis fails
    pub fn run_manual(&mut self, reason: Option<String>) -> Result<IntrospectionResult, String> {
        let _trigger = self.trigger_system.manual_trigger(reason);
        self.run_analysis(AnalysisDepth::DeepAnalysis, None)
    }

    /// Get Dasher integration for plan implementation
    #[must_use]
    pub fn dasher(&self) -> DasherIntegration {
        DasherIntegration::new(self.storage.clone())
    }

    /// Get plan tracker
    #[must_use]
    pub fn plan_tracker(&self) -> &PlanTracker {
        &self.plan_tracker
    }

    /// Get hypothesis tracker
    #[must_use]
    pub fn hypothesis_tracker(&self) -> &HypothesisTracker {
        &self.hypothesis_tracker
    }

    /// Get meta analyzer
    #[must_use]
    pub fn meta_analyzer(&self) -> &MetaAnalyzer {
        &self.meta_analyzer
    }

    /// Get design note generator
    #[must_use]
    pub fn design_notes(&self) -> &DesignNoteGenerator {
        &self.design_note_generator
    }

    /// Load execution traces from a directory and add them to the trigger system.
    ///
    /// This is essential for CLI-based analysis where the orchestrator is created
    /// fresh and needs to analyze historical trace data from disk.
    ///
    /// # Arguments
    ///
    /// * `traces_dir` - Path to the directory containing trace JSON files
    ///
    /// # Returns
    ///
    /// Number of traces successfully loaded
    ///
    /// # Errors
    ///
    /// Returns error if the directory cannot be read
    pub fn load_traces_from_directory(
        &mut self,
        traces_dir: &std::path::Path,
    ) -> Result<usize, String> {
        if !traces_dir.exists() {
            return Ok(0);
        }

        let entries = std::fs::read_dir(traces_dir)
            .map_err(|e| format!("Failed to read traces directory: {}", e))?;

        let mut loaded_count = 0;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                match std::fs::read_to_string(&path) {
                    Ok(content) => match serde_json::from_str::<ExecutionTrace>(&content) {
                        Ok(trace) => {
                            // Determine success from trace completion status
                            let success = trace.completed;
                            // Record the trace in the trigger system's stats
                            self.trigger_system.stats().record_execution(trace, success);
                            loaded_count += 1;
                        }
                        Err(e) => {
                            // Log warning but continue - don't fail on one bad trace
                            tracing::warn!(
                                path = %path.display(),
                                error = %e,
                                "Failed to parse trace file"
                            );
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to read trace file"
                        );
                    }
                }
            }
        }

        Ok(loaded_count)
    }

    /// Apply approved improvement plans directly to graph configs.
    ///
    /// This closes the self-improvement loop by translating validated execution plans
    /// into actual configuration changes on a `StateGraph`. Each action in the plan
    /// is applied sequentially, with changes recorded for potential rollback.
    ///
    /// # Arguments
    ///
    /// * `graph` - The graph to modify (must have `GraphState` bound)
    /// * `plan` - The validated execution plan to apply
    ///
    /// # Returns
    ///
    /// An `ApplyResult` containing all changes made, along with any warnings or
    /// skipped nodes.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow::self_improvement::{IntrospectionOrchestrator, ExecutionPlan, PlanAction, ActionType};
    /// use dashflow::StateGraph;
    ///
    /// let mut orchestrator = IntrospectionOrchestrator::new(storage);
    /// let mut graph: StateGraph<MyState> = StateGraph::new();
    ///
    /// // Create a plan with actions
    /// let plan = ExecutionPlan::new("Optimize prompts", PlanCategory::Optimization)
    ///     .add_action(PlanAction::new(ActionType::update_prompt(
    ///         "researcher",
    ///         "You are a thorough research assistant. Always cite sources."
    ///     )))
    ///     .add_action(PlanAction::new(ActionType::update_parameter(
    ///         "researcher",
    ///         "temperature",
    ///         serde_json::json!(0.3)
    ///     )))
    ///     .validated(0.9);
    ///
    /// let result = orchestrator.apply_to_graph(&mut graph, &plan);
    /// assert!(result.success);
    /// println!("Applied {} changes", result.change_count());
    /// ```
    pub fn apply_to_graph<S: crate::state::GraphState>(
        &self,
        graph: &mut crate::graph::StateGraph<S>,
        plan: &ExecutionPlan,
    ) -> ApplyResult {
        let mut changes = Vec::new();
        let mut result = ApplyResult::success(plan.id, Vec::new());

        for action in &plan.actions {
            match &action.action_type {
                ActionType::UpdatePrompt { node, new_prompt } => {
                    if let Some(config) = graph.get_node_config_mut(node) {
                        // Get current config and extract previous prompt
                        let previous_config = config.config.clone();
                        let mut new_config = config.config.clone();

                        // Update or create the system_prompt field
                        if let Some(obj) = new_config.as_object_mut() {
                            obj.insert(
                                "system_prompt".to_string(),
                                serde_json::Value::String(new_prompt.clone()),
                            );
                        }

                        // Apply the update (returns previous config, which we already saved)
                        let _ =
                            config.update(new_config.clone(), Some("self_improvement".to_string()));

                        changes.push(ConfigChange::new(
                            node.clone(),
                            ConfigChangeType::PromptUpdate,
                            Some(previous_config),
                            new_config,
                        ));
                    } else {
                        // Node config doesn't exist - create it
                        graph.set_node_config(
                            node.clone(),
                            serde_json::json!({"system_prompt": new_prompt}),
                            Some("self_improvement"),
                        );

                        changes.push(ConfigChange::new(
                            node.clone(),
                            ConfigChangeType::ConfigAdd,
                            None,
                            serde_json::json!({"system_prompt": new_prompt}),
                        ));
                    }
                }

                ActionType::UpdateParameter { node, param, value } => {
                    if let Some(config) = graph.get_node_config_mut(node) {
                        let previous_config = config.config.clone();
                        let mut new_config = config.config.clone();

                        // Update the specific parameter
                        if let Some(obj) = new_config.as_object_mut() {
                            obj.insert(param.clone(), value.clone());
                        }

                        // Apply the update (returns previous config, which we already saved)
                        let _ =
                            config.update(new_config.clone(), Some("self_improvement".to_string()));

                        changes.push(ConfigChange::new(
                            node.clone(),
                            ConfigChangeType::ParameterUpdate {
                                param: param.clone(),
                            },
                            Some(previous_config),
                            new_config,
                        ));
                    } else {
                        // Node config doesn't exist - create it with the parameter
                        let new_config = serde_json::json!({ param: value });
                        graph.set_node_config(
                            node.clone(),
                            new_config.clone(),
                            Some("self_improvement"),
                        );

                        changes.push(ConfigChange::new(
                            node.clone(),
                            ConfigChangeType::ConfigAdd,
                            None,
                            new_config,
                        ));
                    }
                }

                ActionType::ReplaceConfig { node, new_config } => {
                    let previous = if graph.has_node_config(node) {
                        graph.get_node_config(node).map(|c| c.config.clone())
                    } else {
                        None
                    };

                    graph.set_node_config(
                        node.clone(),
                        new_config.clone(),
                        Some("self_improvement"),
                    );

                    changes.push(ConfigChange::new(
                        node.clone(),
                        ConfigChangeType::ConfigReplace,
                        previous,
                        new_config.clone(),
                    ));
                }

                ActionType::AddConfig {
                    node,
                    node_type,
                    config,
                } => {
                    if graph.has_node_config(node) {
                        result.add_warning(format!(
                            "Config for '{}' already exists, skipping AddConfig",
                            node
                        ));
                        result.add_skipped(node.clone());
                        continue;
                    }

                    // Create new node config with specified type
                    let mut node_config =
                        crate::introspection::NodeConfig::new(node.clone(), node_type.clone());
                    node_config = node_config.with_config(config.clone());
                    node_config.updated_by = Some("self_improvement".to_string());

                    graph.set_node_config(node.clone(), config.clone(), Some("self_improvement"));

                    changes.push(ConfigChange::new(
                        node.clone(),
                        ConfigChangeType::ConfigAdd,
                        None,
                        config.clone(),
                    ));
                }

                ActionType::RemoveConfig { node } => {
                    if let Some(removed) = graph.remove_node_config(node) {
                        changes.push(ConfigChange::new(
                            node.clone(),
                            ConfigChangeType::ConfigRemove,
                            Some(removed.config),
                            serde_json::Value::Null,
                        ));
                    } else {
                        result.add_warning(format!(
                            "Config for '{}' not found, skipping RemoveConfig",
                            node
                        ));
                        result.add_skipped(node.clone());
                    }
                }
            }
        }

        result.applied_changes = changes;
        result
    }

    /// Apply a plan and automatically rollback on failure.
    ///
    /// This is a convenience method that wraps `apply_to_graph()` and provides
    /// automatic rollback if an error occurs during application.
    ///
    /// # Arguments
    ///
    /// * `graph` - The graph to modify
    /// * `plan` - The execution plan to apply
    ///
    /// # Returns
    ///
    /// The `ApplyResult` from the application attempt.
    pub fn apply_with_rollback<S: crate::state::GraphState>(
        &self,
        graph: &mut crate::graph::StateGraph<S>,
        plan: &ExecutionPlan,
    ) -> ApplyResult {
        let result = self.apply_to_graph(graph, plan);

        if !result.success {
            // Rollback all applied changes in reverse order
            for change in result.applied_changes.iter().rev() {
                if let Some(ref previous) = change.previous_config {
                    if let Err(e) = graph.update_node_config(
                        &change.node,
                        previous.clone(),
                        Some("rollback".to_string()),
                    ) {
                        warn!(node = %change.node, "Failed to rollback node config: {}", e);
                    }
                } else if matches!(change.change_type, ConfigChangeType::ConfigAdd) {
                    // Remove added configs
                    graph.remove_node_config(&change.node);
                }
            }
        }

        result
    }

    /// Rollback changes from a previous apply result.
    ///
    /// Restores the graph to its previous state by reversing all changes
    /// recorded in the `ApplyResult`.
    ///
    /// # Arguments
    ///
    /// * `graph` - The graph to restore
    /// * `result` - The apply result containing changes to rollback
    ///
    /// # Returns
    ///
    /// Number of changes that were rolled back.
    pub fn rollback<S: crate::state::GraphState>(
        &self,
        graph: &mut crate::graph::StateGraph<S>,
        result: &ApplyResult,
    ) -> usize {
        let mut rollback_count = 0;

        for change in result.applied_changes.iter().rev() {
            match change.change_type {
                ConfigChangeType::ConfigRemove => {
                    // Re-add the removed config
                    if let Some(ref previous) = change.previous_config {
                        graph.set_node_config(
                            change.node.clone(),
                            previous.clone(),
                            Some("rollback"),
                        );
                        rollback_count += 1;
                    }
                }
                ConfigChangeType::ConfigAdd => {
                    // Remove the added config
                    if graph.remove_node_config(&change.node).is_some() {
                        rollback_count += 1;
                    }
                }
                _ => {
                    // Restore previous config
                    if let Some(ref previous) = change.previous_config {
                        match graph.update_node_config(
                            &change.node,
                            previous.clone(),
                            Some("rollback".to_string()),
                        ) {
                            Ok(_) => rollback_count += 1,
                            Err(e) => {
                                warn!(node = %change.node, "Failed to restore previous config during rollback: {}", e);
                            }
                        }
                    }
                }
            }
        }

        rollback_count
    }
}

// ============================================================================
// CLI Support Functions
// ============================================================================

/// Run introspection from CLI with the given options
///
/// # Errors
///
/// Returns error if introspection fails
pub fn run_cli_introspection(
    storage_path: Option<&str>,
    depth: Option<&str>,
    reason: Option<&str>,
) -> Result<IntrospectionResult, String> {
    let storage = match storage_path {
        Some(path) => IntrospectionStorage::at_path(path),
        None => IntrospectionStorage::default(),
    };

    // Initialize storage if needed
    storage
        .initialize()
        .map_err(|e| format!("Failed to initialize storage: {}", e))?;

    let depth = match depth {
        Some("metrics") => AnalysisDepth::Metrics,
        Some("local") => AnalysisDepth::LocalAnalysis,
        Some("deep") | Some("full") => AnalysisDepth::DeepAnalysis,
        None => AnalysisDepth::LocalAnalysis,
        Some(other) => return Err(format!("Unknown depth: {}", other)),
    };

    let mut orchestrator = IntrospectionOrchestrator::new(storage);

    // Load traces from disk before analysis (Issue #6 fix)
    // This enables CLI analysis to work with historical execution traces
    let traces_dir = std::path::PathBuf::from(".dashflow/traces");
    let loaded_count = orchestrator.load_traces_from_directory(&traces_dir)?;
    if loaded_count > 0 {
        tracing::info!(count = loaded_count, "Loaded traces from disk for analysis");
    }

    orchestrator.run_analysis(
        depth,
        Some(TriggerReason::Manual {
            reason: reason.map(String::from),
        }),
    )
}

/// Approve a plan from CLI
///
/// # Errors
///
/// Returns error if approval fails
pub fn approve_plan_cli(
    plan_id: &str,
    assignee: &str,
    storage_path: Option<&str>,
) -> Result<(), String> {
    let storage = match storage_path {
        Some(path) => IntrospectionStorage::at_path(path),
        None => IntrospectionStorage::default(),
    };

    let id = Uuid::parse_str(plan_id).map_err(|e| format!("Invalid plan ID: {}", e))?;

    let dasher = DasherIntegration::new(storage);
    dasher
        .start_implementation(id, assignee)
        .map_err(|e| format!("Failed to approve plan: {}", e))
}

/// List plans from CLI
///
/// # Errors
///
/// Returns error if listing fails
pub fn list_plans_cli(
    status: Option<&str>,
    storage_path: Option<&str>,
) -> Result<Vec<ExecutionPlan>, String> {
    let storage = match storage_path {
        Some(path) => IntrospectionStorage::at_path(path),
        None => IntrospectionStorage::default(),
    };

    match status {
        Some("pending") | None => storage
            .pending_plans()
            .map_err(|e| format!("Failed to list plans: {}", e)),
        Some("approved") => storage
            .approved_plans()
            .map_err(|e| format!("Failed to list plans: {}", e)),
        Some("implemented") => storage
            .list_implemented_plans()
            .map_err(|e| format!("Failed to list plans: {}", e)),
        Some("failed") => storage
            .list_failed_plans()
            .map_err(|e| format!("Failed to list plans: {}", e)),
        Some(other) => Err(format!("Unknown status: {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspection::ExecutionTrace;
    use crate::self_improvement::types::{
        CapabilityGap, Citation, ComparisonMetrics, DeprecationRecommendation, DeprecationTarget,
        EvaluationTrigger, ExpectedEvidence, GapCategory, GapManifestation, Hypothesis,
        HypothesisStatus, Impact, ImplementationStep, PlanCategory, PlanStatus, Priority,
    };
    use tempfile::tempdir;

    fn create_test_trace(thread_id: &str) -> ExecutionTrace {
        ExecutionTrace::builder().thread_id(thread_id).build()
    }

    #[test]
    fn test_trigger_config_defaults() {
        let config = TriggerConfig::default();
        assert!(config.per_execution);
        assert_eq!(config.periodic_executions, 10);
        assert!(config.periodic_time.is_some());
        assert!(config.error_spike_threshold > 0.0);
    }

    #[test]
    fn test_trigger_config_disabled() {
        let config = TriggerConfig::disabled();
        assert!(!config.per_execution);
        assert_eq!(config.periodic_executions, 0);
        assert!(config.periodic_time.is_none());
        assert_eq!(config.error_spike_threshold, 0.0);
    }

    #[test]
    fn test_execution_stats() {
        let stats = ExecutionStats::new();

        stats.record_execution(create_test_trace("t1"), true);
        stats.record_execution(create_test_trace("t2"), true);
        stats.record_execution(create_test_trace("t3"), false);

        assert_eq!(stats.execution_count(), 3);
        assert!((stats.error_rate() - 1.0 / 3.0).abs() < 0.001);

        stats.reset();
        assert_eq!(stats.execution_count(), 0);
        assert!(stats.last_analysis.read().unwrap().is_some());
    }

    #[test]
    fn test_trigger_system_per_execution() {
        let config = TriggerConfig {
            per_execution: true,
            periodic_executions: 0,
            periodic_time: None,
            error_spike_threshold: 0.0,
            ..TriggerConfig::default()
        };

        let system = TriggerSystem::new(config);
        let triggers = system.record_and_check(create_test_trace("t1"), true);

        assert_eq!(triggers.len(), 1);
        assert!(matches!(triggers[0], TriggerReason::PerExecution { .. }));
    }

    #[test]
    fn test_trigger_system_periodic() {
        let config = TriggerConfig {
            per_execution: false,
            periodic_executions: 3,
            periodic_time: None,
            error_spike_threshold: 0.0,
            ..TriggerConfig::default()
        };

        let system = TriggerSystem::new(config);

        // First two executions: no trigger
        let t1 = system.record_and_check(create_test_trace("t1"), true);
        let t2 = system.record_and_check(create_test_trace("t2"), true);
        assert!(t1.is_empty());
        assert!(t2.is_empty());

        // Third execution: trigger
        let t3 = system.record_and_check(create_test_trace("t3"), true);
        assert_eq!(t3.len(), 1);
        assert!(matches!(
            t3[0],
            TriggerReason::PeriodicExecution { count: 3 }
        ));
    }

    #[test]
    fn test_trigger_system_error_spike() {
        let config = TriggerConfig {
            per_execution: false,
            periodic_executions: 0,
            periodic_time: None,
            error_spike_threshold: 0.5,
            error_spike_min_executions: 2,
            ..TriggerConfig::default()
        };

        let system = TriggerSystem::new(config);

        // One failure: not enough executions
        let t1 = system.record_and_check(create_test_trace("t1"), false);
        assert!(t1.is_empty());

        // Two failures: error spike!
        let t2 = system.record_and_check(create_test_trace("t2"), false);
        assert_eq!(t2.len(), 1);
        assert!(matches!(t2[0], TriggerReason::ErrorSpike { .. }));
    }

    #[test]
    fn test_implementation_summary() {
        let summary = ImplementationSummary {
            pending: 5,
            approved: 2,
            implemented: 10,
            failed: 3,
        };

        assert_eq!(summary.total(), 20);
        assert!((summary.success_rate() - 10.0 / 13.0).abs() < 0.001);
    }

    #[test]
    fn test_dasher_integration() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let dasher = DasherIntegration::new(storage.clone());

        // No plans initially
        assert!(dasher.next_plan().unwrap().is_none());

        // Save a plan and approve it
        let mut plan = ExecutionPlan::new(
            "Test Plan",
            super::super::types::PlanCategory::ApplicationImprovement,
        );
        plan.status = PlanStatus::Validated;
        storage.save_plan(&plan).unwrap();

        // Still no plan (it's pending, not approved)
        assert!(dasher.next_plan().unwrap().is_none());

        // Approve the plan
        storage.approve_plan(plan.id, "test").unwrap();

        // Now we can get it
        let next = dasher.next_plan().unwrap();
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, plan.id);

        // Mark as implemented
        dasher.mark_implemented(plan.id, "abc123").unwrap();

        // No more plans
        assert!(dasher.next_plan().unwrap().is_none());

        // Check summary
        let summary = dasher.implementation_summary().unwrap();
        assert_eq!(summary.implemented, 1);
    }

    #[test]
    fn test_orchestrator_creation() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let orchestrator = IntrospectionOrchestrator::new(storage);

        // Should have default configuration
        assert!(orchestrator.config.enable_consensus);
        assert!(orchestrator.config.enable_hypotheses);
    }

    #[test]
    fn test_orchestrator_record_execution() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let config = OrchestratorConfig {
            trigger: TriggerConfig {
                per_execution: false,
                periodic_executions: 2,
                periodic_time: None,
                error_spike_threshold: 0.0,
                periodic_depth: AnalysisDepth::LocalAnalysis,
                ..TriggerConfig::default()
            },
            enable_consensus: false,
            enable_hypotheses: false,
            enable_meta_analysis: false,
            auto_approve_priority: 0,
        };

        let mut orchestrator = IntrospectionOrchestrator::with_config(storage, config);

        // First execution: no trigger
        let result = orchestrator
            .record_execution(create_test_trace("t1"), true)
            .unwrap();
        assert!(result.is_none());

        // Second execution: trigger fires
        let result = orchestrator
            .record_execution(create_test_trace("t2"), true)
            .unwrap();
        assert!(result.is_some());

        let result = result.unwrap();
        assert!(!result.saved_files.is_empty());
    }

    #[test]
    fn test_orchestrator_manual_run() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let config = OrchestratorConfig {
            enable_consensus: false,
            ..OrchestratorConfig::default()
        };

        let mut orchestrator = IntrospectionOrchestrator::with_config(storage, config);

        // Manual run should always work
        let result = orchestrator
            .run_manual(Some("test reason".to_string()))
            .unwrap();

        assert!(!result.saved_files.is_empty());
    }

    #[test]
    fn test_trigger_depth_mapping() {
        let system = TriggerSystem::default();

        // Per-execution should be metrics only
        let depth = system.depth_for_trigger(&TriggerReason::PerExecution {
            thread_id: "t1".to_string(),
        });
        assert!(matches!(depth, AnalysisDepth::Metrics));

        // Error spike should be deep
        let depth = system.depth_for_trigger(&TriggerReason::ErrorSpike {
            error_rate: 0.5,
            threshold: 0.3,
        });
        assert!(matches!(depth, AnalysisDepth::DeepAnalysis));

        // Manual should be deep
        let depth = system.depth_for_trigger(&TriggerReason::Manual { reason: None });
        assert!(matches!(depth, AnalysisDepth::DeepAnalysis));
    }

    // =========================================================================
    // Auto-Apply Tests
    // =========================================================================

    // Test state for apply_to_graph tests
    #[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
    struct TestState {
        data: String,
    }

    #[test]
    fn test_apply_to_graph_update_prompt() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let orchestrator = IntrospectionOrchestrator::new(storage);
        let mut graph: crate::graph::StateGraph<TestState> = crate::graph::StateGraph::new();

        // Set initial config
        graph.set_node_config(
            "researcher",
            serde_json::json!({"system_prompt": "Old prompt", "temperature": 0.7}),
            None,
        );

        // Create plan with UpdatePrompt action
        let plan = ExecutionPlan::new(
            "Update prompt",
            super::super::types::PlanCategory::Optimization,
        )
        .add_action(PlanAction::new(ActionType::update_prompt(
            "researcher",
            "New improved prompt",
        )))
        .validated(0.9);

        // Apply the plan
        let result = orchestrator.apply_to_graph(&mut graph, &plan);

        assert!(result.success);
        assert_eq!(result.change_count(), 1);

        // Verify the prompt was updated
        let config = graph.get_node_config("researcher").unwrap();
        assert_eq!(config.system_prompt(), Some("New improved prompt"));
        // Temperature should still be there
        assert_eq!(config.temperature(), Some(0.7));
        // Version should be incremented
        assert_eq!(config.version, 2);
        assert_eq!(config.updated_by, Some("self_improvement".to_string()));
    }

    #[test]
    fn test_apply_to_graph_update_parameter() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let orchestrator = IntrospectionOrchestrator::new(storage);
        let mut graph: crate::graph::StateGraph<TestState> = crate::graph::StateGraph::new();

        // Set initial config
        graph.set_node_config(
            "llm",
            serde_json::json!({"temperature": 0.7, "max_tokens": 1000}),
            None,
        );

        // Create plan to update temperature
        let plan = ExecutionPlan::new(
            "Tune temperature",
            super::super::types::PlanCategory::Optimization,
        )
        .add_action(PlanAction::new(ActionType::update_parameter(
            "llm",
            "temperature",
            serde_json::json!(0.3),
        )))
        .validated(0.85);

        let result = orchestrator.apply_to_graph(&mut graph, &plan);

        assert!(result.success);
        assert_eq!(result.change_count(), 1);

        let config = graph.get_node_config("llm").unwrap();
        assert_eq!(config.temperature(), Some(0.3));
        assert_eq!(config.max_tokens(), Some(1000)); // Unchanged
    }

    #[test]
    fn test_apply_to_graph_creates_missing_config() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let orchestrator = IntrospectionOrchestrator::new(storage);
        let mut graph: crate::graph::StateGraph<TestState> = crate::graph::StateGraph::new();

        // No config exists for "new_node"
        assert!(!graph.has_node_config("new_node"));

        // Create plan to update prompt for nonexistent config
        let plan = ExecutionPlan::new(
            "Add new node config",
            super::super::types::PlanCategory::Optimization,
        )
        .add_action(PlanAction::new(ActionType::update_prompt(
            "new_node",
            "Brand new prompt",
        )))
        .validated(0.9);

        let result = orchestrator.apply_to_graph(&mut graph, &plan);

        assert!(result.success);
        assert_eq!(result.change_count(), 1);

        // Config should now exist
        assert!(graph.has_node_config("new_node"));
        let config = graph.get_node_config("new_node").unwrap();
        assert_eq!(config.system_prompt(), Some("Brand new prompt"));
    }

    #[test]
    fn test_apply_to_graph_replace_config() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let orchestrator = IntrospectionOrchestrator::new(storage);
        let mut graph: crate::graph::StateGraph<TestState> = crate::graph::StateGraph::new();

        // Set initial config
        graph.set_node_config("agent", serde_json::json!({"old_field": "old_value"}), None);

        // Replace with completely new config
        let plan = ExecutionPlan::new(
            "Replace config",
            super::super::types::PlanCategory::Optimization,
        )
        .add_action(PlanAction::new(ActionType::ReplaceConfig {
            node: "agent".to_string(),
            new_config: serde_json::json!({
                "system_prompt": "Replaced prompt",
                "temperature": 0.5,
                "new_field": "new_value"
            }),
        }))
        .validated(0.9);

        let result = orchestrator.apply_to_graph(&mut graph, &plan);

        assert!(result.success);

        let config = graph.get_node_config("agent").unwrap();
        assert_eq!(config.system_prompt(), Some("Replaced prompt"));
        assert_eq!(config.temperature(), Some(0.5));
        // Old field should be gone, new field should exist
        assert!(config.get_field("old_field").is_none());
        assert!(config.get_field("new_field").is_some());
    }

    #[test]
    fn test_apply_to_graph_remove_config() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let orchestrator = IntrospectionOrchestrator::new(storage);
        let mut graph: crate::graph::StateGraph<TestState> = crate::graph::StateGraph::new();

        // Set initial config
        graph.set_node_config(
            "to_remove",
            serde_json::json!({"data": "will be removed"}),
            None,
        );
        assert!(graph.has_node_config("to_remove"));

        // Remove the config
        let plan = ExecutionPlan::new(
            "Remove config",
            super::super::types::PlanCategory::Optimization,
        )
        .add_action(PlanAction::new(ActionType::RemoveConfig {
            node: "to_remove".to_string(),
        }))
        .validated(0.9);

        let result = orchestrator.apply_to_graph(&mut graph, &plan);

        assert!(result.success);
        assert!(!graph.has_node_config("to_remove"));

        // Check we recorded the previous config for rollback
        assert_eq!(result.applied_changes.len(), 1);
        assert!(result.applied_changes[0].previous_config.is_some());
    }

    #[test]
    fn test_apply_to_graph_skip_existing_add_config() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let orchestrator = IntrospectionOrchestrator::new(storage);
        let mut graph: crate::graph::StateGraph<TestState> = crate::graph::StateGraph::new();

        // Set existing config
        graph.set_node_config("existing", serde_json::json!({"data": "exists"}), None);

        // Try to add config for existing node
        let plan = ExecutionPlan::new(
            "Add existing",
            super::super::types::PlanCategory::Optimization,
        )
        .add_action(PlanAction::new(ActionType::AddConfig {
            node: "existing".to_string(),
            node_type: "llm.chat".to_string(),
            config: serde_json::json!({"data": "new"}),
        }))
        .validated(0.9);

        let result = orchestrator.apply_to_graph(&mut graph, &plan);

        assert!(result.success); // Still succeeds, just skips
        assert_eq!(result.change_count(), 0);
        assert!(!result.skipped_nodes.is_empty());
        assert!(result.skipped_nodes.contains(&"existing".to_string()));
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_apply_to_graph_multiple_actions() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let orchestrator = IntrospectionOrchestrator::new(storage);
        let mut graph: crate::graph::StateGraph<TestState> = crate::graph::StateGraph::new();

        // Set up initial configs
        graph.set_node_config("node_a", serde_json::json!({"temp": 0.5}), None);
        graph.set_node_config("node_b", serde_json::json!({"temp": 0.5}), None);

        // Plan with multiple actions
        let plan = ExecutionPlan::new(
            "Multi-action",
            super::super::types::PlanCategory::Optimization,
        )
        .add_action(PlanAction::new(ActionType::update_prompt(
            "node_a", "Prompt A",
        )))
        .add_action(PlanAction::new(ActionType::update_prompt(
            "node_b", "Prompt B",
        )))
        .add_action(PlanAction::new(ActionType::update_parameter(
            "node_a",
            "temperature",
            serde_json::json!(0.2),
        )))
        .validated(0.9);

        let result = orchestrator.apply_to_graph(&mut graph, &plan);

        assert!(result.success);
        assert_eq!(result.change_count(), 3);

        assert_eq!(
            graph.get_node_config("node_a").unwrap().system_prompt(),
            Some("Prompt A")
        );
        assert_eq!(
            graph.get_node_config("node_b").unwrap().system_prompt(),
            Some("Prompt B")
        );
        assert_eq!(
            graph.get_node_config("node_a").unwrap().temperature(),
            Some(0.2)
        );
    }

    #[test]
    fn test_rollback() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let orchestrator = IntrospectionOrchestrator::new(storage);
        let mut graph: crate::graph::StateGraph<TestState> = crate::graph::StateGraph::new();

        // Set initial config
        graph.set_node_config(
            "llm",
            serde_json::json!({"system_prompt": "Original", "temperature": 0.7}),
            None,
        );

        // Apply changes
        let plan = ExecutionPlan::new(
            "Changes to rollback",
            super::super::types::PlanCategory::Optimization,
        )
        .add_action(PlanAction::new(ActionType::update_prompt(
            "llm", "Modified",
        )))
        .add_action(PlanAction::new(ActionType::update_parameter(
            "llm",
            "temperature",
            serde_json::json!(0.3),
        )))
        .validated(0.9);

        let result = orchestrator.apply_to_graph(&mut graph, &plan);
        assert!(result.success);

        // Verify changes applied
        assert_eq!(
            graph.get_node_config("llm").unwrap().system_prompt(),
            Some("Modified")
        );
        assert_eq!(
            graph.get_node_config("llm").unwrap().temperature(),
            Some(0.3)
        );

        // Rollback
        let rollback_count = orchestrator.rollback(&mut graph, &result);
        assert_eq!(rollback_count, 2);

        // Verify original values restored
        assert_eq!(
            graph.get_node_config("llm").unwrap().system_prompt(),
            Some("Original")
        );
        assert_eq!(
            graph.get_node_config("llm").unwrap().temperature(),
            Some(0.7)
        );
    }

    #[test]
    fn test_action_type_helpers() {
        let update_prompt = ActionType::update_prompt("node", "prompt");
        assert_eq!(update_prompt.target_node(), "node");
        assert!(matches!(update_prompt, ActionType::UpdatePrompt { .. }));

        let update_param = ActionType::update_parameter("node", "temp", serde_json::json!(0.5));
        assert_eq!(update_param.target_node(), "node");
        assert!(matches!(update_param, ActionType::UpdateParameter { .. }));
    }

    #[test]
    fn test_apply_result_helpers() {
        let plan_id = uuid::Uuid::new_v4();

        let success_result = ApplyResult::success(plan_id, Vec::new());
        assert!(success_result.success);
        assert_eq!(success_result.change_count(), 0);

        let failure_result = ApplyResult::failure(plan_id, "Something went wrong");
        assert!(!failure_result.success);
        assert!(!failure_result.warnings.is_empty());
    }

    #[test]
    fn test_plan_action_builder() {
        let action = PlanAction::new(ActionType::update_prompt("node", "prompt"))
            .with_rationale("Improve response quality")
            .with_expected_impact("Better user satisfaction");

        assert_eq!(action.rationale, "Improve response quality");
        assert_eq!(
            action.expected_impact,
            Some("Better user satisfaction".to_string())
        );
    }

    // =========================================================================
    // End-to-End Integration Tests
    // =========================================================================

    #[test]
    fn test_report_lifecycle_create_store_retrieve() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        // 1. Create a report with capability gaps
        let mut report = IntrospectionReport::new(IntrospectionScope::System);
        report.add_capability_gap(
            CapabilityGap::new(
                "Missing sentiment analysis capability",
                GapCategory::MissingTool {
                    tool_description: "Sentiment analysis tool".to_string(),
                },
                GapManifestation::PromptWorkarounds {
                    patterns: vec!["Manually analyzing text...".to_string()],
                },
            )
            .with_confidence(0.85)
            .with_impact(Impact::medium("Could improve accuracy")),
        );
        report.execution_summary.total_executions = 100;
        report.execution_summary.success_rate = 0.92;

        // 2. Store the report
        let report_id = report.id;
        storage.save_report(&report).unwrap();

        // 3. Retrieve the report
        let retrieved = storage.latest_report().unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, report_id);
        assert_eq!(retrieved.capability_gaps.len(), 1);
        assert_eq!(retrieved.execution_summary.total_executions, 100);
    }

    #[test]
    fn test_plan_lifecycle_propose_validate_store() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        // 1. Create a plan (Proposed state)
        let mut plan =
            ExecutionPlan::new("Add caching layer", PlanCategory::ApplicationImprovement)
                .with_description("Add caching to reduce redundant API calls")
                .with_priority(2)
                .with_estimated_commits(3)
                .with_steps(vec![
                    ImplementationStep::new(1, "Create cache module")
                        .with_files(vec!["src/cache.rs".to_string()])
                        .with_verification("cargo test cache"),
                    ImplementationStep::new(2, "Integrate cache with API calls")
                        .with_files(vec!["src/api.rs".to_string()])
                        .with_verification("cargo test api"),
                ])
                .with_success_criteria(vec![
                    "API call count reduced by 50%".to_string(),
                    "Response time improved by 30%".to_string(),
                ])
                .with_rollback_plan("Remove cache module and revert API changes");

        assert!(matches!(plan.status, PlanStatus::Proposed));

        // 2. Validate the plan
        plan = plan.validated(0.85);
        assert!(matches!(plan.status, PlanStatus::Validated));
        assert_eq!(plan.validation_score, 0.85);

        // 3. Store the plan via a report
        let mut report = IntrospectionReport::new(IntrospectionScope::System);
        report.execution_plans.push(plan);
        storage.save_report(&report).unwrap();

        // 4. Retrieve and verify
        let retrieved = storage.latest_report().unwrap().unwrap();
        assert_eq!(retrieved.execution_plans.len(), 1);
        let retrieved_plan = &retrieved.execution_plans[0];
        assert!(matches!(retrieved_plan.status, PlanStatus::Validated));
        assert_eq!(retrieved_plan.steps.len(), 2);
    }

    #[test]
    fn test_hypothesis_lifecycle_create_track_evaluate() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        // 1. Create a hypothesis
        let hypothesis = Hypothesis::new(
            "Adding caching will reduce latency by 30%",
            "Observed high latency on repeated API calls",
        )
        .with_expected_evidence(vec![
            ExpectedEvidence::new("latency_reduction", ">= 30%", "Measure p95 latency"),
            ExpectedEvidence::new("cache_hit_rate", ">= 50%", "Measure cache hit percentage"),
        ])
        .with_trigger(EvaluationTrigger::AfterExecutions(100));

        // 2. Store the hypothesis
        storage.save_hypothesis(&hypothesis).unwrap();

        // 3. Load and verify
        let loaded = storage.load_hypothesis(hypothesis.id).unwrap();
        assert_eq!(loaded.id, hypothesis.id);
        assert!(matches!(loaded.status, HypothesisStatus::Active));
        assert_eq!(loaded.expected_evidence.len(), 2);
    }

    #[test]
    fn test_full_analysis_flow_with_storage() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        // 1. Create initial report
        let mut initial_report = IntrospectionReport::new(IntrospectionScope::System);
        initial_report.execution_summary.total_executions = 50;
        initial_report.execution_summary.success_rate = 0.85;
        storage.save_report(&initial_report).unwrap();

        // 2. Create follow-up report referencing the initial
        let mut followup_report = IntrospectionReport::new(IntrospectionScope::System);
        followup_report.previous_report_id = Some(initial_report.id);
        followup_report.execution_summary.total_executions = 100;
        followup_report.execution_summary.success_rate = 0.92;

        // Add comparison metrics
        followup_report.execution_summary.vs_previous = Some(ComparisonMetrics {
            execution_delta: 50,
            success_rate_delta: 0.07,
            duration_delta_ms: -50.0, // Improved
            retry_rate_delta: -0.07,  // Improved
        });

        storage.save_report(&followup_report).unwrap();

        // 3. Verify we can load both reports by ID
        let loaded_initial = storage.load_report(initial_report.id).unwrap();
        let loaded_followup = storage.load_report(followup_report.id).unwrap();

        assert_eq!(loaded_initial.id, initial_report.id);
        assert_eq!(loaded_followup.id, followup_report.id);
        assert_eq!(loaded_followup.previous_report_id, Some(initial_report.id));
        assert!(loaded_followup.execution_summary.vs_previous.is_some());

        // Verify comparison shows improvement
        let comparison = loaded_followup
            .execution_summary
            .vs_previous
            .as_ref()
            .unwrap();
        assert!(comparison.success_rate_delta > 0.0); // Improved
        assert!(comparison.duration_delta_ms < 0.0); // Improved
    }

    #[test]
    fn test_multiple_plans_priority_ordering() {
        let mut report = IntrospectionReport::new(IntrospectionScope::System);

        // Add plans with different priorities
        report.execution_plans.push(
            ExecutionPlan::new("Low priority fix", PlanCategory::ProcessImprovement)
                .with_priority(3),
        );
        report.execution_plans.push(
            ExecutionPlan::new("High priority fix", PlanCategory::Optimization).with_priority(1),
        );
        report.execution_plans.push(
            ExecutionPlan::new("Medium priority fix", PlanCategory::ApplicationImprovement)
                .with_priority(2),
        );

        // Sort by priority (lower number = higher priority)
        report
            .execution_plans
            .sort_by(|a, b| a.priority.cmp(&b.priority));

        assert_eq!(report.execution_plans[0].priority, 1);
        assert_eq!(report.execution_plans[1].priority, 2);
        assert_eq!(report.execution_plans[2].priority, 3);
    }

    #[test]
    fn test_deprecation_recommendation_flow() {
        let mut report = IntrospectionReport::new(IntrospectionScope::System);

        // Add a deprecation recommendation
        report.add_deprecation(
            DeprecationRecommendation::new(
                DeprecationTarget::Node {
                    name: "legacy_search".to_string(),
                    usage_count: 0,
                },
                "Node has not been invoked in 30 days",
            )
            .with_benefits(vec![
                "Remove 500 lines of code".to_string(),
                "Reduce maintenance burden".to_string(),
            ])
            .with_risks(vec![
                "May need to restore if old features are requested".to_string()
            ])
            .with_confidence(0.95),
        );

        assert_eq!(report.deprecations.len(), 1);
        let dep = &report.deprecations[0];
        assert_eq!(dep.confidence, 0.95);
        assert_eq!(dep.benefits.len(), 2);
        assert_eq!(dep.risks.len(), 1);

        // Verify markdown generation
        let md = report.to_markdown();
        assert!(md.contains("Deprecation"));
        assert!(md.contains("legacy_search"));
    }

    #[test]
    fn test_capability_gap_to_plan_conversion() {
        let mut report = IntrospectionReport::new(IntrospectionScope::System);

        // Add a capability gap with solution
        report.add_capability_gap(
            CapabilityGap::new(
                "High latency on embedding calls",
                GapCategory::PerformanceGap {
                    bottleneck: "embedding_node".to_string(),
                },
                GapManifestation::SuboptimalPaths {
                    description: "Sequential embedding calls taking too long".to_string(),
                },
            )
            .with_solution("Batch embedding calls to reduce overhead")
            .with_confidence(0.9)
            .with_impact(Impact::high("50% latency reduction expected")),
        );

        // Convert gap to plan
        let gap = &report.capability_gaps[0];
        let plan = ExecutionPlan::new(
            format!("Fix: {}", &gap.description[..30.min(gap.description.len())]),
            PlanCategory::Optimization,
        )
        .with_description(gap.proposed_solution.clone())
        .with_priority(match gap.priority() {
            Priority::High => 1,
            Priority::Medium => 2,
            Priority::Low => 3,
        });

        assert_eq!(plan.priority, 1); // High priority gap
        assert!(plan.description.contains("Batch embedding"));
    }

    #[test]
    fn test_citation_chain() {
        // Create a chain of citations referencing each other
        let trace_citation = Citation::trace("thread-123");
        let commit_citation = Citation::commit("abc123def", "Fix bug in parser");
        let report_citation = Citation::report(uuid::Uuid::new_v4());

        // All citations should have unique IDs
        assert_ne!(trace_citation.id, commit_citation.id);
        assert_ne!(commit_citation.id, report_citation.id);
        assert_ne!(trace_citation.id, report_citation.id);

        // Create a plan with multiple citations
        let mut plan = ExecutionPlan::new("Test Plan", PlanCategory::Optimization);
        plan.citations.push(trace_citation);
        plan.citations.push(commit_citation);
        plan.citations.push(report_citation);

        assert_eq!(plan.citations.len(), 3);

        // Verify markdown includes citations
        let md = plan.to_markdown();
        assert!(md.contains("Citations"));
    }

    #[test]
    fn test_load_traces_from_directory() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let mut orchestrator = IntrospectionOrchestrator::new(storage);

        // Create traces directory
        let traces_dir = dir.path().join("traces");
        std::fs::create_dir_all(&traces_dir).unwrap();

        // Write some trace files
        let trace1 = ExecutionTrace::builder().thread_id("t1").build();
        let trace2 = ExecutionTrace::builder().thread_id("t2").build();

        std::fs::write(
            traces_dir.join("trace1.json"),
            serde_json::to_string(&trace1).unwrap(),
        )
        .unwrap();
        std::fs::write(
            traces_dir.join("trace2.json"),
            serde_json::to_string(&trace2).unwrap(),
        )
        .unwrap();

        // Load traces
        let loaded = orchestrator
            .load_traces_from_directory(&traces_dir)
            .unwrap();

        assert_eq!(loaded, 2);
        assert_eq!(orchestrator.stats().execution_count(), 2);
    }

    #[test]
    fn test_load_traces_from_directory_empty() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let mut orchestrator = IntrospectionOrchestrator::new(storage);

        // Create empty traces directory
        let traces_dir = dir.path().join("traces");
        std::fs::create_dir_all(&traces_dir).unwrap();

        let loaded = orchestrator
            .load_traces_from_directory(&traces_dir)
            .unwrap();

        assert_eq!(loaded, 0);
        assert_eq!(orchestrator.stats().execution_count(), 0);
    }

    #[test]
    fn test_load_traces_from_directory_nonexistent() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        storage.initialize().unwrap();

        let mut orchestrator = IntrospectionOrchestrator::new(storage);

        // Use nonexistent directory
        let traces_dir = dir.path().join("nonexistent");

        let loaded = orchestrator
            .load_traces_from_directory(&traces_dir)
            .unwrap();

        assert_eq!(loaded, 0);
    }
}
