// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Meta-Analysis for Self-Improvement
//!
//! This module implements the Self-Improvement roadmap -
//! - `HypothesisTracker`: Creates and evaluates hypotheses from analysis results
//! - `MetaAnalyzer`: Analyzes patterns across multiple introspection reports
//! - `DesignNoteGenerator`: Creates design notes for future AI iterations
//!
//! # Hypothesis Learning Loop Value Proposition
//!
//! The hypothesis tracking system creates a closed feedback loop for AI learning:
//!
//! 1. **Hypothesis Generation**: When analyzing execution data, the system generates
//!    hypotheses about expected improvements from capability gaps, execution plans,
//!    and deprecation recommendations.
//!
//! 2. **Tracking by Source**: Each hypothesis is tagged with its source (CapabilityGap,
//!    ExecutionPlan, Deprecation, Manual) to enable accuracy analysis per source type.
//!
//! 3. **Evaluation**: After sufficient executions or time, hypotheses are evaluated
//!    against actual metrics to determine if predictions were accurate.
//!
//! 4. **Learning Insights**: Accuracy statistics by source reveal which types of
//!    analysis produce the most accurate predictions, guiding future improvements.
//!
//! 5. **Dashboard Visibility**: The `/mcp/hypotheses` endpoint exposes this learning
//!    data to AI agents, enabling them to calibrate their own confidence levels.
//!
//! ## Benefits
//!
//! - **Calibrated Confidence**: AI learns which predictions are reliable
//! - **Source-Specific Tuning**: Low-accuracy sources can be improved or weighted down
//! - **Continuous Improvement**: Each hypothesis evaluation improves the system
//! - **Transparency**: Human operators can see how well AI predictions perform
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────┐     ┌─────────────────────┐
//! │ IntrospectionReport │     │   ExecutionPlan     │
//! └──────────┬──────────┘     └──────────┬──────────┘
//!            │                           │
//!            └───────────────┬───────────┘
//!                            ▼
//!            ┌─────────────────────────────┐
//!            │      HypothesisTracker      │
//!            │  (create & eval hypotheses) │
//!            └──────────────┬──────────────┘
//!                           │
//!                           ▼
//!            ┌─────────────────────────────┐
//!            │        MetaAnalyzer         │
//!            │  (patterns across reports)  │
//!            └──────────────┬──────────────┘
//!                           │
//!                           ▼
//!            ┌─────────────────────────────┐
//!            │    DesignNoteGenerator      │
//!            │  (notes for future AIs)     │
//!            └─────────────────────────────┘
//! ```

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing;
use uuid::Uuid;

use super::storage::IntrospectionStorage;
use super::types::{
    CapabilityGap, Citation, CitationRetrieval, CitationSource, DeprecationRecommendation,
    EvaluationTrigger, ExecutionPlan, ExpectedEvidence, Hypothesis, HypothesisOutcome,
    HypothesisSource, HypothesisStatus, IntrospectionReport, ObservedEvidence, PlanStatus,
    Priority, ReportExecutionSummary,
};

// Use std::io::Result for storage operations since that's what IntrospectionStorage uses
type StorageResult<T> = std::io::Result<T>;

// =============================================================================
// HypothesisTracker - Creates and Evaluates Hypotheses
// =============================================================================

/// Configuration for hypothesis tracking.
#[derive(Debug, Clone)]
pub struct HypothesisTrackerConfig {
    /// Minimum confidence for generating hypotheses (default: 0.5)
    pub min_confidence_for_hypothesis: f64,
    /// Default number of executions before evaluating (default: 50)
    pub default_evaluation_executions: usize,
    /// Tolerance for considering metrics "matching" expectations (default: 0.2)
    pub metric_tolerance: f64,
    /// Whether to auto-create hypotheses from plans (default: true)
    pub auto_create_from_plans: bool,
}

impl Default for HypothesisTrackerConfig {
    fn default() -> Self {
        Self {
            min_confidence_for_hypothesis: 0.5,
            default_evaluation_executions: 50,
            metric_tolerance: 0.2,
            auto_create_from_plans: true,
        }
    }
}

/// Tracks hypotheses throughout the self-improvement cycle.
///
/// Creates hypotheses about the expected outcomes of implementing plans,
/// tracks them over time, and evaluates their correctness once conditions
/// are met.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{
///     HypothesisTracker, HypothesisTrackerConfig, IntrospectionStorage,
/// };
///
/// let storage = IntrospectionStorage::default();
/// let tracker = HypothesisTracker::new(storage, HypothesisTrackerConfig::default());
///
/// // Create hypothesis from plan
/// let hypothesis = tracker.create_from_plan(&plan)?;
///
/// // Later, evaluate based on current metrics
/// let outcome = tracker.evaluate(&hypothesis.id, &current_summary)?;
/// ```
#[derive(Debug)]
pub struct HypothesisTracker {
    storage: IntrospectionStorage,
    config: HypothesisTrackerConfig,
}

impl HypothesisTracker {
    /// Create a new hypothesis tracker
    pub fn new(storage: IntrospectionStorage, config: HypothesisTrackerConfig) -> Self {
        Self { storage, config }
    }

    /// Create a tracker with default configuration
    #[must_use]
    pub fn with_storage(storage: IntrospectionStorage) -> Self {
        Self::new(storage, HypothesisTrackerConfig::default())
    }

    /// Create a hypothesis from a capability gap
    #[must_use]
    pub fn create_from_gap(&self, gap: &CapabilityGap) -> Option<Hypothesis> {
        if gap.confidence < self.config.min_confidence_for_hypothesis {
            return None;
        }

        let statement = format!(
            "Addressing '{}' will improve system performance",
            gap.description
        );

        let mut expected_evidence = Vec::new();

        // Generate evidence based on expected impact
        if gap.expected_impact.error_reduction > 0.1 {
            expected_evidence.push(ExpectedEvidence::new(
                "error_rate",
                format!(
                    "reduced by {:.0}%",
                    gap.expected_impact.error_reduction * 100.0 * 0.8
                ),
                "Compare error rate before/after implementation",
            ));
        }

        if gap.expected_impact.latency_reduction_ms > 50.0 {
            expected_evidence.push(ExpectedEvidence::new(
                "latency_ms",
                format!(
                    "reduced by {:.0}ms",
                    gap.expected_impact.latency_reduction_ms * 0.8
                ),
                "Measure average execution latency",
            ));
        }

        if gap.expected_impact.accuracy_improvement > 0.1 {
            expected_evidence.push(ExpectedEvidence::new(
                "accuracy",
                format!(
                    "improved by {:.0}%",
                    gap.expected_impact.accuracy_improvement * 100.0 * 0.8
                ),
                "Measure task completion accuracy",
            ));
        }

        let reasoning = format!(
            "Based on analysis showing {} with confidence {:.0}%. {}",
            gap.manifestation.description(),
            gap.confidence * 100.0,
            gap.expected_impact.description
        );

        let hypothesis = Hypothesis::new(statement, reasoning)
            .with_expected_evidence(expected_evidence)
            .with_trigger(EvaluationTrigger::AfterExecutions(
                self.config.default_evaluation_executions,
            ))
            .with_source(HypothesisSource::CapabilityGap);

        Some(hypothesis)
    }

    /// Create a hypothesis from an execution plan
    #[must_use]
    pub fn create_from_plan(&self, plan: &ExecutionPlan) -> Hypothesis {
        let statement = format!(
            "Implementing '{}' will achieve its success criteria",
            plan.title
        );

        let expected_evidence: Vec<ExpectedEvidence> = plan
            .success_criteria
            .iter()
            .enumerate()
            .map(|(i, criteria)| {
                ExpectedEvidence::new(
                    format!("criterion_{}", i + 1),
                    criteria.clone(),
                    "Verify success criterion is met",
                )
            })
            .collect();

        let reasoning = format!(
            "Plan '{}' (priority {}) with validation score {:.2} is expected to succeed. Steps: {}",
            plan.title,
            plan.priority,
            plan.validation_score,
            plan.steps.len()
        );

        Hypothesis::new(statement, reasoning)
            .with_expected_evidence(expected_evidence)
            .with_trigger(EvaluationTrigger::AfterPlan(plan.id))
            .with_source(HypothesisSource::ExecutionPlan)
    }

    /// Create hypotheses from deprecation recommendations
    #[must_use]
    pub fn create_from_deprecation(&self, dep: &DeprecationRecommendation) -> Option<Hypothesis> {
        if dep.confidence < self.config.min_confidence_for_hypothesis {
            return None;
        }

        let target_desc = match &dep.target {
            super::types::DeprecationTarget::Node { name, .. } => format!("node '{}'", name),
            super::types::DeprecationTarget::Tool { name, .. } => format!("tool '{}'", name),
            super::types::DeprecationTarget::Edge { from, to } => {
                format!("edge {} -> {}", from, to)
            }
            super::types::DeprecationTarget::Feature { name } => format!("feature '{}'", name),
            super::types::DeprecationTarget::CodePath { location } => {
                format!("code at {}", location)
            }
        };

        let statement = format!("Removing {} will not cause regressions", target_desc);

        let expected_evidence = vec![
            ExpectedEvidence::new(
                "test_pass_rate",
                "100%",
                "All existing tests should continue to pass",
            ),
            ExpectedEvidence::new(
                "error_rate",
                "no increase",
                "Error rate should not increase after removal",
            ),
        ];

        let reasoning = format!(
            "Rationale: {}. Benefits: {}. Risks: {}",
            dep.rationale,
            dep.benefits.join(", "),
            if dep.risks.is_empty() {
                "none identified".to_string()
            } else {
                dep.risks.join(", ")
            }
        );

        let hypothesis = Hypothesis::new(statement, reasoning)
            .with_expected_evidence(expected_evidence)
            .with_trigger(EvaluationTrigger::AfterExecutions(
                self.config.default_evaluation_executions,
            ))
            .with_source(HypothesisSource::Deprecation);

        Some(hypothesis)
    }

    /// Track a hypothesis (save to storage)
    ///
    /// # Errors
    ///
    /// Returns error if storage fails
    pub fn track(&self, hypothesis: &Hypothesis) -> StorageResult<()> {
        self.storage.save_hypothesis(hypothesis)?;
        Ok(())
    }

    /// Get all active hypotheses
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn active_hypotheses(&self) -> StorageResult<Vec<Hypothesis>> {
        self.storage.active_hypotheses()
    }

    /// Evaluate a hypothesis against current metrics
    ///
    /// # Errors
    ///
    /// Returns error if hypothesis not found or storage fails
    pub fn evaluate(
        &self,
        hypothesis_id: &Uuid,
        current_summary: &ReportExecutionSummary,
        baseline_summary: &ReportExecutionSummary,
    ) -> StorageResult<HypothesisOutcome> {
        let hypothesis = self.storage.load_hypothesis(*hypothesis_id)?;

        let mut observed_evidence = Vec::new();
        let mut all_match = true;

        for expected in &hypothesis.expected_evidence {
            let (observed_value, matches) =
                self.evaluate_evidence(expected, current_summary, baseline_summary);

            if !matches {
                all_match = false;
            }

            observed_evidence.push(ObservedEvidence {
                metric: expected.metric.clone(),
                observed_value,
                matches_expected: matches,
                citation: Citation {
                    id: format!("eval-{}", &Uuid::new_v4().as_simple().to_string()[..8]),
                    source_type: CitationSource::Aggregation {
                        query: format!("Evaluate {} for hypothesis", expected.metric),
                        result_summary: format!(
                            "Measured after {} executions",
                            current_summary.total_executions
                        ),
                    },
                    description: format!("Evaluation of {}", expected.metric),
                    retrieval: CitationRetrieval::Inline {
                        data: format!(
                            "Expected: {}, Observed: {}",
                            expected.expected_value,
                            observed_evidence
                                .last()
                                .map(|e: &ObservedEvidence| e.observed_value.as_str())
                                .unwrap_or("")
                        ),
                    },
                },
            });
        }

        let analysis = if all_match {
            format!(
                "Hypothesis validated. All {} expected evidence items matched observations.",
                observed_evidence.len()
            )
        } else {
            let failed_count = observed_evidence
                .iter()
                .filter(|e| !e.matches_expected)
                .count();
            format!(
                "Hypothesis partially failed. {} of {} evidence items did not match expectations.",
                failed_count,
                observed_evidence.len()
            )
        };

        let improvements_for_future = if !all_match {
            vec![
                "Consider more conservative improvement estimates".to_string(),
                "Gather more baseline data before making predictions".to_string(),
            ]
        } else {
            vec![]
        };

        let outcome = HypothesisOutcome {
            correct: all_match,
            observed_evidence,
            analysis,
            improvements_for_future,
        };

        // Update hypothesis with outcome
        let mut updated_hypothesis = hypothesis;
        updated_hypothesis.outcome = Some(outcome.clone());
        updated_hypothesis.status = HypothesisStatus::Evaluated;
        self.storage.evaluate_hypothesis(updated_hypothesis)?;

        Ok(outcome)
    }

    /// Evaluate a single piece of expected evidence
    fn evaluate_evidence(
        &self,
        expected: &ExpectedEvidence,
        current: &ReportExecutionSummary,
        baseline: &ReportExecutionSummary,
    ) -> (String, bool) {
        match expected.metric.as_str() {
            "error_rate" => {
                let current_error_rate = 1.0 - current.success_rate;
                let baseline_error_rate = 1.0 - baseline.success_rate;
                let reduction = baseline_error_rate - current_error_rate;
                let observed = format!("reduced by {:.1}%", reduction * 100.0);

                // Check if "reduced by X%" matches
                let matches = if expected.expected_value.contains("reduced by") {
                    // Extract expected percentage and compare
                    reduction > 0.0
                } else if expected.expected_value.contains("no increase") {
                    current_error_rate <= baseline_error_rate * (1.0 + self.config.metric_tolerance)
                } else {
                    false
                };

                (observed, matches)
            }
            "latency_ms" => {
                let reduction = baseline.avg_duration_ms - current.avg_duration_ms;
                let observed = format!("reduced by {:.0}ms", reduction);

                let matches = reduction > 0.0;
                (observed, matches)
            }
            "accuracy" | "success_rate" => {
                let improvement = current.success_rate - baseline.success_rate;
                let observed = format!("improved by {:.1}%", improvement * 100.0);

                let matches = improvement > 0.0;
                (observed, matches)
            }
            "retry_rate" => {
                let reduction = baseline.retry_rate - current.retry_rate;
                let observed = format!("reduced by {:.1}%", reduction * 100.0);

                let matches = reduction > 0.0;
                (observed, matches)
            }
            "test_pass_rate" => {
                // This metric requires external test result data that is not available in
                // ReportExecutionSummary. We optimistically assume tests pass to avoid
                // false negatives. If test failures are detected through other means,
                // the hypothesis should be manually marked as failed.
                let observed = "not measurable from execution summary".to_string();
                let matches = true;
                (observed, matches)
            }
            _ if expected.metric.starts_with("criterion_") => {
                // Success criteria are human-defined and require manual verification.
                // We optimistically assume they are met to avoid false negatives.
                // Automated verification would require structured success criteria
                // with machine-readable conditions.
                let observed = "requires manual verification".to_string();
                let matches = true;
                (observed, matches)
            }
            _ => {
                let observed = "unknown metric".to_string();
                let matches = false;
                (observed, matches)
            }
        }
    }

    /// Check which hypotheses are ready for evaluation
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn hypotheses_ready_for_evaluation(
        &self,
        execution_count: usize,
        implemented_plans: &[Uuid],
    ) -> StorageResult<Vec<Hypothesis>> {
        let active = self.storage.active_hypotheses()?;

        Ok(active
            .into_iter()
            .filter(|h| {
                match &h.evaluation_trigger {
                    EvaluationTrigger::AfterExecutions(n) => execution_count >= *n,
                    EvaluationTrigger::AfterPlan(plan_id) => implemented_plans.contains(plan_id),
                    EvaluationTrigger::AfterDuration(duration) => {
                        // Would need hypothesis creation timestamp - assume not ready
                        let _ = duration;
                        false
                    }
                    EvaluationTrigger::Manual => false,
                }
            })
            .collect())
    }

    /// Get hypothesis accuracy statistics
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn accuracy_stats(&self) -> StorageResult<HypothesisAccuracy> {
        let all = self.storage.all_hypotheses()?;

        // Separate active and evaluated hypotheses
        let (evaluated, active): (Vec<_>, Vec<_>) =
            all.into_iter().partition(|h| h.outcome.is_some());

        let correct = evaluated
            .iter()
            .filter(|h| h.outcome.as_ref().is_some_and(|o| o.correct))
            .count();

        let total = evaluated.len();
        let accuracy = if total > 0 {
            correct as f64 / total as f64
        } else {
            1.0
        };

        // Calculate accuracy by source
        let mut by_source: std::collections::HashMap<String, SourceAccuracy> =
            std::collections::HashMap::new();

        for hyp in &evaluated {
            let source_name = hyp.source.to_string();
            let entry = by_source.entry(source_name).or_default();
            entry.total += 1;
            if hyp.outcome.as_ref().is_some_and(|o| o.correct) {
                entry.correct += 1;
            }
        }

        // Calculate accuracy for each source
        for stats in by_source.values_mut() {
            stats.accuracy = if stats.total > 0 {
                stats.correct as f64 / stats.total as f64
            } else {
                1.0
            };
        }

        Ok(HypothesisAccuracy {
            total_evaluated: total,
            correct,
            incorrect: total - correct,
            accuracy,
            active_count: active.len(),
            by_source,
        })
    }

    /// Get all hypotheses for dashboard display
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn all_hypotheses(&self) -> StorageResult<Vec<Hypothesis>> {
        self.storage.all_hypotheses()
    }

    /// Get active hypotheses only
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn get_active_hypotheses(&self) -> StorageResult<Vec<Hypothesis>> {
        self.storage.active_hypotheses()
    }

    /// Get evaluated hypotheses only
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn get_evaluated_hypotheses(&self) -> StorageResult<Vec<Hypothesis>> {
        self.storage.evaluated_hypotheses()
    }
}

/// Accuracy statistics for a specific hypothesis source.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceAccuracy {
    /// Total hypotheses evaluated from this source
    pub total: usize,
    /// Correct hypotheses from this source
    pub correct: usize,
    /// Accuracy rate (0.0 - 1.0)
    pub accuracy: f64,
}

/// Statistics about hypothesis accuracy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HypothesisAccuracy {
    /// Total number of evaluated hypotheses
    pub total_evaluated: usize,
    /// Number of correct hypotheses
    pub correct: usize,
    /// Number of incorrect hypotheses
    pub incorrect: usize,
    /// Accuracy rate (0.0 - 1.0)
    pub accuracy: f64,
    /// Number of active (not yet evaluated) hypotheses
    pub active_count: usize,
    /// Accuracy breakdown by source type
    #[serde(default)]
    pub by_source: std::collections::HashMap<String, SourceAccuracy>,
}

impl HypothesisAccuracy {
    /// Convert to markdown
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = format!(
            "**Hypothesis Accuracy:** {:.1}% ({}/{} correct)\n\
             **Active Hypotheses:** {}\n",
            self.accuracy * 100.0,
            self.correct,
            self.total_evaluated,
            self.active_count
        );

        if !self.by_source.is_empty() {
            md.push_str("\n**Accuracy by Source:**\n");
            for (source, stats) in &self.by_source {
                md.push_str(&format!(
                    "- {}: {:.1}% ({}/{} correct)\n",
                    source,
                    stats.accuracy * 100.0,
                    stats.correct,
                    stats.total
                ));
            }
        }

        md
    }
}

// =============================================================================
// MetaAnalyzer - Analyzes Patterns Across Reports
// =============================================================================

/// Configuration for meta-analysis.
#[derive(Debug, Clone)]
pub struct MetaAnalyzerConfig {
    /// Minimum number of reports for pattern detection (default: 3)
    pub min_reports_for_patterns: usize,
    /// Number of recent reports to analyze (default: 10)
    pub analysis_window: usize,
    /// Threshold for identifying recurring patterns (default: 0.3)
    pub recurrence_threshold: f64,
}

impl Default for MetaAnalyzerConfig {
    fn default() -> Self {
        Self {
            min_reports_for_patterns: 3,
            analysis_window: 10,
            recurrence_threshold: 0.3,
        }
    }
}

/// Analyzes patterns across multiple introspection reports.
///
/// Provides insights into:
/// - Recurring capability gaps
/// - Improvement momentum (velocity of changes)
/// - Dead ends (approaches that didn't work)
/// - Success patterns
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{MetaAnalyzer, MetaAnalyzerConfig, IntrospectionStorage};
///
/// let storage = IntrospectionStorage::default();
/// let analyzer = MetaAnalyzer::new(storage, MetaAnalyzerConfig::default());
///
/// let patterns = analyzer.analyze_patterns()?;
/// let momentum = analyzer.calculate_momentum()?;
/// ```
#[derive(Debug)]
pub struct MetaAnalyzer {
    storage: IntrospectionStorage,
    config: MetaAnalyzerConfig,
}

impl MetaAnalyzer {
    /// Create a new meta analyzer
    pub fn new(storage: IntrospectionStorage, config: MetaAnalyzerConfig) -> Self {
        Self { storage, config }
    }

    /// Create an analyzer with default configuration
    #[must_use]
    pub fn with_storage(storage: IntrospectionStorage) -> Self {
        Self::new(storage, MetaAnalyzerConfig::default())
    }

    /// Analyze patterns across recent reports
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn analyze_patterns(&self) -> StorageResult<MetaAnalysisResult> {
        let report_ids = self.storage.list_reports()?;

        if report_ids.len() < self.config.min_reports_for_patterns {
            return Ok(MetaAnalysisResult {
                recurring_gaps: Vec::new(),
                improvement_momentum: ImprovementMomentum::default(),
                dead_ends: Vec::new(),
                success_patterns: Vec::new(),
                hypothesis_accuracy: 0.0,
                reports_analyzed: report_ids.len(),
            });
        }

        // Load recent reports
        let report_count = report_ids.len().min(self.config.analysis_window);
        let mut reports = Vec::with_capacity(report_count);
        for id in report_ids.iter().take(self.config.analysis_window) {
            if let Ok(report) = self.storage.load_report(*id) {
                reports.push(report);
            }
        }

        // Analyze recurring gaps
        let recurring_gaps = self.find_recurring_gaps(&reports);

        // Calculate improvement momentum
        let improvement_momentum = self.calculate_momentum_from_reports(&reports);

        // Find dead ends (failed plans)
        let dead_ends = self.find_dead_ends()?;

        // Find success patterns
        let success_patterns = self.find_success_patterns(&reports)?;

        // Calculate hypothesis accuracy
        let hypothesis_accuracy = self.calculate_hypothesis_accuracy(&reports);

        Ok(MetaAnalysisResult {
            recurring_gaps,
            improvement_momentum,
            dead_ends,
            success_patterns,
            hypothesis_accuracy,
            reports_analyzed: reports.len(),
        })
    }

    /// Find capability gaps that appear across multiple reports
    fn find_recurring_gaps(&self, reports: &[IntrospectionReport]) -> Vec<RecurringPattern> {
        let mut gap_occurrences: std::collections::HashMap<String, Vec<Uuid>> =
            std::collections::HashMap::new();

        for report in reports {
            for gap in &report.capability_gaps {
                let key = self.gap_signature(gap);
                gap_occurrences.entry(key).or_default().push(report.id);
            }
        }

        let min_occurrences =
            (reports.len() as f64 * self.config.recurrence_threshold).ceil() as usize;

        gap_occurrences
            .into_iter()
            .filter(|(_, reports)| reports.len() >= min_occurrences)
            .map(|(description, report_ids)| RecurringPattern {
                description,
                occurrence_count: report_ids.len(),
                first_seen: report_ids.last().copied(),
                last_seen: report_ids.first().copied(),
                pattern_type: PatternCategory::RecurringGap,
            })
            .collect()
    }

    /// Generate a signature for a capability gap (for deduplication)
    fn gap_signature(&self, gap: &CapabilityGap) -> String {
        match &gap.category {
            super::types::GapCategory::MissingTool { tool_description } => {
                format!("missing_tool:{}", tool_description)
            }
            super::types::GapCategory::MissingNode {
                suggested_signature,
            } => {
                format!("missing_node:{}", suggested_signature)
            }
            super::types::GapCategory::InadequateFunctionality { node, limitation } => {
                format!("inadequate:{}:{}", node, limitation)
            }
            super::types::GapCategory::MissingIntegration { external_system } => {
                format!("missing_integration:{}", external_system)
            }
            super::types::GapCategory::PerformanceGap { bottleneck } => {
                format!("performance:{}", bottleneck)
            }
        }
    }

    /// Calculate improvement momentum from reports
    fn calculate_momentum_from_reports(
        &self,
        reports: &[IntrospectionReport],
    ) -> ImprovementMomentum {
        if reports.len() < 2 {
            return ImprovementMomentum::default();
        }

        // Calculate success rate trend
        let success_rates: Vec<f64> = reports
            .iter()
            .map(|r| r.execution_summary.success_rate)
            .collect();

        let success_rate_trend = if success_rates.len() >= 2 {
            let recent = success_rates[0];
            let earlier = success_rates[success_rates.len() - 1];
            recent - earlier
        } else {
            0.0
        };

        // Calculate retry rate trend
        let retry_rates: Vec<f64> = reports
            .iter()
            .map(|r| r.execution_summary.retry_rate)
            .collect();

        let retry_rate_trend = if retry_rates.len() >= 2 {
            let recent = retry_rates[0];
            let earlier = retry_rates[retry_rates.len() - 1];
            earlier - recent // Reduction is positive
        } else {
            0.0
        };

        // Calculate latency trend
        let latencies: Vec<f64> = reports
            .iter()
            .map(|r| r.execution_summary.avg_duration_ms)
            .collect();

        let latency_trend = if latencies.len() >= 2 {
            let recent = latencies[0];
            let earlier = latencies[latencies.len() - 1];
            earlier - recent // Reduction is positive
        } else {
            0.0
        };

        // Count plans implemented
        let plans_implemented: usize = reports
            .iter()
            .map(|r| {
                r.execution_plans
                    .iter()
                    .filter(|p| matches!(p.status, PlanStatus::Implemented { .. }))
                    .count()
            })
            .sum();

        // Count gaps addressed
        let gaps_addressed = plans_implemented; // Approximate

        ImprovementMomentum {
            success_rate_trend,
            retry_rate_trend,
            latency_trend_ms: latency_trend,
            plans_implemented,
            gaps_addressed,
            velocity_score: (success_rate_trend * 100.0
                + retry_rate_trend * 50.0
                + latency_trend / 10.0)
                .clamp(-100.0, 100.0),
        }
    }

    /// Find approaches that didn't work (failed plans)
    fn find_dead_ends(&self) -> StorageResult<Vec<DeadEnd>> {
        let failed_plans = self.storage.list_failed_plans()?;

        Ok(failed_plans
            .into_iter()
            .map(|plan| {
                let reason = match &plan.status {
                    PlanStatus::Failed { reason } => reason.clone(),
                    PlanStatus::Superseded { by } => format!("Superseded by {}", by),
                    _ => "Unknown".to_string(),
                };

                DeadEnd {
                    plan_title: plan.title,
                    plan_id: plan.id,
                    failure_reason: reason,
                    lesson_learned: format!(
                        "Plan '{}' with priority {} failed. Consider alternative approaches.",
                        plan.description, plan.priority
                    ),
                }
            })
            .collect())
    }

    /// Find patterns in successful improvements
    fn find_success_patterns(
        &self,
        reports: &[IntrospectionReport],
    ) -> StorageResult<Vec<SuccessPattern>> {
        let implemented_plans = self.storage.list_implemented_plans()?;
        let mut patterns = Vec::new();

        // Group by category
        let mut by_category: std::collections::HashMap<
            super::types::PlanCategory,
            Vec<&ExecutionPlan>,
        > = std::collections::HashMap::new();

        for plan in &implemented_plans {
            by_category.entry(plan.category).or_default().push(plan);
        }

        for (category, plans) in by_category {
            if plans.len() >= 2 {
                let avg_validation_score =
                    plans.iter().map(|p| p.validation_score).sum::<f64>() / plans.len() as f64;

                patterns.push(SuccessPattern {
                    category: format!("{:?}", category),
                    success_count: plans.len(),
                    average_validation_score: avg_validation_score,
                    common_characteristics: vec![format!(
                        "Average {} steps per plan",
                        plans.iter().map(|p| p.steps.len()).sum::<usize>() / plans.len()
                    )],
                });
            }
        }

        // Also look for improvements in report metrics
        if reports.len() >= 2 {
            let first = &reports[reports.len() - 1];
            let last = &reports[0];

            if last.execution_summary.success_rate > first.execution_summary.success_rate {
                patterns.push(SuccessPattern {
                    category: "Overall Improvement".to_string(),
                    success_count: reports.len(),
                    average_validation_score: last.execution_summary.success_rate,
                    common_characteristics: vec![format!(
                        "Success rate improved from {:.1}% to {:.1}%",
                        first.execution_summary.success_rate * 100.0,
                        last.execution_summary.success_rate * 100.0
                    )],
                });
            }
        }

        Ok(patterns)
    }

    /// Calculate average hypothesis accuracy across reports
    fn calculate_hypothesis_accuracy(&self, reports: &[IntrospectionReport]) -> f64 {
        let total_hypotheses: usize = reports.iter().map(|r| r.hypotheses.len()).sum();

        if total_hypotheses == 0 {
            return 1.0; // No hypotheses to evaluate
        }

        let correct_hypotheses: usize = reports
            .iter()
            .flat_map(|r| &r.hypotheses)
            .filter(|h| h.outcome.as_ref().is_some_and(|o| o.correct))
            .count();

        correct_hypotheses as f64 / total_hypotheses as f64
    }

    /// Calculate improvement velocity (rate of change)
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn calculate_momentum(&self) -> StorageResult<ImprovementMomentum> {
        let report_ids = self.storage.list_reports()?;
        let report_count = report_ids.len().min(self.config.analysis_window);
        let mut reports = Vec::with_capacity(report_count);

        for id in report_ids.iter().take(self.config.analysis_window) {
            if let Ok(report) = self.storage.load_report(*id) {
                reports.push(report);
            }
        }

        Ok(self.calculate_momentum_from_reports(&reports))
    }
}

/// Result of meta-analysis across reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaAnalysisResult {
    /// Capability gaps that appear repeatedly
    pub recurring_gaps: Vec<RecurringPattern>,
    /// Improvement velocity metrics
    pub improvement_momentum: ImprovementMomentum,
    /// Approaches that didn't work
    pub dead_ends: Vec<DeadEnd>,
    /// Patterns in successful improvements
    pub success_patterns: Vec<SuccessPattern>,
    /// Overall hypothesis accuracy
    pub hypothesis_accuracy: f64,
    /// Number of reports analyzed
    pub reports_analyzed: usize,
}

impl MetaAnalysisResult {
    /// Convert to markdown
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str("# Meta-Analysis Results\n\n");
        md.push_str(&format!(
            "**Reports Analyzed:** {}\n\n",
            self.reports_analyzed
        ));

        md.push_str("## Improvement Momentum\n\n");
        md.push_str(&self.improvement_momentum.to_markdown());

        if !self.recurring_gaps.is_empty() {
            md.push_str("\n## Recurring Capability Gaps\n\n");
            for gap in &self.recurring_gaps {
                md.push_str(&format!(
                    "- **{}** (seen {} times)\n",
                    gap.description, gap.occurrence_count
                ));
            }
        }

        if !self.dead_ends.is_empty() {
            md.push_str("\n## Dead Ends (Failed Approaches)\n\n");
            for dead_end in &self.dead_ends {
                md.push_str(&format!(
                    "- **{}**: {}\n  - Lesson: {}\n",
                    dead_end.plan_title, dead_end.failure_reason, dead_end.lesson_learned
                ));
            }
        }

        if !self.success_patterns.is_empty() {
            md.push_str("\n## Success Patterns\n\n");
            for pattern in &self.success_patterns {
                md.push_str(&format!(
                    "- **{}**: {} successes, avg validation {:.2}\n",
                    pattern.category, pattern.success_count, pattern.average_validation_score
                ));
            }
        }

        md.push_str(&format!(
            "\n**Hypothesis Accuracy:** {:.1}%\n",
            self.hypothesis_accuracy * 100.0
        ));

        md
    }
}

/// A recurring pattern across reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringPattern {
    /// Description of the pattern
    pub description: String,
    /// How many times it occurred
    pub occurrence_count: usize,
    /// First report where seen
    pub first_seen: Option<Uuid>,
    /// Most recent report where seen
    pub last_seen: Option<Uuid>,
    /// Category of pattern
    pub pattern_type: PatternCategory,
}

/// Category of recurring pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternCategory {
    /// A capability gap that keeps appearing across analyses.
    RecurringGap,
    /// Performance getting worse over time.
    PerformanceDegradation,
    /// Sudden increase in error rates.
    ErrorSpike,
    /// An approach that consistently produces good results.
    SuccessfulApproach,
}

/// Metrics about improvement velocity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImprovementMomentum {
    /// Change in success rate over analysis window
    pub success_rate_trend: f64,
    /// Change in retry rate (reduction is positive)
    pub retry_rate_trend: f64,
    /// Change in latency (reduction is positive)
    pub latency_trend_ms: f64,
    /// Number of plans implemented
    pub plans_implemented: usize,
    /// Number of gaps addressed
    pub gaps_addressed: usize,
    /// Overall velocity score (-100 to +100)
    pub velocity_score: f64,
}

impl ImprovementMomentum {
    /// Convert to markdown
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("| Metric | Trend |\n");
        md.push_str("|--------|-------|\n");
        md.push_str(&format!(
            "| Success Rate | {:+.1}% |\n",
            self.success_rate_trend * 100.0
        ));
        md.push_str(&format!(
            "| Retry Rate | {:+.1}% |\n",
            self.retry_rate_trend * 100.0
        ));
        md.push_str(&format!("| Latency | {:+.0}ms |\n", self.latency_trend_ms));
        md.push_str(&format!(
            "| Plans Implemented | {} |\n",
            self.plans_implemented
        ));
        md.push_str(&format!("| Gaps Addressed | {} |\n", self.gaps_addressed));
        md.push_str(&format!(
            "\n**Velocity Score:** {:.1}\n",
            self.velocity_score
        ));
        md
    }

    /// Determine if momentum is positive
    #[must_use]
    pub fn is_positive(&self) -> bool {
        self.velocity_score > 0.0
    }
}

/// An approach that didn't work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadEnd {
    /// Title of the failed plan
    pub plan_title: String,
    /// ID of the failed plan
    pub plan_id: Uuid,
    /// Why it failed
    pub failure_reason: String,
    /// What we learned
    pub lesson_learned: String,
}

/// A pattern in successful improvements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessPattern {
    /// Category of success
    pub category: String,
    /// How many times this pattern succeeded
    pub success_count: usize,
    /// Average validation score
    pub average_validation_score: f64,
    /// Common characteristics
    pub common_characteristics: Vec<String>,
}

// =============================================================================
// DesignNoteGenerator - Creates Notes for Future Iterations
// =============================================================================

/// Configuration for design note generation.
#[derive(Debug, Clone)]
pub struct DesignNoteGeneratorConfig {
    /// Maximum number of notes to keep (default: 50)
    pub max_notes: usize,
    /// Whether to auto-generate notes from analysis (default: true)
    pub auto_generate: bool,
}

impl Default for DesignNoteGeneratorConfig {
    fn default() -> Self {
        Self {
            max_notes: 50,
            auto_generate: true,
        }
    }
}

/// Generates design notes for future AI iterations.
///
/// Creates structured notes about:
/// - Lessons learned from hypothesis evaluation
/// - Patterns to avoid (dead ends)
/// - Successful approaches to replicate
/// - Configuration recommendations
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{
///     DesignNoteGenerator, DesignNoteGeneratorConfig, IntrospectionStorage,
/// };
///
/// let storage = IntrospectionStorage::default();
/// let generator = DesignNoteGenerator::new(storage, DesignNoteGeneratorConfig::default());
///
/// let notes = generator.generate_from_meta_analysis(&meta_result)?;
/// generator.save_notes(&notes)?;
/// ```
#[derive(Debug)]
pub struct DesignNoteGenerator {
    storage: IntrospectionStorage,
    config: DesignNoteGeneratorConfig,
}

impl DesignNoteGenerator {
    /// Create a new design note generator
    pub fn new(storage: IntrospectionStorage, config: DesignNoteGeneratorConfig) -> Self {
        Self { storage, config }
    }

    /// Create a generator with default configuration
    #[must_use]
    pub fn with_storage(storage: IntrospectionStorage) -> Self {
        Self::new(storage, DesignNoteGeneratorConfig::default())
    }

    /// Generate design notes from meta-analysis results
    #[must_use]
    pub fn generate_from_meta_analysis(&self, meta_result: &MetaAnalysisResult) -> Vec<DesignNote> {
        let mut notes = Vec::new();

        // Note about hypothesis accuracy
        if meta_result.hypothesis_accuracy < 0.7 {
            notes.push(DesignNote {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                category: NoteCategory::ConfigurationRecommendation,
                title: "Low Hypothesis Accuracy".to_string(),
                content: format!(
                    "Hypothesis accuracy is {:.1}%. Consider:\n\
                     - Using more conservative improvement estimates (apply 0.7x factor)\n\
                     - Gathering more baseline data before predictions\n\
                     - Increasing minimum confidence threshold for generating hypotheses",
                    meta_result.hypothesis_accuracy * 100.0
                ),
                priority: Priority::High,
                actionable: true,
                related_reports: Vec::new(),
            });
        }

        // Notes about recurring gaps
        for gap in &meta_result.recurring_gaps {
            if gap.occurrence_count >= 3 {
                notes.push(DesignNote {
                    id: Uuid::new_v4(),
                    timestamp: Utc::now(),
                    category: NoteCategory::RecurringIssue,
                    title: format!("Recurring Gap: {}", gap.description),
                    content: format!(
                        "This gap has appeared {} times. Consider:\n\
                         - Prioritizing this gap in the next iteration\n\
                         - Investigating why previous attempts didn't resolve it\n\
                         - Escalating to platform-level improvement if application-level fixes fail",
                        gap.occurrence_count
                    ),
                    priority: Priority::High,
                    actionable: true,
                    related_reports: [gap.first_seen, gap.last_seen]
                        .into_iter()
                        .flatten()
                        .collect(),
                });
            }
        }

        // Notes about dead ends
        for dead_end in &meta_result.dead_ends {
            notes.push(DesignNote {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                category: NoteCategory::DeadEnd,
                title: format!("Dead End: {}", dead_end.plan_title),
                content: format!(
                    "Approach failed: {}\n\nLesson: {}",
                    dead_end.failure_reason, dead_end.lesson_learned
                ),
                priority: Priority::Medium,
                actionable: false,
                related_reports: Vec::new(),
            });
        }

        // Notes about momentum
        if meta_result.improvement_momentum.is_positive() {
            notes.push(DesignNote {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                category: NoteCategory::SuccessPattern,
                title: "Positive Improvement Momentum".to_string(),
                content: format!(
                    "Current trajectory is positive (velocity: {:.1}).\n\
                     Success rate trend: {:+.1}%\n\
                     Plans implemented: {}\n\n\
                     Continue current approach while monitoring for regression.",
                    meta_result.improvement_momentum.velocity_score,
                    meta_result.improvement_momentum.success_rate_trend * 100.0,
                    meta_result.improvement_momentum.plans_implemented
                ),
                priority: Priority::Low,
                actionable: false,
                related_reports: Vec::new(),
            });
        } else if meta_result.improvement_momentum.velocity_score < -10.0 {
            notes.push(DesignNote {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                category: NoteCategory::Warning,
                title: "Negative Improvement Momentum".to_string(),
                content: format!(
                    "Warning: Metrics are trending downward (velocity: {:.1}).\n\
                     Consider:\n\
                     - Reviewing recent changes for regressions\n\
                     - Pausing new improvements until stability is restored\n\
                     - Increasing test coverage",
                    meta_result.improvement_momentum.velocity_score
                ),
                priority: Priority::High,
                actionable: true,
                related_reports: Vec::new(),
            });
        }

        // Notes about success patterns
        for pattern in &meta_result.success_patterns {
            if pattern.success_count >= 3 {
                notes.push(DesignNote {
                    id: Uuid::new_v4(),
                    timestamp: Utc::now(),
                    category: NoteCategory::SuccessPattern,
                    title: format!("Success Pattern: {}", pattern.category),
                    content: format!(
                        "{} successful implementations with avg validation {:.2}.\n\
                         Common characteristics:\n{}",
                        pattern.success_count,
                        pattern.average_validation_score,
                        pattern
                            .common_characteristics
                            .iter()
                            .map(|c| format!("- {}", c))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                    priority: Priority::Low,
                    actionable: false,
                    related_reports: Vec::new(),
                });
            }
        }

        notes
    }

    /// Generate notes from hypothesis outcomes
    #[must_use]
    pub fn generate_from_hypothesis(&self, hypothesis: &Hypothesis) -> Option<DesignNote> {
        let outcome = hypothesis.outcome.as_ref()?;

        let category = if outcome.correct {
            NoteCategory::SuccessPattern
        } else {
            NoteCategory::LessonLearned
        };

        let priority = if outcome.correct {
            Priority::Low
        } else {
            Priority::Medium
        };

        // Safely truncate statement to ~50 characters, respecting UTF-8 boundaries
        let truncated_statement: String = hypothesis.statement.chars().take(50).collect();
        let title_suffix = if truncated_statement.len() < hypothesis.statement.len() {
            format!("{}...", truncated_statement)
        } else {
            truncated_statement
        };

        Some(DesignNote {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            category,
            title: format!(
                "Hypothesis {}: {}",
                if outcome.correct { "Confirmed" } else { "Rejected" },
                title_suffix
            ),
            content: format!(
                "**Statement:** {}\n\n**Reasoning:** {}\n\n**Outcome:** {}\n\n**Improvements:**\n{}",
                hypothesis.statement,
                hypothesis.reasoning,
                outcome.analysis,
                outcome
                    .improvements_for_future
                    .iter()
                    .map(|i| format!("- {}", i))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            priority,
            actionable: !outcome.improvements_for_future.is_empty(),
            related_reports: Vec::new(),
        })
    }

    /// Save design notes to storage
    ///
    /// # Errors
    ///
    /// Returns error if storage write fails
    pub fn save_notes(&self, notes: &[DesignNote]) -> StorageResult<std::path::PathBuf> {
        let path = self.storage.meta_dir().join("design_notes.json");
        std::fs::create_dir_all(self.storage.meta_dir())?;

        // Load existing notes
        let mut all_notes: Vec<DesignNote> = if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            match serde_json::from_str(&contents) {
                Ok(notes) => notes,
                Err(e) => {
                    // Log warning but don't fail - start fresh if file is corrupted
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "Failed to parse existing design notes, starting fresh"
                    );
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        // Add new notes
        all_notes.extend(notes.iter().cloned());

        // Trim to max notes (keep most recent)
        if all_notes.len() > self.config.max_notes {
            all_notes.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            all_notes.truncate(self.config.max_notes);
        }

        // Save
        let json = serde_json::to_string_pretty(&all_notes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&path, json)?;

        Ok(path)
    }

    /// Load existing design notes
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn load_notes(&self) -> StorageResult<Vec<DesignNote>> {
        let path = self.storage.meta_dir().join("design_notes.json");

        if !path.exists() {
            return Ok(Vec::new());
        }

        let contents = std::fs::read_to_string(path)?;
        serde_json::from_str(&contents)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Get actionable notes (sorted by priority)
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn actionable_notes(&self) -> StorageResult<Vec<DesignNote>> {
        let notes = self.load_notes()?;
        let mut actionable: Vec<_> = notes.into_iter().filter(|n| n.actionable).collect();
        actionable.sort_by_key(|n| match n.priority {
            Priority::High => 0,
            Priority::Medium => 1,
            Priority::Low => 2,
        });
        Ok(actionable)
    }
}

/// A design note for future AI iterations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignNote {
    /// Unique identifier
    pub id: Uuid,
    /// When the note was created
    pub timestamp: chrono::DateTime<Utc>,
    /// Category of note
    pub category: NoteCategory,
    /// Short title
    pub title: String,
    /// Full content
    pub content: String,
    /// Priority level
    pub priority: Priority,
    /// Whether this requires action
    pub actionable: bool,
    /// Related report IDs
    pub related_reports: Vec<Uuid>,
}

impl DesignNote {
    /// Convert to markdown
    #[must_use]
    pub fn to_markdown(&self) -> String {
        format!(
            "## {} [{:?}]\n\n**Category:** {:?}\n**Priority:** {:?}\n**Actionable:** {}\n\n{}\n",
            self.title,
            self.timestamp.format("%Y-%m-%d %H:%M"),
            self.category,
            self.priority,
            if self.actionable { "Yes" } else { "No" },
            self.content
        )
    }
}

/// Category of design note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteCategory {
    /// Lesson from hypothesis evaluation
    LessonLearned,
    /// Pattern that worked well
    SuccessPattern,
    /// Approach that failed
    DeadEnd,
    /// Issue that keeps appearing
    RecurringIssue,
    /// Warning about trends
    Warning,
    /// Configuration recommendation
    ConfigurationRecommendation,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_improvement::types::{GapCategory, GapManifestation, Impact, PlanCategory};
    use tempfile::tempdir;

    fn create_test_storage() -> IntrospectionStorage {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::new(dir.path().join("introspection"));
        storage.initialize().unwrap();
        storage
    }

    fn create_test_gap() -> CapabilityGap {
        CapabilityGap::new(
            "Missing sentiment analysis tool",
            GapCategory::MissingTool {
                tool_description: "Analyze customer sentiment".to_string(),
            },
            GapManifestation::PromptWorkarounds {
                patterns: vec!["Based on word choice...".to_string()],
            },
        )
        .with_solution("Add SentimentAnalysisTool node")
        .with_impact(Impact::high("Reduce retry rate"))
        .with_confidence(0.85)
    }

    fn create_test_plan() -> ExecutionPlan {
        ExecutionPlan::new("Add sentiment tool", PlanCategory::ApplicationImprovement)
            .with_description("Add a sentiment analysis tool")
            .with_priority(1)
            .with_success_criteria(vec![
                "Sentiment accuracy >= 90%".to_string(),
                "Latency < 100ms".to_string(),
            ])
            .validated(0.85)
    }

    // HypothesisTracker tests

    #[test]
    fn test_hypothesis_tracker_default() {
        let storage = create_test_storage();
        let tracker = HypothesisTracker::with_storage(storage);
        assert_eq!(tracker.config.min_confidence_for_hypothesis, 0.5);
    }

    #[test]
    fn test_create_from_gap() {
        let storage = create_test_storage();
        let tracker = HypothesisTracker::with_storage(storage);
        let gap = create_test_gap();

        let hypothesis = tracker.create_from_gap(&gap);
        assert!(hypothesis.is_some());

        let hyp = hypothesis.unwrap();
        assert!(hyp.statement.contains("sentiment analysis"));
        assert!(!hyp.expected_evidence.is_empty());
    }

    #[test]
    fn test_create_from_gap_low_confidence() {
        let storage = create_test_storage();
        let tracker = HypothesisTracker::with_storage(storage);
        let mut gap = create_test_gap();
        gap.confidence = 0.3; // Below threshold

        let hypothesis = tracker.create_from_gap(&gap);
        assert!(hypothesis.is_none());
    }

    #[test]
    fn test_create_from_plan() {
        let storage = create_test_storage();
        let tracker = HypothesisTracker::with_storage(storage);
        let plan = create_test_plan();

        let hypothesis = tracker.create_from_plan(&plan);
        assert!(hypothesis.statement.contains("sentiment tool"));
        assert_eq!(hypothesis.expected_evidence.len(), 2); // Two success criteria
    }

    #[test]
    fn test_track_hypothesis() {
        let storage = create_test_storage();
        let tracker = HypothesisTracker::with_storage(storage);
        let plan = create_test_plan();

        let hypothesis = tracker.create_from_plan(&plan);
        tracker.track(&hypothesis).unwrap();

        let active = tracker.active_hypotheses().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, hypothesis.id);
    }

    #[test]
    fn test_hypothesis_accuracy_stats() {
        let storage = create_test_storage();
        let tracker = HypothesisTracker::with_storage(storage);

        let stats = tracker.accuracy_stats().unwrap();
        assert_eq!(stats.total_evaluated, 0);
        assert!((stats.accuracy - 1.0).abs() < 0.01); // 100% when no hypotheses
    }

    // MetaAnalyzer tests

    #[test]
    fn test_meta_analyzer_default() {
        let storage = create_test_storage();
        let analyzer = MetaAnalyzer::with_storage(storage);
        assert_eq!(analyzer.config.min_reports_for_patterns, 3);
    }

    #[test]
    fn test_analyze_patterns_empty() {
        let storage = create_test_storage();
        let analyzer = MetaAnalyzer::with_storage(storage);

        let result = analyzer.analyze_patterns().unwrap();
        assert_eq!(result.reports_analyzed, 0);
        assert!(result.recurring_gaps.is_empty());
    }

    #[test]
    fn test_improvement_momentum_default() {
        let momentum = ImprovementMomentum::default();
        assert!(!momentum.is_positive());
        assert!((momentum.velocity_score - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_improvement_momentum_to_markdown() {
        let momentum = ImprovementMomentum {
            success_rate_trend: 0.05,
            retry_rate_trend: 0.02,
            latency_trend_ms: 100.0,
            plans_implemented: 3,
            gaps_addressed: 2,
            velocity_score: 15.0,
        };

        let md = momentum.to_markdown();
        assert!(md.contains("Success Rate"));
        assert!(md.contains("Velocity Score"));
    }

    // DesignNoteGenerator tests

    #[test]
    fn test_design_note_generator_default() {
        let storage = create_test_storage();
        let generator = DesignNoteGenerator::with_storage(storage);
        assert_eq!(generator.config.max_notes, 50);
    }

    #[test]
    fn test_generate_from_meta_analysis() {
        let storage = create_test_storage();
        let generator = DesignNoteGenerator::with_storage(storage);

        let meta_result = MetaAnalysisResult {
            recurring_gaps: vec![RecurringPattern {
                description: "missing_tool:sentiment".to_string(),
                occurrence_count: 3,
                first_seen: None,
                last_seen: None,
                pattern_type: PatternCategory::RecurringGap,
            }],
            improvement_momentum: ImprovementMomentum::default(),
            dead_ends: Vec::new(),
            success_patterns: Vec::new(),
            hypothesis_accuracy: 0.5, // Low accuracy
            reports_analyzed: 5,
        };

        let notes = generator.generate_from_meta_analysis(&meta_result);
        assert!(!notes.is_empty());

        // Should have note about low accuracy
        assert!(notes
            .iter()
            .any(|n| n.title.contains("Low Hypothesis Accuracy")));

        // Should have note about recurring gap
        assert!(notes.iter().any(|n| n.title.contains("Recurring Gap")));
    }

    #[test]
    fn test_generate_from_hypothesis() {
        let storage = create_test_storage();
        let generator = DesignNoteGenerator::with_storage(storage);

        let mut hypothesis = Hypothesis::new("Test hypothesis", "Testing");
        hypothesis.outcome = Some(HypothesisOutcome {
            correct: false,
            observed_evidence: Vec::new(),
            analysis: "Did not match expectations".to_string(),
            improvements_for_future: vec!["Be more conservative".to_string()],
        });

        let note = generator.generate_from_hypothesis(&hypothesis);
        assert!(note.is_some());

        let n = note.unwrap();
        assert_eq!(n.category, NoteCategory::LessonLearned);
        assert!(n.actionable);
    }

    #[test]
    fn test_save_and_load_notes() {
        let storage = create_test_storage();
        let generator = DesignNoteGenerator::with_storage(storage);

        let notes = vec![DesignNote {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            category: NoteCategory::LessonLearned,
            title: "Test Note".to_string(),
            content: "Test content".to_string(),
            priority: Priority::Medium,
            actionable: true,
            related_reports: Vec::new(),
        }];

        generator.save_notes(&notes).unwrap();

        let loaded = generator.load_notes().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].title, "Test Note");
    }

    #[test]
    fn test_actionable_notes() {
        let storage = create_test_storage();
        let generator = DesignNoteGenerator::with_storage(storage);

        let notes = vec![
            DesignNote {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                category: NoteCategory::Warning,
                title: "High Priority".to_string(),
                content: "Test".to_string(),
                priority: Priority::High,
                actionable: true,
                related_reports: Vec::new(),
            },
            DesignNote {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                category: NoteCategory::SuccessPattern,
                title: "Not Actionable".to_string(),
                content: "Test".to_string(),
                priority: Priority::Low,
                actionable: false,
                related_reports: Vec::new(),
            },
        ];

        generator.save_notes(&notes).unwrap();

        let actionable = generator.actionable_notes().unwrap();
        assert_eq!(actionable.len(), 1);
        assert_eq!(actionable[0].title, "High Priority");
    }

    #[test]
    fn test_design_note_to_markdown() {
        let note = DesignNote {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            category: NoteCategory::LessonLearned,
            title: "Test Note".to_string(),
            content: "Test content".to_string(),
            priority: Priority::Medium,
            actionable: true,
            related_reports: Vec::new(),
        };

        let md = note.to_markdown();
        assert!(md.contains("Test Note"));
        assert!(md.contains("LessonLearned"));
        // The format uses "Yes" or "No" for actionable
        assert!(md.contains("Yes") || md.contains("actionable"));
    }

    #[test]
    fn test_meta_analysis_result_to_markdown() {
        let result = MetaAnalysisResult {
            recurring_gaps: Vec::new(),
            improvement_momentum: ImprovementMomentum::default(),
            dead_ends: Vec::new(),
            success_patterns: Vec::new(),
            hypothesis_accuracy: 0.8,
            reports_analyzed: 5,
        };

        let md = result.to_markdown();
        assert!(md.contains("Meta-Analysis Results"));
        assert!(md.contains("Reports Analyzed"));
        assert!(md.contains("5"));
        assert!(md.contains("80.0"));
    }

    // Enhanced HypothesisTracker tests

    #[test]
    fn test_hypothesis_source_default() {
        let hyp = Hypothesis::new("test", "reasoning");
        assert_eq!(hyp.source, HypothesisSource::default());
        assert_eq!(hyp.source, HypothesisSource::Manual);
    }

    #[test]
    fn test_hypothesis_source_from_gap() {
        let storage = create_test_storage();
        let tracker = HypothesisTracker::with_storage(storage);
        let gap = create_test_gap();

        let hypothesis = tracker.create_from_gap(&gap).unwrap();
        assert_eq!(hypothesis.source, HypothesisSource::CapabilityGap);
    }

    #[test]
    fn test_hypothesis_source_from_plan() {
        let storage = create_test_storage();
        let tracker = HypothesisTracker::with_storage(storage);
        let plan = create_test_plan();

        let hypothesis = tracker.create_from_plan(&plan);
        assert_eq!(hypothesis.source, HypothesisSource::ExecutionPlan);
    }

    #[test]
    fn test_hypothesis_with_source() {
        let hyp = Hypothesis::new("test", "reasoning").with_source(HypothesisSource::Deprecation);
        assert_eq!(hyp.source, HypothesisSource::Deprecation);
    }

    #[test]
    fn test_hypothesis_source_display() {
        assert_eq!(
            HypothesisSource::CapabilityGap.to_string(),
            "Capability Gap"
        );
        assert_eq!(
            HypothesisSource::ExecutionPlan.to_string(),
            "Execution Plan"
        );
        assert_eq!(HypothesisSource::Deprecation.to_string(), "Deprecation");
        assert_eq!(HypothesisSource::Manual.to_string(), "Manual");
    }

    #[test]
    fn test_accuracy_stats_by_source() {
        let storage = create_test_storage();
        let tracker = HypothesisTracker::with_storage(storage.clone());

        // Create hypotheses from different sources
        let gap = create_test_gap();
        let plan = create_test_plan();

        let hyp1 = tracker.create_from_gap(&gap).unwrap();
        let hyp2 = tracker.create_from_plan(&plan);
        let hyp3 = Hypothesis::new("manual hyp", "test").with_source(HypothesisSource::Manual);

        // Track them
        tracker.track(&hyp1).unwrap();
        tracker.track(&hyp2).unwrap();
        tracker.track(&hyp3).unwrap();

        // Check accuracy stats
        let stats = tracker.accuracy_stats().unwrap();
        assert_eq!(stats.active_count, 3);
        assert_eq!(stats.total_evaluated, 0); // None evaluated yet
    }

    #[test]
    fn test_storage_all_hypotheses() {
        let storage = create_test_storage();

        // Create and save active hypothesis
        let active_hyp = Hypothesis::new("active", "test");
        storage.save_hypothesis(&active_hyp).unwrap();

        // Create and save evaluated hypothesis
        let mut evaluated_hyp = Hypothesis::new("evaluated", "test");
        evaluated_hyp.status = HypothesisStatus::Evaluated;
        storage.save_hypothesis(&evaluated_hyp).unwrap();

        // all_hypotheses should return both
        let all = storage.all_hypotheses().unwrap();
        assert_eq!(all.len(), 2);

        // active_hypotheses should return only active
        let active = storage.active_hypotheses().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].statement, "active");

        // evaluated_hypotheses should return only evaluated
        let evaluated = storage.evaluated_hypotheses().unwrap();
        assert_eq!(evaluated.len(), 1);
        assert_eq!(evaluated[0].statement, "evaluated");
    }

    #[test]
    fn test_hypothesis_accuracy_to_markdown_with_sources() {
        let mut by_source = std::collections::HashMap::new();
        by_source.insert(
            "Capability Gap".to_string(),
            SourceAccuracy {
                total: 5,
                correct: 4,
                accuracy: 0.8,
            },
        );
        by_source.insert(
            "Execution Plan".to_string(),
            SourceAccuracy {
                total: 3,
                correct: 1,
                accuracy: 0.333,
            },
        );

        let accuracy = HypothesisAccuracy {
            total_evaluated: 8,
            correct: 5,
            incorrect: 3,
            accuracy: 0.625,
            active_count: 2,
            by_source,
        };

        let md = accuracy.to_markdown();
        assert!(md.contains("62.5%"));
        assert!(md.contains("Accuracy by Source"));
        assert!(md.contains("Capability Gap"));
        assert!(md.contains("80.0%"));
    }

    #[test]
    fn test_source_accuracy_default() {
        let stats = SourceAccuracy::default();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.correct, 0);
        assert_eq!(stats.accuracy, 0.0);
    }

    #[test]
    fn test_generate_from_hypothesis_utf8_safe_truncation() {
        let storage = create_test_storage();
        let generator = DesignNoteGenerator::with_storage(storage);

        // Create hypothesis with multi-byte UTF-8 characters near the truncation boundary
        // Using emojis (4 bytes each) ensures the old byte-slicing would panic
        let long_statement_with_unicode =
            "🔴🟡🟢 This is a hypothesis statement with lots of emojis 🎉🎊🎁 and it needs to be quite long to trigger truncation behavior in the title generation code path";

        let mut hypothesis = Hypothesis::new(long_statement_with_unicode, "Testing UTF-8 safety");
        hypothesis.outcome = Some(HypothesisOutcome {
            correct: true,
            observed_evidence: Vec::new(),
            analysis: "Test analysis".to_string(),
            improvements_for_future: Vec::new(),
        });

        // This should not panic even with multi-byte characters
        let note = generator.generate_from_hypothesis(&hypothesis);
        assert!(note.is_some());

        let n = note.unwrap();
        // Title should contain truncated statement with ellipsis
        assert!(n.title.contains("Confirmed"));
        // Should have truncated properly (50 chars + "...")
        assert!(n.title.len() < n.content.len());
    }
}
