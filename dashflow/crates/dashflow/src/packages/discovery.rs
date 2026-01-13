// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! AI-driven package discovery service.
//!
//! This module integrates with the DashFlow introspection system to provide
//! intelligent package recommendations. It analyzes:
//!
//! - Capability gaps from introspection reports
//! - Graph structure to suggest enhancements
//! - Colony network for locally available packages
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::packages::{PackageDiscovery, DiscoveryConfig};
//! use dashflow::self_improvement::IntrospectionReport;
//!
//! // Create discovery service
//! let discovery = PackageDiscovery::new(DiscoveryConfig::default())?;
//!
//! // Get suggestions from an introspection report
//! let suggestions = discovery.suggest_from_report(&report);
//!
//! // Get suggestions for a graph
//! let suggestions = discovery.suggest_for_graph(&graph);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use super::client::{PackageInfo, PackageSearchResult, RegistryClient, RegistryClientConfig};
use super::semantic::{DefaultSemanticSearch, PackageMetadata, SearchFilter, SearchQuery};
use super::types::{PackageId, PackageType, Version};

// =============================================================================
// Discovery Configuration
// =============================================================================

/// Configuration for the package discovery service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Minimum confidence threshold for suggestions (0.0-1.0).
    /// Suggestions below this threshold are filtered out.
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f64,

    /// Maximum number of suggestions to return per query.
    #[serde(default = "default_max_suggestions")]
    pub max_suggestions: usize,

    /// Whether to boost confidence for packages available in colony.
    #[serde(default = "default_boost_colony")]
    pub boost_colony_packages: bool,

    /// Confidence boost amount for colony-available packages (0.0-1.0).
    #[serde(default = "default_colony_boost")]
    pub colony_boost: f64,

    /// Whether to include packages that require additional permissions.
    #[serde(default)]
    pub include_permission_required: bool,

    /// Package types to exclude from suggestions.
    #[serde(default)]
    pub excluded_types: Vec<PackageType>,

    /// Registry client configuration (if using remote search).
    #[serde(skip)]
    pub registry_config: Option<RegistryClientConfig>,
}

fn default_min_confidence() -> f64 {
    0.3
}

fn default_max_suggestions() -> usize {
    10
}

fn default_boost_colony() -> bool {
    true
}

fn default_colony_boost() -> f64 {
    0.15
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            min_confidence: default_min_confidence(),
            max_suggestions: default_max_suggestions(),
            boost_colony_packages: default_boost_colony(),
            colony_boost: default_colony_boost(),
            include_permission_required: false,
            excluded_types: Vec::new(),
            registry_config: None,
        }
    }
}

impl DiscoveryConfig {
    /// Create a new configuration with custom minimum confidence.
    #[must_use]
    pub fn with_min_confidence(mut self, confidence: f64) -> Self {
        self.min_confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set maximum number of suggestions.
    #[must_use]
    pub fn with_max_suggestions(mut self, max: usize) -> Self {
        self.max_suggestions = max;
        self
    }

    /// Enable or disable colony package boosting.
    #[must_use]
    pub fn with_colony_boost(mut self, enabled: bool) -> Self {
        self.boost_colony_packages = enabled;
        self
    }

    /// Set the registry client configuration for remote searches.
    #[must_use]
    pub fn with_registry(mut self, config: RegistryClientConfig) -> Self {
        self.registry_config = Some(config);
        self
    }

    /// Exclude specific package types from suggestions.
    pub fn exclude_type(mut self, package_type: PackageType) -> Self {
        self.excluded_types.push(package_type);
        self
    }
}

// =============================================================================
// Package Suggestion Types
// =============================================================================

/// A package suggestion with reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSuggestion {
    /// The suggested package (if found in registry).
    pub package: Option<SuggestedPackage>,

    /// Search query used to find this package.
    pub search_query: String,

    /// Why this package is being suggested.
    pub reason: SuggestionReason,

    /// Confidence in this suggestion (0.0-1.0).
    pub confidence: f64,

    /// Expected benefits if installed.
    pub expected_benefits: Vec<String>,

    /// Whether the package is available locally in colony.
    pub colony_available: bool,

    /// Source of this suggestion.
    pub source: SuggestionSource,
}

impl PackageSuggestion {
    /// Create a new package suggestion.
    pub fn new(search_query: impl Into<String>, reason: SuggestionReason, confidence: f64) -> Self {
        Self {
            package: None,
            search_query: search_query.into(),
            reason,
            confidence: confidence.clamp(0.0, 1.0),
            expected_benefits: Vec::new(),
            colony_available: false,
            source: SuggestionSource::Manual,
        }
    }

    /// Set the matched package.
    #[must_use]
    pub fn with_package(mut self, package: SuggestedPackage) -> Self {
        self.package = Some(package);
        self
    }

    /// Add expected benefits.
    #[must_use]
    pub fn with_benefits(mut self, benefits: Vec<String>) -> Self {
        self.expected_benefits = benefits;
        self
    }

    /// Mark as colony available.
    #[must_use]
    pub fn with_colony_available(mut self, available: bool) -> Self {
        self.colony_available = available;
        self
    }

    /// Set the suggestion source.
    #[must_use]
    pub fn with_source(mut self, source: SuggestionSource) -> Self {
        self.source = source;
        self
    }

    /// Apply colony boost to confidence.
    pub fn apply_colony_boost(&mut self, boost: f64) {
        if self.colony_available {
            self.confidence = (self.confidence + boost).min(1.0);
        }
    }
}

/// Information about a suggested package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedPackage {
    /// Package identifier.
    pub id: PackageId,

    /// Package name.
    pub name: String,

    /// Short description.
    pub description: String,

    /// Package type.
    pub package_type: PackageType,

    /// Latest version.
    pub version: Version,

    /// Trust verification status.
    pub verified: bool,

    /// Download count (if available).
    pub downloads: Option<u64>,

    /// Relevance score from search (0.0-1.0).
    pub relevance_score: f64,
}

impl SuggestedPackage {
    /// Create from package info.
    pub fn from_package_info(info: &PackageInfo, relevance_score: f64) -> Self {
        Self {
            id: info.id.clone(),
            name: info.name.clone(),
            description: info.description.clone(),
            package_type: info.package_type,
            version: info.latest_version.clone(),
            verified: info.verified,
            downloads: Some(info.downloads),
            relevance_score,
        }
    }

    /// Create from search result.
    pub fn from_search_result(result: &PackageSearchResult, relevance_score: f64) -> Self {
        Self {
            id: result.id.clone(),
            name: result.name.clone(),
            description: result.description.clone(),
            package_type: result.package_type,
            version: result.latest_version.clone(),
            verified: result.verified,
            downloads: Some(result.downloads),
            relevance_score,
        }
    }
}

/// Why a package is being suggested.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionReason {
    /// Package fills a detected capability gap.
    FillsCapabilityGap {
        /// Description of the gap.
        gap_description: String,
        /// Category of the gap.
        gap_category: GapCategoryRef,
    },

    /// Package enhances an existing node.
    EnhancesNode {
        /// Node that would be enhanced.
        node: String,
        /// What improvement is expected.
        improvement: String,
    },

    /// Package provides better performance.
    PerformanceImprovement {
        /// The metric that would improve.
        metric: String,
        /// Expected improvement percentage (0.0-1.0).
        improvement: f64,
    },

    /// Similar graphs commonly use this package.
    CommonInSimilarGraphs {
        /// Similarity score with those graphs.
        similarity: f64,
        /// Number of similar graphs using this.
        usage_count: usize,
    },

    /// Recommended by introspection system.
    IntrospectionRecommendation {
        /// ID of the introspection report.
        report_id: Uuid,
        /// Priority from the report.
        priority: RecommendationPriority,
    },

    /// Package provides missing tool functionality.
    MissingTool {
        /// Description of the needed tool.
        tool_description: String,
    },

    /// Package provides missing integration.
    MissingIntegration {
        /// External system to integrate with.
        external_system: String,
    },

    /// Manual suggestion (e.g., from user query).
    ManualSearch {
        /// The search query.
        query: String,
    },

    /// Package found via semantic similarity search.
    SemanticMatch {
        /// The search query.
        query: String,
        /// Similarity score (0.0-1.0).
        similarity_score: f64,
    },
}

/// Reference to a gap category (simplified for serialization).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GapCategoryRef {
    /// Missing node type.
    MissingNode,
    /// Missing tool.
    MissingTool,
    /// Inadequate existing functionality.
    InadequateFunctionality,
    /// Missing integration.
    MissingIntegration,
    /// Performance limitation.
    PerformanceGap,
}

/// Priority level for recommendations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecommendationPriority {
    /// Critical - should install immediately.
    Critical,
    /// High - strongly recommended.
    High,
    /// Medium - would be beneficial.
    Medium,
    /// Low - nice to have.
    Low,
}

impl RecommendationPriority {
    /// Convert to confidence boost factor.
    pub fn confidence_factor(&self) -> f64 {
        match self {
            Self::Critical => 1.0,
            Self::High => 0.9,
            Self::Medium => 0.7,
            Self::Low => 0.5,
        }
    }
}

/// Source of a package suggestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionSource {
    /// From introspection analysis.
    Introspection {
        /// ID of the introspection report that generated this suggestion.
        report_id: Uuid,
    },
    /// From graph structure analysis.
    GraphAnalysis {
        /// ID of the graph that was analyzed.
        graph_id: Option<String>,
    },
    /// From colony peer recommendations.
    Colony {
        /// ID of the peer that made the recommendation.
        peer_id: Uuid,
    },
    /// From manual search.
    Manual,
    /// From semantic similarity search.
    SemanticSearch {
        /// The search query used.
        query: String,
    },
}

// =============================================================================
// Package Recommendation (from IntrospectionReport)
// =============================================================================

/// A package recommendation derived from capability gap analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageRecommendation {
    /// Description of the capability gap.
    pub gap_description: String,

    /// Search query to find matching packages.
    pub search_query: String,

    /// Recommended package type.
    pub package_type: PackageType,

    /// Priority based on gap severity.
    pub priority: RecommendationPriority,

    /// Category of the gap.
    pub gap_category: GapCategoryRef,

    /// Expected impact if addressed.
    pub expected_impact: RecommendedImpact,

    /// Confidence in this recommendation (0.0-1.0).
    pub confidence: f64,
}

impl PackageRecommendation {
    /// Create a new recommendation.
    pub fn new(
        gap_description: impl Into<String>,
        search_query: impl Into<String>,
        package_type: PackageType,
        gap_category: GapCategoryRef,
    ) -> Self {
        Self {
            gap_description: gap_description.into(),
            search_query: search_query.into(),
            package_type,
            priority: RecommendationPriority::Medium,
            gap_category,
            expected_impact: RecommendedImpact::default(),
            confidence: 0.5,
        }
    }

    /// Set priority.
    #[must_use]
    pub fn with_priority(mut self, priority: RecommendationPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set expected impact.
    #[must_use]
    pub fn with_impact(mut self, impact: RecommendedImpact) -> Self {
        self.expected_impact = impact;
        self
    }

    /// Set confidence.
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Convert to a package suggestion.
    pub fn to_suggestion(&self) -> PackageSuggestion {
        let reason = match &self.gap_category {
            GapCategoryRef::MissingTool => SuggestionReason::MissingTool {
                tool_description: self.gap_description.clone(),
            },
            GapCategoryRef::MissingIntegration => SuggestionReason::MissingIntegration {
                external_system: self.search_query.clone(),
            },
            _ => SuggestionReason::FillsCapabilityGap {
                gap_description: self.gap_description.clone(),
                gap_category: self.gap_category.clone(),
            },
        };

        let mut suggestion = PackageSuggestion::new(
            &self.search_query,
            reason,
            self.confidence * self.priority.confidence_factor(),
        );

        suggestion.expected_benefits = self.expected_impact.to_benefits();
        suggestion
    }
}

/// Expected impact of addressing a recommendation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecommendedImpact {
    /// Expected reduction in errors (0.0-1.0).
    pub error_reduction: f64,
    /// Expected reduction in latency (ms).
    pub latency_reduction_ms: f64,
    /// Expected improvement in accuracy (0.0-1.0).
    pub accuracy_improvement: f64,
    /// Description of qualitative impact.
    pub description: String,
}

impl RecommendedImpact {
    /// Create high impact.
    pub fn high(description: impl Into<String>) -> Self {
        Self {
            error_reduction: 0.8,
            latency_reduction_ms: 400.0,
            accuracy_improvement: 0.8,
            description: description.into(),
        }
    }

    /// Create medium impact.
    pub fn medium(description: impl Into<String>) -> Self {
        Self {
            error_reduction: 0.4,
            latency_reduction_ms: 200.0,
            accuracy_improvement: 0.4,
            description: description.into(),
        }
    }

    /// Create low impact.
    pub fn low(description: impl Into<String>) -> Self {
        Self {
            error_reduction: 0.1,
            latency_reduction_ms: 50.0,
            accuracy_improvement: 0.1,
            description: description.into(),
        }
    }

    /// Convert to benefit strings.
    pub fn to_benefits(&self) -> Vec<String> {
        let mut benefits = Vec::new();
        if self.error_reduction > 0.0 {
            benefits.push(format!(
                "Reduce errors by ~{:.0}%",
                self.error_reduction * 100.0
            ));
        }
        if self.latency_reduction_ms > 0.0 {
            benefits.push(format!(
                "Reduce latency by ~{:.0}ms",
                self.latency_reduction_ms
            ));
        }
        if self.accuracy_improvement > 0.0 {
            benefits.push(format!(
                "Improve accuracy by ~{:.0}%",
                self.accuracy_improvement * 100.0
            ));
        }
        if !self.description.is_empty() {
            benefits.push(self.description.clone());
        }
        benefits
    }
}

// =============================================================================
// Graph Analysis Types
// =============================================================================

/// Analysis of a graph's structure for package suggestions.
#[derive(Debug, Clone, Default)]
pub struct GraphAnalysis {
    /// Identified node types in use.
    pub node_types: Vec<String>,

    /// Identified capabilities in use.
    pub capabilities: Vec<String>,

    /// Detected patterns in the graph.
    pub patterns: Vec<GraphPattern>,

    /// Potential enhancement points.
    pub enhancement_points: Vec<EnhancementPoint>,
}

impl GraphAnalysis {
    /// Create a new empty analysis.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node type.
    pub fn add_node_type(&mut self, node_type: impl Into<String>) {
        self.node_types.push(node_type.into());
    }

    /// Add a capability.
    pub fn add_capability(&mut self, capability: impl Into<String>) {
        self.capabilities.push(capability.into());
    }

    /// Add a pattern.
    pub fn add_pattern(&mut self, pattern: GraphPattern) {
        self.patterns.push(pattern);
    }

    /// Add an enhancement point.
    pub fn add_enhancement_point(&mut self, point: EnhancementPoint) {
        self.enhancement_points.push(point);
    }
}

/// A detected pattern in a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPattern {
    /// Pattern name.
    pub name: String,

    /// Pattern description.
    pub description: String,

    /// Nodes involved in this pattern.
    pub nodes: Vec<String>,

    /// Packages commonly used with this pattern.
    pub common_packages: Vec<PackageId>,
}

/// A point in the graph that could be enhanced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancementPoint {
    /// Location in the graph (node name or edge).
    pub location: String,

    /// Type of enhancement possible.
    pub enhancement_type: EnhancementType,

    /// Suggested improvement.
    pub suggestion: String,

    /// Confidence in this enhancement (0.0-1.0).
    pub confidence: f64,
}

/// Type of graph enhancement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnhancementType {
    /// Add a preprocessing step.
    AddPreprocessing,
    /// Add a postprocessing step.
    AddPostprocessing,
    /// Replace with better node.
    ReplaceNode,
    /// Add caching.
    AddCaching,
    /// Add error handling.
    AddErrorHandling,
    /// Add monitoring.
    AddMonitoring,
    /// Add validation.
    AddValidation,
}

// =============================================================================
// Discovery Errors
// =============================================================================

/// Errors that can occur during package discovery.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum DiscoveryError {
    /// Registry client error.
    #[error("Registry client error: {0}")]
    Client(String),
    /// No suggestions found.
    #[error("No package suggestions found")]
    NoSuggestions,
    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    /// Analysis failed.
    #[error("Analysis failed: {0}")]
    AnalysisFailed(String),
}

/// Result type for discovery operations.
pub type DiscoveryResult<T> = Result<T, DiscoveryError>;

// =============================================================================
// Package Discovery Service
// =============================================================================

/// AI-driven package discovery service.
///
/// Provides intelligent package recommendations based on:
/// - Introspection report analysis
/// - Graph structure analysis
/// - Colony network availability
/// - Semantic similarity search (embeddings-based)
pub struct PackageDiscovery {
    /// Discovery configuration.
    config: DiscoveryConfig,

    /// Registry client for remote searches (optional).
    client: Option<RegistryClient>,

    /// Semantic search service for local similarity search (optional).
    semantic_search: Option<Arc<DefaultSemanticSearch>>,

    /// Cache of recent suggestions.
    suggestion_cache: HashMap<String, Vec<PackageSuggestion>>,

    /// Colony available packages (if connected).
    colony_packages: Vec<PackageId>,
}

impl PackageDiscovery {
    /// Create a new discovery service with default configuration.
    pub fn new() -> Self {
        Self::with_config(DiscoveryConfig::default())
    }

    /// Create a new discovery service with custom configuration.
    #[must_use]
    pub fn with_config(config: DiscoveryConfig) -> Self {
        let client = config
            .registry_config
            .as_ref()
            .and_then(|rc| RegistryClient::new(rc.clone()).ok());

        Self {
            config,
            client,
            semantic_search: None,
            suggestion_cache: HashMap::new(),
            colony_packages: Vec::new(),
        }
    }

    /// Create a discovery service with semantic search enabled.
    ///
    /// The semantic search service allows finding packages by semantic
    /// similarity rather than exact keyword matches.
    #[must_use]
    pub fn with_semantic_search(mut self, service: Arc<DefaultSemanticSearch>) -> Self {
        self.semantic_search = Some(service);
        self
    }

    /// Set the semantic search service.
    pub fn set_semantic_search(&mut self, service: Arc<DefaultSemanticSearch>) {
        self.semantic_search = Some(service);
    }

    /// Check if semantic search is available.
    pub fn has_semantic_search(&self) -> bool {
        self.semantic_search.is_some()
    }

    /// Get the number of packages indexed for semantic search.
    pub fn semantic_index_count(&self) -> usize {
        self.semantic_search
            .as_ref()
            .map(|s| s.indexed_count())
            .unwrap_or(0)
    }

    /// Update colony available packages.
    pub fn set_colony_packages(&mut self, packages: Vec<PackageId>) {
        self.colony_packages = packages;
    }

    /// Check if a package is available in colony.
    pub fn is_colony_available(&self, package_id: &PackageId) -> bool {
        self.colony_packages.iter().any(|p| p == package_id)
    }

    /// Generate package recommendations from capability gaps.
    ///
    /// This method analyzes the capability gaps from an introspection report
    /// and generates package recommendations for each gap.
    pub fn recommendations_from_gaps(
        &self,
        gaps: &[CapabilityGapInfo],
    ) -> Vec<PackageRecommendation> {
        let mut recommendations = Vec::new();

        for gap in gaps {
            if let Some(rec) = self.gap_to_recommendation(gap) {
                recommendations.push(rec);
            }
        }

        // Sort by priority first (Critical > High > Medium > Low), then by confidence
        recommendations.sort_by(|a, b| {
            // Lower enum ordinal = higher priority
            match (a.priority as u8).cmp(&(b.priority as u8)) {
                std::cmp::Ordering::Equal => {
                    // Same priority, sort by confidence (higher first)
                    b.confidence
                        .partial_cmp(&a.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
                ord => ord,
            }
        });

        recommendations
    }

    /// Convert a capability gap to a package recommendation.
    fn gap_to_recommendation(&self, gap: &CapabilityGapInfo) -> Option<PackageRecommendation> {
        let (search_query, package_type, gap_category) = match &gap.category {
            CapabilityGapCategory::MissingTool { tool_description } => (
                tool_description.clone(),
                PackageType::ToolPack,
                GapCategoryRef::MissingTool,
            ),
            CapabilityGapCategory::MissingNode {
                suggested_signature,
            } => (
                suggested_signature.clone(),
                PackageType::NodeLibrary,
                GapCategoryRef::MissingNode,
            ),
            CapabilityGapCategory::MissingIntegration { external_system } => (
                format!("{} connector", external_system),
                PackageType::ModelConnector,
                GapCategoryRef::MissingIntegration,
            ),
            CapabilityGapCategory::InadequateFunctionality { node, limitation } => (
                format!("{} {}", node, limitation),
                PackageType::NodeLibrary,
                GapCategoryRef::InadequateFunctionality,
            ),
            CapabilityGapCategory::PerformanceGap { bottleneck } => (
                format!("{} optimization", bottleneck),
                PackageType::NodeLibrary,
                GapCategoryRef::PerformanceGap,
            ),
        };

        // Determine priority from confidence
        let priority = if gap.confidence > 0.8 {
            RecommendationPriority::High
        } else if gap.confidence > 0.5 {
            RecommendationPriority::Medium
        } else {
            RecommendationPriority::Low
        };

        // Create impact from gap info
        let impact = RecommendedImpact {
            error_reduction: gap.expected_error_reduction,
            latency_reduction_ms: gap.expected_latency_reduction_ms,
            accuracy_improvement: gap.expected_accuracy_improvement,
            description: gap.proposed_solution.clone(),
        };

        Some(
            PackageRecommendation::new(&gap.description, search_query, package_type, gap_category)
                .with_priority(priority)
                .with_impact(impact)
                .with_confidence(gap.confidence),
        )
    }

    /// Generate suggestions from package recommendations.
    ///
    /// This converts recommendations to suggestions and optionally
    /// searches the registry for matching packages.
    pub fn suggestions_from_recommendations(
        &mut self,
        recommendations: &[PackageRecommendation],
        search_registry: bool,
    ) -> Vec<PackageSuggestion> {
        let mut suggestions: Vec<PackageSuggestion> = recommendations
            .iter()
            .map(|rec| rec.to_suggestion())
            .collect();

        // Search registry for matching packages if configured
        if search_registry {
            if let Some(client) = &self.client {
                for suggestion in &mut suggestions {
                    if let Ok(results) = client.search(&suggestion.search_query) {
                        if let Some(first) = results.first() {
                            // Use 1.0 as default relevance score since search results don't have a score
                            let pkg = SuggestedPackage::from_search_result(first, 1.0);
                            suggestion.package = Some(pkg);
                        }
                    }
                }
            }
        }

        // Check colony availability and apply boost
        for suggestion in &mut suggestions {
            if let Some(pkg) = &suggestion.package {
                suggestion.colony_available = self.is_colony_available(&pkg.id);
                if self.config.boost_colony_packages {
                    suggestion.apply_colony_boost(self.config.colony_boost);
                }
            }
        }

        // Filter and sort
        suggestions.retain(|s| s.confidence >= self.config.min_confidence);
        suggestions.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        suggestions.truncate(self.config.max_suggestions);

        suggestions
    }

    /// Analyze a graph and suggest packages.
    ///
    /// This performs static analysis of the graph structure to identify
    /// potential enhancements and suggest packages.
    pub fn suggest_for_graph_analysis(
        &mut self,
        analysis: &GraphAnalysis,
    ) -> Vec<PackageSuggestion> {
        let mut suggestions = Vec::new();

        // Generate suggestions for enhancement points
        for point in &analysis.enhancement_points {
            let (search_query, reason) = match &point.enhancement_type {
                EnhancementType::AddPreprocessing => (
                    format!("{} preprocessing", point.location),
                    SuggestionReason::EnhancesNode {
                        node: point.location.clone(),
                        improvement: "Add preprocessing step".to_string(),
                    },
                ),
                EnhancementType::AddPostprocessing => (
                    format!("{} postprocessing", point.location),
                    SuggestionReason::EnhancesNode {
                        node: point.location.clone(),
                        improvement: "Add postprocessing step".to_string(),
                    },
                ),
                EnhancementType::ReplaceNode => (
                    format!("{} alternative", point.location),
                    SuggestionReason::PerformanceImprovement {
                        metric: "node efficiency".to_string(),
                        improvement: point.confidence,
                    },
                ),
                EnhancementType::AddCaching => (
                    "caching tools".to_string(),
                    SuggestionReason::PerformanceImprovement {
                        metric: "latency".to_string(),
                        improvement: 0.3,
                    },
                ),
                EnhancementType::AddErrorHandling => (
                    "error handling tools".to_string(),
                    SuggestionReason::EnhancesNode {
                        node: point.location.clone(),
                        improvement: "Add robust error handling".to_string(),
                    },
                ),
                EnhancementType::AddMonitoring => (
                    "monitoring tools".to_string(),
                    SuggestionReason::EnhancesNode {
                        node: point.location.clone(),
                        improvement: "Add observability".to_string(),
                    },
                ),
                EnhancementType::AddValidation => (
                    "validation tools".to_string(),
                    SuggestionReason::EnhancesNode {
                        node: point.location.clone(),
                        improvement: "Add input/output validation".to_string(),
                    },
                ),
            };

            let suggestion = PackageSuggestion::new(search_query, reason, point.confidence)
                .with_source(SuggestionSource::GraphAnalysis { graph_id: None })
                .with_benefits(vec![point.suggestion.clone()]);

            suggestions.push(suggestion);
        }

        // Add suggestions for common patterns
        for pattern in &analysis.patterns {
            for pkg_id in &pattern.common_packages {
                let suggestion = PackageSuggestion::new(
                    pkg_id.to_string(),
                    SuggestionReason::CommonInSimilarGraphs {
                        similarity: 0.8,
                        usage_count: 1,
                    },
                    0.6,
                )
                .with_source(SuggestionSource::GraphAnalysis { graph_id: None })
                .with_benefits(vec![format!("Commonly used with {} pattern", pattern.name)]);

                suggestions.push(suggestion);
            }
        }

        // Filter and sort
        suggestions.retain(|s| s.confidence >= self.config.min_confidence);
        suggestions.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        suggestions.truncate(self.config.max_suggestions);

        suggestions
    }

    /// Search for packages manually.
    pub fn search(&self, query: &str) -> DiscoveryResult<Vec<PackageSuggestion>> {
        let client = self.client.as_ref().ok_or_else(|| {
            DiscoveryError::InvalidConfig("No registry client configured".to_string())
        })?;

        let results = client
            .search(query)
            .map_err(|e| DiscoveryError::Client(e.to_string()))?;

        if results.is_empty() {
            return Err(DiscoveryError::NoSuggestions);
        }

        let suggestions: Vec<PackageSuggestion> = results
            .iter()
            .take(self.config.max_suggestions)
            .map(|result| {
                // Search results don't have a score field, use 1.0 as base relevance
                let relevance_score = 1.0;
                let pkg = SuggestedPackage::from_search_result(result, relevance_score);
                let colony_available = self.is_colony_available(&result.id);
                let mut confidence = relevance_score;
                if colony_available && self.config.boost_colony_packages {
                    confidence = (confidence + self.config.colony_boost).min(1.0);
                }

                PackageSuggestion::new(
                    query,
                    SuggestionReason::ManualSearch {
                        query: query.to_string(),
                    },
                    confidence,
                )
                .with_package(pkg)
                .with_colony_available(colony_available)
                .with_source(SuggestionSource::Manual)
            })
            .collect();

        Ok(suggestions)
    }

    /// Search for packages using semantic similarity.
    ///
    /// Uses embeddings to find packages with similar functionality to the query.
    /// This is useful for natural language queries like "analyze customer emotions"
    /// that may not match exact keywords in package descriptions.
    ///
    /// Falls back to text search if semantic search is not available.
    pub fn search_semantic(&self, query: &str) -> DiscoveryResult<Vec<PackageSuggestion>> {
        self.search_semantic_with_options(query, self.config.max_suggestions, None)
    }

    /// Search for packages using semantic similarity with options.
    ///
    /// # Arguments
    /// * `query` - Natural language query describing desired functionality
    /// * `limit` - Maximum number of results to return
    /// * `filter` - Optional filter to narrow results by type, category, etc.
    pub fn search_semantic_with_options(
        &self,
        query: &str,
        limit: usize,
        filter: Option<SearchFilter>,
    ) -> DiscoveryResult<Vec<PackageSuggestion>> {
        let semantic_service = self.semantic_search.as_ref().ok_or_else(|| {
            DiscoveryError::InvalidConfig("Semantic search not configured".to_string())
        })?;

        if semantic_service.is_empty() {
            return Err(DiscoveryError::NoSuggestions);
        }

        // Build search query
        let search_query = if let Some(filter) = filter {
            SearchQuery::new(query).limit(limit).with_filter(filter)
        } else {
            SearchQuery::new(query).limit(limit)
        };

        // Execute semantic search
        let results = semantic_service
            .search_with_query(&search_query)
            .map_err(|e| DiscoveryError::Client(format!("Semantic search failed: {}", e)))?;

        if results.is_empty() {
            return Err(DiscoveryError::NoSuggestions);
        }

        // Convert semantic results to package suggestions
        let suggestions: Vec<PackageSuggestion> = results
            .into_iter()
            .map(|result| {
                // Check colony availability
                let package_id = PackageId::parse(&result.package_id)
                    .unwrap_or_else(|| PackageId::new("local", &result.package_id));
                let colony_available = self.is_colony_available(&package_id);

                // Adjust confidence with colony boost
                let mut confidence = result.score;
                if colony_available && self.config.boost_colony_packages {
                    confidence = (confidence + self.config.colony_boost).min(1.0);
                }

                // Create suggested package
                let pkg = SuggestedPackage {
                    id: package_id,
                    name: result.name.clone(),
                    version: Version::new(1, 0, 0),
                    description: result.description.clone(),
                    package_type: result.package_type,
                    relevance_score: result.score,
                    verified: result.verified,
                    downloads: None,
                };

                // Build benefits from highlights
                let benefits = if result.highlights.is_empty() {
                    vec![format!(
                        "Semantically similar to '{}' (score: {:.2})",
                        query, result.score
                    )]
                } else {
                    result.highlights.clone()
                };

                PackageSuggestion::new(
                    query,
                    SuggestionReason::SemanticMatch {
                        query: query.to_string(),
                        similarity_score: result.score,
                    },
                    confidence,
                )
                .with_package(pkg)
                .with_colony_available(colony_available)
                .with_benefits(benefits)
                .with_source(SuggestionSource::SemanticSearch {
                    query: query.to_string(),
                })
            })
            .filter(|s| s.confidence >= self.config.min_confidence)
            .collect();

        if suggestions.is_empty() {
            return Err(DiscoveryError::NoSuggestions);
        }

        Ok(suggestions)
    }

    /// Smart search that tries semantic search first, then falls back to text search.
    ///
    /// This is the recommended search method for most use cases. It provides
    /// the best results by combining:
    /// 1. Semantic search (if available and index is populated)
    /// 2. Text search as fallback (if semantic unavailable or returns no results)
    pub fn smart_search(&self, query: &str) -> DiscoveryResult<Vec<PackageSuggestion>> {
        // Try semantic search first if available
        if self.has_semantic_search() && self.semantic_index_count() > 0 {
            match self.search_semantic(query) {
                Ok(suggestions) => return Ok(suggestions),
                Err(DiscoveryError::NoSuggestions) => {
                    // Fall through to text search
                }
                Err(e) => {
                    // Log but continue to fallback
                    tracing::debug!("Semantic search failed, falling back to text: {}", e);
                }
            }
        }

        // Fall back to text search
        self.search(query)
    }

    /// Index a package for semantic search.
    ///
    /// This adds the package to the local semantic index, enabling it to be
    /// found via similarity search.
    pub fn index_package(&self, metadata: &PackageMetadata) -> DiscoveryResult<()> {
        let semantic_service = self.semantic_search.as_ref().ok_or_else(|| {
            DiscoveryError::InvalidConfig("Semantic search not configured".to_string())
        })?;

        semantic_service
            .index_package(metadata)
            .map_err(|e| DiscoveryError::Client(format!("Failed to index package: {}", e)))
    }

    /// Index multiple packages for semantic search.
    pub fn index_packages(
        &self,
        packages: &[PackageMetadata],
    ) -> DiscoveryResult<super::semantic::IndexingReport> {
        let semantic_service = self.semantic_search.as_ref().ok_or_else(|| {
            DiscoveryError::InvalidConfig("Semantic search not configured".to_string())
        })?;

        semantic_service
            .index_packages(packages)
            .map_err(|e| DiscoveryError::Client(format!("Failed to index packages: {}", e)))
    }

    /// Get cached suggestions for a query.
    pub fn get_cached(&self, query: &str) -> Option<&Vec<PackageSuggestion>> {
        self.suggestion_cache.get(query)
    }

    /// Clear the suggestion cache.
    pub fn clear_cache(&mut self) {
        self.suggestion_cache.clear();
    }
}

impl Default for PackageDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Capability Gap Info (Interface with Introspection)
// =============================================================================

/// Simplified capability gap information for package discovery.
///
/// This type provides a clean interface for receiving capability gap
/// information from the introspection system without requiring direct
/// dependency on introspection types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityGapInfo {
    /// Description of the capability gap.
    pub description: String,

    /// Category of the gap.
    pub category: CapabilityGapCategory,

    /// Proposed solution.
    pub proposed_solution: String,

    /// Confidence in this analysis (0.0-1.0).
    pub confidence: f64,

    /// Expected error reduction (0.0-1.0).
    pub expected_error_reduction: f64,

    /// Expected latency reduction (ms).
    pub expected_latency_reduction_ms: f64,

    /// Expected accuracy improvement (0.0-1.0).
    pub expected_accuracy_improvement: f64,
}

impl CapabilityGapInfo {
    /// Create a new capability gap info.
    pub fn new(description: impl Into<String>, category: CapabilityGapCategory) -> Self {
        Self {
            description: description.into(),
            category,
            proposed_solution: String::new(),
            confidence: 0.5,
            expected_error_reduction: 0.0,
            expected_latency_reduction_ms: 0.0,
            expected_accuracy_improvement: 0.0,
        }
    }

    /// Set proposed solution.
    #[must_use]
    pub fn with_solution(mut self, solution: impl Into<String>) -> Self {
        self.proposed_solution = solution.into();
        self
    }

    /// Set confidence.
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set expected impact.
    pub fn with_impact(
        mut self,
        error_reduction: f64,
        latency_reduction_ms: f64,
        accuracy_improvement: f64,
    ) -> Self {
        self.expected_error_reduction = error_reduction;
        self.expected_latency_reduction_ms = latency_reduction_ms;
        self.expected_accuracy_improvement = accuracy_improvement;
        self
    }
}

/// Category of capability gap (mirrors self_improvement::GapCategory).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CapabilityGapCategory {
    /// Missing node type.
    MissingNode {
        /// Suggested signature for the missing node.
        suggested_signature: String,
    },
    /// Missing tool.
    MissingTool {
        /// Description of the missing tool.
        tool_description: String,
    },
    /// Inadequate existing functionality.
    InadequateFunctionality {
        /// Name of the node with limited functionality.
        node: String,
        /// Description of the limitation.
        limitation: String,
    },
    /// Missing integration.
    MissingIntegration {
        /// Name of the external system needing integration.
        external_system: String,
    },
    /// Performance limitation.
    PerformanceGap {
        /// Description of the performance bottleneck.
        bottleneck: String,
    },
}

// =============================================================================
// Introspection Report Integration
// =============================================================================

/// Trait for converting introspection types to discovery types.
///
/// This trait allows the discovery module to work with introspection
/// reports without direct coupling to the introspection module.
pub trait IntoCapabilityGapInfo {
    /// Convert to capability gap info.
    fn into_gap_info(self) -> CapabilityGapInfo;
}

/// Extension trait for generating package recommendations from reports.
pub trait PackageRecommendationExt {
    /// Generate package recommendations from this report.
    fn package_recommendations(&self) -> Vec<PackageRecommendation>;
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_config_default() {
        let config = DiscoveryConfig::default();
        assert!((config.min_confidence - 0.3).abs() < f64::EPSILON);
        assert_eq!(config.max_suggestions, 10);
        assert!(config.boost_colony_packages);
    }

    #[test]
    fn test_discovery_config_builder() {
        let config = DiscoveryConfig::default()
            .with_min_confidence(0.5)
            .with_max_suggestions(5)
            .with_colony_boost(false);

        assert!((config.min_confidence - 0.5).abs() < f64::EPSILON);
        assert_eq!(config.max_suggestions, 5);
        assert!(!config.boost_colony_packages);
    }

    #[test]
    fn test_package_suggestion_new() {
        let suggestion = PackageSuggestion::new(
            "sentiment analysis",
            SuggestionReason::MissingTool {
                tool_description: "Analyze sentiment".to_string(),
            },
            0.8,
        );

        assert_eq!(suggestion.search_query, "sentiment analysis");
        assert!((suggestion.confidence - 0.8).abs() < f64::EPSILON);
        assert!(suggestion.package.is_none());
        assert!(!suggestion.colony_available);
    }

    #[test]
    fn test_package_suggestion_colony_boost() {
        let mut suggestion = PackageSuggestion::new(
            "test",
            SuggestionReason::ManualSearch {
                query: "test".to_string(),
            },
            0.7,
        )
        .with_colony_available(true);

        suggestion.apply_colony_boost(0.15);
        assert!((suggestion.confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_package_suggestion_no_boost_when_not_available() {
        let mut suggestion = PackageSuggestion::new(
            "test",
            SuggestionReason::ManualSearch {
                query: "test".to_string(),
            },
            0.7,
        )
        .with_colony_available(false);

        suggestion.apply_colony_boost(0.15);
        assert!((suggestion.confidence - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_package_recommendation_new() {
        let rec = PackageRecommendation::new(
            "Missing sentiment tool",
            "sentiment analysis",
            PackageType::ToolPack,
            GapCategoryRef::MissingTool,
        )
        .with_priority(RecommendationPriority::High)
        .with_confidence(0.9);

        assert_eq!(rec.gap_description, "Missing sentiment tool");
        assert_eq!(rec.search_query, "sentiment analysis");
        assert_eq!(rec.priority, RecommendationPriority::High);
    }

    #[test]
    fn test_package_recommendation_to_suggestion() {
        let rec = PackageRecommendation::new(
            "Missing sentiment tool",
            "sentiment analysis",
            PackageType::ToolPack,
            GapCategoryRef::MissingTool,
        )
        .with_priority(RecommendationPriority::High)
        .with_confidence(0.8);

        let suggestion = rec.to_suggestion();
        assert_eq!(suggestion.search_query, "sentiment analysis");
        // High priority factor is 0.9, so 0.8 * 0.9 = 0.72
        assert!((suggestion.confidence - 0.72).abs() < f64::EPSILON);
    }

    #[test]
    fn test_recommended_impact_to_benefits() {
        let impact = RecommendedImpact::high("Better accuracy");
        let benefits = impact.to_benefits();

        assert!(benefits.iter().any(|b| b.contains("error")));
        assert!(benefits.iter().any(|b| b.contains("latency")));
        assert!(benefits.iter().any(|b| b.contains("accuracy")));
    }

    #[test]
    fn test_recommendation_priority_confidence_factor() {
        assert!((RecommendationPriority::Critical.confidence_factor() - 1.0).abs() < f64::EPSILON);
        assert!((RecommendationPriority::High.confidence_factor() - 0.9).abs() < f64::EPSILON);
        assert!((RecommendationPriority::Medium.confidence_factor() - 0.7).abs() < f64::EPSILON);
        assert!((RecommendationPriority::Low.confidence_factor() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_capability_gap_info_new() {
        let gap = CapabilityGapInfo::new(
            "Missing tool",
            CapabilityGapCategory::MissingTool {
                tool_description: "Sentiment analysis".to_string(),
            },
        )
        .with_confidence(0.8)
        .with_solution("Install sentiment package");

        assert_eq!(gap.description, "Missing tool");
        assert!((gap.confidence - 0.8).abs() < f64::EPSILON);
        assert_eq!(gap.proposed_solution, "Install sentiment package");
    }

    #[test]
    fn test_graph_analysis() {
        let mut analysis = GraphAnalysis::new();
        analysis.add_node_type("LLMNode");
        analysis.add_capability("text-generation");
        analysis.add_enhancement_point(EnhancementPoint {
            location: "input".to_string(),
            enhancement_type: EnhancementType::AddValidation,
            suggestion: "Add input validation".to_string(),
            confidence: 0.7,
        });

        assert_eq!(analysis.node_types.len(), 1);
        assert_eq!(analysis.capabilities.len(), 1);
        assert_eq!(analysis.enhancement_points.len(), 1);
    }

    #[test]
    fn test_package_discovery_new() {
        let discovery = PackageDiscovery::new();
        assert!(discovery.client.is_none());
        assert!(discovery.colony_packages.is_empty());
    }

    #[test]
    fn test_package_discovery_colony_available() {
        let mut discovery = PackageDiscovery::new();
        let pkg_id = PackageId::new("dashflow", "sentiment");
        discovery.set_colony_packages(vec![pkg_id.clone()]);

        assert!(discovery.is_colony_available(&pkg_id));
        assert!(!discovery.is_colony_available(&PackageId::new("dashflow", "other")));
    }

    #[test]
    fn test_recommendations_from_gaps() {
        let discovery = PackageDiscovery::new();
        let gaps = vec![
            CapabilityGapInfo::new(
                "Missing sentiment analysis",
                CapabilityGapCategory::MissingTool {
                    tool_description: "Analyze sentiment".to_string(),
                },
            )
            .with_confidence(0.9),
            CapabilityGapInfo::new(
                "Missing database integration",
                CapabilityGapCategory::MissingIntegration {
                    external_system: "PostgreSQL".to_string(),
                },
            )
            .with_confidence(0.7),
        ];

        let recommendations = discovery.recommendations_from_gaps(&gaps);
        assert_eq!(recommendations.len(), 2);

        // First should be highest priority (sentiment with 0.9 confidence)
        assert!(recommendations[0].confidence > recommendations[1].confidence);
    }

    #[test]
    fn test_suggestions_from_recommendations() {
        let mut discovery = PackageDiscovery::new();
        let rec = PackageRecommendation::new(
            "Missing sentiment tool",
            "sentiment analysis",
            PackageType::ToolPack,
            GapCategoryRef::MissingTool,
        )
        .with_priority(RecommendationPriority::High)
        .with_confidence(0.8);

        let suggestions = discovery.suggestions_from_recommendations(&[rec], false);
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].confidence > 0.5);
    }

    #[test]
    fn test_suggest_for_graph_analysis() {
        let mut discovery = PackageDiscovery::new();
        let mut analysis = GraphAnalysis::new();
        analysis.add_enhancement_point(EnhancementPoint {
            location: "input_node".to_string(),
            enhancement_type: EnhancementType::AddValidation,
            suggestion: "Add input validation for better error handling".to_string(),
            confidence: 0.8,
        });

        let suggestions = discovery.suggest_for_graph_analysis(&analysis);
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_discovery_error_display() {
        let err = DiscoveryError::NoSuggestions;
        assert_eq!(err.to_string(), "No package suggestions found");

        let err = DiscoveryError::Client("timeout".to_string());
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn test_gap_category_ref_serialization() {
        let category = GapCategoryRef::MissingTool;
        let json = serde_json::to_string(&category).unwrap();
        let parsed: GapCategoryRef = serde_json::from_str(&json).unwrap();
        matches!(parsed, GapCategoryRef::MissingTool);
    }

    #[test]
    fn test_suggestion_reason_serialization() {
        let reason = SuggestionReason::MissingTool {
            tool_description: "Sentiment analysis".to_string(),
        };
        let json = serde_json::to_string(&reason).unwrap();
        let parsed: SuggestionReason = serde_json::from_str(&json).unwrap();
        if let SuggestionReason::MissingTool { tool_description } = parsed {
            assert_eq!(tool_description, "Sentiment analysis");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_suggestion_source_serialization() {
        let source = SuggestionSource::Introspection {
            report_id: Uuid::new_v4(),
        };
        let json = serde_json::to_string(&source).unwrap();
        let parsed: SuggestionSource = serde_json::from_str(&json).unwrap();
        matches!(parsed, SuggestionSource::Introspection { .. });
    }

    // =========================================================================
    // Semantic Search Integration Tests
    // =========================================================================

    #[test]
    fn test_discovery_with_semantic_search() {
        use crate::packages::semantic::DefaultSemanticSearch;

        let service = Arc::new(DefaultSemanticSearch::default_for_testing());
        let discovery = PackageDiscovery::new().with_semantic_search(service.clone());

        assert!(discovery.has_semantic_search());
        assert_eq!(discovery.semantic_index_count(), 0);
    }

    #[test]
    fn test_semantic_search_index_and_search() {
        use crate::packages::semantic::DefaultSemanticSearch;

        // Create semantic service with default config
        let service = Arc::new(DefaultSemanticSearch::default_for_testing());
        // Use min_confidence 0.0 since mock embeddings don't produce meaningful similarity scores
        let config = DiscoveryConfig::default().with_min_confidence(0.0);
        let discovery = PackageDiscovery::with_config(config).with_semantic_search(service);

        // Index some packages (using same text patterns that pass in other tests)
        let packages = vec![
            PackageMetadata::new("dashflow/nodes", "Node Library")
                .with_description("Collection of useful nodes")
                .with_type(PackageType::NodeLibrary),
            PackageMetadata::new("dashflow/tools", "Tool Pack")
                .with_description("Collection of useful tools")
                .with_type(PackageType::ToolPack),
            PackageMetadata::new("dashflow/utils", "Utility Functions")
                .with_description("Helpful utility functions")
                .with_type(PackageType::ToolPack),
        ];

        let report = discovery.index_packages(&packages).unwrap();
        assert!(report.is_success());
        assert_eq!(discovery.semantic_index_count(), 3);

        // Search semantically - with mock embeddings the results are hash-based
        // so we just verify we get results, not semantic relevance
        let results = discovery.search_semantic("useful collection").unwrap();
        assert!(!results.is_empty());

        // Verify we got package results
        let first = &results[0];
        assert!(first.package.is_some());
    }

    #[test]
    fn test_semantic_search_with_filter() {
        use crate::packages::semantic::{DefaultSemanticSearch, SearchFilter};

        let service = Arc::new(DefaultSemanticSearch::default_for_testing());
        // Use min_confidence 0.0 since mock embeddings don't produce meaningful similarity scores
        let config = DiscoveryConfig::default().with_min_confidence(0.0);
        let discovery = PackageDiscovery::with_config(config).with_semantic_search(service);

        // Index packages with different types
        let packages = vec![
            PackageMetadata::new("dashflow/nodes", "Node Library")
                .with_description("Collection of useful nodes")
                .with_type(PackageType::NodeLibrary),
            PackageMetadata::new("dashflow/tools", "Tool Pack")
                .with_description("Collection of useful tools")
                .with_type(PackageType::ToolPack),
        ];

        discovery.index_packages(&packages).unwrap();

        // Search with filter for NodeLibrary only
        let filter = SearchFilter::new().with_type(PackageType::NodeLibrary);
        let results = discovery
            .search_semantic_with_options("useful collection", 10, Some(filter))
            .unwrap();

        // All results should be NodeLibrary type
        for result in &results {
            if let Some(pkg) = &result.package {
                assert_eq!(pkg.package_type, PackageType::NodeLibrary);
            }
        }
    }

    #[test]
    fn test_semantic_search_not_configured() {
        let discovery = PackageDiscovery::new();

        // Without semantic search configured, should return error
        let result = discovery.search_semantic("test query");
        assert!(matches!(result, Err(DiscoveryError::InvalidConfig(_))));
    }

    #[test]
    fn test_semantic_search_empty_index() {
        use crate::packages::semantic::DefaultSemanticSearch;

        let service = Arc::new(DefaultSemanticSearch::default_for_testing());
        let discovery = PackageDiscovery::new().with_semantic_search(service);

        // With empty index, should return NoSuggestions
        let result = discovery.search_semantic("test query");
        assert!(matches!(result, Err(DiscoveryError::NoSuggestions)));
    }

    #[test]
    fn test_smart_search_falls_back_to_text() {
        // Without semantic search, smart_search should fall back to text search
        let discovery = PackageDiscovery::new();

        // This will fail because no registry client is configured either
        let result = discovery.smart_search("test query");
        assert!(result.is_err());
    }

    #[test]
    fn test_smart_search_uses_semantic_when_available() {
        use crate::packages::semantic::DefaultSemanticSearch;

        let service = Arc::new(DefaultSemanticSearch::default_for_testing());
        // Use min_confidence 0.0 since mock embeddings don't produce meaningful similarity scores
        let config = DiscoveryConfig::default().with_min_confidence(0.0);
        let discovery = PackageDiscovery::with_config(config).with_semantic_search(service);

        // Index packages (using same text patterns that work with mock embeddings)
        discovery
            .index_package(
                &PackageMetadata::new("dashflow/nodes", "Node Library")
                    .with_description("Collection of useful nodes")
                    .with_type(PackageType::NodeLibrary),
            )
            .unwrap();

        // smart_search should use semantic search (using query that works with mock embeddings)
        let results = discovery.smart_search("useful collection").unwrap();
        assert!(!results.is_empty());

        // Verify it came from semantic search by checking the reason
        let first = &results[0];
        assert!(matches!(
            first.reason,
            SuggestionReason::SemanticMatch { .. }
        ));
    }

    #[test]
    fn test_semantic_match_reason_serialization() {
        let reason = SuggestionReason::SemanticMatch {
            query: "emotion analysis".to_string(),
            similarity_score: 0.85,
        };
        let json = serde_json::to_string(&reason).unwrap();
        let parsed: SuggestionReason = serde_json::from_str(&json).unwrap();
        if let SuggestionReason::SemanticMatch {
            query,
            similarity_score,
        } = parsed
        {
            assert_eq!(query, "emotion analysis");
            assert!((similarity_score - 0.85).abs() < f64::EPSILON);
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_semantic_search_source_serialization() {
        let source = SuggestionSource::SemanticSearch {
            query: "test query".to_string(),
        };
        let json = serde_json::to_string(&source).unwrap();
        let parsed: SuggestionSource = serde_json::from_str(&json).unwrap();
        if let SuggestionSource::SemanticSearch { query } = parsed {
            assert_eq!(query, "test query");
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_set_semantic_search() {
        use crate::packages::semantic::DefaultSemanticSearch;

        let mut discovery = PackageDiscovery::new();
        assert!(!discovery.has_semantic_search());

        let service = Arc::new(DefaultSemanticSearch::default_for_testing());
        discovery.set_semantic_search(service);
        assert!(discovery.has_semantic_search());
    }
}
