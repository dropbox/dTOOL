// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Plan Generation, Validation, and Tracking for Self-Improvement
//!
//! This module implements the Self-Improvement roadmap -
//! - `PlanGenerator`: Creates ExecutionPlans from analysis results (capability gaps,
//!   deprecation recommendations, retrospective insights)
//! - `PlanValidator`: Validates plans against multi-model consensus results
//! - `PlanTracker`: Tracks plan status over time and manages plan lifecycle
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────┐     ┌─────────────────────┐     ┌─────────────────────┐
//! │   CapabilityGap     │     │ DeprecationRec.     │     │ RetrospectiveInsight│
//! └──────────┬──────────┘     └──────────┬──────────┘     └──────────┬──────────┘
//!            │                           │                           │
//!            └───────────────────────────┼───────────────────────────┘
//!                                        ▼
//!                           ┌─────────────────────┐
//!                           │   PlanGenerator     │
//!                           └──────────┬──────────┘
//!                                      │
//!                                      ▼
//!                           ┌─────────────────────┐
//!                           │  ExecutionPlan[]    │
//!                           └──────────┬──────────┘
//!                                      │
//!                                      ▼
//!                           ┌─────────────────────┐
//!                           │   PlanValidator     │──── ConsensusResult
//!                           └──────────┬──────────┘
//!                                      │
//!                                      ▼
//!                           ┌─────────────────────┐
//!                           │ Validated Plans     │
//!                           └──────────┬──────────┘
//!                                      │
//!                                      ▼
//!                           ┌─────────────────────┐
//!                           │   PlanTracker       │
//!                           └─────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::self_improvement::{
//!     PlanGenerator, PlanGeneratorConfig, PlanValidator, PlanValidatorConfig,
//!     PlanTracker, CapabilityGap, DeprecationRecommendation,
//! };
//!
//! // Generate plans from analysis
//! let generator = PlanGenerator::new(PlanGeneratorConfig::default());
//! let plans = generator.generate_from_gaps(&capability_gaps);
//!
//! // Validate against consensus
//! let validator = PlanValidator::new(PlanValidatorConfig::default());
//! let validated_plans = validator.validate(&plans, &consensus_result);
//!
//! // Track plan progress
//! let tracker = PlanTracker::new(storage);
//! tracker.track(&validated_plans)?;
//! ```

use chrono::Utc;
use uuid::Uuid;

use super::storage::IntrospectionStorage;
use super::types::{
    Assessment, CapabilityGap, ConsensusResult, CritiqueSeverity, DeprecationRecommendation,
    ExecutionPlan, GapCategory, ImplementationStep, ImprovementProposal, MissingToolAnalysis,
    PlanCategory, PlanStatus, Priority, ProposalSource, RetrospectiveAnalysis,
};

// Use std::io::Result for storage operations since that's what IntrospectionStorage uses
type StorageResult<T> = std::io::Result<T>;

// =============================================================================
// PlanGenerator - Creates ExecutionPlans from Analysis Results
// =============================================================================

/// Configuration for plan generation.
#[derive(Debug, Clone)]
pub struct PlanGeneratorConfig {
    /// Minimum confidence threshold for generating plans (default: 0.6)
    pub min_confidence: f64,
    /// Default estimated commits for capability gap plans (default: 2)
    pub default_gap_commits: u8,
    /// Default estimated commits for deprecation plans (default: 1)
    pub default_deprecation_commits: u8,
    /// Whether to automatically add test steps to plans (default: true)
    pub auto_add_test_steps: bool,
    /// Whether to automatically add rollback plans (default: true)
    pub auto_add_rollback: bool,
}

impl Default for PlanGeneratorConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.6,
            default_gap_commits: 2,
            default_deprecation_commits: 1,
            auto_add_test_steps: true,
            auto_add_rollback: true,
        }
    }
}

/// Generates ExecutionPlans from analysis results.
///
/// The PlanGenerator transforms capability gaps, deprecation recommendations,
/// and retrospective insights into actionable ExecutionPlans with:
/// - Prioritized implementation steps
/// - Success criteria
/// - Rollback plans
/// - Citations to supporting evidence
///
/// # Example
///
/// ```rust
/// use dashflow::self_improvement::{
///     PlanGenerator, PlanGeneratorConfig, CapabilityGap, GapCategory, GapManifestation,
/// };
///
/// let generator = PlanGenerator::new(PlanGeneratorConfig::default());
///
/// let gap = CapabilityGap::new(
///     "Missing sentiment analysis",
///     GapCategory::MissingTool { tool_description: "Analyze sentiment".to_string() },
///     GapManifestation::Errors { count: 5, sample_messages: vec![] },
/// ).with_confidence(0.85);
///
/// let plans = generator.generate_from_gaps(&[gap]);
/// assert_eq!(plans.len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct PlanGenerator {
    config: PlanGeneratorConfig,
}

impl Default for PlanGenerator {
    fn default() -> Self {
        Self::new(PlanGeneratorConfig::default())
    }
}

impl PlanGenerator {
    /// Create a new plan generator with the given configuration
    #[must_use]
    pub fn new(config: PlanGeneratorConfig) -> Self {
        Self { config }
    }

    /// Generate execution plans from capability gaps
    #[must_use]
    pub fn generate_from_gaps(&self, gaps: &[CapabilityGap]) -> Vec<ExecutionPlan> {
        gaps.iter()
            .filter(|g| g.confidence >= self.config.min_confidence)
            .map(|gap| self.gap_to_plan(gap))
            .collect()
    }

    /// Generate execution plans from deprecation recommendations
    #[must_use]
    pub fn generate_from_deprecations(
        &self,
        deprecations: &[DeprecationRecommendation],
    ) -> Vec<ExecutionPlan> {
        deprecations
            .iter()
            .filter(|d| d.confidence >= self.config.min_confidence)
            .map(|dep| self.deprecation_to_plan(dep))
            .collect()
    }

    /// Generate execution plans from retrospective analysis
    #[must_use]
    pub fn generate_from_retrospective(
        &self,
        retrospective: &RetrospectiveAnalysis,
    ) -> Vec<ExecutionPlan> {
        let mut plans = Vec::new();

        // Generate plans from missing tools
        for tool in &retrospective.missing_tools {
            plans.push(self.missing_tool_to_plan(tool));
        }

        // Generate plans from platform insights
        for (i, insight) in retrospective.platform_insights.iter().enumerate() {
            plans.push(self.insight_to_plan(insight, i, PlanCategory::PlatformImprovement));
        }

        // Generate plans from application insights (lower priority)
        for (i, insight) in retrospective.application_insights.iter().enumerate() {
            plans.push(self.insight_to_plan(insight, i, PlanCategory::ApplicationImprovement));
        }

        plans
    }

    /// Generate improvement proposals from gaps and deprecations
    ///
    /// This is useful for creating proposals before consensus validation.
    #[must_use]
    pub fn generate_proposals(
        &self,
        gaps: &[CapabilityGap],
        deprecations: &[DeprecationRecommendation],
    ) -> Vec<ImprovementProposal> {
        let mut proposals = Vec::new();

        for (i, gap) in gaps.iter().enumerate() {
            if gap.confidence >= self.config.min_confidence {
                proposals.push(ImprovementProposal {
                    id: Uuid::new_v4(),
                    title: self.gap_to_title(gap),
                    description: self.gap_to_description(gap),
                    source: ProposalSource::CapabilityGap { gap_index: i },
                    initial_confidence: gap.confidence,
                    evidence: gap.evidence.clone(),
                });
            }
        }

        for (i, dep) in deprecations.iter().enumerate() {
            if dep.confidence >= self.config.min_confidence {
                proposals.push(ImprovementProposal {
                    id: Uuid::new_v4(),
                    title: self.deprecation_to_title(dep),
                    description: self.deprecation_to_description(dep),
                    source: ProposalSource::Deprecation { dep_index: i },
                    initial_confidence: dep.confidence,
                    evidence: dep.evidence.clone(),
                });
            }
        }

        proposals
    }

    /// Convert a capability gap to an execution plan
    fn gap_to_plan(&self, gap: &CapabilityGap) -> ExecutionPlan {
        let title = self.gap_to_title(gap);
        let category = self.gap_to_category(gap);
        let priority = self.priority_to_u8(gap.priority());

        let mut plan = ExecutionPlan::new(title, category)
            .with_description(self.gap_to_description(gap))
            .with_priority(priority)
            .with_estimated_commits(self.config.default_gap_commits);

        // Generate implementation steps
        let steps = self.generate_gap_steps(gap);
        plan = plan.with_steps(steps);

        // Generate success criteria
        let criteria = self.generate_gap_criteria(gap);
        plan = plan.with_success_criteria(criteria);

        // Add rollback plan
        if self.config.auto_add_rollback {
            plan = plan.with_rollback_plan(self.generate_rollback_plan(gap));
        }

        // Add citations from the gap
        plan.citations = gap.evidence.clone();

        plan
    }

    /// Convert a deprecation recommendation to an execution plan
    fn deprecation_to_plan(&self, dep: &DeprecationRecommendation) -> ExecutionPlan {
        let title = self.deprecation_to_title(dep);

        let mut plan = ExecutionPlan::new(title, PlanCategory::ProcessImprovement)
            .with_description(self.deprecation_to_description(dep))
            .with_priority(3) // Deprecations are typically lower priority
            .with_estimated_commits(self.config.default_deprecation_commits);

        // Generate deprecation steps
        let steps = self.generate_deprecation_steps(dep);
        plan = plan.with_steps(steps);

        // Generate success criteria
        let criteria = vec![
            "All usages removed or migrated".to_string(),
            "All tests pass".to_string(),
            "No runtime errors".to_string(),
        ];
        plan = plan.with_success_criteria(criteria);

        // Add rollback plan
        if self.config.auto_add_rollback {
            plan = plan.with_rollback_plan(format!(
                "Revert deprecation changes. {}",
                dep.migration_path
                    .as_deref()
                    .unwrap_or("Restore original code.")
            ));
        }

        plan.citations = dep.evidence.clone();

        plan
    }

    /// Convert a missing tool analysis to an execution plan
    fn missing_tool_to_plan(&self, tool: &MissingToolAnalysis) -> ExecutionPlan {
        let title = format!("Add {}", tool.tool_name);

        let mut plan = ExecutionPlan::new(title, PlanCategory::ApplicationImprovement)
            .with_description(format!(
                "Add {} to address: {}. Expected benefit: {}",
                tool.tool_name, tool.description, tool.benefit
            ))
            .with_priority(2)
            .with_estimated_commits(self.config.default_gap_commits);

        let steps = vec![
            ImplementationStep::new(1, format!("Create {} module", tool.tool_name))
                .with_files(vec![format!(
                    "src/tools/{}.rs",
                    tool.tool_name.to_lowercase()
                )])
                .with_verification("cargo check"),
            ImplementationStep::new(2, "Add unit tests")
                .with_files(vec![format!(
                    "src/tools/{}_tests.rs",
                    tool.tool_name.to_lowercase()
                )])
                .with_verification("cargo test"),
            ImplementationStep::new(3, "Integrate with graph")
                .with_files(vec!["src/graph.rs".to_string()])
                .with_verification("integration tests"),
        ];
        plan = plan.with_steps(steps);

        let criteria = vec![
            format!(
                "{} correctly implements required functionality",
                tool.tool_name
            ),
            "All unit tests pass".to_string(),
            "Integration tests pass".to_string(),
        ];
        plan = plan.with_success_criteria(criteria);

        if self.config.auto_add_rollback {
            plan = plan.with_rollback_plan(format!(
                "Remove {} and related integrations",
                tool.tool_name
            ));
        }

        plan
    }

    /// Convert an insight string to an execution plan
    fn insight_to_plan(
        &self,
        insight: &str,
        index: usize,
        category: PlanCategory,
    ) -> ExecutionPlan {
        let insight_snippet: String = insight.chars().take(50).collect();
        let title = format!("Insight #{}: {}", index + 1, insight_snippet);

        ExecutionPlan::new(title, category)
            .with_description(insight.to_string())
            .with_priority(4) // Insights are typically lower priority
            .with_estimated_commits(1)
            .with_steps(vec![ImplementationStep::new(
                1,
                "Investigate and implement insight",
            )
            .with_verification("Manual review")])
            .with_success_criteria(vec!["Insight addressed".to_string()])
    }

    /// Generate implementation steps for a capability gap
    fn generate_gap_steps(&self, gap: &CapabilityGap) -> Vec<ImplementationStep> {
        let mut steps = Vec::new();

        match &gap.category {
            GapCategory::MissingTool { tool_description } => {
                steps.push(
                    ImplementationStep::new(1, format!("Create tool: {tool_description}"))
                        .with_files(vec!["src/tools/new_tool.rs".to_string()])
                        .with_verification("cargo check"),
                );
                steps.push(
                    ImplementationStep::new(2, "Add tool tests")
                        .with_files(vec!["src/tools/new_tool_tests.rs".to_string()])
                        .with_verification("cargo test"),
                );
                steps.push(
                    ImplementationStep::new(3, "Integrate tool with graph")
                        .with_files(vec!["src/graph.rs".to_string()])
                        .with_verification("integration tests"),
                );
            }
            GapCategory::MissingNode {
                suggested_signature,
            } => {
                steps.push(
                    ImplementationStep::new(
                        1,
                        format!("Create node with signature: {suggested_signature}"),
                    )
                    .with_files(vec!["src/nodes/new_node.rs".to_string()])
                    .with_verification("cargo check"),
                );
                steps.push(
                    ImplementationStep::new(2, "Add node to graph")
                        .with_files(vec!["src/graph.rs".to_string()])
                        .with_verification("cargo test"),
                );
            }
            GapCategory::InadequateFunctionality { node, limitation } => {
                steps.push(
                    ImplementationStep::new(1, format!("Enhance {node} to address: {limitation}"))
                        .with_files(vec![format!("src/nodes/{node}.rs")])
                        .with_verification("cargo check"),
                );
                steps.push(
                    ImplementationStep::new(2, "Update tests for enhanced functionality")
                        .with_verification("cargo test"),
                );
            }
            GapCategory::MissingIntegration { external_system } => {
                steps.push(
                    ImplementationStep::new(1, format!("Add integration for {external_system}"))
                        .with_files(vec![format!("src/integrations/{external_system}.rs")])
                        .with_verification("cargo check"),
                );
                steps.push(
                    ImplementationStep::new(2, "Add integration tests")
                        .with_verification("cargo test --features integration"),
                );
            }
            GapCategory::PerformanceGap { bottleneck } => {
                steps.push(
                    ImplementationStep::new(1, format!("Profile and optimize: {bottleneck}"))
                        .with_verification("benchmark before/after"),
                );
                steps.push(
                    ImplementationStep::new(2, "Validate performance improvement")
                        .with_verification("cargo bench"),
                );
            }
        }

        // Add test step if configured
        if self.config.auto_add_test_steps {
            let next_order = Self::next_step_order(&steps);
            steps.push(
                ImplementationStep::new(next_order, "Run full test suite")
                    .with_verification("cargo test --all"),
            );
        }

        steps
    }

    /// Generate deprecation steps
    fn generate_deprecation_steps(
        &self,
        dep: &DeprecationRecommendation,
    ) -> Vec<ImplementationStep> {
        let mut steps = Vec::new();

        // Step 1: Add deprecation warning
        steps.push(
            ImplementationStep::new(1, "Add deprecation warning")
                .with_verification("cargo check for deprecation warnings"),
        );

        // Step 2: Migration (if path exists)
        if let Some(migration) = &dep.migration_path {
            steps.push(
                ImplementationStep::new(2, format!("Migrate usages: {migration}"))
                    .with_verification("cargo test"),
            );
        }

        // Step 3: Remove deprecated code
        let next_order = Self::next_step_order(&steps);
        steps.push(
            ImplementationStep::new(next_order, "Remove deprecated code")
                .with_verification("cargo check && cargo test"),
        );

        // Step 4: Final verification
        let final_order = Self::next_step_order(&steps);
        steps.push(
            ImplementationStep::new(final_order, "Verify no usages remain")
                .with_verification("grep for removed identifiers"),
        );

        steps
    }

    fn next_step_order(steps: &[ImplementationStep]) -> u8 {
        u8::try_from(steps.len())
            .map(|n| n.saturating_add(1))
            .unwrap_or(u8::MAX)
    }

    /// Generate success criteria for a capability gap
    fn generate_gap_criteria(&self, gap: &CapabilityGap) -> Vec<String> {
        let mut criteria = Vec::new();

        // Add criteria based on impact
        if gap.expected_impact.error_reduction > 0.1 {
            criteria.push(format!(
                "Error rate reduced by at least {:.0}%",
                gap.expected_impact.error_reduction * 100.0 * 0.8 // Slightly conservative
            ));
        }

        if gap.expected_impact.latency_reduction_ms > 50.0 {
            criteria.push(format!(
                "Latency reduced by at least {:.0}ms",
                gap.expected_impact.latency_reduction_ms * 0.8
            ));
        }

        if gap.expected_impact.accuracy_improvement > 0.1 {
            criteria.push(format!(
                "Accuracy improved by at least {:.0}%",
                gap.expected_impact.accuracy_improvement * 100.0 * 0.8
            ));
        }

        // Always add test criteria
        criteria.push("All existing tests pass".to_string());
        criteria.push("New functionality has test coverage".to_string());

        criteria
    }

    /// Generate a rollback plan for a capability gap
    fn generate_rollback_plan(&self, gap: &CapabilityGap) -> String {
        match &gap.category {
            GapCategory::MissingTool { tool_description } => {
                format!(
                    "Remove the {} tool and revert to previous behavior",
                    tool_description
                )
            }
            GapCategory::MissingNode {
                suggested_signature,
            } => {
                format!(
                    "Remove the new node ({}) and revert graph changes",
                    suggested_signature
                )
            }
            GapCategory::InadequateFunctionality { node, .. } => {
                format!("Revert changes to {} node", node)
            }
            GapCategory::MissingIntegration { external_system } => {
                format!("Remove {} integration", external_system)
            }
            GapCategory::PerformanceGap { .. } => {
                "Revert optimization changes if performance regresses".to_string()
            }
        }
    }

    /// Convert gap to title
    fn gap_to_title(&self, gap: &CapabilityGap) -> String {
        match &gap.category {
            GapCategory::MissingTool { tool_description } => {
                format!("Add Tool: {}", tool_description)
            }
            GapCategory::MissingNode {
                suggested_signature,
            } => {
                format!("Add Node: {}", suggested_signature)
            }
            GapCategory::InadequateFunctionality { node, limitation } => {
                format!("Enhance {}: {}", node, limitation)
            }
            GapCategory::MissingIntegration { external_system } => {
                format!("Add Integration: {}", external_system)
            }
            GapCategory::PerformanceGap { bottleneck } => {
                format!("Optimize: {}", bottleneck)
            }
        }
    }

    /// Convert gap to description
    fn gap_to_description(&self, gap: &CapabilityGap) -> String {
        format!(
            "{}. Proposed solution: {}",
            gap.description, gap.proposed_solution
        )
    }

    /// Convert deprecation to title
    fn deprecation_to_title(&self, dep: &DeprecationRecommendation) -> String {
        match &dep.target {
            super::types::DeprecationTarget::Node { name, .. } => {
                format!("Deprecate Node: {}", name)
            }
            super::types::DeprecationTarget::Tool { name, .. } => {
                format!("Deprecate Tool: {}", name)
            }
            super::types::DeprecationTarget::Edge { from, to } => {
                format!("Deprecate Edge: {} -> {}", from, to)
            }
            super::types::DeprecationTarget::Feature { name } => {
                format!("Deprecate Feature: {}", name)
            }
            super::types::DeprecationTarget::CodePath { location } => {
                format!("Deprecate Code: {}", location)
            }
        }
    }

    /// Convert deprecation to description
    fn deprecation_to_description(&self, dep: &DeprecationRecommendation) -> String {
        let mut desc = format!("Rationale: {}", dep.rationale);
        if !dep.benefits.is_empty() {
            desc.push_str(&format!(". Benefits: {}", dep.benefits.join(", ")));
        }
        if !dep.risks.is_empty() {
            desc.push_str(&format!(". Risks: {}", dep.risks.join(", ")));
        }
        desc
    }

    /// Convert gap category to plan category
    fn gap_to_category(&self, gap: &CapabilityGap) -> PlanCategory {
        match &gap.category {
            GapCategory::MissingTool { .. } | GapCategory::MissingNode { .. } => {
                PlanCategory::ApplicationImprovement
            }
            GapCategory::InadequateFunctionality { .. } => PlanCategory::ApplicationImprovement,
            GapCategory::MissingIntegration { .. } => PlanCategory::PlatformImprovement,
            GapCategory::PerformanceGap { .. } => PlanCategory::Optimization,
        }
    }

    /// Convert priority enum to u8
    fn priority_to_u8(&self, priority: Priority) -> u8 {
        match priority {
            Priority::High => 1,
            Priority::Medium => 2,
            Priority::Low => 3,
        }
    }
}

// =============================================================================
// PlanValidator - Validates Plans Against Consensus
// =============================================================================

/// Configuration for plan validation.
#[derive(Debug, Clone)]
pub struct PlanValidatorConfig {
    /// Minimum consensus score to validate a plan (default: 0.6)
    pub min_consensus_score: f64,
    /// Minimum number of positive reviews (default: 2)
    pub min_positive_reviews: usize,
    /// Whether to reject plans with critical critiques (default: true)
    pub reject_on_critical_critique: bool,
    /// Score adjustment for each severity level
    pub severity_weights: SeverityWeights,
}

/// Weights for different critique severities.
#[derive(Debug, Clone)]
pub struct SeverityWeights {
    /// Score penalty for minor issues.
    pub minor: f64,
    /// Score penalty for moderate issues.
    pub moderate: f64,
    /// Score penalty for major issues.
    pub major: f64,
    /// Score penalty for critical issues.
    pub critical: f64,
}

impl Default for SeverityWeights {
    fn default() -> Self {
        Self {
            minor: 0.05,
            moderate: 0.15,
            major: 0.30,
            critical: 0.50,
        }
    }
}

impl Default for PlanValidatorConfig {
    fn default() -> Self {
        Self {
            min_consensus_score: 0.6,
            min_positive_reviews: 2,
            reject_on_critical_critique: true,
            severity_weights: SeverityWeights::default(),
        }
    }
}

/// Result of plan validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// The validated plan
    pub plan: ExecutionPlan,
    /// Whether the plan passed validation
    pub passed: bool,
    /// Validation score (0.0-1.0)
    pub score: f64,
    /// Reasons for failure (if any)
    pub failure_reasons: Vec<String>,
    /// Modifications suggested by validators
    pub suggested_modifications: Vec<String>,
}

/// Validates ExecutionPlans against multi-model consensus results.
///
/// The PlanValidator examines consensus results and determines whether
/// plans should be approved for implementation based on:
/// - Overall consensus score
/// - Number of positive reviews
/// - Severity of critiques
/// - Agreement vs disagreement points
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{PlanValidator, PlanValidatorConfig};
///
/// let validator = PlanValidator::new(PlanValidatorConfig::default());
/// let results = validator.validate(&plans, &consensus_result);
///
/// for result in results {
///     if result.passed {
///         println!("Plan {} validated with score {:.2}", result.plan.title, result.score);
///     } else {
///         println!("Plan {} failed: {:?}", result.plan.title, result.failure_reasons);
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PlanValidator {
    config: PlanValidatorConfig,
}

impl Default for PlanValidator {
    fn default() -> Self {
        Self::new(PlanValidatorConfig::default())
    }
}

impl PlanValidator {
    /// Create a new validator with the given configuration
    #[must_use]
    pub fn new(config: PlanValidatorConfig) -> Self {
        Self { config }
    }

    /// Validate plans against consensus result
    #[must_use]
    pub fn validate(
        &self,
        plans: &[ExecutionPlan],
        consensus: &ConsensusResult,
    ) -> Vec<ValidationResult> {
        plans
            .iter()
            .map(|plan| self.validate_single_plan(plan.clone(), consensus))
            .collect()
    }

    /// Validate plans and return only those that pass
    #[must_use]
    pub fn validate_and_filter(
        &self,
        plans: &[ExecutionPlan],
        consensus: &ConsensusResult,
    ) -> Vec<ExecutionPlan> {
        self.validate(plans, consensus)
            .into_iter()
            .filter(|r| r.passed)
            .map(|r| r.plan.validated(r.score))
            .collect()
    }

    /// Validate a single plan
    fn validate_single_plan(
        &self,
        plan: ExecutionPlan,
        consensus: &ConsensusResult,
    ) -> ValidationResult {
        let mut failure_reasons = Vec::new();
        let mut score = consensus.consensus_score;

        // Check consensus score
        if consensus.consensus_score < self.config.min_consensus_score {
            failure_reasons.push(format!(
                "Consensus score {:.2} below threshold {:.2}",
                consensus.consensus_score, self.config.min_consensus_score
            ));
        }

        // Count positive reviews
        let positive_count = consensus
            .reviews
            .iter()
            .filter(|r| matches!(r.assessment, Assessment::Agree | Assessment::StronglyAgree))
            .count();

        if positive_count < self.config.min_positive_reviews {
            failure_reasons.push(format!(
                "Only {} positive reviews, need {}",
                positive_count, self.config.min_positive_reviews
            ));
        }

        // Check for critical critiques
        let mut has_critical = false;
        for review in &consensus.reviews {
            for critique in &review.critiques {
                // Adjust score based on critique severity
                let penalty = match critique.severity {
                    CritiqueSeverity::Minor => self.config.severity_weights.minor,
                    CritiqueSeverity::Moderate => self.config.severity_weights.moderate,
                    CritiqueSeverity::Major => self.config.severity_weights.major,
                    CritiqueSeverity::Critical => {
                        has_critical = true;
                        self.config.severity_weights.critical
                    }
                };
                score -= penalty;
            }
        }

        if has_critical && self.config.reject_on_critical_critique {
            failure_reasons.push("Plan has critical critique from reviewer".to_string());
        }

        // Collect suggested modifications
        let suggested_modifications: Vec<String> = consensus
            .reviews
            .iter()
            .flat_map(|r| r.suggestions.clone())
            .chain(consensus.modifications.clone())
            .collect();

        // Determine if passed
        let passed = failure_reasons.is_empty() && score >= self.config.min_consensus_score;

        ValidationResult {
            plan,
            passed,
            score: score.max(0.0),
            failure_reasons,
            suggested_modifications,
        }
    }
}

// =============================================================================
// PlanTracker - Tracks Plan Status Over Time
// =============================================================================

/// Configuration for plan tracking.
#[derive(Debug, Clone)]
pub struct PlanTrackerConfig {
    /// Whether to auto-archive completed plans (default: true)
    pub auto_archive_completed: bool,
    /// Days before a stale plan is flagged (default: 14)
    pub stale_threshold_days: u32,
}

impl Default for PlanTrackerConfig {
    fn default() -> Self {
        Self {
            auto_archive_completed: true,
            stale_threshold_days: 14,
        }
    }
}

/// Tracks ExecutionPlan status over time.
///
/// The PlanTracker manages the lifecycle of plans:
/// - Pending → In Progress → Implemented/Failed
/// - Tracks which plans are stale
/// - Archives completed plans
/// - Provides status summaries
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{PlanTracker, PlanTrackerConfig, IntrospectionStorage};
///
/// let storage = IntrospectionStorage::default();
/// let tracker = PlanTracker::new(storage, PlanTrackerConfig::default());
///
/// // Add a new plan
/// tracker.track_plan(&plan)?;
///
/// // Update status
/// tracker.mark_in_progress(&plan.id, "AI Worker #42")?;
/// tracker.mark_implemented(&plan.id, "abc1234")?;
///
/// // Get summary
/// let summary = tracker.summary()?;
/// println!("Active plans: {}", summary.in_progress);
/// ```
#[derive(Debug)]
pub struct PlanTracker {
    storage: IntrospectionStorage,
    config: PlanTrackerConfig,
}

impl PlanTracker {
    /// Create a new plan tracker
    pub fn new(storage: IntrospectionStorage, config: PlanTrackerConfig) -> Self {
        Self { storage, config }
    }

    /// Create a tracker with default configuration
    #[must_use]
    pub fn with_storage(storage: IntrospectionStorage) -> Self {
        Self::new(storage, PlanTrackerConfig::default())
    }

    /// Track a new plan (save to pending)
    ///
    /// # Errors
    ///
    /// Returns error if storage fails
    pub fn track_plan(&self, plan: &ExecutionPlan) -> StorageResult<()> {
        self.storage.save_plan(plan)?;
        Ok(())
    }

    /// Track multiple plans
    ///
    /// # Errors
    ///
    /// Returns error if any storage operation fails
    pub fn track_plans(&self, plans: &[ExecutionPlan]) -> StorageResult<()> {
        for plan in plans {
            self.track_plan(plan)?;
        }
        Ok(())
    }

    /// Mark a plan as in progress
    ///
    /// # Errors
    ///
    /// Returns error if plan not found or storage fails
    pub fn mark_in_progress(&self, plan_id: &Uuid, assignee: &str) -> StorageResult<ExecutionPlan> {
        self.storage.approve_plan(*plan_id, assignee.to_string())?;
        self.storage.load_plan(*plan_id)
    }

    /// Mark a plan as implemented
    ///
    /// # Errors
    ///
    /// Returns error if plan not found or storage fails
    pub fn mark_implemented(
        &self,
        plan_id: &Uuid,
        commit_hash: &str,
    ) -> StorageResult<ExecutionPlan> {
        self.storage
            .complete_plan(*plan_id, commit_hash.to_string())?;
        self.storage.load_plan(*plan_id)
    }

    /// Mark a plan as failed
    ///
    /// # Errors
    ///
    /// Returns error if plan not found or storage fails
    pub fn mark_failed(&self, plan_id: &Uuid, reason: &str) -> StorageResult<ExecutionPlan> {
        self.storage.fail_plan(*plan_id, reason.to_string())?;
        self.storage.load_plan(*plan_id)
    }

    /// Mark a plan as superseded by another
    ///
    /// # Errors
    ///
    /// Returns error if plan not found or storage fails
    pub fn mark_superseded(
        &self,
        plan_id: &Uuid,
        superseded_by: &Uuid,
    ) -> StorageResult<ExecutionPlan> {
        self.storage
            .update_plan_status(*plan_id, PlanStatus::Superseded { by: *superseded_by })?;
        self.storage.load_plan(*plan_id)
    }

    /// Get all pending plans
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn pending_plans(&self) -> StorageResult<Vec<ExecutionPlan>> {
        self.storage.list_pending_plans()
    }

    /// Get all in-progress plans
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn in_progress_plans(&self) -> StorageResult<Vec<ExecutionPlan>> {
        self.storage.approved_plans()
    }

    /// Get plan summary statistics
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn summary(&self) -> StorageResult<PlanSummary> {
        let pending = self.storage.list_pending_plans()?;
        let approved = self.storage.approved_plans()?;
        let implemented = self.storage.list_implemented_plans()?;
        let failed = self.storage.list_failed_plans()?;

        let in_progress = approved
            .iter()
            .filter(|p| matches!(p.status, PlanStatus::InProgress { .. }))
            .count();

        let validated = pending
            .iter()
            .filter(|p| matches!(p.status, PlanStatus::Validated))
            .count();

        let proposed = pending
            .iter()
            .filter(|p| matches!(p.status, PlanStatus::Proposed))
            .count();

        // Count stale plans
        let now = Utc::now();
        let stale_threshold = chrono::Duration::days(i64::from(self.config.stale_threshold_days));
        let stale = approved
            .iter()
            .filter(|p| {
                if let PlanStatus::InProgress { started, .. } = &p.status {
                    now.signed_duration_since(*started) > stale_threshold
                } else {
                    false
                }
            })
            .count();

        Ok(PlanSummary {
            proposed,
            validated,
            in_progress,
            implemented: implemented.len(),
            failed: failed.len(),
            stale,
        })
    }

    /// Get stale plans (in progress too long)
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn stale_plans(&self) -> StorageResult<Vec<ExecutionPlan>> {
        let plans = self.storage.approved_plans()?;
        let now = Utc::now();
        let threshold = chrono::Duration::days(i64::from(self.config.stale_threshold_days));

        Ok(plans
            .into_iter()
            .filter(|p| {
                if let PlanStatus::InProgress { started, .. } = &p.status {
                    now.signed_duration_since(*started) > threshold
                } else {
                    false
                }
            })
            .collect())
    }

    /// Get the next plan to work on (highest priority pending validated plan)
    ///
    /// # Errors
    ///
    /// Returns error if storage read fails
    pub fn next_plan(&self) -> StorageResult<Option<ExecutionPlan>> {
        let plans = self.storage.list_pending_plans()?;

        Ok(plans
            .into_iter()
            .filter(|p| matches!(p.status, PlanStatus::Validated))
            .min_by_key(|p| p.priority))
    }
}

/// Summary of plan statuses.
#[derive(Debug, Clone, Default)]
pub struct PlanSummary {
    /// Plans proposed but not yet validated
    pub proposed: usize,
    /// Plans validated and ready for implementation
    pub validated: usize,
    /// Plans currently being implemented
    pub in_progress: usize,
    /// Plans successfully implemented
    pub implemented: usize,
    /// Plans that failed
    pub failed: usize,
    /// Plans that have been in progress too long
    pub stale: usize,
}

impl PlanSummary {
    /// Get total active plans (not implemented or failed)
    #[must_use]
    pub fn active(&self) -> usize {
        self.proposed + self.validated + self.in_progress
    }

    /// Get total completed plans (implemented + failed)
    #[must_use]
    pub fn completed(&self) -> usize {
        self.implemented + self.failed
    }

    /// Get success rate of completed plans
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        let total = self.completed();
        if total == 0 {
            1.0
        } else {
            self.implemented as f64 / total as f64
        }
    }

    /// Convert to markdown
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("## Plan Summary\n\n");
        md.push_str("| Status | Count |\n");
        md.push_str("|--------|-------|\n");
        md.push_str(&format!("| Proposed | {} |\n", self.proposed));
        md.push_str(&format!("| Validated | {} |\n", self.validated));
        md.push_str(&format!("| In Progress | {} |\n", self.in_progress));
        md.push_str(&format!("| Implemented | {} |\n", self.implemented));
        md.push_str(&format!("| Failed | {} |\n", self.failed));
        md.push_str(&format!("| Stale | {} |\n", self.stale));
        md.push_str(&format!(
            "\n**Success Rate:** {:.1}%\n",
            self.success_rate() * 100.0
        ));
        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_improvement::types::{GapManifestation, Impact, ModelIdentifier, ModelReview};
    use tempfile::tempdir;

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

    fn create_test_deprecation() -> DeprecationRecommendation {
        DeprecationRecommendation::new(
            super::super::types::DeprecationTarget::Node {
                name: "legacy_node".to_string(),
                usage_count: 0,
            },
            "Node has not been used in 30 days",
        )
        .with_benefits(vec!["Remove 100 lines of code".to_string()])
        .with_confidence(0.9)
    }

    fn create_test_consensus(score: f64, validated: bool) -> ConsensusResult {
        ConsensusResult {
            reviews: vec![
                ModelReview {
                    model: ModelIdentifier::Anthropic {
                        model: "claude-3".to_string(),
                    },
                    assessment: Assessment::Agree,
                    critiques: vec![],
                    suggestions: vec!["Consider caching".to_string()],
                    confidence: 0.8,
                    raw_response: "Looks good".to_string(),
                },
                ModelReview {
                    model: ModelIdentifier::OpenAI {
                        model: "gpt-4".to_string(),
                    },
                    assessment: Assessment::Agree,
                    critiques: vec![],
                    suggestions: vec![],
                    confidence: 0.85,
                    raw_response: "Approved".to_string(),
                },
            ],
            consensus_score: score,
            agreements: vec!["Implementation approach is sound".to_string()],
            disagreements: vec![],
            synthesis: "Both models agree on the approach".to_string(),
            validated,
            modifications: vec![],
        }
    }

    // PlanGenerator tests

    #[test]
    fn test_plan_generator_default() {
        let generator = PlanGenerator::default();
        assert_eq!(generator.config.min_confidence, 0.6);
    }

    #[test]
    fn test_generate_from_gaps() {
        let generator = PlanGenerator::default();
        let gap = create_test_gap();
        let plans = generator.generate_from_gaps(&[gap]);

        assert_eq!(plans.len(), 1);
        assert!(plans[0].title.contains("Analyze customer sentiment"));
        // Priority is calculated from impact.score() * confidence
        // Impact::high() score is ~0.8, confidence is 0.85, so 0.68 → Medium (2)
        assert_eq!(plans[0].priority, 2); // Medium priority
        assert!(!plans[0].steps.is_empty());
        assert!(!plans[0].success_criteria.is_empty());
    }

    #[test]
    fn test_generate_from_gaps_filters_low_confidence() {
        let generator = PlanGenerator::default();
        let mut gap = create_test_gap();
        gap.confidence = 0.3; // Below threshold

        let plans = generator.generate_from_gaps(&[gap]);
        assert!(plans.is_empty());
    }

    #[test]
    fn test_generate_from_deprecations() {
        let generator = PlanGenerator::default();
        let dep = create_test_deprecation();
        let plans = generator.generate_from_deprecations(&[dep]);

        assert_eq!(plans.len(), 1);
        assert!(plans[0].title.contains("legacy_node"));
        assert_eq!(plans[0].category, PlanCategory::ProcessImprovement);
    }

    #[test]
    fn test_generate_proposals() {
        let generator = PlanGenerator::default();
        let gap = create_test_gap();
        let dep = create_test_deprecation();

        let proposals = generator.generate_proposals(&[gap], &[dep]);
        assert_eq!(proposals.len(), 2);

        let gap_proposal = proposals
            .iter()
            .find(|p| matches!(p.source, ProposalSource::CapabilityGap { .. }));
        assert!(gap_proposal.is_some());

        let dep_proposal = proposals
            .iter()
            .find(|p| matches!(p.source, ProposalSource::Deprecation { .. }));
        assert!(dep_proposal.is_some());
    }

    #[test]
    fn test_generate_gap_steps_for_missing_tool() {
        let generator = PlanGenerator::default();
        let gap = create_test_gap();
        let plans = generator.generate_from_gaps(&[gap]);

        assert!(!plans[0].steps.is_empty());
        assert!(plans[0]
            .steps
            .iter()
            .any(|s| s.action.contains("Create tool")));
        assert!(plans[0].steps.iter().any(|s| s.action.contains("test")));
    }

    #[test]
    fn test_generate_gap_steps_for_performance() {
        let generator = PlanGenerator::default();
        let gap = CapabilityGap::new(
            "Slow database queries",
            GapCategory::PerformanceGap {
                bottleneck: "N+1 query pattern".to_string(),
            },
            GapManifestation::SuboptimalPaths {
                description: "Slow response times".to_string(),
            },
        )
        .with_confidence(0.8);

        let plans = generator.generate_from_gaps(&[gap]);
        assert_eq!(plans[0].category, PlanCategory::Optimization);
        assert!(plans[0].steps.iter().any(|s| s.action.contains("optimize")));
    }

    // PlanValidator tests

    #[test]
    fn test_plan_validator_default() {
        let validator = PlanValidator::default();
        assert_eq!(validator.config.min_consensus_score, 0.6);
    }

    #[test]
    fn test_validate_passing_plan() {
        let validator = PlanValidator::default();
        let generator = PlanGenerator::default();
        let gap = create_test_gap();
        let plans = generator.generate_from_gaps(&[gap]);
        let consensus = create_test_consensus(0.85, true);

        let results = validator.validate(&plans, &consensus);
        assert_eq!(results.len(), 1);
        assert!(results[0].passed);
        assert!(results[0].score >= 0.6);
    }

    #[test]
    fn test_validate_failing_plan_low_score() {
        let validator = PlanValidator::default();
        let generator = PlanGenerator::default();
        let gap = create_test_gap();
        let plans = generator.generate_from_gaps(&[gap]);
        let consensus = create_test_consensus(0.4, false); // Below threshold

        let results = validator.validate(&plans, &consensus);
        assert!(!results[0].passed);
        assert!(results[0]
            .failure_reasons
            .iter()
            .any(|r| r.contains("below threshold")));
    }

    #[test]
    fn test_validate_and_filter() {
        let validator = PlanValidator::default();
        let generator = PlanGenerator::default();
        let gap = create_test_gap();
        let plans = generator.generate_from_gaps(&[gap]);
        let consensus = create_test_consensus(0.85, true);

        let validated = validator.validate_and_filter(&plans, &consensus);
        assert_eq!(validated.len(), 1);
        assert!(matches!(validated[0].status, PlanStatus::Validated));
    }

    #[test]
    fn test_validate_collects_suggestions() {
        let validator = PlanValidator::default();
        let generator = PlanGenerator::default();
        let gap = create_test_gap();
        let plans = generator.generate_from_gaps(&[gap]);
        let consensus = create_test_consensus(0.85, true);

        let results = validator.validate(&plans, &consensus);
        assert!(!results[0].suggested_modifications.is_empty());
        assert!(results[0]
            .suggested_modifications
            .iter()
            .any(|s| s.contains("caching")));
    }

    // PlanSummary tests

    #[test]
    fn test_plan_summary_active() {
        let summary = PlanSummary {
            proposed: 2,
            validated: 3,
            in_progress: 1,
            implemented: 5,
            failed: 1,
            stale: 0,
        };

        assert_eq!(summary.active(), 6);
        assert_eq!(summary.completed(), 6);
    }

    #[test]
    fn test_plan_summary_success_rate() {
        let summary = PlanSummary {
            proposed: 0,
            validated: 0,
            in_progress: 0,
            implemented: 8,
            failed: 2,
            stale: 0,
        };

        assert!((summary.success_rate() - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_plan_summary_success_rate_no_completed() {
        let summary = PlanSummary::default();
        assert!((summary.success_rate() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_plan_summary_to_markdown() {
        let summary = PlanSummary {
            proposed: 1,
            validated: 2,
            in_progress: 1,
            implemented: 5,
            failed: 1,
            stale: 0,
        };

        let md = summary.to_markdown();
        assert!(md.contains("Plan Summary"));
        assert!(md.contains("Implemented"));
        assert!(md.contains("Success Rate"));
    }

    #[test]
    fn test_plan_tracker_mark_in_progress_moves_to_approved() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::new(dir.path().join("introspection"));
        storage.initialize().unwrap();
        let tracker = PlanTracker::new(storage, PlanTrackerConfig::default());

        let plan = ExecutionPlan::new("Test Plan", PlanCategory::ApplicationImprovement);
        tracker.track_plan(&plan).unwrap();

        let updated = tracker.mark_in_progress(&plan.id, "AI Worker #1").unwrap();
        assert!(matches!(updated.status, PlanStatus::InProgress { .. }));

        let in_progress = tracker.in_progress_plans().unwrap();
        assert_eq!(in_progress.len(), 1);
        assert_eq!(in_progress[0].id, plan.id);
    }

    #[test]
    fn test_plan_tracker_mark_implemented_persists_commit_hash() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::new(dir.path().join("introspection"));
        storage.initialize().unwrap();
        let tracker = PlanTracker::new(storage, PlanTrackerConfig::default());

        let plan = ExecutionPlan::new("Test Plan", PlanCategory::ApplicationImprovement);
        tracker.track_plan(&plan).unwrap();

        let updated = tracker.mark_implemented(&plan.id, "abc123").unwrap();
        match updated.status {
            PlanStatus::Implemented { commit_hash, .. } => assert_eq!(commit_hash, "abc123"),
            other => panic!("unexpected status: {other:?}"),
        }
    }

    #[test]
    fn test_plan_tracker_mark_failed_persists_reason() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::new(dir.path().join("introspection"));
        storage.initialize().unwrap();
        let tracker = PlanTracker::new(storage, PlanTrackerConfig::default());

        let plan = ExecutionPlan::new("Test Plan", PlanCategory::ApplicationImprovement);
        tracker.track_plan(&plan).unwrap();

        let updated = tracker.mark_failed(&plan.id, "bad things").unwrap();
        match updated.status {
            PlanStatus::Failed { reason } => assert_eq!(reason, "bad things"),
            other => panic!("unexpected status: {other:?}"),
        }
    }

    #[test]
    fn test_plan_tracker_mark_superseded_persists_by() {
        let dir = tempdir().unwrap();
        let storage = IntrospectionStorage::new(dir.path().join("introspection"));
        storage.initialize().unwrap();
        let tracker = PlanTracker::new(storage, PlanTrackerConfig::default());

        let plan = ExecutionPlan::new("Test Plan", PlanCategory::ApplicationImprovement);
        tracker.track_plan(&plan).unwrap();

        let superseded_by = Uuid::new_v4();
        let updated = tracker.mark_superseded(&plan.id, &superseded_by).unwrap();
        match updated.status {
            PlanStatus::Superseded { by } => assert_eq!(by, superseded_by),
            other => panic!("unexpected status: {other:?}"),
        }
    }
}
