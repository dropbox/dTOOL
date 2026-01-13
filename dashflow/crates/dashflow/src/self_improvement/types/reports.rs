// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]
// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Introspection report types - the primary output of the self-improvement system.
//!
//! This module provides the `IntrospectionReport` struct which aggregates
//! all analysis results from a self-improvement cycle.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::citations::Citation;
use super::common::Priority;
use super::consensus::ConsensusResult;
use super::gaps::{
    CapabilityGap, DeprecationRecommendation, DeprecationTarget, GapCategory, GapManifestation,
    ImprovementProposal, RetrospectiveAnalysis,
};
use super::hypothesis::Hypothesis;
use super::plans::ExecutionPlan;

// =============================================================================
// IntrospectionReport - Complete Self-Analysis Report
// =============================================================================

/// A complete introspection report for one analysis cycle.
///
/// This is the primary output of the self-improvement system. It contains
/// capability gap analysis, deprecation recommendations, retrospective insights,
/// multi-model consensus results, and validated execution plans.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{
///     IntrospectionReport, IntrospectionScope, IntrospectionStorage,
///     CapabilityGap, GapCategory, GapManifestation,
/// };
///
/// // Create a new report
/// let mut report = IntrospectionReport::new(IntrospectionScope::System);
///
/// // Add a capability gap
/// report.add_capability_gap(
///     CapabilityGap::new(
///         "Missing sentiment analysis",
///         GapCategory::MissingTool {
///             tool_description: "Analyze text sentiment".to_string(),
///         },
///         GapManifestation::PromptWorkarounds {
///             patterns: vec!["Based on word choice...".to_string()],
///         },
///     )
///     .with_confidence(0.85),
/// );
///
/// // Generate human-readable output
/// let markdown = report.to_markdown();
///
/// // Save for later analysis
/// let storage = IntrospectionStorage::default();
/// storage.save_report(&report)?;
/// ```
///
/// # Errors
///
/// - [`serde_json::Error`] - Returned by [`IntrospectionReport::to_json`] and
///   [`IntrospectionReport::from_json`] on
///   serialization/deserialization failure
///
/// # See Also
///
/// - [`IntrospectionScope`] - Defines what was analyzed (system, graph, execution)
/// - [`CapabilityGap`] - Individual capability gap analysis
/// - [`ExecutionPlan`] - Actionable improvement plans
/// - [`crate::self_improvement::IntrospectionStorage`] - Persist and retrieve reports
/// - [`Hypothesis`] - Predictions about outcomes
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IntrospectionReport {
    /// Unique identifier for this report
    pub id: Uuid,

    /// When this analysis was performed
    pub timestamp: DateTime<Utc>,

    /// Scope of analysis
    pub scope: IntrospectionScope,

    /// Statistical summary of analyzed executions
    pub execution_summary: ReportExecutionSummary,

    /// Capability gap analysis results
    pub capability_gaps: Vec<CapabilityGap>,

    /// Deprecation recommendations
    pub deprecations: Vec<DeprecationRecommendation>,

    /// Retrospective insights
    pub retrospective: RetrospectiveAnalysis,

    /// Generated improvement proposals (pre-validation)
    pub proposals: Vec<ImprovementProposal>,

    /// Multi-model consensus results
    pub consensus: Option<ConsensusResult>,

    /// Final validated execution plans
    pub execution_plans: Vec<ExecutionPlan>,

    /// Hypotheses about future outcomes
    pub hypotheses: Vec<Hypothesis>,

    /// Links to source data (execution traces, logs)
    pub citations: Vec<Citation>,

    /// Previous report ID for chain tracking
    pub previous_report_id: Option<Uuid>,
}

impl IntrospectionReport {
    /// Create a new empty introspection report
    #[must_use]
    pub fn new(scope: IntrospectionScope) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            scope,
            execution_summary: ReportExecutionSummary::default(),
            capability_gaps: Vec::new(),
            deprecations: Vec::new(),
            retrospective: RetrospectiveAnalysis::default(),
            proposals: Vec::new(),
            consensus: None,
            execution_plans: Vec::new(),
            hypotheses: Vec::new(),
            citations: Vec::new(),
            previous_report_id: None,
        }
    }

    /// Convert report to JSON string
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parse report from JSON string
    ///
    /// # Errors
    ///
    /// Returns error if deserialization fails
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Convert report to markdown for human consumption
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!(
            "# Introspection Report: {}\n\n",
            self.timestamp.format("%Y-%m-%dT%H:%M:%S")
        ));
        md.push_str(&format!("**Report ID:** {}\n", self.id));
        md.push_str(&format!("**Scope:** {}\n", self.scope.description()));

        if let Some(prev) = &self.previous_report_id {
            md.push_str(&format!("**Previous Report:** {prev}\n"));
        }

        md.push_str("\n## Execution Summary\n\n");
        md.push_str(&self.execution_summary.to_markdown());

        if !self.capability_gaps.is_empty() {
            md.push_str("\n## Capability Gap Analysis\n\n");
            for (i, gap) in self.capability_gaps.iter().enumerate() {
                md.push_str(&format!(
                    "### Gap {}: {} [{:?}]\n\n",
                    i + 1,
                    gap.description,
                    gap.priority()
                ));
                md.push_str(&gap.to_markdown());
                md.push_str("\n---\n\n");
            }
        }

        if !self.deprecations.is_empty() {
            md.push_str("\n## Deprecation Recommendations\n\n");
            for (i, dep) in self.deprecations.iter().enumerate() {
                md.push_str(&format!("### Deprecation {}: {:?}\n\n", i + 1, dep.target));
                md.push_str(&dep.to_markdown());
                md.push_str("\n---\n\n");
            }
        }

        if let Some(consensus) = &self.consensus {
            md.push_str("\n## Multi-Model Consensus\n\n");
            md.push_str(&consensus.to_markdown());
        }

        if !self.execution_plans.is_empty() {
            md.push_str("\n## Execution Plans\n\n");
            for plan in &self.execution_plans {
                md.push_str(&format!(
                    "### Plan {}: {} [VALIDATED]\n\n",
                    plan.id, plan.title
                ));
                md.push_str(&plan.to_markdown());
                md.push_str("\n---\n\n");
            }
        }

        if !self.hypotheses.is_empty() {
            md.push_str("\n## Hypotheses\n\n");
            for hyp in &self.hypotheses {
                md.push_str(&format!("### Hypothesis {}: {}\n\n", hyp.id, hyp.statement));
                md.push_str(&hyp.to_markdown());
                md.push_str("\n---\n\n");
            }
        }

        if !self.citations.is_empty() {
            md.push_str("\n## Citations\n\n");
            for citation in &self.citations {
                md.push_str(&format!("- {}: {}\n", citation.id, citation.description));
            }
        }

        md
    }

    /// Add a capability gap to the report
    pub fn add_capability_gap(&mut self, gap: CapabilityGap) {
        // Add citations from gap to report citations
        self.citations.extend(gap.evidence.clone());
        self.capability_gaps.push(gap);
    }

    /// Add a deprecation recommendation to the report
    pub fn add_deprecation(&mut self, dep: DeprecationRecommendation) {
        self.citations.extend(dep.evidence.clone());
        self.deprecations.push(dep);
    }

    /// Add an execution plan to the report
    pub fn add_execution_plan(&mut self, plan: ExecutionPlan) {
        self.citations.extend(plan.citations.clone());
        self.execution_plans.push(plan);
    }

    /// Add a hypothesis to the report
    pub fn add_hypothesis(&mut self, hypothesis: Hypothesis) {
        self.hypotheses.push(hypothesis);
    }

    // =========================================================================
    // Package Contribution Generation
    // =========================================================================
    // These methods bridge IntrospectionReport to the package contribution system,
    // enabling AI agents to contribute bug reports, improvements, and package
    // requests based on capability gap analysis.

    /// Generate bug reports from capability gaps that indicate inadequate functionality.
    #[must_use]
    pub fn generate_bug_reports<F>(
        &self,
        reporter: crate::packages::ReporterIdentity,
        package_lookup: F,
    ) -> Vec<crate::packages::PackageBugReport>
    where
        F: Fn(&str) -> Option<crate::packages::ContributionPackageRef>,
    {
        use crate::packages::{
            BugSeverity, DiscoveryMethod, Evidence, PackageBugReport, SuggestedFix,
        };

        let mut reports = Vec::new();

        for gap in &self.capability_gaps {
            // Only generate bug reports for inadequate functionality gaps
            if let GapCategory::InadequateFunctionality { node, limitation } = &gap.category {
                // Try to find the package that provides this node
                if let Some(package_ref) = package_lookup(node) {
                    let severity = match gap.priority() {
                        Priority::High => BugSeverity::High,
                        Priority::Medium => BugSeverity::Medium,
                        Priority::Low => BugSeverity::Low,
                    };

                    let mut bug_report = PackageBugReport::new(package_ref, reporter.clone())
                        .with_title(format!("Inadequate functionality in {}", node))
                        .with_description(format!(
                            "Node {} has limitation: {}\n\n{}",
                            node, limitation, gap.description
                        ))
                        .with_discovery(DiscoveryMethod::Introspection {
                            report_id: self.id,
                            gap_description: Some(gap.description.clone()),
                        })
                        .with_severity(severity)
                        .with_evidence(Evidence::introspection_report(self.id))
                        .with_label("ai-discovered")
                        .with_label("inadequate-functionality");

                    // Add manifestation-based reproduction info
                    if let Some(reproduction) =
                        self.create_reproduction_from_manifestation(&gap.manifestation)
                    {
                        bug_report = bug_report.with_reproduction(reproduction);
                    }

                    // Add suggested fix if a solution is proposed
                    if !gap.proposed_solution.is_empty() {
                        bug_report = bug_report.with_suggested_fix(
                            SuggestedFix::new(&gap.proposed_solution)
                                .with_confidence(gap.confidence),
                        );
                    }

                    // Add evidence from citations
                    for citation in &gap.evidence {
                        bug_report = bug_report.with_evidence(Evidence::citation(
                            format!("{:?}", citation.source_type),
                            citation.description.clone(),
                        ));
                    }

                    reports.push(bug_report);
                }
            }
        }

        reports
    }

    /// Generate package requests from capability gaps that indicate missing tools or nodes.
    #[must_use]
    pub fn generate_package_requests(
        &self,
        requester: crate::packages::ReporterIdentity,
    ) -> Vec<crate::packages::NewPackageRequest> {
        use crate::packages::{Evidence, NewPackageRequest, PackageType, RequestPriority};

        let mut requests = Vec::new();

        for gap in &self.capability_gaps {
            let (needed_functionality, package_type, title) = match &gap.category {
                GapCategory::MissingTool { tool_description } => (
                    tool_description.clone(),
                    Some(PackageType::ToolPack),
                    format!("Tool needed: {}", tool_description),
                ),
                GapCategory::MissingNode {
                    suggested_signature,
                } => (
                    suggested_signature.clone(),
                    Some(PackageType::NodeLibrary),
                    format!("Node needed: {}", suggested_signature),
                ),
                GapCategory::MissingIntegration { external_system } => (
                    format!("{} connector", external_system),
                    Some(PackageType::ModelConnector),
                    format!("Integration needed: {}", external_system),
                ),
                // Skip other gap types for package requests
                _ => continue,
            };

            let priority = match gap.priority() {
                Priority::High => RequestPriority::High,
                Priority::Medium => RequestPriority::Medium,
                Priority::Low => RequestPriority::Low,
            };

            let mut request = NewPackageRequest::new(requester.clone(), needed_functionality)
                .with_title(title)
                .with_use_case(gap.description.clone())
                .with_priority(priority)
                .with_evidence(Evidence::introspection_report(self.id))
                .with_label("ai-requested");

            if let Some(pkg_type) = package_type {
                request = request.with_package_type(pkg_type);
            }

            // Add suggested approach if we have a proposed solution
            if !gap.proposed_solution.is_empty() {
                request = request.with_suggested_approach(&gap.proposed_solution);
            }

            // Add evidence from citations
            for citation in &gap.evidence {
                request = request.with_evidence(Evidence::citation(
                    format!("{:?}", citation.source_type),
                    citation.description.clone(),
                ));
            }

            requests.push(request);
        }

        requests
    }

    /// Generate improvement suggestions from capability gaps that indicate performance issues
    /// or from deprecation recommendations.
    #[must_use]
    pub fn generate_improvements<F>(
        &self,
        suggester: crate::packages::ReporterIdentity,
        package_lookup: F,
    ) -> Vec<crate::packages::PackageImprovement>
    where
        F: Fn(&str) -> Option<crate::packages::ContributionPackageRef>,
    {
        use crate::packages::{
            Evidence, ExpectedImpact, ImpactLevel, ImpactMetric, ImprovementPriority,
            ImprovementType, PackageImprovement,
        };

        let mut improvements = Vec::new();

        // Generate improvements from performance gaps
        for gap in &self.capability_gaps {
            if let GapCategory::PerformanceGap { bottleneck } = &gap.category {
                // Try to find the package responsible for the bottleneck
                if let Some(package_ref) = package_lookup(bottleneck) {
                    let priority = match gap.priority() {
                        Priority::High => ImprovementPriority::High,
                        Priority::Medium => ImprovementPriority::Medium,
                        Priority::Low => ImprovementPriority::Low,
                    };

                    let impact_level = match gap.priority() {
                        Priority::High => ImpactLevel::High,
                        Priority::Medium => ImpactLevel::Medium,
                        Priority::Low => ImpactLevel::Low,
                    };

                    let mut improvement = PackageImprovement::new(package_ref, suggester.clone())
                        .with_title(format!("Performance improvement for {}", bottleneck))
                        .with_improvement_type(ImprovementType::Performance {
                            metric: "latency_ms".to_string(),
                            current: gap.expected_impact.latency_reduction_ms + 100.0, // Estimate current
                            target: 100.0, // Target latency
                            unit: Some("ms".to_string()),
                        })
                        .with_description(format!(
                            "{}\n\nBottleneck: {}",
                            gap.description, bottleneck
                        ))
                        .with_expected_impact(
                            ExpectedImpact::new(impact_level, &gap.expected_impact.description)
                                .with_metric(
                                    ImpactMetric::new(
                                        "latency_ms",
                                        gap.expected_impact.latency_reduction_ms + 100.0,
                                        100.0,
                                    )
                                    .with_unit("ms"),
                                ),
                        )
                        .with_priority(priority)
                        .with_evidence(Evidence::introspection_report(self.id))
                        .with_label("ai-suggested")
                        .with_label("performance");

                    // Add evidence from citations
                    for citation in &gap.evidence {
                        improvement = improvement.with_evidence(Evidence::citation(
                            format!("{:?}", citation.source_type),
                            citation.description.clone(),
                        ));
                    }

                    improvements.push(improvement);
                }
            }
        }

        // Generate improvements from deprecation recommendations
        for dep in &self.deprecations {
            let component_name = match &dep.target {
                DeprecationTarget::Node { name, .. } => name.clone(),
                DeprecationTarget::Tool { name, .. } => name.clone(),
                DeprecationTarget::Feature { name } => name.clone(),
                DeprecationTarget::CodePath { location } => location.clone(),
                DeprecationTarget::Edge { from, to } => format!("{}->{}", from, to),
            };

            if let Some(package_ref) = package_lookup(&component_name) {
                let priority = if dep.confidence > 0.8 {
                    ImprovementPriority::High
                } else if dep.confidence > 0.5 {
                    ImprovementPriority::Medium
                } else {
                    ImprovementPriority::Low
                };

                let impact_level = if dep.confidence > 0.8 {
                    ImpactLevel::High
                } else if dep.confidence > 0.5 {
                    ImpactLevel::Medium
                } else {
                    ImpactLevel::Low
                };

                let mut improvement = PackageImprovement::new(package_ref, suggester.clone())
                    .with_title(format!("Deprecation suggestion: {}", component_name))
                    .with_improvement_type(ImprovementType::Feature {
                        description: format!(
                            "Consider removing or simplifying: {}",
                            component_name
                        ),
                        use_cases: dep.benefits.clone(),
                    })
                    .with_description(format!(
                        "{}\n\nBenefits: {}\n\nRisks: {}",
                        dep.rationale,
                        dep.benefits.join(", "),
                        dep.risks.join(", ")
                    ))
                    .with_expected_impact(ExpectedImpact::new(impact_level, dep.rationale.clone()))
                    .with_priority(priority)
                    .with_evidence(Evidence::introspection_report(self.id))
                    .with_label("ai-suggested")
                    .with_label("deprecation");

                // Add evidence from citations
                for citation in &dep.evidence {
                    improvement = improvement.with_evidence(Evidence::citation(
                        format!("{:?}", citation.source_type),
                        citation.description.clone(),
                    ));
                }

                improvements.push(improvement);
            }
        }

        improvements
    }

    /// Helper to create reproduction steps from gap manifestation
    fn create_reproduction_from_manifestation(
        &self,
        manifestation: &GapManifestation,
    ) -> Option<crate::packages::ReproductionSteps> {
        use crate::packages::ReproductionSteps;

        match manifestation {
            GapManifestation::Errors {
                count,
                sample_messages,
            } => Some(
                ReproductionSteps::new(
                    "No error",
                    format!(
                        "Error occurred ({} times): {}",
                        count,
                        sample_messages.join("; ")
                    ),
                )
                .with_step("Execute the graph with typical input")
                .with_step("Observe the error in the affected node")
                .reproducible(*count > 1),
            ),
            GapManifestation::Retries {
                rate,
                affected_nodes,
            } => Some(
                ReproductionSteps::new(
                    "Success without retries",
                    format!("High retry rate ({:.1}%)", rate * 100.0),
                )
                .with_step("Execute the graph multiple times")
                .with_step(format!(
                    "Observe retry patterns in nodes: {}",
                    affected_nodes.join(", ")
                ))
                .reproducible(true),
            ),
            GapManifestation::SuboptimalPaths { description } => Some(
                ReproductionSteps::new("Optimal execution path", description.clone())
                    .with_step("Execute the graph")
                    .with_step("Trace the execution path")
                    .reproducible(true),
            ),
            _ => None,
        }
    }
}

// =============================================================================
// IntrospectionScope - Scope of Analysis
// =============================================================================

/// Scope of introspection analysis.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum IntrospectionScope {
    /// Single graph execution.
    Execution {
        /// ID of the execution thread being analyzed.
        thread_id: String,
    },
    /// Multiple executions of same graph.
    GraphAggregate {
        /// ID of the graph being analyzed.
        graph_id: String,
        /// Number of executions in the aggregate.
        execution_count: usize,
    },
    /// Time-based window.
    TimeWindow {
        /// Start of the time window.
        start: DateTime<Utc>,
        /// End of the time window.
        end: DateTime<Utc>,
    },
    /// Periodic analysis (e.g., every N executions).
    Periodic {
        /// Number of executions per period.
        period: usize,
        /// Current iteration number.
        iteration: usize,
    },
    /// Full system analysis.
    System,
}

impl IntrospectionScope {
    /// Get a human-readable description of the scope
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::Execution { thread_id } => format!("Execution (thread: {thread_id})"),
            Self::GraphAggregate {
                graph_id,
                execution_count,
            } => format!("GraphAggregate ({graph_id}, {execution_count} executions)"),
            Self::TimeWindow { start, end } => format!(
                "TimeWindow ({} to {})",
                start.format("%Y-%m-%d %H:%M"),
                end.format("%Y-%m-%d %H:%M")
            ),
            Self::Periodic { period, iteration } => {
                format!("Periodic (every {period} executions, iteration {iteration})")
            }
            Self::System => "System-wide".to_string(),
        }
    }
}

// =============================================================================
// ReportExecutionSummary - Statistical Summary
// =============================================================================

/// Statistical summary of analyzed executions for reports.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ReportExecutionSummary {
    /// Total number of executions analyzed
    pub total_executions: usize,
    /// Number of successful executions
    pub successful_executions: usize,
    /// Number of failed executions
    pub failed_executions: usize,
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
    /// Average execution duration in milliseconds
    pub avg_duration_ms: f64,
    /// Total tokens used across all executions
    pub total_tokens: u64,
    /// Average tokens per execution
    pub avg_tokens: f64,
    /// Retry rate (0.0 - 1.0)
    pub retry_rate: f64,
    /// Comparison with previous period (if available)
    pub vs_previous: Option<ComparisonMetrics>,
}

impl ReportExecutionSummary {
    /// Convert to markdown table
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("| Metric | Value |");
        if self.vs_previous.is_some() {
            md.push_str(" vs Previous |");
        }
        md.push_str("\n|--------|-------|");
        if self.vs_previous.is_some() {
            md.push_str("-------------|");
        }
        md.push('\n');

        let vs = self.vs_previous.as_ref();
        md.push_str(&format!(
            "| Total Executions | {} |{}\n",
            self.total_executions,
            vs.map_or(String::new(), |v| format!(" {:+} |", v.execution_delta))
        ));
        md.push_str(&format!(
            "| Success Rate | {:.1}% |{}\n",
            self.success_rate * 100.0,
            vs.map_or(String::new(), |v| format!(
                " {:+.1}% |",
                v.success_rate_delta * 100.0
            ))
        ));
        md.push_str(&format!(
            "| Avg Duration | {:.1}ms |{}\n",
            self.avg_duration_ms,
            vs.map_or(String::new(), |v| format!(
                " {:+.1}ms |",
                v.duration_delta_ms
            ))
        ));
        md.push_str(&format!(
            "| Retry Rate | {:.1}% |{}\n",
            self.retry_rate * 100.0,
            vs.map_or(String::new(), |v| format!(
                " {:+.1}% |",
                v.retry_rate_delta * 100.0
            ))
        ));

        md
    }
}

// =============================================================================
// ComparisonMetrics - Period Comparison
// =============================================================================

/// Comparison with previous analysis period.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ComparisonMetrics {
    /// Change in execution count
    pub execution_delta: i64,
    /// Change in success rate
    pub success_rate_delta: f64,
    /// Change in average duration (ms)
    pub duration_delta_ms: f64,
    /// Change in retry rate
    pub retry_rate_delta: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_introspection_report_creation() {
        let report = IntrospectionReport::new(IntrospectionScope::System);
        assert!(!report.id.is_nil());
        assert!(report.capability_gaps.is_empty());
        assert!(report.execution_plans.is_empty());
    }

    #[test]
    fn test_introspection_report_json_roundtrip() {
        let mut report = IntrospectionReport::new(IntrospectionScope::Execution {
            thread_id: "test-123".to_string(),
        });
        report.add_capability_gap(
            CapabilityGap::new(
                "Missing sentiment analysis",
                GapCategory::MissingTool {
                    tool_description: "Sentiment analysis tool".to_string(),
                },
                GapManifestation::PromptWorkarounds {
                    patterns: vec!["Based on word choice...".to_string()],
                },
            )
            .with_confidence(0.85),
        );

        let json = report.to_json().unwrap();
        let parsed = IntrospectionReport::from_json(&json).unwrap();
        assert_eq!(parsed.id, report.id);
        assert_eq!(parsed.capability_gaps.len(), 1);
    }

    #[test]
    fn test_report_to_markdown() {
        let mut report = IntrospectionReport::new(IntrospectionScope::System);
        report.execution_summary.total_executions = 47;
        report.execution_summary.success_rate = 0.894;

        let md = report.to_markdown();
        assert!(md.contains("Introspection Report"));
        assert!(md.contains("System-wide"));
        assert!(md.contains("47"));
    }

    #[test]
    fn test_scope_description() {
        let scope = IntrospectionScope::GraphAggregate {
            graph_id: "customer_service".to_string(),
            execution_count: 47,
        };
        let desc = scope.description();
        assert!(desc.contains("customer_service"));
        assert!(desc.contains("47"));
    }

    #[test]
    fn test_execution_summary_to_markdown() {
        let summary = ReportExecutionSummary {
            total_executions: 100,
            successful_executions: 95,
            failed_executions: 5,
            success_rate: 0.95,
            avg_duration_ms: 150.0,
            total_tokens: 50000,
            avg_tokens: 500.0,
            retry_rate: 0.05,
            vs_previous: Some(ComparisonMetrics {
                execution_delta: 10,
                success_rate_delta: 0.02,
                duration_delta_ms: -20.0,
                retry_rate_delta: -0.01,
            }),
        };

        let md = summary.to_markdown();
        assert!(md.contains("Total Executions"));
        assert!(md.contains("100"));
        assert!(md.contains("vs Previous"));
    }
}
