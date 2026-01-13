// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for contribution system
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! # Package Contribution System
//!
//! AI agents can contribute back to the package ecosystem through bug reports,
//! improvement suggestions, and package requests. This module implements the
//! contribution types and submission client.
//!
//! ## Contribution Types
//!
//! - **Bug Reports**: Report issues discovered in packages
//! - **Improvements**: Suggest performance, feature, or quality improvements
//! - **Package Requests**: Request new packages for missing functionality
//! - **Fixes**: Submit patches for existing packages
//!
//! ## Example - Bug Report
//!
//! ```rust,ignore
//! use dashflow::packages::contributions::*;
//!
//! // Create a bug report from introspection
//! let report = PackageBugReport::new(
//!     PackageRef::new("dashflow/sentiment-analysis", "1.2.0"),
//!     ReporterIdentity::ai("MyApp", None),
//! )
//! .with_description("Incorrect sentiment score for neutral text")
//! .with_discovery(DiscoveryMethod::RuntimeError {
//!     error_type: "IncorrectOutput".to_string(),
//!     trace: "sentiment_score returned -0.5 for neutral input".to_string(),
//! })
//! .with_evidence(Evidence::Citation {
//!     source: "execution_log".to_string(),
//!     content: "Input: 'The sky is blue.' Output: -0.5".to_string(),
//! });
//!
//! // Submit to registry
//! let client = ContributionClient::new(signer);
//! let id = client.submit_bug_report(report).await?;
//! ```
//!
//! ## Example - Improvement Suggestion
//!
//! ```rust,ignore
//! use dashflow::packages::contributions::*;
//!
//! let improvement = PackageImprovement::new(
//!     PackageRef::new("dashflow/sentiment-analysis", "1.2.0"),
//!     ReporterIdentity::ai("MyApp", None),
//! )
//! .with_improvement_type(ImprovementType::Performance {
//!     metric: "latency_ms".to_string(),
//!     current: 150.0,
//!     target: 50.0,
//! })
//! .with_description("Use batch processing to reduce per-item latency")
//! .with_implementation(Implementation::new("Enable batch mode for multiple inputs"));
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use super::dashswarm::DASHSWARM_DEFAULT_URL;
use super::{PackageId, PackageType, Signature, Version};

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during contribution operations
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum ContributionError {
    /// Invalid contribution data
    #[error("Invalid contribution data: {0}")]
    InvalidData(String),
    /// Signing failed
    #[error("Signing failed: {0}")]
    SigningFailed(String),
    /// Network error during submission
    #[error("Network error: {0}")]
    NetworkError(String),
    /// Registry rejected the contribution
    #[error("Contribution rejected: {0}")]
    Rejected(String),
    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    /// Rate limited.
    #[error("Rate limited, retry after {retry_after_secs} seconds")]
    RateLimited {
        /// Seconds to wait before retrying.
        retry_after_secs: u64,
    },
    /// Contribution not found
    #[error("Contribution not found: {0}")]
    NotFound(Uuid),
}

/// Result type for contribution operations
pub type ContributionResult<T> = Result<T, ContributionError>;

// ============================================================================
// Reporter Identity
// ============================================================================

/// Identity of the contribution reporter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReporterIdentity {
    /// Unique app identifier
    pub app_id: Uuid,
    /// App name
    pub app_name: String,
    /// Organization (if any)
    pub organization: Option<String>,
    /// Public key for verification
    pub public_key: Option<String>,
    /// Is this an AI agent?
    pub is_ai: bool,
    /// Human owner (for AI agents)
    pub human_owner: Option<String>,
    /// Contact email (optional)
    pub email: Option<String>,
}

impl ReporterIdentity {
    /// Create a new AI agent identity
    #[must_use]
    pub fn ai(app_name: impl Into<String>, human_owner: Option<String>) -> Self {
        Self {
            app_id: Uuid::new_v4(),
            app_name: app_name.into(),
            organization: None,
            public_key: None,
            is_ai: true,
            human_owner,
            email: None,
        }
    }

    /// Create a new human contributor identity
    #[must_use]
    pub fn human(name: impl Into<String>, email: Option<String>) -> Self {
        Self {
            app_id: Uuid::new_v4(),
            app_name: name.into(),
            organization: None,
            public_key: None,
            is_ai: false,
            human_owner: None,
            email,
        }
    }

    /// Set the organization
    #[must_use]
    pub fn with_organization(mut self, org: impl Into<String>) -> Self {
        self.organization = Some(org.into());
        self
    }

    /// Set the public key
    #[must_use]
    pub fn with_public_key(mut self, key: impl Into<String>) -> Self {
        self.public_key = Some(key.into());
        self
    }

    /// Set the app ID
    #[must_use]
    pub fn with_app_id(mut self, id: Uuid) -> Self {
        self.app_id = id;
        self
    }

    /// Set the contact email
    #[must_use]
    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }
}

impl Default for ReporterIdentity {
    fn default() -> Self {
        Self::ai("Unknown", None)
    }
}

// ============================================================================
// Package Reference
// ============================================================================

/// Reference to a specific package version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionPackageRef {
    /// Package identifier
    pub id: PackageId,
    /// Package version
    pub version: Version,
    /// Package hash (for verification)
    pub hash: Option<String>,
}

impl ContributionPackageRef {
    /// Create a new package reference
    #[must_use]
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
        let id_str = id.into();
        let version_str = version.into();
        Self {
            id: PackageId::parse(&id_str).unwrap_or_else(|| PackageId::new("unknown", &id_str)),
            version: Version::parse(&version_str).unwrap_or_else(|| Version::new(0, 0, 0)),
            hash: None,
        }
    }

    /// Create from existing types
    #[must_use]
    pub fn from_parts(id: PackageId, version: Version) -> Self {
        Self {
            id,
            version,
            hash: None,
        }
    }

    /// Set the package hash
    #[must_use]
    pub fn with_hash(mut self, hash: impl Into<String>) -> Self {
        self.hash = Some(hash.into());
        self
    }
}

// ============================================================================
// Evidence
// ============================================================================

/// Evidence supporting a contribution (bug report, improvement, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Evidence {
    /// Citation from logs, documentation, or code
    Citation {
        /// Source of the citation (e.g., "execution_log", "documentation")
        source: String,
        /// The cited content
        content: String,
    },
    /// Metric measurement
    Metric {
        /// Metric name (e.g., "latency_ms", "error_rate")
        name: String,
        /// Measured value
        value: f64,
        /// Expected value (if applicable)
        expected: Option<f64>,
        /// Unit of measurement
        unit: Option<String>,
    },
    /// Screenshot or image evidence (base64 encoded)
    Screenshot {
        /// Base64-encoded image data
        data: String,
        /// Image format (e.g., "png", "jpeg")
        format: String,
        /// Description of what the screenshot shows
        description: String,
    },
    /// Stack trace or error output
    StackTrace {
        /// The stack trace
        trace: String,
        /// Error message
        error_message: Option<String>,
    },
    /// Reproduction code or script
    ReproductionCode {
        /// The code that reproduces the issue
        code: String,
        /// Language (e.g., "rust", "python")
        language: String,
    },
    /// Reference to an introspection report
    IntrospectionReport {
        /// Report ID
        report_id: Uuid,
        /// Relevant section of the report
        section: Option<String>,
    },
}

impl Evidence {
    /// Create a citation evidence
    #[must_use]
    pub fn citation(source: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Citation {
            source: source.into(),
            content: content.into(),
        }
    }

    /// Create a metric evidence
    #[must_use]
    pub fn metric(name: impl Into<String>, value: f64) -> Self {
        Self::Metric {
            name: name.into(),
            value,
            expected: None,
            unit: None,
        }
    }

    /// Create a stack trace evidence
    #[must_use]
    pub fn stack_trace(trace: impl Into<String>) -> Self {
        Self::StackTrace {
            trace: trace.into(),
            error_message: None,
        }
    }

    /// Create an introspection report reference
    #[must_use]
    pub fn introspection_report(report_id: Uuid) -> Self {
        Self::IntrospectionReport {
            report_id,
            section: None,
        }
    }
}

// ============================================================================
// Bug Report Types
// ============================================================================

/// How a bug was discovered
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum DiscoveryMethod {
    /// Introspection system detected it
    Introspection {
        /// Introspection report ID
        report_id: Uuid,
        /// Gap or issue description
        gap_description: Option<String>,
    },
    /// Runtime error during execution
    RuntimeError {
        /// Error type
        error_type: String,
        /// Error trace or message
        trace: String,
    },
    /// Test failure
    TestFailure {
        /// Test name
        test_name: String,
        /// Expected vs actual result
        expected: Option<String>,
        /// Actual result
        actual: Option<String>,
    },
    /// Manual discovery by user or developer
    Manual {
        /// How it was discovered
        description: Option<String>,
    },
    /// Fuzzing or automated testing
    AutomatedTesting {
        /// Testing framework used
        framework: String,
        /// Test case that triggered the bug
        test_case: Option<String>,
    },
    /// Security scan or audit
    SecurityScan {
        /// Scanner name
        scanner: String,
        /// Vulnerability ID (e.g., CVE)
        vulnerability_id: Option<String>,
    },
}

impl DiscoveryMethod {
    /// Create introspection discovery method
    #[must_use]
    pub fn introspection(report_id: Uuid) -> Self {
        Self::Introspection {
            report_id,
            gap_description: None,
        }
    }

    /// Create runtime error discovery method
    #[must_use]
    pub fn runtime_error(error_type: impl Into<String>, trace: impl Into<String>) -> Self {
        Self::RuntimeError {
            error_type: error_type.into(),
            trace: trace.into(),
        }
    }

    /// Create test failure discovery method
    #[must_use]
    pub fn test_failure(test_name: impl Into<String>) -> Self {
        Self::TestFailure {
            test_name: test_name.into(),
            expected: None,
            actual: None,
        }
    }

    /// Create manual discovery method
    #[must_use]
    pub fn manual() -> Self {
        Self::Manual { description: None }
    }
}

/// Reproduction steps for a bug
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReproductionSteps {
    /// Environment requirements (DashFlow version, OS, etc.)
    pub environment: Vec<String>,
    /// Ordered steps to reproduce
    pub steps: Vec<String>,
    /// Input data that triggers the bug
    pub input_data: Option<String>,
    /// Expected behavior
    pub expected_behavior: String,
    /// Actual behavior
    pub actual_behavior: String,
    /// Is the bug reproducible consistently?
    pub reproducible: bool,
    /// Reproduction rate (e.g., "always", "50%", "intermittent")
    pub reproduction_rate: Option<String>,
}

impl ReproductionSteps {
    /// Create new reproduction steps
    #[must_use]
    pub fn new(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self {
            environment: Vec::new(),
            steps: Vec::new(),
            input_data: None,
            expected_behavior: expected.into(),
            actual_behavior: actual.into(),
            reproducible: true,
            reproduction_rate: None,
        }
    }

    /// Add an environment requirement
    #[must_use]
    pub fn with_environment(mut self, env: impl Into<String>) -> Self {
        self.environment.push(env.into());
        self
    }

    /// Add a reproduction step
    #[must_use]
    pub fn with_step(mut self, step: impl Into<String>) -> Self {
        self.steps.push(step.into());
        self
    }

    /// Set input data
    #[must_use]
    pub fn with_input(mut self, input: impl Into<String>) -> Self {
        self.input_data = Some(input.into());
        self
    }

    /// Set reproducibility
    #[must_use]
    pub fn reproducible(mut self, reproducible: bool) -> Self {
        self.reproducible = reproducible;
        self
    }
}

/// Suggested fix for a bug
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedFix {
    /// Description of the fix
    pub description: String,
    /// Diff/patch (in unified diff format)
    pub patch: Option<String>,
    /// Confidence in the fix (0.0 to 1.0)
    pub confidence: f64,
    /// Has the fix been tested?
    pub tested: bool,
    /// Test results (if tested)
    pub test_results: Option<String>,
}

impl SuggestedFix {
    /// Create a new suggested fix
    #[must_use]
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            patch: None,
            confidence: 0.5,
            tested: false,
            test_results: None,
        }
    }

    /// Set the patch
    #[must_use]
    pub fn with_patch(mut self, patch: impl Into<String>) -> Self {
        self.patch = Some(patch.into());
        self
    }

    /// Set confidence level
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Mark as tested
    #[must_use]
    pub fn tested(mut self, test_results: Option<String>) -> Self {
        self.tested = true;
        self.test_results = test_results;
        self
    }
}

/// Bug severity levels
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BugSeverity {
    /// Cosmetic or minor issue
    Low,
    /// Annoying but workaroundable
    #[default]
    Medium,
    /// Significant impact on functionality
    High,
    /// Critical - data loss, security, or crash
    Critical,
}

impl BugSeverity {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// AI-generated bug report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageBugReport {
    /// Unique report ID
    pub id: Uuid,
    /// Package being reported
    pub package: ContributionPackageRef,
    /// Reporter identity
    pub reporter: ReporterIdentity,
    /// Bug title (short summary)
    pub title: String,
    /// Bug description (detailed)
    pub description: String,
    /// How was it discovered?
    pub discovery_method: DiscoveryMethod,
    /// Reproduction steps
    pub reproduction: Option<ReproductionSteps>,
    /// Evidence supporting the report
    pub evidence: Vec<Evidence>,
    /// Bug severity
    pub severity: BugSeverity,
    /// Suggested fix (if any)
    pub suggested_fix: Option<SuggestedFix>,
    /// Labels/tags
    pub labels: Vec<String>,
    /// When the report was created
    pub created_at: DateTime<Utc>,
    /// Signature (filled when submitted)
    pub signature: Option<Signature>,
}

impl PackageBugReport {
    /// Create a new bug report
    #[must_use]
    pub fn new(package: ContributionPackageRef, reporter: ReporterIdentity) -> Self {
        Self {
            id: Uuid::new_v4(),
            package,
            reporter,
            title: String::new(),
            description: String::new(),
            discovery_method: DiscoveryMethod::manual(),
            reproduction: None,
            evidence: Vec::new(),
            severity: BugSeverity::default(),
            suggested_fix: None,
            labels: Vec::new(),
            created_at: Utc::now(),
            signature: None,
        }
    }

    /// Set the bug title
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the bug description
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set the discovery method
    #[must_use]
    pub fn with_discovery(mut self, method: DiscoveryMethod) -> Self {
        self.discovery_method = method;
        self
    }

    /// Set reproduction steps
    #[must_use]
    pub fn with_reproduction(mut self, steps: ReproductionSteps) -> Self {
        self.reproduction = Some(steps);
        self
    }

    /// Add evidence
    #[must_use]
    pub fn with_evidence(mut self, evidence: Evidence) -> Self {
        self.evidence.push(evidence);
        self
    }

    /// Set severity
    #[must_use]
    pub fn with_severity(mut self, severity: BugSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Set suggested fix
    #[must_use]
    pub fn with_suggested_fix(mut self, fix: SuggestedFix) -> Self {
        self.suggested_fix = Some(fix);
        self
    }

    /// Add a label
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Validate the bug report
    pub fn validate(&self) -> ContributionResult<()> {
        if self.title.is_empty() {
            return Err(ContributionError::InvalidData("Title is required".into()));
        }
        if self.description.is_empty() {
            return Err(ContributionError::InvalidData(
                "Description is required".into(),
            ));
        }
        Ok(())
    }
}

// ============================================================================
// Improvement Suggestion Types
// ============================================================================

/// Type of improvement being suggested
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImprovementType {
    /// Performance optimization
    Performance {
        /// Metric being improved (e.g., "latency_ms", "memory_mb")
        metric: String,
        /// Current value
        current: f64,
        /// Target value
        target: f64,
        /// Unit of measurement
        unit: Option<String>,
    },
    /// New feature addition
    Feature {
        /// Feature description
        description: String,
        /// Use cases for the feature
        use_cases: Vec<String>,
    },
    /// Better error handling
    ErrorHandling {
        /// Error type being improved
        error_type: String,
        /// Current behavior
        current_behavior: String,
        /// Proposed behavior
        proposed_behavior: String,
    },
    /// Documentation improvement
    Documentation {
        /// What documentation needs improvement
        section: String,
        /// What's missing or unclear
        issue: String,
    },
    /// Test coverage improvement
    Testing {
        /// Current coverage (if known)
        current_coverage: Option<f64>,
        /// Target coverage
        target_coverage: Option<f64>,
        /// Missing test scenarios
        missing_scenarios: Vec<String>,
    },
    /// API improvement
    Api {
        /// API element being improved
        element: String,
        /// Current issues
        issues: Vec<String>,
        /// Proposed changes
        proposed: String,
    },
    /// Security improvement
    Security {
        /// Security concern being addressed
        concern: String,
        /// Current vulnerability level
        vulnerability_level: Option<String>,
        /// Proposed mitigation
        mitigation: String,
    },
}

impl ImprovementType {
    /// Create a performance improvement
    #[must_use]
    pub fn performance(metric: impl Into<String>, current: f64, target: f64) -> Self {
        Self::Performance {
            metric: metric.into(),
            current,
            target,
            unit: None,
        }
    }

    /// Create a feature improvement
    #[must_use]
    pub fn feature(description: impl Into<String>) -> Self {
        Self::Feature {
            description: description.into(),
            use_cases: Vec::new(),
        }
    }

    /// Create an error handling improvement
    #[must_use]
    pub fn error_handling(
        error_type: impl Into<String>,
        current: impl Into<String>,
        proposed: impl Into<String>,
    ) -> Self {
        Self::ErrorHandling {
            error_type: error_type.into(),
            current_behavior: current.into(),
            proposed_behavior: proposed.into(),
        }
    }

    /// Create a documentation improvement
    #[must_use]
    pub fn documentation(section: impl Into<String>, issue: impl Into<String>) -> Self {
        Self::Documentation {
            section: section.into(),
            issue: issue.into(),
        }
    }
}

/// Expected impact of an improvement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedImpact {
    /// Overall impact level
    pub level: ImpactLevel,
    /// Specific metrics affected
    pub metrics: Vec<ImpactMetric>,
    /// Description of expected impact
    pub description: String,
}

impl ExpectedImpact {
    /// Create a new expected impact
    #[must_use]
    pub fn new(level: ImpactLevel, description: impl Into<String>) -> Self {
        Self {
            level,
            metrics: Vec::new(),
            description: description.into(),
        }
    }

    /// Add an impact metric
    #[must_use]
    pub fn with_metric(mut self, metric: ImpactMetric) -> Self {
        self.metrics.push(metric);
        self
    }
}

/// Impact level
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImpactLevel {
    /// Minimal impact
    Low,
    /// Moderate impact
    #[default]
    Medium,
    /// Significant impact
    High,
    /// Transformative impact
    Critical,
}

/// Specific metric being impacted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactMetric {
    /// Metric name
    pub name: String,
    /// Current value
    pub current: f64,
    /// Expected value after improvement
    pub expected: f64,
    /// Unit of measurement
    pub unit: Option<String>,
}

impl ImpactMetric {
    /// Create a new impact metric
    #[must_use]
    pub fn new(name: impl Into<String>, current: f64, expected: f64) -> Self {
        Self {
            name: name.into(),
            current,
            expected,
            unit: None,
        }
    }

    /// Set the unit
    #[must_use]
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Calculate improvement percentage
    #[must_use]
    pub fn improvement_percent(&self) -> f64 {
        if self.current == 0.0 {
            return 0.0;
        }
        ((self.expected - self.current) / self.current.abs()) * 100.0
    }
}

/// Implementation details for an improvement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Implementation {
    /// Description of the implementation approach
    pub description: String,
    /// Code changes (diff format)
    pub changes: Option<String>,
    /// New files to create
    pub new_files: Vec<NewFile>,
    /// Test cases for the implementation
    pub tests: Vec<TestCase>,
    /// Has the implementation been validated?
    pub validated: bool,
    /// Validation results
    pub validation_results: Option<String>,
}

impl Implementation {
    /// Create a new implementation
    #[must_use]
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            changes: None,
            new_files: Vec::new(),
            tests: Vec::new(),
            validated: false,
            validation_results: None,
        }
    }

    /// Set the code changes
    #[must_use]
    pub fn with_changes(mut self, changes: impl Into<String>) -> Self {
        self.changes = Some(changes.into());
        self
    }

    /// Add a new file
    #[must_use]
    pub fn with_file(mut self, path: impl Into<String>, content: impl Into<String>) -> Self {
        self.new_files.push(NewFile {
            path: path.into(),
            content: content.into(),
        });
        self
    }

    /// Add a test case
    #[must_use]
    pub fn with_test(mut self, test: TestCase) -> Self {
        self.tests.push(test);
        self
    }

    /// Mark as validated
    #[must_use]
    pub fn validated(mut self, results: Option<String>) -> Self {
        self.validated = true;
        self.validation_results = results;
        self
    }
}

/// A new file to be created
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewFile {
    /// File path (relative to package root)
    pub path: String,
    /// File content
    pub content: String,
}

/// A test case for an implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    /// Test name
    pub name: String,
    /// Test description
    pub description: String,
    /// Test code
    pub code: Option<String>,
    /// Expected outcome
    pub expected_outcome: String,
}

impl TestCase {
    /// Create a new test case
    #[must_use]
    pub fn new(name: impl Into<String>, expected: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            code: None,
            expected_outcome: expected.into(),
        }
    }

    /// Set the description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the test code
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

/// AI-generated improvement suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageImprovement {
    /// Unique suggestion ID
    pub id: Uuid,
    /// Package to improve
    pub package: ContributionPackageRef,
    /// Suggester identity
    pub suggester: ReporterIdentity,
    /// Improvement title
    pub title: String,
    /// Type of improvement
    pub improvement_type: ImprovementType,
    /// Detailed description
    pub description: String,
    /// Evidence supporting the suggestion
    pub evidence: Vec<Evidence>,
    /// Implementation details (if available)
    pub implementation: Option<Implementation>,
    /// Expected impact
    pub expected_impact: ExpectedImpact,
    /// Priority
    pub priority: ImprovementPriority,
    /// Labels/tags
    pub labels: Vec<String>,
    /// When the suggestion was created
    pub created_at: DateTime<Utc>,
    /// Signature (filled when submitted)
    pub signature: Option<Signature>,
}

impl PackageImprovement {
    /// Create a new improvement suggestion
    #[must_use]
    pub fn new(package: ContributionPackageRef, suggester: ReporterIdentity) -> Self {
        Self {
            id: Uuid::new_v4(),
            package,
            suggester,
            title: String::new(),
            improvement_type: ImprovementType::feature(""),
            description: String::new(),
            evidence: Vec::new(),
            implementation: None,
            expected_impact: ExpectedImpact::new(ImpactLevel::Medium, ""),
            priority: ImprovementPriority::default(),
            labels: Vec::new(),
            created_at: Utc::now(),
            signature: None,
        }
    }

    /// Set the title
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the improvement type
    #[must_use]
    pub fn with_improvement_type(mut self, improvement_type: ImprovementType) -> Self {
        self.improvement_type = improvement_type;
        self
    }

    /// Set the description
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Add evidence
    #[must_use]
    pub fn with_evidence(mut self, evidence: Evidence) -> Self {
        self.evidence.push(evidence);
        self
    }

    /// Set implementation details
    #[must_use]
    pub fn with_implementation(mut self, impl_: Implementation) -> Self {
        self.implementation = Some(impl_);
        self
    }

    /// Set expected impact
    #[must_use]
    pub fn with_expected_impact(mut self, impact: ExpectedImpact) -> Self {
        self.expected_impact = impact;
        self
    }

    /// Set priority
    #[must_use]
    pub fn with_priority(mut self, priority: ImprovementPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Add a label
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Validate the improvement suggestion
    pub fn validate(&self) -> ContributionResult<()> {
        if self.title.is_empty() {
            return Err(ContributionError::InvalidData("Title is required".into()));
        }
        if self.description.is_empty() {
            return Err(ContributionError::InvalidData(
                "Description is required".into(),
            ));
        }
        Ok(())
    }
}

/// Improvement priority levels
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImprovementPriority {
    /// Nice to have
    Low,
    /// Should be addressed eventually
    #[default]
    Medium,
    /// Should be addressed soon
    High,
    /// Critical, needs immediate attention
    Critical,
}

impl ImprovementPriority {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

// ============================================================================
// Package Request Types
// ============================================================================

/// Request for a new package (different from sharing::PackageRequest)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPackageRequest {
    /// Unique request ID
    pub id: Uuid,
    /// Requester identity
    pub requester: ReporterIdentity,
    /// Request title
    pub title: String,
    /// What functionality is needed
    pub needed_functionality: String,
    /// Why it's needed (use case)
    pub use_case: String,
    /// Evidence of need
    pub evidence: Vec<Evidence>,
    /// Suggested implementation approach
    pub suggested_approach: Option<String>,
    /// Suggested package type
    pub package_type: Option<PackageType>,
    /// Similar existing packages (that don't meet the need)
    pub similar_packages: Vec<SimilarPackage>,
    /// Priority
    pub priority: RequestPriority,
    /// Labels/tags
    pub labels: Vec<String>,
    /// When the request was created
    pub created_at: DateTime<Utc>,
    /// Signature (filled when submitted)
    pub signature: Option<Signature>,
}

impl NewPackageRequest {
    /// Create a new package request
    #[must_use]
    pub fn new(requester: ReporterIdentity, needed_functionality: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            requester,
            title: String::new(),
            needed_functionality: needed_functionality.into(),
            use_case: String::new(),
            evidence: Vec::new(),
            suggested_approach: None,
            package_type: None,
            similar_packages: Vec::new(),
            priority: RequestPriority::default(),
            labels: Vec::new(),
            created_at: Utc::now(),
            signature: None,
        }
    }

    /// Set the title
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the use case
    #[must_use]
    pub fn with_use_case(mut self, use_case: impl Into<String>) -> Self {
        self.use_case = use_case.into();
        self
    }

    /// Add evidence
    #[must_use]
    pub fn with_evidence(mut self, evidence: Evidence) -> Self {
        self.evidence.push(evidence);
        self
    }

    /// Set suggested approach
    #[must_use]
    pub fn with_suggested_approach(mut self, approach: impl Into<String>) -> Self {
        self.suggested_approach = Some(approach.into());
        self
    }

    /// Set package type
    #[must_use]
    pub fn with_package_type(mut self, package_type: PackageType) -> Self {
        self.package_type = Some(package_type);
        self
    }

    /// Add a similar package
    #[must_use]
    pub fn with_similar_package(mut self, pkg: SimilarPackage) -> Self {
        self.similar_packages.push(pkg);
        self
    }

    /// Set priority
    #[must_use]
    pub fn with_priority(mut self, priority: RequestPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Add a label
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Validate the request
    pub fn validate(&self) -> ContributionResult<()> {
        if self.title.is_empty() {
            return Err(ContributionError::InvalidData("Title is required".into()));
        }
        if self.needed_functionality.is_empty() {
            return Err(ContributionError::InvalidData(
                "Needed functionality is required".into(),
            ));
        }
        if self.use_case.is_empty() {
            return Err(ContributionError::InvalidData(
                "Use case is required".into(),
            ));
        }
        Ok(())
    }
}

/// Reference to a similar package that doesn't meet the need
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarPackage {
    /// Package ID
    pub id: PackageId,
    /// Why it doesn't meet the need
    pub limitation: String,
}

impl SimilarPackage {
    /// Create a new similar package reference
    #[must_use]
    pub fn new(id: PackageId, limitation: impl Into<String>) -> Self {
        Self {
            id,
            limitation: limitation.into(),
        }
    }
}

/// Request priority levels
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestPriority {
    /// Nice to have
    Low,
    /// Would improve workflow
    #[default]
    Medium,
    /// Significantly blocking work
    High,
    /// Critical, no workaround
    Critical,
}

// ============================================================================
// Package Fix Types
// ============================================================================

/// A fix submitted for an existing package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageFix {
    /// Unique fix ID
    pub id: Uuid,
    /// Package being fixed
    pub package: ContributionPackageRef,
    /// Fixer identity
    pub fixer: ReporterIdentity,
    /// Fix title
    pub title: String,
    /// Related bug report ID (if any)
    pub bug_report_id: Option<Uuid>,
    /// Fix description
    pub description: String,
    /// The fix implementation
    pub implementation: Implementation,
    /// Breaking change?
    pub breaking: bool,
    /// Labels/tags
    pub labels: Vec<String>,
    /// When the fix was created
    pub created_at: DateTime<Utc>,
    /// Signature (filled when submitted)
    pub signature: Option<Signature>,
}

impl PackageFix {
    /// Create a new package fix
    #[must_use]
    pub fn new(
        package: ContributionPackageRef,
        fixer: ReporterIdentity,
        implementation: Implementation,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            package,
            fixer,
            title: String::new(),
            bug_report_id: None,
            description: String::new(),
            implementation,
            breaking: false,
            labels: Vec::new(),
            created_at: Utc::now(),
            signature: None,
        }
    }

    /// Set the title
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Link to a bug report
    #[must_use]
    pub fn for_bug_report(mut self, bug_report_id: Uuid) -> Self {
        self.bug_report_id = Some(bug_report_id);
        self
    }

    /// Set the description
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Mark as breaking change
    #[must_use]
    pub fn breaking(mut self) -> Self {
        self.breaking = true;
        self
    }

    /// Add a label
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Validate the fix
    pub fn validate(&self) -> ContributionResult<()> {
        if self.title.is_empty() {
            return Err(ContributionError::InvalidData("Title is required".into()));
        }
        if self.description.is_empty() {
            return Err(ContributionError::InvalidData(
                "Description is required".into(),
            ));
        }
        if self.implementation.description.is_empty() {
            return Err(ContributionError::InvalidData(
                "Implementation description is required".into(),
            ));
        }
        Ok(())
    }
}

// ============================================================================
// Contribution Wrapper
// ============================================================================

/// Any type of contribution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Contribution {
    /// Bug report
    BugReport(PackageBugReport),
    /// Improvement suggestion
    Improvement(PackageImprovement),
    /// New package request
    Request(NewPackageRequest),
    /// Package fix
    Fix(PackageFix),
}

impl Contribution {
    /// Get the contribution ID
    #[must_use]
    pub fn id(&self) -> Uuid {
        match self {
            Self::BugReport(r) => r.id,
            Self::Improvement(i) => i.id,
            Self::Request(r) => r.id,
            Self::Fix(f) => f.id,
        }
    }

    /// Get the contribution type as a string
    #[must_use]
    pub fn type_str(&self) -> &'static str {
        match self {
            Self::BugReport(_) => "bug_report",
            Self::Improvement(_) => "improvement",
            Self::Request(_) => "request",
            Self::Fix(_) => "fix",
        }
    }

    /// Get the reporter identity
    #[must_use]
    pub fn reporter(&self) -> &ReporterIdentity {
        match self {
            Self::BugReport(r) => &r.reporter,
            Self::Improvement(i) => &i.suggester,
            Self::Request(r) => &r.requester,
            Self::Fix(f) => &f.fixer,
        }
    }

    /// Get the creation timestamp
    #[must_use]
    pub fn created_at(&self) -> DateTime<Utc> {
        match self {
            Self::BugReport(r) => r.created_at,
            Self::Improvement(i) => i.created_at,
            Self::Request(r) => r.created_at,
            Self::Fix(f) => f.created_at,
        }
    }

    /// Validate the contribution
    pub fn validate(&self) -> ContributionResult<()> {
        match self {
            Self::BugReport(r) => r.validate(),
            Self::Improvement(i) => i.validate(),
            Self::Request(r) => r.validate(),
            Self::Fix(f) => f.validate(),
        }
    }
}

// ============================================================================
// Contribution Status
// ============================================================================

/// Status of a submitted contribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionStatus {
    /// Contribution ID
    pub id: Uuid,
    /// Contribution type
    pub contribution_type: String,
    /// Current status
    pub status: ContributionState,
    /// Status message
    pub message: Option<String>,
    /// Reviewer comments
    pub reviewer_comments: Vec<ReviewerComment>,
    /// When last updated
    pub updated_at: DateTime<Utc>,
}

/// Contribution state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributionState {
    /// Submitted, awaiting review
    Pending,
    /// Under review
    InReview,
    /// Needs more information
    NeedsInfo,
    /// Approved
    Approved,
    /// Rejected
    Rejected,
    /// Implemented/Fixed
    Resolved,
    /// Closed without action
    Closed,
}

impl ContributionState {
    /// Is this a terminal state?
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Approved | Self::Rejected | Self::Resolved | Self::Closed
        )
    }

    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InReview => "in_review",
            Self::NeedsInfo => "needs_info",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Resolved => "resolved",
            Self::Closed => "closed",
        }
    }
}

/// Reviewer comment on a contribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewerComment {
    /// Reviewer name
    pub reviewer: String,
    /// Comment content
    pub content: String,
    /// When commented
    pub timestamp: DateTime<Utc>,
}

// ============================================================================
// Contribution Client
// ============================================================================

/// Configuration for the contribution client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionClientConfig {
    /// Registry URL for submitting contributions
    pub registry_url: String,
    /// Timeout for submissions (seconds)
    pub timeout_secs: u64,
    /// Retry count on failure
    pub retry_count: u32,
    /// Reporter identity
    pub reporter: ReporterIdentity,
}

impl ContributionClientConfig {
    /// Create a new configuration
    #[must_use]
    pub fn new(registry_url: impl Into<String>, reporter: ReporterIdentity) -> Self {
        Self {
            registry_url: registry_url.into(),
            timeout_secs: 30,
            retry_count: 3,
            reporter,
        }
    }

    /// Create configuration for the official registry
    #[must_use]
    pub fn official(reporter: ReporterIdentity) -> Self {
        Self::new(DASHSWARM_DEFAULT_URL, reporter)
    }

    /// Set timeout
    #[must_use]
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set retry count
    #[must_use]
    pub fn with_retry_count(mut self, count: u32) -> Self {
        self.retry_count = count;
        self
    }
}

impl Default for ContributionClientConfig {
    fn default() -> Self {
        Self::new(DASHSWARM_DEFAULT_URL, ReporterIdentity::default())
    }
}

/// Client for submitting contributions to the registry
#[derive(Debug, Clone)]
pub struct ContributionClient {
    /// Client configuration
    config: ContributionClientConfig,
    /// Submitted contributions (for tracking)
    submitted: std::sync::Arc<std::sync::RwLock<Vec<Uuid>>>,
}

impl ContributionClient {
    /// Create a new contribution client
    #[must_use]
    pub fn new(config: ContributionClientConfig) -> Self {
        Self {
            config,
            submitted: std::sync::Arc::new(std::sync::RwLock::new(Vec::new())),
        }
    }

    /// Create a client for the official registry
    #[must_use]
    pub fn official(reporter: ReporterIdentity) -> Self {
        Self::new(ContributionClientConfig::official(reporter))
    }

    /// Get the registry URL
    #[must_use]
    pub fn registry_url(&self) -> &str {
        &self.config.registry_url
    }

    /// Get the reporter identity
    #[must_use]
    pub fn reporter(&self) -> &ReporterIdentity {
        &self.config.reporter
    }

    /// Get submitted contribution IDs
    #[must_use]
    pub fn submitted_ids(&self) -> Vec<Uuid> {
        // SAFETY: Use poison-safe pattern - if a thread panicked while holding the lock,
        // we still want to be able to read submitted IDs rather than crash
        self.submitted
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Submit a bug report
    ///
    /// Note: This is a placeholder that would make an HTTP request in production.
    /// Currently validates and tracks the submission locally.
    pub fn submit_bug_report(&self, report: PackageBugReport) -> ContributionResult<Uuid> {
        report.validate()?;
        let id = report.id;
        // SAFETY: Use poison-safe pattern
        self.submitted
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .push(id);
        Ok(id)
    }

    /// Submit an improvement suggestion
    ///
    /// Note: This is a placeholder that would make an HTTP request in production.
    pub fn submit_improvement(&self, improvement: PackageImprovement) -> ContributionResult<Uuid> {
        improvement.validate()?;
        let id = improvement.id;
        // SAFETY: Use poison-safe pattern
        self.submitted
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .push(id);
        Ok(id)
    }

    /// Submit a package request
    ///
    /// Note: This is a placeholder that would make an HTTP request in production.
    pub fn submit_request(&self, request: NewPackageRequest) -> ContributionResult<Uuid> {
        request.validate()?;
        let id = request.id;
        // SAFETY: Use poison-safe pattern
        self.submitted
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .push(id);
        Ok(id)
    }

    /// Submit a package fix
    ///
    /// Note: This is a placeholder that would make an HTTP request in production.
    pub fn submit_fix(&self, fix: PackageFix) -> ContributionResult<Uuid> {
        fix.validate()?;
        let id = fix.id;
        // SAFETY: Use poison-safe pattern
        self.submitted
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .push(id);
        Ok(id)
    }

    /// Submit any contribution
    pub fn submit(&self, contribution: Contribution) -> ContributionResult<Uuid> {
        match contribution {
            Contribution::BugReport(r) => self.submit_bug_report(r),
            Contribution::Improvement(i) => self.submit_improvement(i),
            Contribution::Request(r) => self.submit_request(r),
            Contribution::Fix(f) => self.submit_fix(f),
        }
    }

    /// Get contribution status (placeholder)
    ///
    /// Note: This would query the registry in production.
    pub fn get_status(&self, id: Uuid) -> ContributionResult<ContributionStatus> {
        // SAFETY: Use poison-safe pattern
        if self
            .submitted
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&id)
        {
            Ok(ContributionStatus {
                id,
                contribution_type: "unknown".to_string(),
                status: ContributionState::Pending,
                message: None,
                reviewer_comments: Vec::new(),
                updated_at: Utc::now(),
            })
        } else {
            Err(ContributionError::NotFound(id))
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
    fn test_reporter_identity_ai() {
        let identity = ReporterIdentity::ai("TestApp", Some("owner@example.com".to_string()))
            .with_organization("TestOrg");

        assert!(identity.is_ai);
        assert_eq!(identity.app_name, "TestApp");
        assert_eq!(identity.human_owner, Some("owner@example.com".to_string()));
        assert_eq!(identity.organization, Some("TestOrg".to_string()));
    }

    #[test]
    fn test_reporter_identity_human() {
        let identity = ReporterIdentity::human("John Doe", Some("john@example.com".to_string()));

        assert!(!identity.is_ai);
        assert_eq!(identity.app_name, "John Doe");
        assert_eq!(identity.email, Some("john@example.com".to_string()));
    }

    #[test]
    fn test_contribution_package_ref() {
        let pkg_ref =
            ContributionPackageRef::new("dashflow/sentiment", "1.2.3").with_hash("abc123");

        assert_eq!(pkg_ref.id.namespace(), "dashflow");
        assert_eq!(pkg_ref.id.name(), "sentiment");
        assert_eq!(pkg_ref.version.major, 1);
        assert_eq!(pkg_ref.hash, Some("abc123".to_string()));
    }

    #[test]
    fn test_evidence_types() {
        let citation = Evidence::citation("logs", "Error occurred");
        if let Evidence::Citation { source, content } = citation {
            assert_eq!(source, "logs");
            assert_eq!(content, "Error occurred");
        } else {
            panic!("Expected Citation");
        }

        let metric = Evidence::metric("latency_ms", 150.0);
        if let Evidence::Metric { name, value, .. } = metric {
            assert_eq!(name, "latency_ms");
            assert!((value - 150.0).abs() < f64::EPSILON);
        } else {
            panic!("Expected Metric");
        }
    }

    #[test]
    fn test_discovery_method() {
        let runtime_err = DiscoveryMethod::runtime_error("TypeError", "null reference");
        if let DiscoveryMethod::RuntimeError { error_type, trace } = runtime_err {
            assert_eq!(error_type, "TypeError");
            assert_eq!(trace, "null reference");
        } else {
            panic!("Expected RuntimeError");
        }
    }

    #[test]
    fn test_reproduction_steps() {
        let steps = ReproductionSteps::new("Success", "Failure")
            .with_environment("DashFlow 1.11.0")
            .with_step("Call the API")
            .with_step("Pass invalid input")
            .reproducible(true);

        assert_eq!(steps.expected_behavior, "Success");
        assert_eq!(steps.actual_behavior, "Failure");
        assert_eq!(steps.environment.len(), 1);
        assert_eq!(steps.steps.len(), 2);
        assert!(steps.reproducible);
    }

    #[test]
    fn test_suggested_fix() {
        let fix = SuggestedFix::new("Add null check")
            .with_patch("+ if (x != null)")
            .with_confidence(0.85)
            .tested(Some("All tests pass".to_string()));

        assert_eq!(fix.description, "Add null check");
        assert!(fix.patch.is_some());
        assert!((fix.confidence - 0.85).abs() < f64::EPSILON);
        assert!(fix.tested);
    }

    #[test]
    fn test_bug_report_creation() {
        let pkg = ContributionPackageRef::new("dashflow/test", "1.0.0");
        let reporter = ReporterIdentity::ai("TestApp", None);

        let report = PackageBugReport::new(pkg, reporter)
            .with_title("Test bug")
            .with_description("A test bug description")
            .with_severity(BugSeverity::High)
            .with_label("test");

        assert_eq!(report.title, "Test bug");
        assert_eq!(report.description, "A test bug description");
        assert_eq!(report.severity, BugSeverity::High);
        assert_eq!(report.labels.len(), 1);
        assert!(report.validate().is_ok());
    }

    #[test]
    fn test_bug_report_validation_fails() {
        let pkg = ContributionPackageRef::new("dashflow/test", "1.0.0");
        let reporter = ReporterIdentity::ai("TestApp", None);

        let report = PackageBugReport::new(pkg, reporter);
        assert!(report.validate().is_err());
    }

    #[test]
    fn test_improvement_type() {
        let perf = ImprovementType::performance("latency_ms", 150.0, 50.0);
        if let ImprovementType::Performance {
            metric,
            current,
            target,
            ..
        } = perf
        {
            assert_eq!(metric, "latency_ms");
            assert!((current - 150.0).abs() < f64::EPSILON);
            assert!((target - 50.0).abs() < f64::EPSILON);
        } else {
            panic!("Expected Performance");
        }
    }

    #[test]
    fn test_impact_metric() {
        let metric = ImpactMetric::new("latency_ms", 100.0, 50.0);
        let improvement = metric.improvement_percent();
        assert!((improvement - (-50.0)).abs() < f64::EPSILON); // 50% reduction
    }

    #[test]
    fn test_implementation() {
        let impl_ = Implementation::new("Add caching")
            .with_changes("+ cache.insert(key, value)")
            .with_file("src/cache.rs", "pub struct Cache {}")
            .with_test(TestCase::new("test_cache", "Cache hit"))
            .validated(Some("Tests pass".to_string()));

        assert_eq!(impl_.description, "Add caching");
        assert!(impl_.changes.is_some());
        assert_eq!(impl_.new_files.len(), 1);
        assert_eq!(impl_.tests.len(), 1);
        assert!(impl_.validated);
    }

    #[test]
    fn test_package_improvement() {
        let pkg = ContributionPackageRef::new("dashflow/test", "1.0.0");
        let suggester = ReporterIdentity::ai("TestApp", None);

        let improvement = PackageImprovement::new(pkg, suggester)
            .with_title("Performance improvement")
            .with_description("Reduce latency")
            .with_improvement_type(ImprovementType::performance("latency_ms", 100.0, 50.0))
            .with_priority(ImprovementPriority::High);

        assert_eq!(improvement.title, "Performance improvement");
        assert_eq!(improvement.priority, ImprovementPriority::High);
        assert!(improvement.validate().is_ok());
    }

    #[test]
    fn test_new_package_request() {
        let requester = ReporterIdentity::ai("TestApp", None);

        let request =
            NewPackageRequest::new(requester, "Sentiment analysis for multiple languages")
                .with_title("Multi-language sentiment")
                .with_use_case("Analyze customer feedback in various languages")
                .with_package_type(PackageType::NodeLibrary)
                .with_priority(RequestPriority::High);

        assert_eq!(request.title, "Multi-language sentiment");
        assert_eq!(request.package_type, Some(PackageType::NodeLibrary));
        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_package_fix() {
        let pkg = ContributionPackageRef::new("dashflow/test", "1.0.0");
        let fixer = ReporterIdentity::ai("TestApp", None);
        let impl_ = Implementation::new("Fix null pointer");

        let fix = PackageFix::new(pkg, fixer, impl_)
            .with_title("Fix null pointer exception")
            .with_description("Add null check before dereferencing")
            .for_bug_report(Uuid::new_v4())
            .breaking();

        assert_eq!(fix.title, "Fix null pointer exception");
        assert!(fix.breaking);
        assert!(fix.bug_report_id.is_some());
        assert!(fix.validate().is_ok());
    }

    #[test]
    fn test_contribution_wrapper() {
        let pkg = ContributionPackageRef::new("dashflow/test", "1.0.0");
        let reporter = ReporterIdentity::ai("TestApp", None);

        let report = PackageBugReport::new(pkg, reporter)
            .with_title("Test")
            .with_description("Test description");

        let contribution = Contribution::BugReport(report);

        assert_eq!(contribution.type_str(), "bug_report");
        assert!(contribution.reporter().is_ai);
    }

    #[test]
    fn test_contribution_state() {
        assert!(!ContributionState::Pending.is_terminal());
        assert!(!ContributionState::InReview.is_terminal());
        assert!(ContributionState::Approved.is_terminal());
        assert!(ContributionState::Rejected.is_terminal());
        assert!(ContributionState::Resolved.is_terminal());
    }

    #[test]
    fn test_contribution_client() {
        let reporter = ReporterIdentity::ai("TestApp", None);
        let client = ContributionClient::official(reporter.clone());

        assert!(client.registry_url().contains("dashswarm.com"));
        assert!(client.submitted_ids().is_empty());
    }

    #[test]
    fn test_contribution_client_submit_bug_report() {
        let reporter = ReporterIdentity::ai("TestApp", None);
        let client = ContributionClient::official(reporter.clone());

        let pkg = ContributionPackageRef::new("dashflow/test", "1.0.0");
        let report = PackageBugReport::new(pkg, reporter)
            .with_title("Test bug")
            .with_description("Test description");

        let id = client.submit_bug_report(report).unwrap();
        assert!(client.submitted_ids().contains(&id));

        let status = client.get_status(id).unwrap();
        assert_eq!(status.status, ContributionState::Pending);
    }

    #[test]
    fn test_contribution_client_submit_improvement() {
        let reporter = ReporterIdentity::ai("TestApp", None);
        let client = ContributionClient::official(reporter.clone());

        let pkg = ContributionPackageRef::new("dashflow/test", "1.0.0");
        let improvement = PackageImprovement::new(pkg, reporter)
            .with_title("Improvement")
            .with_description("Description");

        let id = client.submit_improvement(improvement).unwrap();
        assert!(client.submitted_ids().contains(&id));
    }

    #[test]
    fn test_contribution_client_submit_request() {
        let reporter = ReporterIdentity::ai("TestApp", None);
        let client = ContributionClient::official(reporter.clone());

        let request = NewPackageRequest::new(reporter, "New functionality")
            .with_title("New package")
            .with_use_case("Use case");

        let id = client.submit_request(request).unwrap();
        assert!(client.submitted_ids().contains(&id));
    }

    #[test]
    fn test_contribution_client_submit_fix() {
        let reporter = ReporterIdentity::ai("TestApp", None);
        let client = ContributionClient::official(reporter.clone());

        let pkg = ContributionPackageRef::new("dashflow/test", "1.0.0");
        let impl_ = Implementation::new("Fix description");
        let fix = PackageFix::new(pkg, reporter, impl_)
            .with_title("Fix title")
            .with_description("Fix description");

        let id = client.submit_fix(fix).unwrap();
        assert!(client.submitted_ids().contains(&id));
    }

    #[test]
    fn test_contribution_client_not_found() {
        let reporter = ReporterIdentity::ai("TestApp", None);
        let client = ContributionClient::official(reporter);

        let result = client.get_status(Uuid::new_v4());
        assert!(matches!(result, Err(ContributionError::NotFound(_))));
    }

    #[test]
    fn test_bug_severity_ordering() {
        assert!(BugSeverity::Low < BugSeverity::Medium);
        assert!(BugSeverity::Medium < BugSeverity::High);
        assert!(BugSeverity::High < BugSeverity::Critical);
    }

    #[test]
    fn test_improvement_priority_ordering() {
        assert!(ImprovementPriority::Low < ImprovementPriority::Medium);
        assert!(ImprovementPriority::Medium < ImprovementPriority::High);
        assert!(ImprovementPriority::High < ImprovementPriority::Critical);
    }
}
