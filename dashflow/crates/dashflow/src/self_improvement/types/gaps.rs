// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Capability gap and deprecation analysis types.
//!
//! This module provides types for analyzing missing or needed functionality,
//! deprecation recommendations, and retrospective analysis.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::citations::Citation;
use super::common::Priority;
use super::reports::ReportExecutionSummary;

// =============================================================================
// CapabilityGap - Missing or Needed Functionality
// =============================================================================

/// Analysis of missing or needed functionality.
///
/// Identifies what capabilities the AI lacks and would benefit from.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityGap {
    /// What capability is missing or needed
    pub description: String,

    /// Evidence from execution traces
    pub evidence: Vec<Citation>,

    /// How this gap manifested (errors, retries, workarounds)
    pub manifestation: GapManifestation,

    /// Proposed solution
    pub proposed_solution: String,

    /// Expected impact if addressed
    pub expected_impact: Impact,

    /// Confidence in this analysis (0.0-1.0)
    pub confidence: f64,

    /// Category of gap
    pub category: GapCategory,
}

impl CapabilityGap {
    /// Create a new capability gap
    #[must_use]
    pub fn new(
        description: impl Into<String>,
        category: GapCategory,
        manifestation: GapManifestation,
    ) -> Self {
        Self {
            description: description.into(),
            evidence: Vec::new(),
            manifestation,
            proposed_solution: String::new(),
            expected_impact: Impact::default(),
            confidence: 0.5,
            category,
        }
    }

    /// Add evidence citation
    #[must_use]
    pub fn with_evidence(mut self, evidence: Vec<Citation>) -> Self {
        self.evidence = evidence;
        self
    }

    /// Set proposed solution
    #[must_use]
    pub fn with_solution(mut self, solution: impl Into<String>) -> Self {
        self.proposed_solution = solution.into();
        self
    }

    /// Set expected impact
    #[must_use]
    pub fn with_impact(mut self, impact: Impact) -> Self {
        self.expected_impact = impact;
        self
    }

    /// Set confidence level
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Get priority based on impact and confidence
    #[must_use]
    pub fn priority(&self) -> Priority {
        let score = self.expected_impact.score() * self.confidence;
        if score > 0.7 {
            Priority::High
        } else if score > 0.4 {
            Priority::Medium
        } else {
            Priority::Low
        }
    }

    /// Convert to markdown
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("**Category:** {:?}\n\n", self.category));
        md.push_str(&format!(
            "**Evidence:** {}\n\n",
            self.evidence
                .iter()
                .map(|c| format!("[{}]", c.id))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        md.push_str(&format!(
            "**Manifestation:**\n{}\n\n",
            self.manifestation.description()
        ));
        md.push_str(&format!(
            "**Proposed Solution:** {}\n\n",
            self.proposed_solution
        ));
        md.push_str(&format!(
            "**Expected Impact:**\n{}\n\n",
            self.expected_impact.to_markdown()
        ));
        md.push_str(&format!(
            "**Confidence:** {:.0}%\n",
            self.confidence * 100.0
        ));
        md
    }
}

// =============================================================================
// GapCategory - Category of Capability Gap
// =============================================================================

/// Category of capability gap.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum GapCategory {
    /// Missing node type
    MissingNode {
        /// Suggested Rust signature / trait shape for the missing node.
        suggested_signature: String,
    },
    /// Missing tool
    MissingTool {
        /// Description of the tool that should exist (purpose + inputs/outputs).
        tool_description: String,
    },
    /// Inadequate existing functionality
    InadequateFunctionality {
        /// Node or component that exists but is insufficient.
        node: String,
        /// Description of the limitation observed in practice.
        limitation: String,
    },
    /// Missing integration
    MissingIntegration {
        /// External system/service that should be integrated (e.g. Slack, Jira).
        external_system: String,
    },
    /// Performance limitation
    PerformanceGap {
        /// Description of the performance bottleneck (hot path, resource, etc.).
        bottleneck: String,
    },
}

// =============================================================================
// GapManifestation - How Gap Manifested
// =============================================================================

/// How a capability gap manifested during execution.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum GapManifestation {
    /// Explicit errors
    Errors {
        /// Number of observed error occurrences.
        count: usize,
        /// Representative error messages for debugging and classification.
        sample_messages: Vec<String>,
    },
    /// High retry rates
    Retries {
        /// Observed retry rate (0.0-1.0).
        rate: f64,
        /// Names of nodes where retries were concentrated.
        affected_nodes: Vec<String>,
    },
    /// Manual workarounds in prompts
    PromptWorkarounds {
        /// Prompt patterns that compensate for missing functionality.
        patterns: Vec<String>,
    },
    /// Suboptimal execution paths
    SuboptimalPaths {
        /// Description of how execution deviated from the desired/optimal path.
        description: String,
    },
    /// Missing data or context
    MissingContext {
        /// Description of the missing data/context required to proceed.
        what: String,
    },
}

impl GapManifestation {
    /// Get a description of the manifestation
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::Errors {
                count,
                sample_messages,
            } => {
                format!(
                    "Errors ({count} occurrences): {}",
                    sample_messages.join("; ")
                )
            }
            Self::Retries {
                rate,
                affected_nodes,
            } => {
                format!(
                    "High retry rate ({:.1}%) in nodes: {}",
                    rate * 100.0,
                    affected_nodes.join(", ")
                )
            }
            Self::PromptWorkarounds { patterns } => {
                format!("Prompt workarounds detected: {}", patterns.join("; "))
            }
            Self::SuboptimalPaths { description } => {
                format!("Suboptimal execution path: {description}")
            }
            Self::MissingContext { what } => {
                format!("Missing context: {what}")
            }
        }
    }
}

// =============================================================================
// Impact - Expected Impact of Addressing Gap
// =============================================================================

/// Expected impact if a gap is addressed.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Impact {
    /// Expected reduction in errors (0.0 - 1.0)
    pub error_reduction: f64,
    /// Expected reduction in latency (ms)
    pub latency_reduction_ms: f64,
    /// Expected improvement in accuracy (0.0 - 1.0)
    pub accuracy_improvement: f64,
    /// Description of qualitative impact
    pub description: String,
}

impl Impact {
    /// Create a high impact
    #[must_use]
    pub fn high(description: impl Into<String>) -> Self {
        Self {
            error_reduction: 0.8,
            latency_reduction_ms: 400.0,
            accuracy_improvement: 0.8,
            description: description.into(),
        }
    }

    /// Create a medium impact
    #[must_use]
    pub fn medium(description: impl Into<String>) -> Self {
        Self {
            error_reduction: 0.4,
            latency_reduction_ms: 200.0,
            accuracy_improvement: 0.4,
            description: description.into(),
        }
    }

    /// Create a low impact
    #[must_use]
    pub fn low(description: impl Into<String>) -> Self {
        Self {
            error_reduction: 0.1,
            latency_reduction_ms: 50.0,
            accuracy_improvement: 0.1,
            description: description.into(),
        }
    }

    /// Calculate normalized score (0.0 - 1.0)
    #[must_use]
    pub fn score(&self) -> f64 {
        // Weighted combination of factors
        let error_score = self.error_reduction.min(1.0);
        let latency_score = (self.latency_reduction_ms / 500.0).min(1.0);
        let accuracy_score = self.accuracy_improvement.min(1.0);

        (error_score * 0.4 + latency_score * 0.3 + accuracy_score * 0.3).clamp(0.0, 1.0)
    }

    /// Convert to markdown
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!(
            "- Error reduction: ~{:.0}%\n",
            self.error_reduction * 100.0
        ));
        md.push_str(&format!(
            "- Latency reduction: ~{:.0}ms\n",
            self.latency_reduction_ms
        ));
        md.push_str(&format!(
            "- Accuracy improvement: ~{:.0}%\n",
            self.accuracy_improvement * 100.0
        ));
        if !self.description.is_empty() {
            md.push_str(&format!("- {}\n", self.description));
        }
        md
    }
}

// =============================================================================
// DeprecationRecommendation - Removal/Simplification Recommendation
// =============================================================================

/// Recommendation to remove or simplify functionality.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeprecationRecommendation {
    /// What to deprecate
    pub target: DeprecationTarget,

    /// Why it's extraneous
    pub rationale: String,

    /// Evidence of non-use or redundancy
    pub evidence: Vec<Citation>,

    /// What gains from removal
    pub benefits: Vec<String>,

    /// Risks of removal
    pub risks: Vec<String>,

    /// Confidence in recommendation (0.0-1.0)
    pub confidence: f64,

    /// Suggested migration path if any dependencies exist
    pub migration_path: Option<String>,
}

impl DeprecationRecommendation {
    /// Create a new deprecation recommendation
    #[must_use]
    pub fn new(target: DeprecationTarget, rationale: impl Into<String>) -> Self {
        Self {
            target,
            rationale: rationale.into(),
            evidence: Vec::new(),
            benefits: Vec::new(),
            risks: Vec::new(),
            confidence: 0.5,
            migration_path: None,
        }
    }

    /// Add evidence
    #[must_use]
    pub fn with_evidence(mut self, evidence: Vec<Citation>) -> Self {
        self.evidence = evidence;
        self
    }

    /// Add benefits
    #[must_use]
    pub fn with_benefits(mut self, benefits: Vec<String>) -> Self {
        self.benefits = benefits;
        self
    }

    /// Add risks
    #[must_use]
    pub fn with_risks(mut self, risks: Vec<String>) -> Self {
        self.risks = risks;
        self
    }

    /// Set confidence
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set migration path
    #[must_use]
    pub fn with_migration_path(mut self, path: impl Into<String>) -> Self {
        self.migration_path = Some(path.into());
        self
    }

    /// Convert to markdown
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("**Rationale:** {}\n\n", self.rationale));
        md.push_str(&format!(
            "**Evidence:** {}\n\n",
            self.evidence
                .iter()
                .map(|c| format!("[{}]", c.id))
                .collect::<Vec<_>>()
                .join(", ")
        ));

        if !self.benefits.is_empty() {
            md.push_str("**Benefits:**\n");
            for b in &self.benefits {
                md.push_str(&format!("- {b}\n"));
            }
            md.push('\n');
        }

        if !self.risks.is_empty() {
            md.push_str("**Risks:**\n");
            for r in &self.risks {
                md.push_str(&format!("- {r}\n"));
            }
            md.push('\n');
        }

        md.push_str(&format!(
            "**Confidence:** {:.0}%\n",
            self.confidence * 100.0
        ));

        if let Some(path) = &self.migration_path {
            md.push_str(&format!("\n**Migration Path:** {path}\n"));
        }

        md
    }
}

// =============================================================================
// DeprecationTarget - What to Deprecate
// =============================================================================

/// What to deprecate.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum DeprecationTarget {
    /// Deprecate an existing graph node.
    Node {
        /// Node name or identifier.
        name: String,
        /// Observed usage count (executions, references, etc.).
        usage_count: usize,
    },
    /// Deprecate an existing tool.
    Tool {
        /// Tool name or identifier.
        name: String,
        /// Most recent observed usage, if known.
        last_used: Option<DateTime<Utc>>,
    },
    /// Deprecate an edge/transition.
    Edge {
        /// Source node.
        from: String,
        /// Destination node.
        to: String,
    },
    /// Deprecate a feature flag or capability.
    Feature {
        /// Feature name or identifier.
        name: String,
    },
    /// Deprecate a specific code path.
    CodePath {
        /// Location string (module path, file:line, etc.).
        location: String,
    },
}

// =============================================================================
// RetrospectiveAnalysis - What Should Have Been Done Differently
// =============================================================================

/// What should have been done differently.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RetrospectiveAnalysis {
    /// What actually happened
    pub actual_execution: ReportExecutionSummary,

    /// What would have been better (counterfactuals)
    pub counterfactuals: Vec<Counterfactual>,

    /// Tools/systems that would have helped
    pub missing_tools: Vec<MissingToolAnalysis>,

    /// Application-specific insights
    pub application_insights: Vec<String>,

    /// Task-specific insights
    pub task_insights: Vec<String>,

    /// Platform-level insights (DashFlow improvements)
    pub platform_insights: Vec<String>,
}

// =============================================================================
// Counterfactual - Alternative That Could Have Been Taken
// =============================================================================

/// A counterfactual - what could have been done differently.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Counterfactual {
    /// What could have been done differently
    pub alternative: String,

    /// Expected outcome if alternative was taken
    pub expected_outcome: String,

    /// Why this wasn't done originally
    pub why_not_taken: String,

    /// Confidence that alternative would be better
    pub confidence: f64,
}

// =============================================================================
// MissingToolAnalysis - Analysis of Missing Tool
// =============================================================================

/// Analysis of a missing tool that would have helped.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MissingToolAnalysis {
    /// Name of the missing tool
    pub tool_name: String,

    /// What it would do
    pub description: String,

    /// How it would have helped
    pub benefit: String,

    /// Estimated impact
    pub impact: Impact,
}

// =============================================================================
// ImprovementProposal - Pre-Validation Proposal
// =============================================================================

/// An improvement proposal before validation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImprovementProposal {
    /// Unique identifier
    pub id: Uuid,

    /// Title of the proposal
    pub title: String,

    /// Detailed description
    pub description: String,

    /// Source of this proposal
    pub source: ProposalSource,

    /// Initial confidence before validation
    pub initial_confidence: f64,

    /// Supporting evidence
    pub evidence: Vec<Citation>,
}

/// Source of an improvement proposal.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ProposalSource {
    /// Derived from a capability gap analysis.
    CapabilityGap {
        /// Index of the gap in the originating analysis set.
        gap_index: usize,
    },
    /// Derived from a deprecation recommendation.
    Deprecation {
        /// Index of the deprecation recommendation in the originating set.
        dep_index: usize,
    },
    /// Derived from retrospective analysis of an execution.
    Retrospective,
    /// Derived from automated pattern detection across traces.
    PatternDetection,
    /// Authored manually.
    Manual,
}

/// Convert a CapabilityGap into an ImprovementProposal.
impl From<&CapabilityGap> for ImprovementProposal {
    fn from(gap: &CapabilityGap) -> Self {
        ImprovementProposal {
            id: Uuid::new_v4(),
            title: format!("Address: {}", gap.description),
            description: format!(
                "Improvement proposal generated from capability gap: {}",
                gap.description
            ),
            source: ProposalSource::CapabilityGap { gap_index: 0 },
            initial_confidence: 0.8,
            evidence: gap.evidence.clone(),
        }
    }
}

impl From<CapabilityGap> for ImprovementProposal {
    fn from(gap: CapabilityGap) -> Self {
        ImprovementProposal::from(&gap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_gap_manifestation_description() {
        let errors = GapManifestation::Errors {
            count: 5,
            sample_messages: vec!["Error 1".to_string()],
        };
        assert!(errors.description().contains("5 occurrences"));

        let retries = GapManifestation::Retries {
            rate: 0.15,
            affected_nodes: vec!["node1".to_string()],
        };
        assert!(retries.description().contains("15.0%"));
    }

    #[test]
    fn test_capability_gap_to_improvement_proposal() {
        let gap = CapabilityGap::new(
            "Missing sentiment analysis",
            GapCategory::MissingTool {
                tool_description: "Sentiment analysis tool".to_string(),
            },
            GapManifestation::PromptWorkarounds {
                patterns: vec!["Manual analysis".to_string()],
            },
        );

        let proposal: ImprovementProposal = gap.into();
        assert!(proposal.title.contains("Address:"));
        assert!(matches!(
            proposal.source,
            ProposalSource::CapabilityGap { .. }
        ));
    }
}
