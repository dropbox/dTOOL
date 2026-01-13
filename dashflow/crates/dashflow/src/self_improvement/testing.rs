// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for test utilities
// - expect_used: Test builders use expect() for assertions and setup
#![allow(clippy::expect_used)]

//! Test utilities for the self-improvement module.
//!
//! Provides builders, fixtures, and assertion helpers to make testing
//! self-improvement functionality easier and more consistent.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use dashflow::self_improvement::testing::*;
//!
//! // Use builders for custom test data
//! let report = TestReportBuilder::new()
//!     .with_executions(100)
//!     .with_success_rate(0.95)
//!     .with_capability_gap("Missing tool")
//!     .build();
//!
//! // Use fixtures for common scenarios
//! let plan = fixture_validated_plan();
//!
//! // Use assertions for validation
//! assert_report_healthy(&report);
//! assert_plan_ready_for_execution(&plan);
//! ```

use crate::self_improvement::types::{
    CapabilityGap, Citation, DeprecationRecommendation, DeprecationTarget, EvaluationTrigger,
    ExecutionPlan, ExpectedEvidence, GapCategory, GapManifestation, Hypothesis, HypothesisOutcome,
    HypothesisStatus, Impact, ImplementationStep, IntrospectionReport, IntrospectionScope,
    ObservedEvidence, PlanCategory, PlanStatus, Priority, ReportExecutionSummary,
};
use uuid::Uuid;

// =============================================================================
// Test Builders
// =============================================================================

/// Builder for creating test `IntrospectionReport` instances.
///
/// # Example
///
/// ```rust,ignore
/// let report = TestReportBuilder::new()
///     .with_executions(500)
///     .with_success_rate(0.92)
///     .with_avg_duration_ms(150.0)
///     .with_capability_gap("Missing caching layer")
///     .with_deprecation("legacy_node")
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct TestReportBuilder {
    scope: IntrospectionScope,
    total_executions: usize,
    success_rate: f64,
    avg_duration_ms: f64,
    capability_gaps: Vec<String>,
    deprecations: Vec<String>,
    plans: Vec<ExecutionPlan>,
}

impl Default for TestReportBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestReportBuilder {
    /// Create a new builder with sensible defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scope: IntrospectionScope::System,
            total_executions: 100,
            success_rate: 0.90,
            avg_duration_ms: 200.0,
            capability_gaps: Vec::new(),
            deprecations: Vec::new(),
            plans: Vec::new(),
        }
    }

    /// Set the introspection scope.
    #[must_use]
    pub fn with_scope(mut self, scope: IntrospectionScope) -> Self {
        self.scope = scope;
        self
    }

    /// Set total execution count.
    #[must_use]
    pub fn with_executions(mut self, count: usize) -> Self {
        self.total_executions = count;
        self
    }

    /// Set success rate (0.0 - 1.0).
    #[must_use]
    pub fn with_success_rate(mut self, rate: f64) -> Self {
        self.success_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Set average duration in milliseconds.
    #[must_use]
    pub fn with_avg_duration_ms(mut self, ms: f64) -> Self {
        self.avg_duration_ms = ms;
        self
    }

    /// Add a capability gap with the given description.
    #[must_use]
    pub fn with_capability_gap(mut self, description: impl Into<String>) -> Self {
        self.capability_gaps.push(description.into());
        self
    }

    /// Add a deprecation for the given node name.
    #[must_use]
    pub fn with_deprecation(mut self, node_name: impl Into<String>) -> Self {
        self.deprecations.push(node_name.into());
        self
    }

    /// Add an execution plan.
    #[must_use]
    pub fn with_plan(mut self, plan: ExecutionPlan) -> Self {
        self.plans.push(plan);
        self
    }

    /// Build the `IntrospectionReport`.
    #[must_use]
    pub fn build(self) -> IntrospectionReport {
        let mut report = IntrospectionReport::new(self.scope);
        report.execution_summary.total_executions = self.total_executions;
        report.execution_summary.success_rate = self.success_rate;
        report.execution_summary.avg_duration_ms = self.avg_duration_ms;

        for gap_desc in self.capability_gaps {
            report.add_capability_gap(
                CapabilityGap::new(
                    &gap_desc,
                    GapCategory::PerformanceGap {
                        bottleneck: "test_node".to_string(),
                    },
                    GapManifestation::SuboptimalPaths {
                        description: "Detected during testing".to_string(),
                    },
                )
                .with_confidence(0.8),
            );
        }

        for node_name in self.deprecations {
            report.add_deprecation(
                DeprecationRecommendation::new(
                    DeprecationTarget::Node {
                        name: node_name,
                        usage_count: 0,
                    },
                    "Unused for 30+ days",
                )
                .with_confidence(0.9),
            );
        }

        report.execution_plans = self.plans;
        report
    }
}

/// Builder for creating test `ExecutionPlan` instances.
///
/// # Example
///
/// ```rust,ignore
/// let plan = TestPlanBuilder::new("Optimize search")
///     .with_category(PlanCategory::Optimization)
///     .with_priority(1)
///     .with_step("Implement caching", vec!["cache.rs"])
///     .with_step("Add tests", vec!["cache_test.rs"])
///     .validated(0.9)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct TestPlanBuilder {
    title: String,
    description: String,
    category: PlanCategory,
    priority: u8,
    steps: Vec<(String, Vec<String>)>,
    success_criteria: Vec<String>,
    citations: Vec<Citation>,
    status: PlanStatus,
    validation_score: f64,
}

impl TestPlanBuilder {
    /// Create a new builder with the given title.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: String::new(),
            category: PlanCategory::Optimization,
            priority: 2,
            steps: Vec::new(),
            success_criteria: Vec::new(),
            citations: Vec::new(),
            status: PlanStatus::Proposed,
            validation_score: 0.0,
        }
    }

    /// Set the plan description.
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the plan category.
    #[must_use]
    pub fn with_category(mut self, category: PlanCategory) -> Self {
        self.category = category;
        self
    }

    /// Set the priority (1 = highest).
    #[must_use]
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Add an implementation step.
    #[must_use]
    pub fn with_step(mut self, description: impl Into<String>, files: Vec<&str>) -> Self {
        self.steps.push((
            description.into(),
            files.into_iter().map(String::from).collect(),
        ));
        self
    }

    /// Add a success criterion.
    #[must_use]
    pub fn with_criterion(mut self, criterion: impl Into<String>) -> Self {
        self.success_criteria.push(criterion.into());
        self
    }

    /// Add a citation.
    #[must_use]
    pub fn with_citation(mut self, citation: Citation) -> Self {
        self.citations.push(citation);
        self
    }

    /// Mark as validated with the given score.
    #[must_use]
    pub fn validated(mut self, score: f64) -> Self {
        self.status = PlanStatus::Validated;
        self.validation_score = score.clamp(0.0, 1.0);
        self
    }

    /// Mark as in progress (ready for implementation).
    #[must_use]
    pub fn in_progress(mut self) -> Self {
        self.status = PlanStatus::InProgress {
            started: chrono::Utc::now(),
            assignee: "test-assignee".to_string(),
        };
        self
    }

    /// Build the `ExecutionPlan`.
    #[must_use]
    pub fn build(self) -> ExecutionPlan {
        let mut plan = ExecutionPlan::new(&self.title, self.category)
            .with_description(&self.description)
            .with_priority(self.priority);

        let steps: Vec<ImplementationStep> = self
            .steps
            .into_iter()
            .enumerate()
            .map(|(i, (desc, files))| {
                ImplementationStep::new((i + 1) as u8, &desc).with_files(files)
            })
            .collect();

        plan = plan.with_steps(steps);
        plan = plan.with_success_criteria(self.success_criteria);
        plan.citations = self.citations;

        match self.status {
            PlanStatus::Validated => {
                plan = plan.validated(self.validation_score);
            }
            PlanStatus::InProgress { .. } => {
                plan = plan.validated(self.validation_score);
                plan.status = PlanStatus::InProgress {
                    started: chrono::Utc::now(),
                    assignee: "test-assignee".to_string(),
                };
            }
            _ => {}
        }

        plan
    }
}

/// Builder for creating test `Hypothesis` instances.
///
/// # Example
///
/// ```rust,ignore
/// let hypothesis = TestHypothesisBuilder::new("Caching improves latency")
///     .with_rationale("Repeated calls show similar patterns")
///     .with_expected_evidence("latency_reduction", ">= 30%")
///     .with_trigger_after_executions(100)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct TestHypothesisBuilder {
    statement: String,
    rationale: String,
    expected_evidence: Vec<(String, String)>,
    trigger: Option<EvaluationTrigger>,
    status: HypothesisStatus,
}

impl TestHypothesisBuilder {
    /// Create a new builder with the given hypothesis statement.
    #[must_use]
    pub fn new(statement: impl Into<String>) -> Self {
        Self {
            statement: statement.into(),
            rationale: "Generated for testing".to_string(),
            expected_evidence: Vec::new(),
            trigger: None,
            status: HypothesisStatus::Active,
        }
    }

    /// Set the rationale.
    #[must_use]
    pub fn with_rationale(mut self, rationale: impl Into<String>) -> Self {
        self.rationale = rationale.into();
        self
    }

    /// Add expected evidence.
    #[must_use]
    pub fn with_expected_evidence(
        mut self,
        metric_name: impl Into<String>,
        threshold: impl Into<String>,
    ) -> Self {
        self.expected_evidence
            .push((metric_name.into(), threshold.into()));
        self
    }

    /// Set evaluation trigger after N executions.
    #[must_use]
    pub fn with_trigger_after_executions(mut self, count: usize) -> Self {
        self.trigger = Some(EvaluationTrigger::AfterExecutions(count));
        self
    }

    /// Mark as evaluated with confirmed outcome.
    #[must_use]
    pub fn confirmed(mut self) -> Self {
        self.status = HypothesisStatus::Evaluated;
        self
    }

    /// Mark as evaluated with rejected outcome.
    #[must_use]
    pub fn rejected(mut self) -> Self {
        self.status = HypothesisStatus::Evaluated;
        self
    }

    /// Build the `Hypothesis`.
    #[must_use]
    pub fn build(self) -> Hypothesis {
        let evidence: Vec<ExpectedEvidence> = self
            .expected_evidence
            .into_iter()
            .map(|(metric, threshold)| {
                ExpectedEvidence::new(&metric, &threshold, format!("Measure {}", metric))
            })
            .collect();

        let mut hypothesis =
            Hypothesis::new(&self.statement, &self.rationale).with_expected_evidence(evidence);

        if let Some(trigger) = self.trigger {
            hypothesis = hypothesis.with_trigger(trigger);
        }

        hypothesis.status = self.status.clone();

        // If evaluated, set outcome based on the original intent
        if matches!(self.status, HypothesisStatus::Evaluated) {
            // Default to confirmed for evaluated hypotheses
            hypothesis.outcome = Some(HypothesisOutcome {
                correct: true,
                observed_evidence: vec![ObservedEvidence {
                    metric: "test_metric".to_string(),
                    observed_value: "test_value".to_string(),
                    matches_expected: true,
                    citation: Citation::trace("test-evaluation"),
                }],
                analysis: "Test evaluation".to_string(),
                improvements_for_future: Vec::new(),
            });
        }

        hypothesis
    }
}

/// Builder for creating test `CapabilityGap` instances.
///
/// # Example
///
/// ```rust,ignore
/// let gap = TestGapBuilder::new("High latency on embeddings")
///     .performance_gap("embedding_node")
///     .with_solution("Add batch processing")
///     .with_confidence(0.9)
///     .high_impact("50% latency reduction")
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct TestGapBuilder {
    description: String,
    category: GapCategory,
    manifestation: GapManifestation,
    solution: String,
    confidence: f64,
    impact: Option<Impact>,
}

impl TestGapBuilder {
    /// Create a new builder with the given description.
    #[must_use]
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            category: GapCategory::PerformanceGap {
                bottleneck: "unknown".to_string(),
            },
            manifestation: GapManifestation::SuboptimalPaths {
                description: "Detected during analysis".to_string(),
            },
            solution: String::new(),
            confidence: 0.7,
            impact: None,
        }
    }

    /// Set as a performance gap.
    #[must_use]
    pub fn performance_gap(mut self, bottleneck: impl Into<String>) -> Self {
        self.category = GapCategory::PerformanceGap {
            bottleneck: bottleneck.into(),
        };
        self
    }

    /// Set as a missing tool gap.
    #[must_use]
    pub fn missing_tool(mut self, tool_description: impl Into<String>) -> Self {
        self.category = GapCategory::MissingTool {
            tool_description: tool_description.into(),
        };
        self
    }

    /// Set as an inadequate functionality gap.
    #[must_use]
    pub fn inadequate_functionality(
        mut self,
        node: impl Into<String>,
        limitation: impl Into<String>,
    ) -> Self {
        self.category = GapCategory::InadequateFunctionality {
            node: node.into(),
            limitation: limitation.into(),
        };
        self
    }

    /// Set the proposed solution.
    #[must_use]
    pub fn with_solution(mut self, solution: impl Into<String>) -> Self {
        self.solution = solution.into();
        self
    }

    /// Set confidence level.
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set high impact.
    #[must_use]
    pub fn high_impact(mut self, description: impl Into<String>) -> Self {
        self.impact = Some(Impact::high(description));
        self
    }

    /// Set medium impact.
    #[must_use]
    pub fn medium_impact(mut self, description: impl Into<String>) -> Self {
        self.impact = Some(Impact::medium(description));
        self
    }

    /// Build the `CapabilityGap`.
    #[must_use]
    pub fn build(self) -> CapabilityGap {
        let mut gap =
            CapabilityGap::new(&self.description, self.category, self.manifestation.clone())
                .with_confidence(self.confidence);

        if !self.solution.is_empty() {
            gap = gap.with_solution(&self.solution);
        }

        if let Some(impact) = self.impact {
            gap = gap.with_impact(impact);
        }

        gap
    }
}

// =============================================================================
// Test Fixtures
// =============================================================================

/// Create a fixture for a healthy system report.
///
/// Returns a report with:
/// - 1000 executions
/// - 95% success rate
/// - 150ms average duration
/// - No capability gaps
/// - No deprecations
#[must_use]
pub fn fixture_healthy_report() -> IntrospectionReport {
    TestReportBuilder::new()
        .with_executions(1000)
        .with_success_rate(0.95)
        .with_avg_duration_ms(150.0)
        .build()
}

/// Create a fixture for a report with issues.
///
/// Returns a report with:
/// - 500 executions
/// - 75% success rate (below healthy threshold)
/// - 400ms average duration (high latency)
/// - 2 capability gaps
/// - 1 deprecation
#[must_use]
pub fn fixture_report_with_issues() -> IntrospectionReport {
    TestReportBuilder::new()
        .with_executions(500)
        .with_success_rate(0.75)
        .with_avg_duration_ms(400.0)
        .with_capability_gap("High latency on search calls")
        .with_capability_gap("Missing caching layer")
        .with_deprecation("legacy_formatter")
        .build()
}

/// Create a fixture for a validated plan ready for execution.
///
/// Returns a plan that is ready for execution with:
/// - High priority
/// - 2 implementation steps
/// - Success criteria defined
/// - Validated with 0.9 score
#[must_use]
pub fn fixture_validated_plan() -> ExecutionPlan {
    TestPlanBuilder::new("Implement caching layer")
        .with_description("Add Redis-based caching for API responses")
        .with_category(PlanCategory::Optimization)
        .with_priority(1)
        .with_step(
            "Add Redis client dependency",
            vec!["Cargo.toml", "src/cache/mod.rs"],
        )
        .with_step(
            "Implement caching middleware",
            vec!["src/cache/middleware.rs"],
        )
        .with_criterion("Cache hit rate >= 80%")
        .with_criterion("Latency reduced by >= 30%")
        .validated(0.9)
        .build()
}

/// Create a fixture for a proposed (pending review) plan.
#[must_use]
pub fn fixture_proposed_plan() -> ExecutionPlan {
    TestPlanBuilder::new("Refactor error handling")
        .with_description("Consolidate error types")
        .with_category(PlanCategory::ProcessImprovement)
        .with_priority(2)
        .with_step("Create unified error type", vec!["src/error.rs"])
        .with_criterion("All errors derive from unified type")
        .build()
}

/// Create a fixture for an active hypothesis.
#[must_use]
pub fn fixture_active_hypothesis() -> Hypothesis {
    TestHypothesisBuilder::new("Batching API calls will reduce latency by 40%")
        .with_rationale("Current implementation makes sequential calls")
        .with_expected_evidence("latency_reduction", ">= 40%")
        .with_expected_evidence("throughput_increase", ">= 20%")
        .with_trigger_after_executions(200)
        .build()
}

/// Create a fixture for a confirmed hypothesis.
#[must_use]
pub fn fixture_confirmed_hypothesis() -> Hypothesis {
    TestHypothesisBuilder::new("Caching improves response time")
        .with_rationale("Repeated queries hit same data")
        .with_expected_evidence("cache_hit_rate", ">= 50%")
        .confirmed()
        .build()
}

/// Create a fixture for a high-impact capability gap.
#[must_use]
pub fn fixture_high_impact_gap() -> CapabilityGap {
    TestGapBuilder::new("Missing rate limiting causes API throttling")
        .missing_tool("Rate limiter for external API calls")
        .with_solution("Implement token bucket rate limiter")
        .with_confidence(0.95)
        .high_impact("Eliminate 429 errors, improve reliability")
        .build()
}

// =============================================================================
// Assertion Helpers
// =============================================================================

/// Assert that a report represents a healthy system.
///
/// A healthy report has:
/// - Success rate >= 90%
/// - Average duration <= 300ms
/// - No critical capability gaps
///
/// # Panics
///
/// Panics if the report does not meet health criteria.
pub fn assert_report_healthy(report: &IntrospectionReport) {
    assert!(
        report.execution_summary.success_rate >= 0.90,
        "Report success rate {} is below healthy threshold (0.90)",
        report.execution_summary.success_rate
    );

    assert!(
        report.execution_summary.avg_duration_ms <= 300.0,
        "Report avg duration {}ms exceeds healthy threshold (300ms)",
        report.execution_summary.avg_duration_ms
    );

    let high_priority_gaps: Vec<_> = report
        .capability_gaps
        .iter()
        .filter(|g| matches!(g.priority(), Priority::High))
        .collect();

    assert!(
        high_priority_gaps.is_empty(),
        "Report has {} high-priority capability gaps: {:?}",
        high_priority_gaps.len(),
        high_priority_gaps
            .iter()
            .map(|g| &g.description)
            .collect::<Vec<_>>()
    );
}

/// Assert that a plan is ready for execution.
///
/// A ready plan:
/// - Has status Validated or InProgress
/// - Has at least one implementation step
/// - Has at least one success criterion
///
/// # Panics
///
/// Panics if the plan is not ready for execution.
pub fn assert_plan_ready_for_execution(plan: &ExecutionPlan) {
    assert!(
        matches!(
            plan.status,
            PlanStatus::Validated | PlanStatus::InProgress { .. }
        ),
        "Plan status {:?} is not Validated or InProgress",
        plan.status
    );

    assert!(!plan.steps.is_empty(), "Plan has no implementation steps");

    assert!(
        !plan.success_criteria.is_empty(),
        "Plan has no success criteria"
    );
}

/// Assert that a plan has been validated.
///
/// # Panics
///
/// Panics if the plan is not validated or has low validation score.
pub fn assert_plan_validated(plan: &ExecutionPlan, min_score: f64) {
    assert!(
        matches!(
            plan.status,
            PlanStatus::Validated | PlanStatus::InProgress { .. } | PlanStatus::Implemented { .. }
        ),
        "Plan status {:?} is not Validated or beyond",
        plan.status
    );

    assert!(
        plan.validation_score >= min_score,
        "Plan validation score {} is below minimum {}",
        plan.validation_score,
        min_score
    );
}

/// Assert that a hypothesis has expected evidence defined.
///
/// # Panics
///
/// Panics if the hypothesis has no expected evidence.
pub fn assert_hypothesis_has_evidence(hypothesis: &Hypothesis) {
    assert!(
        !hypothesis.expected_evidence.is_empty(),
        "Hypothesis '{}' has no expected evidence defined",
        hypothesis.statement
    );
}

/// Assert that a hypothesis has been evaluated.
///
/// # Panics
///
/// Panics if the hypothesis is not in Evaluated status with an outcome.
pub fn assert_hypothesis_evaluated(hypothesis: &Hypothesis) {
    assert!(
        matches!(hypothesis.status, HypothesisStatus::Evaluated),
        "Hypothesis status {:?} is not Evaluated",
        hypothesis.status
    );
    assert!(
        hypothesis.outcome.is_some(),
        "Hypothesis has Evaluated status but no outcome"
    );
}

/// Assert that a capability gap has a proposed solution.
///
/// # Panics
///
/// Panics if the gap has no proposed solution.
pub fn assert_gap_has_solution(gap: &CapabilityGap) {
    assert!(
        !gap.proposed_solution.is_empty(),
        "Capability gap '{}' has no proposed solution",
        gap.description
    );
}

/// Assert that execution summary shows improvement over previous.
///
/// # Panics
///
/// Panics if there's no comparison or metrics haven't improved.
pub fn assert_execution_improved(summary: &ReportExecutionSummary) {
    let comparison = summary
        .vs_previous
        .as_ref()
        .expect("ReportExecutionSummary has no comparison to previous");

    assert!(
        comparison.success_rate_delta >= 0.0,
        "Success rate decreased by {}",
        comparison.success_rate_delta.abs()
    );

    assert!(
        comparison.duration_delta_ms <= 0.0,
        "Duration increased by {}ms",
        comparison.duration_delta_ms
    );
}

// =============================================================================
// Test Data Generators
// =============================================================================

/// Generate a unique thread ID for testing.
#[must_use]
pub fn generate_test_thread_id() -> String {
    format!("test-thread-{}", Uuid::new_v4())
}

/// Generate a batch of test capability gaps.
#[must_use]
pub fn generate_test_gaps(count: usize) -> Vec<CapabilityGap> {
    (0..count)
        .map(|i| {
            TestGapBuilder::new(format!("Test gap {}", i + 1))
                .performance_gap(format!("node_{}", i))
                .with_confidence(0.7 + (i as f64 * 0.05).min(0.25))
                .build()
        })
        .collect()
}

/// Generate a batch of test execution plans.
#[must_use]
pub fn generate_test_plans(count: usize) -> Vec<ExecutionPlan> {
    (0..count)
        .map(|i| {
            TestPlanBuilder::new(format!("Test plan {}", i + 1))
                .with_priority((i % 3 + 1) as u8)
                .with_step(format!("Step 1 for plan {}", i + 1), vec!["file.rs"])
                .build()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_builder_defaults() {
        let report = TestReportBuilder::new().build();
        assert_eq!(report.execution_summary.total_executions, 100);
        assert!((report.execution_summary.success_rate - 0.90).abs() < f64::EPSILON);
    }

    #[test]
    fn test_report_builder_with_gaps() {
        let report = TestReportBuilder::new()
            .with_capability_gap("Gap 1")
            .with_capability_gap("Gap 2")
            .build();
        assert_eq!(report.capability_gaps.len(), 2);
    }

    #[test]
    fn test_report_builder_with_deprecations() {
        let report = TestReportBuilder::new()
            .with_deprecation("old_node")
            .build();
        assert_eq!(report.deprecations.len(), 1);
    }

    #[test]
    fn test_plan_builder_basic() {
        let plan = TestPlanBuilder::new("Test Plan")
            .with_priority(1)
            .with_step("Do something", vec!["file.rs"])
            .build();
        assert_eq!(plan.title, "Test Plan");
        assert_eq!(plan.priority, 1);
        assert_eq!(plan.steps.len(), 1);
    }

    #[test]
    fn test_plan_builder_validated() {
        let plan = TestPlanBuilder::new("Validated Plan")
            .validated(0.85)
            .build();
        assert!(matches!(plan.status, PlanStatus::Validated));
        assert!((plan.validation_score - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_plan_builder_in_progress() {
        let plan = TestPlanBuilder::new("InProgress Plan")
            .in_progress()
            .build();
        assert!(matches!(plan.status, PlanStatus::InProgress { .. }));
    }

    #[test]
    fn test_hypothesis_builder_basic() {
        let hypothesis = TestHypothesisBuilder::new("Test hypothesis")
            .with_expected_evidence("metric", ">= 0.5")
            .build();
        assert_eq!(hypothesis.statement, "Test hypothesis");
        assert_eq!(hypothesis.expected_evidence.len(), 1);
    }

    #[test]
    fn test_hypothesis_builder_evaluated() {
        let hypothesis = TestHypothesisBuilder::new("Evaluated").confirmed().build();
        assert!(matches!(hypothesis.status, HypothesisStatus::Evaluated));
        assert!(hypothesis.outcome.is_some());
    }

    #[test]
    fn test_gap_builder_performance() {
        let gap = TestGapBuilder::new("Slow node")
            .performance_gap("slow_node")
            .with_solution("Optimize it")
            .high_impact("50% improvement")
            .build();
        assert!(matches!(gap.category, GapCategory::PerformanceGap { .. }));
        assert!(!gap.proposed_solution.is_empty());
    }

    #[test]
    fn test_fixture_healthy_report() {
        let report = fixture_healthy_report();
        assert_report_healthy(&report);
    }

    #[test]
    fn test_fixture_validated_plan() {
        let plan = fixture_validated_plan();
        assert_plan_ready_for_execution(&plan);
    }

    #[test]
    fn test_fixture_active_hypothesis() {
        let hypothesis = fixture_active_hypothesis();
        assert_hypothesis_has_evidence(&hypothesis);
    }

    #[test]
    fn test_fixture_high_impact_gap() {
        let gap = fixture_high_impact_gap();
        assert_gap_has_solution(&gap);
    }

    #[test]
    fn test_generate_test_gaps() {
        let gaps = generate_test_gaps(5);
        assert_eq!(gaps.len(), 5);
        for gap in &gaps {
            assert!(gap.confidence >= 0.7);
        }
    }

    #[test]
    fn test_generate_test_plans() {
        let plans = generate_test_plans(3);
        assert_eq!(plans.len(), 3);
        for plan in &plans {
            assert!(!plan.steps.is_empty());
        }
    }

    // NOTE: The following tests use #[should_panic] INTENTIONALLY.
    // These test assertion helper functions (assert_report_healthy, assert_plan_ready_for_execution)
    // which are designed to panic when validation fails, similar to std::assert!().
    // Testing that assertions panic is the correct behavior here.

    #[test]
    #[should_panic(expected = "success rate")]
    fn test_assert_report_healthy_fails_on_low_success() {
        let report = TestReportBuilder::new().with_success_rate(0.5).build();
        assert_report_healthy(&report);
    }

    #[test]
    #[should_panic(expected = "not Validated")]
    fn test_assert_plan_ready_fails_on_proposed() {
        let plan = TestPlanBuilder::new("Proposed").build();
        assert_plan_ready_for_execution(&plan);
    }
}
