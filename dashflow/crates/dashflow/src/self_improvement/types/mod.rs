// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Core types for the Self-Improving Introspection System.
//!
//! This module defines the data structures for AI self-improvement through
//! structured introspection, hypothesis tracking, and execution planning.
//!
//! ## Module Organization
//!
//! The types are organized into submodules by domain:
//!
//! - [`common`] - Shared types (ModelIdentifier, Priority, AnalysisDepth)
//! - [`citations`] - Reference and traceability types (Citation, CitationSource)
//! - [`consensus`] - Multi-model consensus types (ConsensusResult, ModelReview)
//! - [`hypothesis`] - Hypothesis tracking types (Hypothesis, ExpectedEvidence)
//! - [`gaps`] - Capability gap analysis (CapabilityGap, Impact, DeprecationRecommendation)
//! - [`plans`] - Execution plan types (ExecutionPlan, PlanAction, ConfigChange)
//! - [`reports`] - Report types (IntrospectionReport, IntrospectionScope)
//!
//! See `archive/roadmaps/ROADMAP_SELF_IMPROVEMENT.md` for design documentation (archived).

// Submodules
pub mod citations;
pub mod common;
pub mod consensus;
pub mod gaps;
pub mod hypothesis;
pub mod plans;
pub mod reports;

// Re-export all public types for backwards compatibility
// Users can import from types:: directly or from the submodules

// From common
pub use common::{AnalysisDepth, ModelIdentifier, Priority};

// From citations
pub use citations::{Citation, CitationRetrieval, CitationSource};

// From consensus
pub use consensus::{
    Assessment, ConsensusResult, Critique, CritiqueSeverity, Disagreement, ModelReview,
};

// From hypothesis
pub use hypothesis::{
    EvaluationTrigger, ExpectedEvidence, Hypothesis, HypothesisOutcome, HypothesisSource,
    HypothesisStatus, ObservedEvidence,
};

// From gaps
pub use gaps::{
    CapabilityGap, Counterfactual, DeprecationRecommendation, DeprecationTarget, GapCategory,
    GapManifestation, Impact, ImprovementProposal, MissingToolAnalysis, ProposalSource,
    RetrospectiveAnalysis,
};

// From plans
pub use plans::{
    ActionType, ApplyResult, ConfigChange, ConfigChangeType, ExecutionPlan, ImplementationStep,
    PlanAction, PlanCategory, PlanStatus,
};

// From reports
pub use reports::{
    ComparisonMetrics, IntrospectionReport, IntrospectionScope, ReportExecutionSummary,
};

#[cfg(test)]
mod tests {
    use super::*;

    // Integration tests that verify types work together across modules

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
    fn test_capability_gap_priority() {
        let high_gap = CapabilityGap::new(
            "Critical gap",
            GapCategory::PerformanceGap {
                bottleneck: "slow".to_string(),
            },
            GapManifestation::Errors {
                count: 10,
                sample_messages: vec![],
            },
        )
        .with_impact(Impact::high("Major improvement"))
        .with_confidence(0.9);

        assert_eq!(high_gap.priority(), Priority::High);

        let low_gap = CapabilityGap::new(
            "Minor gap",
            GapCategory::MissingTool {
                tool_description: "nice to have".to_string(),
            },
            GapManifestation::MissingContext {
                what: "minor".to_string(),
            },
        )
        .with_impact(Impact::low("Small improvement"))
        .with_confidence(0.3);

        assert_eq!(low_gap.priority(), Priority::Low);
    }

    #[test]
    fn test_execution_plan_creation() {
        let plan = ExecutionPlan::new("Add sentiment tool", PlanCategory::ApplicationImprovement)
            .with_description("Add a sentiment analysis tool")
            .with_priority(1)
            .with_estimated_commits(2)
            .with_steps(vec![
                ImplementationStep::new(1, "Create tool module")
                    .with_files(vec!["src/tools/sentiment.rs".to_string()])
                    .with_verification("cargo test"),
                ImplementationStep::new(2, "Add to graph")
                    .with_files(vec!["src/graph.rs".to_string()])
                    .with_verification("integration tests"),
            ])
            .with_success_criteria(vec![
                "Sentiment accuracy >= 90%".to_string(),
                "Latency < 100ms".to_string(),
            ])
            .validated(0.85);

        assert_eq!(plan.priority, 1);
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.validation_score, 0.85);
        assert!(matches!(plan.status, PlanStatus::Validated));
    }

    #[test]
    fn test_hypothesis_creation() {
        let hypothesis = Hypothesis::new(
            "Retry rate will drop below 6%",
            "Sentiment tool will reduce sentiment-related retries",
        )
        .with_expected_evidence(vec![ExpectedEvidence::new(
            "retry_rate",
            "< 6%",
            "Measure retry rate over 50 executions",
        )])
        .with_trigger(EvaluationTrigger::AfterExecutions(50));

        assert!(matches!(hypothesis.status, HypothesisStatus::Active));
        assert!(hypothesis.outcome.is_none());
    }

    #[test]
    fn test_citation_creation() {
        let trace_citation = Citation::trace("thread-abc123");
        assert!(trace_citation.id.starts_with("trace-"));

        let commit_citation = Citation::commit("abc1234567", "Add feature");
        assert!(commit_citation.id.starts_with("commit-"));

        let report_citation = Citation::report(uuid::Uuid::new_v4());
        assert!(report_citation.id.starts_with("report-"));
    }

    #[test]
    fn test_impact_scoring() {
        let high = Impact::high("test");
        assert!(high.score() > 0.3);

        let medium = Impact::medium("test");
        assert!(medium.score() > 0.1);
        assert!(medium.score() < high.score());

        let low = Impact::low("test");
        assert!(low.score() < medium.score());
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
    fn test_deprecation_recommendation() {
        let dep = DeprecationRecommendation::new(
            DeprecationTarget::Node {
                name: "legacy_node".to_string(),
                usage_count: 0,
            },
            "Node has not been used in 30 days",
        )
        .with_benefits(vec!["Remove 100 lines of code".to_string()])
        .with_risks(vec!["None identified".to_string()])
        .with_confidence(0.95);

        assert_eq!(dep.confidence, 0.95);
        assert_eq!(dep.benefits.len(), 1);
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

    // =========================================================================
    // Package Contribution Generation Tests
    // =========================================================================

    #[test]
    fn test_generate_bug_reports_from_inadequate_functionality() {
        use crate::packages::{ContributionPackageRef, ReporterIdentity};

        let mut report = IntrospectionReport::new(IntrospectionScope::System);

        // Add an inadequate functionality gap
        report.add_capability_gap(
            CapabilityGap::new(
                "Sentiment node returns incorrect scores for neutral text",
                GapCategory::InadequateFunctionality {
                    node: "sentiment_analyzer".to_string(),
                    limitation: "Cannot handle neutral text properly".to_string(),
                },
                GapManifestation::Errors {
                    count: 5,
                    sample_messages: vec!["Incorrect score for neutral input".to_string()],
                },
            )
            .with_solution("Fix neutral text handling in sentiment node")
            .with_impact(Impact::high("Improve accuracy"))
            .with_confidence(0.85),
        );

        let reporter = ReporterIdentity::ai("TestApp", None);

        // Package lookup function that always returns a package ref
        let package_lookup = |node: &str| {
            if node == "sentiment_analyzer" {
                Some(ContributionPackageRef::new("dashflow/sentiment", "1.0.0"))
            } else {
                None
            }
        };

        let bug_reports = report.generate_bug_reports(reporter, package_lookup);

        assert_eq!(bug_reports.len(), 1);
        let bug_report = &bug_reports[0];
        assert!(bug_report.title.contains("sentiment_analyzer"));
        assert!(bug_report
            .description
            .contains("Cannot handle neutral text"));
        assert!(bug_report.suggested_fix.is_some());
        assert!(bug_report.labels.contains(&"ai-discovered".to_string()));
        assert!(bug_report
            .labels
            .contains(&"inadequate-functionality".to_string()));
    }

    #[test]
    fn test_generate_bug_reports_skips_non_inadequate_gaps() {
        use crate::packages::{ContributionPackageRef, ReporterIdentity};

        let mut report = IntrospectionReport::new(IntrospectionScope::System);

        // Add a missing tool gap (not inadequate functionality)
        report.add_capability_gap(CapabilityGap::new(
            "Missing sentiment tool",
            GapCategory::MissingTool {
                tool_description: "Sentiment analysis tool".to_string(),
            },
            GapManifestation::PromptWorkarounds {
                patterns: vec!["Based on word choice...".to_string()],
            },
        ));

        let reporter = ReporterIdentity::ai("TestApp", None);
        let package_lookup = |_: &str| Some(ContributionPackageRef::new("dashflow/test", "1.0.0"));

        let bug_reports = report.generate_bug_reports(reporter, package_lookup);
        assert!(bug_reports.is_empty()); // MissingTool gaps don't generate bug reports
    }

    #[test]
    fn test_generate_package_requests_from_missing_tool() {
        use crate::packages::{PackageType, ReporterIdentity};

        let mut report = IntrospectionReport::new(IntrospectionScope::System);

        // Add a missing tool gap
        report.add_capability_gap(
            CapabilityGap::new(
                "Need a sentiment analysis tool for customer feedback",
                GapCategory::MissingTool {
                    tool_description: "Sentiment analysis for text".to_string(),
                },
                GapManifestation::PromptWorkarounds {
                    patterns: vec!["Manually analyzing word choice...".to_string()],
                },
            )
            .with_solution("Create a sentiment-analysis package")
            .with_confidence(0.9),
        );

        let requester = ReporterIdentity::ai("TestApp", None);
        let requests = report.generate_package_requests(requester);

        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert!(request.title.contains("Tool needed"));
        assert_eq!(request.package_type, Some(PackageType::ToolPack));
        assert!(request.suggested_approach.is_some());
        assert!(request.labels.contains(&"ai-requested".to_string()));
    }

    #[test]
    fn test_generate_package_requests_from_missing_node() {
        use crate::packages::{PackageType, ReporterIdentity};

        let mut report = IntrospectionReport::new(IntrospectionScope::System);

        // Add a missing node gap
        report.add_capability_gap(CapabilityGap::new(
            "Need a data validation node",
            GapCategory::MissingNode {
                suggested_signature: "ValidationNode<T: Validate>".to_string(),
            },
            GapManifestation::SuboptimalPaths {
                description: "Validation done manually in each node".to_string(),
            },
        ));

        let requester = ReporterIdentity::ai("TestApp", None);
        let requests = report.generate_package_requests(requester);

        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert!(request.title.contains("Node needed"));
        assert_eq!(request.package_type, Some(PackageType::NodeLibrary));
    }

    #[test]
    fn test_generate_package_requests_from_missing_integration() {
        use crate::packages::{PackageType, ReporterIdentity};

        let mut report = IntrospectionReport::new(IntrospectionScope::System);

        // Add a missing integration gap
        report.add_capability_gap(CapabilityGap::new(
            "Need Stripe integration for payments",
            GapCategory::MissingIntegration {
                external_system: "Stripe".to_string(),
            },
            GapManifestation::MissingContext {
                what: "Payment processing capabilities".to_string(),
            },
        ));

        let requester = ReporterIdentity::ai("TestApp", None);
        let requests = report.generate_package_requests(requester);

        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert!(request.title.contains("Integration needed"));
        assert!(request.needed_functionality.contains("Stripe connector"));
        assert_eq!(request.package_type, Some(PackageType::ModelConnector));
    }

    #[test]
    fn test_generate_improvements_from_performance_gap() {
        use crate::packages::{ContributionPackageRef, ReporterIdentity};

        let mut report = IntrospectionReport::new(IntrospectionScope::System);

        // Add a performance gap
        report.add_capability_gap(
            CapabilityGap::new(
                "LLM call node is slow",
                GapCategory::PerformanceGap {
                    bottleneck: "llm_node".to_string(),
                },
                GapManifestation::SuboptimalPaths {
                    description: "High latency in LLM calls".to_string(),
                },
            )
            .with_impact(Impact {
                error_reduction: 0.0,
                latency_reduction_ms: 200.0,
                accuracy_improvement: 0.0,
                description: "Reduce latency".to_string(),
            })
            .with_confidence(0.8),
        );

        let suggester = ReporterIdentity::ai("TestApp", None);
        let package_lookup = |node: &str| {
            if node == "llm_node" {
                Some(ContributionPackageRef::new("dashflow/llm", "1.0.0"))
            } else {
                None
            }
        };

        let improvements = report.generate_improvements(suggester, package_lookup);

        assert_eq!(improvements.len(), 1);
        let improvement = &improvements[0];
        assert!(improvement.title.contains("Performance improvement"));
        assert!(improvement.labels.contains(&"performance".to_string()));
    }
}
