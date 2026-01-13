//! Contribution System for AI-Native Package Registry
//!
//! This module provides structured contribution types and multi-model review
//! for AI contributions to packages.
//!
//! # Key Features
//!
//! - **Structured Contributions**: Bug reports, improvements, package requests, and fixes
//!   are typed schemas, not free-form text
//! - **Multi-Model Review**: Multiple AI models review contributions for consensus
//! - **Auto-Approve Policies**: Configurable policies for automatic approval
//! - **Evidence-Based**: Contributions include verifiable evidence (traces, reproduction steps)
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_registry::contribution::{
//!     BugReport, BugCategory, BugSeverity, ContributionReviewer, ReviewConfig,
//! };
//!
//! // Create a structured bug report
//! let bug = BugReport::builder()
//!     .title("Fails on emoji input")
//!     .category(BugCategory::RuntimeError)
//!     .severity(BugSeverity::Medium)
//!     .description("SentimentNode throws ParseError when input contains emoji")
//!     .occurrence_rate(0.03)
//!     .build()?;
//!
//! // Submit for multi-model review
//! let result = reviewer.review(&Contribution::Bug(bug)).await?;
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::content_hash::ContentHash;
use crate::error::{RegistryError, Result};
use crate::signature::{PublicKey, Signature};

// ============================================================================
// Contributor Identity
// ============================================================================

/// Information about a contributor (AI agent or human)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contributor {
    /// Unique identifier for the contributor
    pub id: Uuid,

    /// Display name
    pub name: String,

    /// Public key for signature verification
    pub public_key: PublicKey,

    /// Whether this is an AI agent
    pub is_ai: bool,

    /// Agent app ID (if AI)
    pub app_id: Option<Uuid>,

    /// Model identifier (if AI)
    pub model: Option<String>,
}

impl Contributor {
    /// Create a new AI contributor
    pub fn ai(name: impl Into<String>, app_id: Uuid, model: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            public_key: PublicKey::default(),
            is_ai: true,
            app_id: Some(app_id),
            model: Some(model.into()),
        }
    }

    /// Create a new human contributor
    pub fn human(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            public_key: PublicKey::default(),
            is_ai: false,
            app_id: None,
            model: None,
        }
    }

    /// Set the public key
    pub fn with_public_key(mut self, key: PublicKey) -> Self {
        self.public_key = key;
        self
    }
}

// ============================================================================
// Bug Report Types
// ============================================================================

/// Category of bug
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BugCategory {
    /// Runtime error (panic, exception)
    RuntimeError,
    /// Logic error (wrong output)
    LogicError,
    /// Performance issue
    Performance,
    /// Memory issue (leak, overflow)
    Memory,
    /// Security vulnerability
    Security,
    /// Documentation error
    Documentation,
    /// API mismatch
    ApiMismatch,
    /// Other
    Other,
}

/// Severity of bug
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BugSeverity {
    /// Low impact, workaround available
    Low,
    /// Medium impact, affects some functionality
    Medium,
    /// High impact, major functionality broken
    High,
    /// Critical, security or data loss risk
    Critical,
}

/// Condition that triggers the bug
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerCondition {
    /// Field being checked
    pub field: String,

    /// Comparison operator
    pub operator: TriggerOperator,

    /// Value to compare against
    pub value: serde_json::Value,
}

/// Operators for trigger conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    Contains,
    Matches,
}

/// Evidence supporting a bug report
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BugEvidence {
    /// Trace IDs where bug was observed
    pub traces: Vec<String>,

    /// Error messages observed
    pub error_messages: Vec<String>,

    /// Reproduction steps
    pub reproduction_steps: Vec<ReproductionStep>,

    /// Stack traces (if available)
    pub stack_traces: Vec<String>,

    /// Sample inputs that trigger the bug
    pub sample_inputs: Vec<serde_json::Value>,

    /// Sample outputs (erroneous)
    pub sample_outputs: Vec<serde_json::Value>,
}

/// A step to reproduce a bug
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReproductionStep {
    /// Action to perform
    pub action: String,

    /// Parameters for the action
    pub params: HashMap<String, serde_json::Value>,

    /// Expected result
    pub expected: Option<String>,

    /// Actual result
    pub actual: Option<String>,
}

/// Suggested fix for a bug
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedFix {
    /// Description of the fix
    pub description: String,

    /// Confidence in the fix (0-1)
    pub confidence: f64,

    /// Unified diff of the fix
    pub diff: Option<String>,

    /// Files affected
    pub files: Vec<String>,
}

/// Structured bug report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BugReport {
    /// Unique ID for this report
    pub id: Uuid,

    /// Bug title
    pub title: String,

    /// Category of bug
    pub category: BugCategory,

    /// Severity level
    pub severity: BugSeverity,

    /// Detailed description
    pub description: String,

    /// Conditions that trigger the bug
    pub trigger_conditions: Vec<TriggerCondition>,

    /// Occurrence rate (0-1) if known
    pub occurrence_rate: Option<f64>,

    /// Sample count for occurrence rate
    pub sample_count: Option<u64>,

    /// Evidence supporting the report
    pub evidence: BugEvidence,

    /// Suggested fix (if any)
    pub suggested_fix: Option<SuggestedFix>,

    /// When the bug was first observed
    pub first_observed: DateTime<Utc>,

    /// When the bug was last observed
    pub last_observed: DateTime<Utc>,
}

impl BugReport {
    /// Create a new bug report builder
    pub fn builder() -> BugReportBuilder {
        BugReportBuilder::new()
    }

    /// Calculate a priority score (0-100)
    pub fn priority_score(&self) -> u32 {
        let severity_score = match self.severity {
            BugSeverity::Low => 10,
            BugSeverity::Medium => 30,
            BugSeverity::High => 60,
            BugSeverity::Critical => 100,
        };

        let occurrence_factor = self
            .occurrence_rate
            .map(|r| (r * 100.0) as u32)
            .unwrap_or(50);

        let evidence_bonus = if !self.evidence.reproduction_steps.is_empty() {
            10
        } else {
            0
        };

        let fix_bonus = if self.suggested_fix.is_some() { 5 } else { 0 };

        (severity_score + occurrence_factor / 2 + evidence_bonus + fix_bonus).min(100)
    }
}

/// Builder for BugReport
#[derive(Debug, Default)]
pub struct BugReportBuilder {
    title: Option<String>,
    category: Option<BugCategory>,
    severity: Option<BugSeverity>,
    description: Option<String>,
    trigger_conditions: Vec<TriggerCondition>,
    occurrence_rate: Option<f64>,
    sample_count: Option<u64>,
    evidence: BugEvidence,
    suggested_fix: Option<SuggestedFix>,
}

impl BugReportBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn category(mut self, category: BugCategory) -> Self {
        self.category = Some(category);
        self
    }

    pub fn severity(mut self, severity: BugSeverity) -> Self {
        self.severity = Some(severity);
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn trigger_condition(mut self, condition: TriggerCondition) -> Self {
        self.trigger_conditions.push(condition);
        self
    }

    pub fn occurrence_rate(mut self, rate: f64) -> Self {
        self.occurrence_rate = Some(rate);
        self
    }

    pub fn sample_count(mut self, count: u64) -> Self {
        self.sample_count = Some(count);
        self
    }

    pub fn evidence(mut self, evidence: BugEvidence) -> Self {
        self.evidence = evidence;
        self
    }

    pub fn suggested_fix(mut self, fix: SuggestedFix) -> Self {
        self.suggested_fix = Some(fix);
        self
    }

    pub fn build(self) -> Result<BugReport> {
        let title = self
            .title
            .ok_or_else(|| RegistryError::Validation("Bug report requires title".into()))?;
        let category = self.category.unwrap_or(BugCategory::Other);
        let severity = self.severity.unwrap_or(BugSeverity::Medium);
        let description = self.description.unwrap_or_default();

        let now = Utc::now();

        Ok(BugReport {
            id: Uuid::new_v4(),
            title,
            category,
            severity,
            description,
            trigger_conditions: self.trigger_conditions,
            occurrence_rate: self.occurrence_rate,
            sample_count: self.sample_count,
            evidence: self.evidence,
            suggested_fix: self.suggested_fix,
            first_observed: now,
            last_observed: now,
        })
    }
}

// ============================================================================
// Improvement Proposal Types
// ============================================================================

/// Category of improvement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImprovementCategory {
    /// Performance optimization
    Performance,
    /// API enhancement
    Api,
    /// New capability
    NewCapability,
    /// Documentation
    Documentation,
    /// Testing
    Testing,
    /// Code quality
    CodeQuality,
    /// Security hardening
    Security,
    /// Other
    Other,
}

/// Impact level of an improvement
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactLevel {
    /// Minor convenience improvement
    Minor,
    /// Moderate improvement
    Moderate,
    /// Significant improvement
    Significant,
    /// Major improvement
    Major,
}

/// A structured improvement proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementProposal {
    /// Unique ID
    pub id: Uuid,

    /// Title of the proposal
    pub title: String,

    /// Category of improvement
    pub category: ImprovementCategory,

    /// Expected impact
    pub impact: ImpactLevel,

    /// Detailed description
    pub description: String,

    /// Motivation / rationale
    pub motivation: String,

    /// Proposed solution
    pub solution: String,

    /// Alternative solutions considered
    pub alternatives: Vec<AlternativeSolution>,

    /// Breaking changes (if any)
    pub breaking_changes: Vec<String>,

    /// Estimated effort (in abstract units)
    pub effort_estimate: Option<EffortEstimate>,

    /// Implementation diff (if available)
    pub implementation_diff: Option<String>,

    /// When proposed
    pub proposed_at: DateTime<Utc>,
}

impl ImprovementProposal {
    /// Create a new improvement proposal builder
    pub fn builder() -> ImprovementProposalBuilder {
        ImprovementProposalBuilder::new()
    }
}

/// An alternative solution considered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternativeSolution {
    /// Description of the alternative
    pub description: String,

    /// Why it was not chosen
    pub rejection_reason: String,
}

/// Effort estimate for an improvement
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffortEstimate {
    /// Very small, < 100 lines
    Trivial,
    /// Small, 100-500 lines
    Small,
    /// Medium, 500-2000 lines
    Medium,
    /// Large, 2000-10000 lines
    Large,
    /// Very large, > 10000 lines
    Massive,
}

/// Builder for ImprovementProposal
#[derive(Debug, Default)]
pub struct ImprovementProposalBuilder {
    title: Option<String>,
    category: Option<ImprovementCategory>,
    impact: Option<ImpactLevel>,
    description: Option<String>,
    motivation: Option<String>,
    solution: Option<String>,
    alternatives: Vec<AlternativeSolution>,
    breaking_changes: Vec<String>,
    effort_estimate: Option<EffortEstimate>,
    implementation_diff: Option<String>,
}

impl ImprovementProposalBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn category(mut self, category: ImprovementCategory) -> Self {
        self.category = Some(category);
        self
    }

    pub fn impact(mut self, impact: ImpactLevel) -> Self {
        self.impact = Some(impact);
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn motivation(mut self, motivation: impl Into<String>) -> Self {
        self.motivation = Some(motivation.into());
        self
    }

    pub fn solution(mut self, solution: impl Into<String>) -> Self {
        self.solution = Some(solution.into());
        self
    }

    pub fn alternative(mut self, alt: AlternativeSolution) -> Self {
        self.alternatives.push(alt);
        self
    }

    pub fn breaking_change(mut self, change: impl Into<String>) -> Self {
        self.breaking_changes.push(change.into());
        self
    }

    pub fn effort_estimate(mut self, estimate: EffortEstimate) -> Self {
        self.effort_estimate = Some(estimate);
        self
    }

    pub fn implementation_diff(mut self, diff: impl Into<String>) -> Self {
        self.implementation_diff = Some(diff.into());
        self
    }

    pub fn build(self) -> Result<ImprovementProposal> {
        let title = self
            .title
            .ok_or_else(|| RegistryError::Validation("Improvement requires title".into()))?;

        Ok(ImprovementProposal {
            id: Uuid::new_v4(),
            title,
            category: self.category.unwrap_or(ImprovementCategory::Other),
            impact: self.impact.unwrap_or(ImpactLevel::Minor),
            description: self.description.unwrap_or_default(),
            motivation: self.motivation.unwrap_or_default(),
            solution: self.solution.unwrap_or_default(),
            alternatives: self.alternatives,
            breaking_changes: self.breaking_changes,
            effort_estimate: self.effort_estimate,
            implementation_diff: self.implementation_diff,
            proposed_at: Utc::now(),
        })
    }
}

// ============================================================================
// Package Request Types
// ============================================================================

/// Priority level for a package request
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestPriority {
    /// Nice to have
    Low,
    /// Would be useful
    Medium,
    /// Needed for project
    High,
    /// Blocking work
    Critical,
}

/// A request for a new package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageRequest {
    /// Unique ID
    pub id: Uuid,

    /// Requested package name (suggestion)
    pub suggested_name: String,

    /// Short description of what's needed
    pub title: String,

    /// Detailed description
    pub description: String,

    /// Use cases
    pub use_cases: Vec<String>,

    /// Required capabilities
    pub required_capabilities: Vec<String>,

    /// Similar existing packages (if known)
    pub similar_packages: Vec<SimilarPackage>,

    /// Priority
    pub priority: RequestPriority,

    /// Requester's context
    pub context: String,

    /// When requested
    pub requested_at: DateTime<Utc>,
}

impl PackageRequest {
    /// Create a new package request builder
    pub fn builder() -> PackageRequestBuilder {
        PackageRequestBuilder::new()
    }
}

/// A similar existing package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarPackage {
    /// Package name
    pub name: String,

    /// Why it doesn't meet the need
    pub gap: String,
}

/// Builder for PackageRequest
#[derive(Debug, Default)]
pub struct PackageRequestBuilder {
    suggested_name: Option<String>,
    title: Option<String>,
    description: Option<String>,
    use_cases: Vec<String>,
    required_capabilities: Vec<String>,
    similar_packages: Vec<SimilarPackage>,
    priority: Option<RequestPriority>,
    context: Option<String>,
}

impl PackageRequestBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn suggested_name(mut self, name: impl Into<String>) -> Self {
        self.suggested_name = Some(name.into());
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn use_case(mut self, use_case: impl Into<String>) -> Self {
        self.use_cases.push(use_case.into());
        self
    }

    pub fn required_capability(mut self, capability: impl Into<String>) -> Self {
        self.required_capabilities.push(capability.into());
        self
    }

    pub fn similar_package(mut self, similar: SimilarPackage) -> Self {
        self.similar_packages.push(similar);
        self
    }

    pub fn priority(mut self, priority: RequestPriority) -> Self {
        self.priority = Some(priority);
        self
    }

    pub fn context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn build(self) -> Result<PackageRequest> {
        let title = self
            .title
            .ok_or_else(|| RegistryError::Validation("Package request requires title".into()))?;

        Ok(PackageRequest {
            id: Uuid::new_v4(),
            suggested_name: self.suggested_name.unwrap_or_default(),
            title,
            description: self.description.unwrap_or_default(),
            use_cases: self.use_cases,
            required_capabilities: self.required_capabilities,
            similar_packages: self.similar_packages,
            priority: self.priority.unwrap_or(RequestPriority::Medium),
            context: self.context.unwrap_or_default(),
            requested_at: Utc::now(),
        })
    }
}

// ============================================================================
// Fix Submission Types
// ============================================================================

/// Type of fix
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixType {
    /// Bug fix
    BugFix,
    /// Security patch
    SecurityPatch,
    /// Performance fix
    Performance,
    /// Documentation fix
    Documentation,
    /// Test fix
    TestFix,
    /// Other
    Other,
}

/// A fix submission with code changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixSubmission {
    /// Unique ID
    pub id: Uuid,

    /// Title of the fix
    pub title: String,

    /// Type of fix
    pub fix_type: FixType,

    /// Description of what's fixed
    pub description: String,

    /// Reference to bug report or issue (if any)
    pub fixes_issue: Option<Uuid>,

    /// Unified diff of the changes
    pub diff: String,

    /// Files changed
    pub files_changed: Vec<FileChange>,

    /// Test coverage added
    pub tests_added: Vec<String>,

    /// Breaking changes (if any)
    pub breaking_changes: Vec<String>,

    /// When submitted
    pub submitted_at: DateTime<Utc>,
}

impl FixSubmission {
    /// Create a new fix submission builder
    pub fn builder() -> FixSubmissionBuilder {
        FixSubmissionBuilder::new()
    }

    /// Get total lines changed
    pub fn lines_changed(&self) -> usize {
        self.files_changed
            .iter()
            .map(|f| f.additions + f.deletions)
            .sum()
    }
}

/// A file change in a fix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// File path
    pub path: String,

    /// Change type
    pub change_type: FileChangeType,

    /// Lines added
    pub additions: usize,

    /// Lines deleted
    pub deletions: usize,
}

/// Type of file change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
}

/// Builder for FixSubmission
#[derive(Debug, Default)]
pub struct FixSubmissionBuilder {
    title: Option<String>,
    fix_type: Option<FixType>,
    description: Option<String>,
    fixes_issue: Option<Uuid>,
    diff: Option<String>,
    files_changed: Vec<FileChange>,
    tests_added: Vec<String>,
    breaking_changes: Vec<String>,
}

impl FixSubmissionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn fix_type(mut self, fix_type: FixType) -> Self {
        self.fix_type = Some(fix_type);
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn fixes_issue(mut self, issue_id: Uuid) -> Self {
        self.fixes_issue = Some(issue_id);
        self
    }

    pub fn diff(mut self, diff: impl Into<String>) -> Self {
        self.diff = Some(diff.into());
        self
    }

    pub fn file_change(mut self, change: FileChange) -> Self {
        self.files_changed.push(change);
        self
    }

    pub fn test_added(mut self, test: impl Into<String>) -> Self {
        self.tests_added.push(test.into());
        self
    }

    pub fn breaking_change(mut self, change: impl Into<String>) -> Self {
        self.breaking_changes.push(change.into());
        self
    }

    pub fn build(self) -> Result<FixSubmission> {
        let title = self
            .title
            .ok_or_else(|| RegistryError::Validation("Fix requires title".into()))?;
        let diff = self
            .diff
            .ok_or_else(|| RegistryError::Validation("Fix requires diff".into()))?;

        Ok(FixSubmission {
            id: Uuid::new_v4(),
            title,
            fix_type: self.fix_type.unwrap_or(FixType::BugFix),
            description: self.description.unwrap_or_default(),
            fixes_issue: self.fixes_issue,
            diff,
            files_changed: self.files_changed,
            tests_added: self.tests_added,
            breaking_changes: self.breaking_changes,
            submitted_at: Utc::now(),
        })
    }
}

// ============================================================================
// Unified Contribution Type
// ============================================================================

/// Type of contribution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributionType {
    Bug,
    Improvement,
    Request,
    Fix,
}

/// A contribution to a package
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Contribution {
    /// Bug report
    Bug(BugReport),

    /// Improvement proposal
    Improvement(ImprovementProposal),

    /// Package request
    Request(PackageRequest),

    /// Fix submission
    Fix(FixSubmission),
}

impl Contribution {
    /// Get the contribution type
    pub fn contribution_type(&self) -> ContributionType {
        match self {
            Contribution::Bug(_) => ContributionType::Bug,
            Contribution::Improvement(_) => ContributionType::Improvement,
            Contribution::Request(_) => ContributionType::Request,
            Contribution::Fix(_) => ContributionType::Fix,
        }
    }

    /// Get the contribution ID
    pub fn id(&self) -> Uuid {
        match self {
            Contribution::Bug(b) => b.id,
            Contribution::Improvement(i) => i.id,
            Contribution::Request(r) => r.id,
            Contribution::Fix(f) => f.id,
        }
    }

    /// Get the contribution title
    pub fn title(&self) -> &str {
        match self {
            Contribution::Bug(b) => &b.title,
            Contribution::Improvement(i) => &i.title,
            Contribution::Request(r) => &r.title,
            Contribution::Fix(f) => &f.title,
        }
    }

    /// Get lines changed (for fixes) or 0 for other types
    pub fn lines_changed(&self) -> usize {
        match self {
            Contribution::Fix(f) => f.lines_changed(),
            _ => 0,
        }
    }

    /// Check if this is a low-risk contribution
    pub fn is_low_risk(&self, max_lines: usize) -> bool {
        match self {
            Contribution::Bug(_) => true,     // Bug reports are low risk
            Contribution::Request(_) => true, // Requests are low risk
            Contribution::Improvement(i) => i.breaking_changes.is_empty(),
            Contribution::Fix(f) => f.lines_changed() <= max_lines && f.breaking_changes.is_empty(),
        }
    }
}

/// A signed contribution (signed by the contributor)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedContribution {
    /// The contribution
    pub contribution: Contribution,

    /// Package this contribution is for
    pub package_hash: ContentHash,

    /// Who submitted this
    pub contributor: Contributor,

    /// Signature of the contribution
    pub signature: Signature,

    /// When signed
    pub signed_at: DateTime<Utc>,
}

/// Status of a contribution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributionStatus {
    /// Just submitted, awaiting review
    Submitted,
    /// Under review
    UnderReview,
    /// Awaiting consensus
    AwaitingConsensus,
    /// Auto-approved by consensus
    AutoApproved,
    /// Approved by human
    Approved,
    /// Rejected
    Rejected,
    /// Merged/implemented
    Merged,
    /// Closed (won't fix, duplicate, etc.)
    Closed,
}

// ============================================================================
// Review System Types
// ============================================================================

/// A model's review of a contribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelReviewResult {
    /// Model identifier
    pub model_id: String,

    /// Model name (e.g., "claude-3-opus")
    pub model_name: String,

    /// Overall verdict
    pub verdict: ReviewVerdict,

    /// Confidence in the verdict (0-1)
    pub confidence: f64,

    /// Detailed scores
    pub scores: ReviewScores,

    /// Comments from the model
    pub comments: Vec<String>,

    /// Concerns raised
    pub concerns: Vec<ReviewConcern>,

    /// Suggestions for improvement
    pub suggestions: Vec<String>,

    /// When reviewed
    pub reviewed_at: DateTime<Utc>,
}

/// Review verdict from a model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    /// Approve as-is
    Approve,
    /// Approve with minor suggestions
    ApproveWithSuggestions,
    /// Needs changes before approval
    RequestChanges,
    /// Reject
    Reject,
    /// Unable to determine
    Abstain,
}

impl ReviewVerdict {
    /// Convert to numeric score for consensus calculation
    pub fn score(&self) -> f64 {
        match self {
            ReviewVerdict::Approve => 1.0,
            ReviewVerdict::ApproveWithSuggestions => 0.8,
            ReviewVerdict::RequestChanges => 0.3,
            ReviewVerdict::Reject => 0.0,
            ReviewVerdict::Abstain => 0.5,
        }
    }

    /// Check if this is a positive verdict
    pub fn is_positive(&self) -> bool {
        matches!(
            self,
            ReviewVerdict::Approve | ReviewVerdict::ApproveWithSuggestions
        )
    }
}

/// Detailed scores from a review
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReviewScores {
    /// Quality of the contribution (0-1)
    pub quality: f64,

    /// Relevance to the package (0-1)
    pub relevance: f64,

    /// Completeness of information (0-1)
    pub completeness: f64,

    /// Safety/security assessment (0-1)
    pub safety: f64,

    /// Test coverage (for fixes) (0-1)
    pub test_coverage: f64,
}

impl ReviewScores {
    /// Calculate overall score (weighted average)
    pub fn overall(&self) -> f64 {
        let weights = [0.25, 0.2, 0.2, 0.25, 0.1];
        let scores = [
            self.quality,
            self.relevance,
            self.completeness,
            self.safety,
            self.test_coverage,
        ];

        weights.iter().zip(scores.iter()).map(|(w, s)| w * s).sum()
    }
}

/// A concern raised during review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewConcern {
    /// Severity of concern
    pub severity: ConcernSeverity,

    /// Category of concern
    pub category: ConcernCategory,

    /// Description
    pub description: String,

    /// Suggested resolution
    pub resolution: Option<String>,
}

/// Severity of a review concern
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConcernSeverity {
    /// Minor nitpick
    Minor,
    /// Should be addressed
    Moderate,
    /// Must be addressed
    Major,
    /// Blocking issue
    Critical,
}

/// Category of review concern
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConcernCategory {
    /// Security issue
    Security,
    /// Code quality
    Quality,
    /// Missing tests
    Testing,
    /// Incomplete information
    Incomplete,
    /// Incorrect implementation
    Correctness,
    /// Performance issue
    Performance,
    /// Other
    Other,
}

// ============================================================================
// Consensus and Review Results
// ============================================================================

/// Consensus result from multiple models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    /// Overall consensus score (0-1)
    pub score: f64,

    /// Number of models that participated
    pub model_count: usize,

    /// Number of positive verdicts
    pub positive_count: usize,

    /// Number of negative verdicts
    pub negative_count: usize,

    /// Number of abstentions
    pub abstain_count: usize,

    /// Disagreements between models
    pub disagreements: Vec<String>,

    /// Common concerns raised by multiple models
    pub common_concerns: Vec<String>,

    /// Consensus reached?
    pub consensus_reached: bool,
}

/// Action to take based on review
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ReviewAction {
    /// Auto-approve without human review
    AutoApprove,

    /// Approve but notify human
    NotifyHuman { urgency: Urgency },

    /// Require human approval
    RequireHumanApproval,

    /// Reject with reasons
    Reject { reasons: Vec<String> },

    /// Request changes from contributor
    RequestChanges { changes: Vec<String> },
}

/// Urgency level for notifications
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    Low,
    Medium,
    High,
}

/// Complete review result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    /// Individual model reviews
    pub reviews: Vec<ModelReviewResult>,

    /// Consensus calculation
    pub consensus: ConsensusResult,

    /// Recommended action
    pub action: ReviewAction,

    /// When review completed
    pub completed_at: DateTime<Utc>,
}

// ============================================================================
// Review Configuration
// ============================================================================

/// Policy for auto-approval
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum AutoApprovePolicy {
    /// Never auto-approve, always require human review
    Never,

    /// Auto-approve low-risk contributions
    LowRisk {
        /// Maximum lines changed for auto-approval
        max_lines_changed: usize,
        /// Allowed contribution types
        allowed_types: Vec<ContributionType>,
    },

    /// Auto-approve if high consensus
    HighConsensus {
        /// Threshold for auto-approval (e.g., 0.95)
        threshold: f64,
    },

    /// Auto-approve all with consensus
    Always,
}

impl Default for AutoApprovePolicy {
    fn default() -> Self {
        AutoApprovePolicy::LowRisk {
            max_lines_changed: 100,
            allowed_types: vec![ContributionType::Bug, ContributionType::Request],
        }
    }
}

/// Configuration for the review system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewConfig {
    /// Minimum number of models that must review
    pub min_reviews: usize,

    /// Consensus score threshold (0-1)
    pub consensus_threshold: f64,

    /// Auto-approve policy
    pub auto_approve: AutoApprovePolicy,

    /// Timeout for reviews (seconds)
    pub review_timeout_secs: u64,

    /// Whether to require unanimous positive verdict
    pub require_unanimous: bool,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            min_reviews: 2,
            consensus_threshold: 0.7,
            auto_approve: AutoApprovePolicy::default(),
            review_timeout_secs: 300,
            require_unanimous: false,
        }
    }
}

// ============================================================================
// Model Reviewer Trait
// ============================================================================

/// Trait for AI models that can review contributions
#[async_trait]
pub trait ModelReviewer: Send + Sync {
    /// Get the model identifier
    fn model_id(&self) -> &str;

    /// Get the model name
    fn model_name(&self) -> &str;

    /// Review a contribution
    async fn review(&self, contribution: &Contribution) -> Result<ModelReviewResult>;
}

/// A mock model reviewer for testing
pub struct MockModelReviewer {
    /// Model identifier
    pub id: String,
    /// Model name
    pub name: String,
    /// Default verdict to return
    pub default_verdict: ReviewVerdict,
    /// Default confidence
    pub default_confidence: f64,
}

impl MockModelReviewer {
    /// Create a new mock reviewer
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        verdict: ReviewVerdict,
        confidence: f64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            default_verdict: verdict,
            default_confidence: confidence,
        }
    }

    /// Create an approving reviewer
    pub fn approving(id: impl Into<String>) -> Self {
        Self::new(id, "Mock Approver", ReviewVerdict::Approve, 0.9)
    }

    /// Create a rejecting reviewer
    pub fn rejecting(id: impl Into<String>) -> Self {
        Self::new(id, "Mock Rejecter", ReviewVerdict::Reject, 0.9)
    }
}

#[async_trait]
impl ModelReviewer for MockModelReviewer {
    fn model_id(&self) -> &str {
        &self.id
    }

    fn model_name(&self) -> &str {
        &self.name
    }

    async fn review(&self, _contribution: &Contribution) -> Result<ModelReviewResult> {
        Ok(ModelReviewResult {
            model_id: self.id.clone(),
            model_name: self.name.clone(),
            verdict: self.default_verdict,
            confidence: self.default_confidence,
            scores: ReviewScores {
                quality: 0.8,
                relevance: 0.9,
                completeness: 0.85,
                safety: 0.95,
                test_coverage: 0.7,
            },
            comments: vec!["Review completed successfully.".to_string()],
            concerns: vec![],
            suggestions: vec![],
            reviewed_at: Utc::now(),
        })
    }
}

// ============================================================================
// Contribution Reviewer
// ============================================================================

/// Multi-model contribution reviewer
pub struct ContributionReviewer {
    /// Review models
    reviewers: Vec<Arc<dyn ModelReviewer>>,

    /// Review configuration
    config: ReviewConfig,
}

impl ContributionReviewer {
    /// Create a new contribution reviewer
    pub fn new(config: ReviewConfig) -> Self {
        Self {
            reviewers: Vec::new(),
            config,
        }
    }

    /// Add a model reviewer
    pub fn add_reviewer(&mut self, reviewer: Arc<dyn ModelReviewer>) {
        self.reviewers.push(reviewer);
    }

    /// Add a model reviewer (builder pattern)
    pub fn with_reviewer(mut self, reviewer: Arc<dyn ModelReviewer>) -> Self {
        self.reviewers.push(reviewer);
        self
    }

    /// Get the current configuration
    pub fn config(&self) -> &ReviewConfig {
        &self.config
    }

    /// Review a contribution with multiple models
    pub async fn review(&self, contribution: &Contribution) -> Result<ReviewResult> {
        if self.reviewers.len() < self.config.min_reviews {
            return Err(RegistryError::Validation(format!(
                "Not enough reviewers: have {}, need {}",
                self.reviewers.len(),
                self.config.min_reviews
            )));
        }

        // Get reviews from multiple models in parallel
        let review_futures: Vec<_> = self
            .reviewers
            .iter()
            .map(|r| r.review(contribution))
            .collect();

        let reviews: Vec<ModelReviewResult> = futures::future::join_all(review_futures)
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();

        if reviews.len() < self.config.min_reviews {
            return Err(RegistryError::ReviewFailed(format!(
                "Only {} of {} reviews succeeded",
                reviews.len(),
                self.reviewers.len()
            )));
        }

        // Calculate consensus
        let consensus = self.calculate_consensus(&reviews);

        // Determine action
        let action = self.determine_action(contribution, &consensus);

        Ok(ReviewResult {
            reviews,
            consensus,
            action,
            completed_at: Utc::now(),
        })
    }

    /// Calculate consensus from multiple reviews
    fn calculate_consensus(&self, reviews: &[ModelReviewResult]) -> ConsensusResult {
        if reviews.is_empty() {
            return ConsensusResult {
                score: 0.0,
                model_count: 0,
                positive_count: 0,
                negative_count: 0,
                abstain_count: 0,
                disagreements: vec![],
                common_concerns: vec![],
                consensus_reached: false,
            };
        }

        let mut positive_count = 0;
        let mut negative_count = 0;
        let mut abstain_count = 0;
        let mut score_sum = 0.0;

        for review in reviews {
            score_sum += review.verdict.score();
            match review.verdict {
                ReviewVerdict::Approve | ReviewVerdict::ApproveWithSuggestions => {
                    positive_count += 1
                }
                ReviewVerdict::RequestChanges | ReviewVerdict::Reject => negative_count += 1,
                ReviewVerdict::Abstain => abstain_count += 1,
            }
        }

        let score = score_sum / reviews.len() as f64;
        let model_count = reviews.len();

        // Find disagreements
        let mut disagreements = Vec::new();
        let verdicts: Vec<_> = reviews.iter().map(|r| r.verdict).collect();
        let has_approve = verdicts.iter().any(|v| v.is_positive());
        let has_reject = verdicts
            .iter()
            .any(|v| matches!(v, ReviewVerdict::Reject | ReviewVerdict::RequestChanges));

        if has_approve && has_reject {
            disagreements.push("Models disagree on approval vs rejection".to_string());
        }

        // Find common concerns
        let mut concern_counts: HashMap<String, usize> = HashMap::new();
        for review in reviews {
            for concern in &review.concerns {
                *concern_counts
                    .entry(concern.description.clone())
                    .or_insert(0) += 1;
            }
        }

        let common_concerns: Vec<String> = concern_counts
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .map(|(desc, _)| desc)
            .collect();

        let consensus_reached = if self.config.require_unanimous {
            positive_count == model_count || negative_count == model_count
        } else {
            score >= self.config.consensus_threshold
        };

        ConsensusResult {
            score,
            model_count,
            positive_count,
            negative_count,
            abstain_count,
            disagreements,
            common_concerns,
            consensus_reached,
        }
    }

    /// Determine action based on consensus and policy
    fn determine_action(
        &self,
        contribution: &Contribution,
        consensus: &ConsensusResult,
    ) -> ReviewAction {
        match &self.config.auto_approve {
            AutoApprovePolicy::Never => ReviewAction::RequireHumanApproval,

            AutoApprovePolicy::LowRisk {
                max_lines_changed,
                allowed_types,
            } => {
                if contribution.is_low_risk(*max_lines_changed)
                    && allowed_types.contains(&contribution.contribution_type())
                    && consensus.score >= self.config.consensus_threshold
                {
                    ReviewAction::AutoApprove
                } else {
                    ReviewAction::RequireHumanApproval
                }
            }

            AutoApprovePolicy::HighConsensus { threshold } => {
                if consensus.score >= *threshold {
                    ReviewAction::AutoApprove
                } else if consensus.score >= self.config.consensus_threshold {
                    ReviewAction::NotifyHuman {
                        urgency: Urgency::Low,
                    }
                } else {
                    ReviewAction::RequireHumanApproval
                }
            }

            AutoApprovePolicy::Always => {
                if consensus.score >= self.config.consensus_threshold {
                    ReviewAction::AutoApprove
                } else {
                    ReviewAction::Reject {
                        reasons: consensus.disagreements.clone(),
                    }
                }
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bug_report_builder() {
        let bug = BugReport::builder()
            .title("Test bug")
            .category(BugCategory::RuntimeError)
            .severity(BugSeverity::High)
            .description("Something went wrong")
            .occurrence_rate(0.1)
            .build()
            .unwrap();

        assert_eq!(bug.title, "Test bug");
        assert_eq!(bug.category, BugCategory::RuntimeError);
        assert_eq!(bug.severity, BugSeverity::High);
        assert!(bug.priority_score() > 0);
    }

    #[test]
    fn test_bug_priority_score() {
        let low_bug = BugReport::builder()
            .title("Low priority")
            .severity(BugSeverity::Low)
            .build()
            .unwrap();

        let critical_bug = BugReport::builder()
            .title("Critical bug")
            .severity(BugSeverity::Critical)
            .occurrence_rate(0.9)
            .build()
            .unwrap();

        assert!(critical_bug.priority_score() > low_bug.priority_score());
    }

    #[test]
    fn test_improvement_proposal_builder() {
        let proposal = ImprovementProposal::builder()
            .title("Add feature X")
            .category(ImprovementCategory::NewCapability)
            .impact(ImpactLevel::Significant)
            .motivation("Users need this")
            .solution("Implement X using Y")
            .build()
            .unwrap();

        assert_eq!(proposal.title, "Add feature X");
        assert_eq!(proposal.impact, ImpactLevel::Significant);
    }

    #[test]
    fn test_package_request_builder() {
        let request = PackageRequest::builder()
            .title("Need sentiment analysis")
            .suggested_name("sentiment-analyzer")
            .use_case("Customer feedback analysis")
            .priority(RequestPriority::High)
            .build()
            .unwrap();

        assert_eq!(request.title, "Need sentiment analysis");
        assert_eq!(request.priority, RequestPriority::High);
    }

    #[test]
    fn test_fix_submission_builder() {
        let fix = FixSubmission::builder()
            .title("Fix null pointer")
            .fix_type(FixType::BugFix)
            .diff("--- a/foo.rs\n+++ b/foo.rs")
            .file_change(FileChange {
                path: "src/foo.rs".to_string(),
                change_type: FileChangeType::Modified,
                additions: 5,
                deletions: 2,
            })
            .build()
            .unwrap();

        assert_eq!(fix.title, "Fix null pointer");
        assert_eq!(fix.lines_changed(), 7);
    }

    #[test]
    fn test_contribution_type() {
        let bug = Contribution::Bug(BugReport::builder().title("Test").build().unwrap());

        assert_eq!(bug.contribution_type(), ContributionType::Bug);
        assert!(bug.is_low_risk(100));
    }

    #[test]
    fn test_review_verdict_score() {
        assert!((ReviewVerdict::Approve.score() - 1.0).abs() < f64::EPSILON);
        assert!(ReviewVerdict::Reject.score().abs() < f64::EPSILON);
        assert!(ReviewVerdict::Approve.is_positive());
        assert!(!ReviewVerdict::Reject.is_positive());
    }

    #[test]
    fn test_review_scores_overall() {
        let scores = ReviewScores {
            quality: 0.8,
            relevance: 0.9,
            completeness: 0.85,
            safety: 0.95,
            test_coverage: 0.7,
        };

        let overall = scores.overall();
        assert!(overall > 0.0 && overall <= 1.0);
    }

    #[tokio::test]
    async fn test_mock_model_reviewer() {
        let reviewer = MockModelReviewer::approving("test-model");
        let contribution =
            Contribution::Bug(BugReport::builder().title("Test bug").build().unwrap());

        let result = reviewer.review(&contribution).await.unwrap();
        assert_eq!(result.verdict, ReviewVerdict::Approve);
        assert_eq!(result.model_id, "test-model");
    }

    #[tokio::test]
    async fn test_contribution_reviewer_approval() {
        let config = ReviewConfig {
            min_reviews: 2,
            consensus_threshold: 0.7,
            auto_approve: AutoApprovePolicy::HighConsensus { threshold: 0.8 },
            ..Default::default()
        };

        let reviewer = ContributionReviewer::new(config)
            .with_reviewer(Arc::new(MockModelReviewer::approving("model-1")))
            .with_reviewer(Arc::new(MockModelReviewer::approving("model-2")));

        let contribution =
            Contribution::Bug(BugReport::builder().title("Test bug").build().unwrap());

        let result = reviewer.review(&contribution).await.unwrap();
        assert!(result.consensus.consensus_reached);
        assert!(matches!(result.action, ReviewAction::AutoApprove));
    }

    #[tokio::test]
    async fn test_contribution_reviewer_rejection() {
        let config = ReviewConfig {
            min_reviews: 2,
            consensus_threshold: 0.7,
            auto_approve: AutoApprovePolicy::Always,
            ..Default::default()
        };

        let reviewer = ContributionReviewer::new(config)
            .with_reviewer(Arc::new(MockModelReviewer::rejecting("model-1")))
            .with_reviewer(Arc::new(MockModelReviewer::rejecting("model-2")));

        let contribution =
            Contribution::Bug(BugReport::builder().title("Test bug").build().unwrap());

        let result = reviewer.review(&contribution).await.unwrap();
        assert!(!result.consensus.consensus_reached);
        assert!(matches!(result.action, ReviewAction::Reject { .. }));
    }

    #[tokio::test]
    async fn test_contribution_reviewer_mixed_verdicts() {
        let config = ReviewConfig {
            min_reviews: 2,
            consensus_threshold: 0.7,
            auto_approve: AutoApprovePolicy::Never,
            ..Default::default()
        };

        let reviewer = ContributionReviewer::new(config)
            .with_reviewer(Arc::new(MockModelReviewer::approving("model-1")))
            .with_reviewer(Arc::new(MockModelReviewer::rejecting("model-2")));

        let contribution =
            Contribution::Bug(BugReport::builder().title("Test bug").build().unwrap());

        let result = reviewer.review(&contribution).await.unwrap();
        assert_eq!(result.consensus.positive_count, 1);
        assert_eq!(result.consensus.negative_count, 1);
        assert!(!result.consensus.disagreements.is_empty());
    }

    #[test]
    fn test_contributor_ai() {
        let contributor = Contributor::ai("TestBot", Uuid::new_v4(), "claude-3-opus");
        assert!(contributor.is_ai);
        assert!(contributor.app_id.is_some());
        assert_eq!(contributor.model, Some("claude-3-opus".to_string()));
    }

    #[test]
    fn test_contributor_human() {
        let contributor = Contributor::human("John Doe");
        assert!(!contributor.is_ai);
        assert!(contributor.app_id.is_none());
    }

    #[test]
    fn test_auto_approve_policy_default() {
        let policy = AutoApprovePolicy::default();
        match policy {
            AutoApprovePolicy::LowRisk {
                max_lines_changed,
                allowed_types,
            } => {
                assert_eq!(max_lines_changed, 100);
                assert!(allowed_types.contains(&ContributionType::Bug));
            }
            _ => panic!("Expected LowRisk policy"),
        }
    }
}
