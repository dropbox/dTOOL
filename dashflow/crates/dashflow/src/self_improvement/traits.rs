// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Extensibility traits for the Self-Improvement system.
//!
//! This module provides trait abstractions for the core components of the
//! self-improvement system, enabling:
//! - Custom analyzer implementations
//! - Custom planner implementations
//! - Pluggable storage backends
//! - Generic storable items
//!
//! These traits enable the plugin architecture and allow users
//! to extend the self-improvement system with custom implementations.

use crate::introspection::ExecutionTrace;
use std::any::Any;
use std::fmt::Debug;

use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use super::error::{Result, SelfImprovementError};
use super::types::{
    CapabilityGap, DeprecationRecommendation, ExecutionPlan, Hypothesis, HypothesisStatus,
    ImprovementProposal, IntrospectionReport, PlanStatus, RetrospectiveAnalysis,
};

// =============================================================================
// Storable Trait
// =============================================================================

/// Trait for items that can be saved to and loaded from introspection storage.
///
/// This trait enables generic save/load operations in `IntrospectionStorage`,
/// reducing code duplication across different storable types (ExecutionPlan,
/// Hypothesis, etc.).
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{Storable, ExecutionPlan};
///
/// let plan = ExecutionPlan::new("My Plan", PlanCategory::Performance);
/// assert_eq!(plan.storage_dir_name(), "plans");
/// assert_eq!(plan.status_subdir(), Some("pending"));
/// ```
pub trait Storable: Serialize + DeserializeOwned + Send + Sync {
    /// The directory name under the introspection base dir (e.g., "plans", "hypotheses").
    fn storage_dir_name() -> &'static str;

    /// The unique identifier for this item.
    fn id(&self) -> Uuid;

    /// The status-based subdirectory for saving (e.g., "pending", "active").
    /// Returns `None` if this type doesn't use status-based subdirectories.
    fn status_subdir(&self) -> Option<&'static str> {
        None
    }

    /// All possible subdirectories to search when loading.
    /// Used when loading an item by ID without knowing its current status.
    fn search_subdirs() -> &'static [&'static str] {
        &[]
    }

    /// The entity name for error messages (e.g., "Plan", "Hypothesis").
    fn entity_name() -> &'static str;
}

impl Storable for ExecutionPlan {
    fn storage_dir_name() -> &'static str {
        "plans"
    }

    fn id(&self) -> Uuid {
        self.id
    }

    fn status_subdir(&self) -> Option<&'static str> {
        Some(match &self.status {
            PlanStatus::Proposed => "pending",
            PlanStatus::Validated => "pending",
            PlanStatus::InProgress { .. } => "approved",
            PlanStatus::Implemented { .. } => "implemented",
            PlanStatus::Failed { .. } => "failed",
            PlanStatus::Superseded { .. } => "failed",
        })
    }

    fn search_subdirs() -> &'static [&'static str] {
        &["pending", "approved", "implemented", "failed"]
    }

    fn entity_name() -> &'static str {
        "Plan"
    }
}

impl Storable for Hypothesis {
    fn storage_dir_name() -> &'static str {
        "hypotheses"
    }

    fn id(&self) -> Uuid {
        self.id
    }

    fn status_subdir(&self) -> Option<&'static str> {
        Some(match &self.status {
            HypothesisStatus::Active => "active",
            HypothesisStatus::Pending { .. } => "active",
            HypothesisStatus::Evaluated => "evaluated",
            HypothesisStatus::Superseded { .. } => "evaluated",
        })
    }

    fn search_subdirs() -> &'static [&'static str] {
        &["active", "evaluated"]
    }

    fn entity_name() -> &'static str {
        "Hypothesis"
    }
}

// =============================================================================
// Analyzer Trait
// =============================================================================

/// Output from an analyzer.
///
/// Analyzers can produce different output types. This enum encapsulates
/// all possible analysis results.
#[derive(Debug, Clone)]
pub enum AnalysisOutput {
    /// Capability gaps identified in execution traces
    CapabilityGaps(Vec<CapabilityGap>),
    /// Deprecation recommendations for unused/redundant components
    Deprecations(Vec<DeprecationRecommendation>),
    /// Retrospective analysis with insights and patterns
    Retrospective(RetrospectiveAnalysis),
    /// Custom output type (JSON-serializable)
    Custom {
        /// Type name for deserialization
        type_name: String,
        /// JSON-serialized data
        data: String,
    },
}

impl AnalysisOutput {
    /// Create a custom output from any serializable type.
    pub fn custom<T: serde::Serialize>(type_name: &str, value: &T) -> Result<Self> {
        let data = serde_json::to_string(value)?;
        Ok(Self::Custom {
            type_name: type_name.to_string(),
            data,
        })
    }

    /// Try to extract capability gaps from the output.
    #[must_use]
    pub fn as_capability_gaps(&self) -> Option<&Vec<CapabilityGap>> {
        match self {
            Self::CapabilityGaps(gaps) => Some(gaps),
            _ => None,
        }
    }

    /// Try to extract deprecations from the output.
    #[must_use]
    pub fn as_deprecations(&self) -> Option<&Vec<DeprecationRecommendation>> {
        match self {
            Self::Deprecations(deps) => Some(deps),
            _ => None,
        }
    }

    /// Try to extract retrospective analysis from the output.
    #[must_use]
    pub fn as_retrospective(&self) -> Option<&RetrospectiveAnalysis> {
        match self {
            Self::Retrospective(retro) => Some(retro),
            _ => None,
        }
    }
}

/// Configuration for analyzers.
///
/// Analyzers may need additional context beyond traces.
#[derive(Debug, Clone, Default)]
pub struct AnalyzerContext {
    /// Known node names in the graph (for deprecation analysis)
    pub known_nodes: Vec<String>,
    /// Minimum confidence threshold for results
    pub min_confidence: f64,
    /// Maximum number of results to return
    pub max_results: Option<usize>,
    /// Additional context as key-value pairs
    pub extra: std::collections::HashMap<String, String>,
}

impl AnalyzerContext {
    /// Create a new analyzer context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add known nodes for deprecation analysis.
    #[must_use]
    pub fn with_known_nodes(mut self, nodes: Vec<String>) -> Self {
        self.known_nodes = nodes;
        self
    }

    /// Set minimum confidence threshold.
    #[must_use]
    pub fn with_min_confidence(mut self, confidence: f64) -> Self {
        self.min_confidence = confidence;
        self
    }

    /// Set maximum number of results.
    #[must_use]
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = Some(max);
        self
    }

    /// Add extra context.
    #[must_use]
    pub fn with_extra(mut self, key: &str, value: &str) -> Self {
        self.extra.insert(key.to_string(), value.to_string());
        self
    }
}

/// Trait for execution trace analyzers.
///
/// Analyzers examine execution traces and produce insights about:
/// - Missing capabilities (gaps)
/// - Unused functionality (deprecations)
/// - Patterns and improvements (retrospective)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{Analyzer, AnalysisOutput, AnalyzerContext};
/// use dashflow::introspection::ExecutionTrace;
///
/// struct CustomAnalyzer {
///     threshold: f64,
/// }
///
/// impl Analyzer for CustomAnalyzer {
///     fn name(&self) -> &str {
///         "custom-analyzer"
///     }
///
///     fn analyze(
///         &self,
///         traces: &[ExecutionTrace],
///         context: &AnalyzerContext,
///     ) -> Result<AnalysisOutput, SelfImprovementError> {
///         // Custom analysis logic
///         let gaps = vec![]; // ... analyze traces ...
///         Ok(AnalysisOutput::CapabilityGaps(gaps))
///     }
/// }
/// ```
pub trait Analyzer: Send + Sync + Debug {
    /// Returns the unique name of this analyzer.
    fn name(&self) -> &str;

    /// Returns a description of what this analyzer does.
    fn description(&self) -> &str {
        "No description provided"
    }

    /// Analyzes execution traces and returns insights.
    ///
    /// # Arguments
    /// * `traces` - The execution traces to analyze
    /// * `context` - Additional context for the analysis
    ///
    /// # Returns
    /// Analysis output containing gaps, deprecations, or retrospective insights.
    fn analyze(
        &self,
        traces: &[ExecutionTrace],
        context: &AnalyzerContext,
    ) -> Result<AnalysisOutput>;

    /// Returns the output type this analyzer produces.
    fn output_type(&self) -> AnalyzerOutputType {
        AnalyzerOutputType::Mixed
    }

    /// Returns the minimum number of traces needed for meaningful analysis.
    fn min_traces(&self) -> usize {
        1
    }

    /// Validates that the analyzer can run with the given traces.
    fn validate(&self, traces: &[ExecutionTrace]) -> Result<()> {
        if traces.len() < self.min_traces() {
            return Err(SelfImprovementError::ValidationFailed(format!(
                "Analyzer '{}' requires at least {} traces, got {}",
                self.name(),
                self.min_traces(),
                traces.len()
            )));
        }
        Ok(())
    }

    /// Returns this analyzer as Any for downcasting.
    fn as_any(&self) -> &dyn Any;
}

/// Types of output an analyzer can produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyzerOutputType {
    /// Produces capability gaps
    CapabilityGaps,
    /// Produces deprecation recommendations
    Deprecations,
    /// Produces retrospective analysis
    Retrospective,
    /// Produces custom output
    Custom,
    /// Produces multiple output types
    Mixed,
}

// =============================================================================
// Planner Trait
// =============================================================================

/// Input sources for plan generation.
#[derive(Debug, Clone)]
pub enum PlannerInput {
    /// Generate plans from capability gaps
    Gaps(Vec<CapabilityGap>),
    /// Generate plans from deprecation recommendations
    Deprecations(Vec<DeprecationRecommendation>),
    /// Generate plans from retrospective analysis
    Retrospective(RetrospectiveAnalysis),
    /// Generate proposals from multiple sources.
    Combined {
        /// Capability gaps to address.
        gaps: Vec<CapabilityGap>,
        /// Deprecation recommendations to consider.
        deprecations: Vec<DeprecationRecommendation>,
    },
    /// Custom input (JSON-serialized).
    Custom {
        /// Name of the custom type.
        type_name: String,
        /// JSON-serialized data.
        data: String,
    },
}

impl PlannerInput {
    /// Create a custom input from any serializable type.
    pub fn custom<T: serde::Serialize>(type_name: &str, value: &T) -> Result<Self> {
        let data = serde_json::to_string(value)?;
        Ok(Self::Custom {
            type_name: type_name.to_string(),
            data,
        })
    }
}

/// Output from a planner.
#[derive(Debug, Clone)]
pub enum PlannerOutput {
    /// Execution plans
    Plans(Vec<ExecutionPlan>),
    /// Improvement proposals (higher-level than plans)
    Proposals(Vec<ImprovementProposal>),
}

impl PlannerOutput {
    /// Try to extract plans from the output.
    #[must_use]
    pub fn as_plans(&self) -> Option<&Vec<ExecutionPlan>> {
        match self {
            Self::Plans(plans) => Some(plans),
            _ => None,
        }
    }

    /// Try to extract proposals from the output.
    #[must_use]
    pub fn as_proposals(&self) -> Option<&Vec<ImprovementProposal>> {
        match self {
            Self::Proposals(proposals) => Some(proposals),
            _ => None,
        }
    }
}

/// Trait for plan generators.
///
/// Planners take analysis results and generate actionable execution plans
/// or improvement proposals.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{Planner, PlannerInput, PlannerOutput};
///
/// struct CustomPlanner {
///     priority_threshold: u8,
/// }
///
/// impl Planner for CustomPlanner {
///     fn name(&self) -> &str {
///         "custom-planner"
///     }
///
///     fn generate(&self, input: PlannerInput) -> Result<PlannerOutput, SelfImprovementError> {
///         match input {
///             PlannerInput::Gaps(gaps) => {
///                 let plans = gaps.iter()
///                     .map(|gap| /* generate plan from gap */)
///                     .collect();
///                 Ok(PlannerOutput::Plans(plans))
///             }
///             _ => Err(SelfImprovementError::ValidationFailed(
///                 "Unsupported input type".to_string()
///             ))
///         }
///     }
/// }
/// ```
pub trait Planner: Send + Sync + Debug {
    /// Returns the unique name of this planner.
    fn name(&self) -> &str;

    /// Returns a description of what this planner does.
    fn description(&self) -> &str {
        "No description provided"
    }

    /// Generates plans or proposals from analysis input.
    ///
    /// # Arguments
    /// * `input` - The analysis results to convert to plans
    ///
    /// # Returns
    /// Plans or proposals based on the input.
    fn generate(&self, input: PlannerInput) -> Result<PlannerOutput>;

    /// Returns the input types this planner accepts.
    fn accepted_inputs(&self) -> Vec<PlannerInputType> {
        vec![
            PlannerInputType::Gaps,
            PlannerInputType::Deprecations,
            PlannerInputType::Retrospective,
        ]
    }

    /// Validates that the planner can handle the given input.
    fn validate_input(&self, input: &PlannerInput) -> Result<()> {
        let input_type = match input {
            PlannerInput::Gaps(_) => PlannerInputType::Gaps,
            PlannerInput::Deprecations(_) => PlannerInputType::Deprecations,
            PlannerInput::Retrospective(_) => PlannerInputType::Retrospective,
            PlannerInput::Combined { .. } => PlannerInputType::Combined,
            PlannerInput::Custom { .. } => PlannerInputType::Custom,
        };

        if !self.accepted_inputs().contains(&input_type) {
            return Err(SelfImprovementError::ValidationFailed(format!(
                "Planner '{}' does not accept {:?} input",
                self.name(),
                input_type
            )));
        }
        Ok(())
    }

    /// Returns this planner as Any for downcasting.
    fn as_any(&self) -> &dyn Any;
}

/// Types of input a planner can accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannerInputType {
    /// Capability gaps
    Gaps,
    /// Deprecation recommendations
    Deprecations,
    /// Retrospective analysis
    Retrospective,
    /// Combined gaps and deprecations
    Combined,
    /// Custom input
    Custom,
}

// =============================================================================
// Storage Backend Trait
// =============================================================================

/// Trait for pluggable storage backends.
///
/// The default implementation uses file-based storage, but this trait
/// enables database, cloud, or in-memory storage backends.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{StorageBackend, IntrospectionReport, ExecutionPlan, Hypothesis};
///
/// struct RedisStorageBackend {
///     client: redis::Client,
/// }
///
/// impl StorageBackend for RedisStorageBackend {
///     fn name(&self) -> &str {
///         "redis"
///     }
///
///     fn save_report(&self, report: &IntrospectionReport) -> Result<(), SelfImprovementError> {
///         // Save to Redis
///         Ok(())
///     }
///
///     // ... implement other methods ...
/// }
/// ```
pub trait StorageBackend: Send + Sync + Debug {
    /// Returns the unique name of this storage backend.
    fn name(&self) -> &str;

    /// Returns a description of this storage backend.
    fn description(&self) -> &str {
        "No description provided"
    }

    // =========================================================================
    // Report Operations
    // =========================================================================

    /// Saves an introspection report.
    fn save_report(&self, report: &IntrospectionReport) -> Result<()>;

    /// Loads a report by ID.
    fn load_report(&self, id: &str) -> Result<IntrospectionReport>;

    /// Lists all report IDs.
    fn list_reports(&self) -> Result<Vec<String>>;

    /// Deletes a report by ID.
    fn delete_report(&self, id: &str) -> Result<()>;

    // =========================================================================
    // Plan Operations
    // =========================================================================

    /// Saves an execution plan.
    fn save_plan(&self, plan: &ExecutionPlan) -> Result<()>;

    /// Loads a plan by ID.
    fn load_plan(&self, id: &str) -> Result<ExecutionPlan>;

    /// Lists all plan IDs, optionally filtered by status.
    fn list_plans(&self, status: Option<&str>) -> Result<Vec<String>>;

    /// Deletes a plan by ID.
    fn delete_plan(&self, id: &str) -> Result<()>;

    /// Updates a plan's status.
    fn update_plan_status(&self, id: &str, status: &str) -> Result<()>;

    // =========================================================================
    // Hypothesis Operations
    // =========================================================================

    /// Saves a hypothesis.
    fn save_hypothesis(&self, hypothesis: &Hypothesis) -> Result<()>;

    /// Loads a hypothesis by ID.
    fn load_hypothesis(&self, id: &str) -> Result<Hypothesis>;

    /// Lists all hypothesis IDs.
    fn list_hypotheses(&self, active_only: bool) -> Result<Vec<String>>;

    /// Deletes a hypothesis by ID.
    fn delete_hypothesis(&self, id: &str) -> Result<()>;

    // =========================================================================
    // Lifecycle
    // =========================================================================

    /// Initializes the storage backend.
    fn initialize(&self) -> Result<()> {
        Ok(())
    }

    /// Performs cleanup operations (e.g., remove old data).
    fn cleanup(&self) -> Result<StorageCleanupResult> {
        Ok(StorageCleanupResult::default())
    }

    /// Returns storage statistics.
    fn stats(&self) -> Result<StorageStatistics>;

    /// Checks if the storage backend is healthy.
    fn is_healthy(&self) -> bool {
        true
    }
}

/// Result of a storage cleanup operation.
#[derive(Debug, Clone, Default)]
pub struct StorageCleanupResult {
    /// Number of reports deleted
    pub reports_deleted: usize,
    /// Number of plans deleted
    pub plans_deleted: usize,
    /// Number of hypotheses deleted
    pub hypotheses_deleted: usize,
    /// Bytes freed
    pub bytes_freed: u64,
}

/// Storage statistics.
#[derive(Debug, Clone, Default)]
pub struct StorageStatistics {
    /// Total number of reports
    pub report_count: usize,
    /// Total number of plans
    pub plan_count: usize,
    /// Total number of hypotheses
    pub hypothesis_count: usize,
    /// Total storage size in bytes
    pub total_size_bytes: u64,
}

// =============================================================================
// Analyzer Registry
// =============================================================================

/// Registry for managing multiple analyzers.
#[derive(Default)]
pub struct AnalyzerRegistry {
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl Debug for AnalyzerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyzerRegistry")
            .field("count", &self.analyzers.len())
            .field(
                "names",
                &self.analyzers.iter().map(|a| a.name()).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl AnalyzerRegistry {
    /// Creates a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an analyzer.
    pub fn register<A: Analyzer + 'static>(&mut self, analyzer: A) {
        self.analyzers.push(Box::new(analyzer));
    }

    /// Returns the number of registered analyzers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.analyzers.len()
    }

    /// Returns true if no analyzers are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.analyzers.is_empty()
    }

    /// Gets an analyzer by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn Analyzer> {
        self.analyzers
            .iter()
            .find(|a| a.name() == name)
            .map(|a| a.as_ref())
    }

    /// Lists all registered analyzer names.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.analyzers.iter().map(|a| a.name()).collect()
    }

    /// Runs all analyzers on the given traces.
    pub fn analyze_all(
        &self,
        traces: &[ExecutionTrace],
        context: &AnalyzerContext,
    ) -> Vec<(&str, Result<AnalysisOutput>)> {
        self.analyzers
            .iter()
            .map(|analyzer| {
                let name = analyzer.name();
                let result = analyzer
                    .validate(traces)
                    .and_then(|()| analyzer.analyze(traces, context));
                (name, result)
            })
            .collect()
    }

    /// Returns an iterator over all analyzers.
    pub fn iter(&self) -> impl Iterator<Item = &dyn Analyzer> {
        self.analyzers.iter().map(|a| a.as_ref())
    }
}

// =============================================================================
// Planner Registry
// =============================================================================

/// Registry for managing multiple planners.
#[derive(Default)]
pub struct PlannerRegistry {
    planners: Vec<Box<dyn Planner>>,
}

impl Debug for PlannerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlannerRegistry")
            .field("count", &self.planners.len())
            .field(
                "names",
                &self.planners.iter().map(|p| p.name()).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl PlannerRegistry {
    /// Creates a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a planner.
    pub fn register<P: Planner + 'static>(&mut self, planner: P) {
        self.planners.push(Box::new(planner));
    }

    /// Returns the number of registered planners.
    #[must_use]
    pub fn len(&self) -> usize {
        self.planners.len()
    }

    /// Returns true if no planners are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.planners.is_empty()
    }

    /// Gets a planner by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn Planner> {
        self.planners
            .iter()
            .find(|p| p.name() == name)
            .map(|p| p.as_ref())
    }

    /// Lists all registered planner names.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.planners.iter().map(|p| p.name()).collect()
    }

    /// Runs a specific planner with the given input.
    pub fn generate(&self, planner_name: &str, input: PlannerInput) -> Result<PlannerOutput> {
        let planner = self.get(planner_name).ok_or_else(|| {
            SelfImprovementError::Other(format!("Planner '{}' not found", planner_name))
        })?;
        planner.validate_input(&input)?;
        planner.generate(input)
    }

    /// Returns an iterator over all planners.
    pub fn iter(&self) -> impl Iterator<Item = &dyn Planner> {
        self.planners.iter().map(|p| p.as_ref())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test analyzer implementation
    #[derive(Debug)]
    struct TestAnalyzer {
        name: String,
    }

    impl Analyzer for TestAnalyzer {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A test analyzer"
        }

        fn analyze(
            &self,
            _traces: &[ExecutionTrace],
            _context: &AnalyzerContext,
        ) -> Result<AnalysisOutput> {
            Ok(AnalysisOutput::CapabilityGaps(vec![]))
        }

        fn output_type(&self) -> AnalyzerOutputType {
            AnalyzerOutputType::CapabilityGaps
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    // Test planner implementation
    #[derive(Debug)]
    struct TestPlanner {
        name: String,
    }

    impl Planner for TestPlanner {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A test planner"
        }

        fn generate(&self, _input: PlannerInput) -> Result<PlannerOutput> {
            Ok(PlannerOutput::Plans(vec![]))
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[test]
    fn test_analyzer_registry() {
        let mut registry = AnalyzerRegistry::new();
        assert!(registry.is_empty());

        registry.register(TestAnalyzer {
            name: "test-1".to_string(),
        });
        registry.register(TestAnalyzer {
            name: "test-2".to_string(),
        });

        assert_eq!(registry.len(), 2);
        assert!(!registry.is_empty());

        let names = registry.names();
        assert!(names.contains(&"test-1"));
        assert!(names.contains(&"test-2"));

        let analyzer = registry.get("test-1").unwrap();
        assert_eq!(analyzer.name(), "test-1");
        assert_eq!(analyzer.description(), "A test analyzer");

        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_planner_registry() {
        let mut registry = PlannerRegistry::new();
        assert!(registry.is_empty());

        registry.register(TestPlanner {
            name: "planner-1".to_string(),
        });

        assert_eq!(registry.len(), 1);

        let planner = registry.get("planner-1").unwrap();
        assert_eq!(planner.name(), "planner-1");
    }

    #[test]
    fn test_analyzer_context() {
        let context = AnalyzerContext::new()
            .with_known_nodes(vec!["node1".to_string(), "node2".to_string()])
            .with_min_confidence(0.8)
            .with_max_results(10)
            .with_extra("key", "value");

        assert_eq!(context.known_nodes.len(), 2);
        assert!((context.min_confidence - 0.8).abs() < f64::EPSILON);
        assert_eq!(context.max_results, Some(10));
        assert_eq!(context.extra.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_analysis_output_accessors() {
        let gaps_output = AnalysisOutput::CapabilityGaps(vec![]);
        assert!(gaps_output.as_capability_gaps().is_some());
        assert!(gaps_output.as_deprecations().is_none());
        assert!(gaps_output.as_retrospective().is_none());

        let deps_output = AnalysisOutput::Deprecations(vec![]);
        assert!(deps_output.as_deprecations().is_some());
        assert!(deps_output.as_capability_gaps().is_none());
    }

    #[test]
    fn test_planner_output_accessors() {
        let plans_output = PlannerOutput::Plans(vec![]);
        assert!(plans_output.as_plans().is_some());
        assert!(plans_output.as_proposals().is_none());

        let proposals_output = PlannerOutput::Proposals(vec![]);
        assert!(proposals_output.as_proposals().is_some());
        assert!(proposals_output.as_plans().is_none());
    }

    #[test]
    fn test_storage_cleanup_result() {
        let result = StorageCleanupResult {
            reports_deleted: 5,
            plans_deleted: 10,
            hypotheses_deleted: 3,
            bytes_freed: 1024,
        };

        assert_eq!(result.reports_deleted, 5);
        assert_eq!(result.plans_deleted, 10);
        assert_eq!(result.hypotheses_deleted, 3);
        assert_eq!(result.bytes_freed, 1024);
    }

    #[test]
    fn test_storage_statistics() {
        let stats = StorageStatistics {
            report_count: 10,
            plan_count: 20,
            hypothesis_count: 5,
            total_size_bytes: 10240,
        };

        assert_eq!(stats.report_count, 10);
        assert_eq!(stats.plan_count, 20);
        assert_eq!(stats.hypothesis_count, 5);
        assert_eq!(stats.total_size_bytes, 10240);
    }

    #[test]
    fn test_analyzer_validation() {
        let analyzer = TestAnalyzer {
            name: "test".to_string(),
        };

        // Default min_traces is 1
        assert!(analyzer.validate(&[]).is_err());

        // With one trace it should pass
        let trace = ExecutionTrace::builder().build();
        assert!(analyzer.validate(&[trace]).is_ok());
    }

    #[test]
    fn test_planner_input_validation() {
        let planner = TestPlanner {
            name: "test".to_string(),
        };

        // Default accepts all standard input types
        let gaps_input = PlannerInput::Gaps(vec![]);
        assert!(planner.validate_input(&gaps_input).is_ok());

        let deps_input = PlannerInput::Deprecations(vec![]);
        assert!(planner.validate_input(&deps_input).is_ok());
    }

    #[test]
    fn test_analyze_all() {
        let mut registry = AnalyzerRegistry::new();
        registry.register(TestAnalyzer {
            name: "test-1".to_string(),
        });
        registry.register(TestAnalyzer {
            name: "test-2".to_string(),
        });

        let trace = ExecutionTrace::builder().build();
        let context = AnalyzerContext::new();

        let results = registry.analyze_all(&[trace], &context);
        assert_eq!(results.len(), 2);

        for (name, result) in results {
            assert!(result.is_ok(), "Analyzer {} failed", name);
        }
    }
}
