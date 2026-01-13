// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @dashflow-module
//! @name packages
//! @category runtime
//! @status stable
//!
//! # DashFlow Package Ecosystem
//!
//! This module implements the DashFlow package ecosystem, enabling AI agents to discover,
//! install, share, and contribute packages. The design follows DESIGN_PACKAGE_ECOSYSTEM.md.
//!
//! ## Package Types
//!
//! - **Graph Templates**: Pre-built graph architectures for common use cases
//! - **Node Libraries**: Collections of specialized nodes
//! - **Tool Packs**: Sets of tools for specific domains
//! - **Checkpointer Backends**: Storage implementations for state persistence
//! - **Model Connectors**: LLM provider integrations
//! - **Prompt Libraries**: Curated prompt collections
//!
//! ## Registry Hierarchy
//!
//! 1. Local registry (`~/.dashflow/packages/`)
//! 2. Team/git registries
//! 3. Central registry (dashswarm.com)
//! 4. Colony peers (P2P sharing)
//!
//! ## Example - Local Registry
//!
//! ```rust,ignore
//! use dashflow::packages::{LocalRegistry, PackageManifest, Author, PackageType};
//!
//! // Create a local registry
//! let mut registry = LocalRegistry::new("~/.dashflow/packages")?;
//!
//! // Search for packages
//! let results = registry.search("sentiment analysis");
//!
//! // List installed packages
//! for pkg in registry.list_installed() {
//!     println!("{} v{}", pkg.id, pkg.version);
//! }
//! ```
//!
//! ## Example - HTTP Registry Client
//!
//! ```rust,ignore
//! use dashflow::packages::{RegistryClient, RegistryClientConfig, SearchOptions, PackageType};
//!
//! // Create a client for the central registry
//! let client = RegistryClient::new(RegistryClientConfig::official())?;
//!
//! // Search for packages
//! let results = client.search("sentiment analysis")?;
//!
//! // Search with filters
//! let options = SearchOptions::new("nlp")
//!     .with_type(PackageType::NodeLibrary)
//!     .verified()
//!     .limit(10);
//! let filtered = client.search_with_options(&options)?;
//!
//! // Get package info
//! let info = client.get_package("dashflow/sentiment-analysis")?;
//!
//! // Download a package
//! let download = client.download("dashflow/sentiment-analysis", "1.2.0")?;
//! if download.verify() {
//!     // Package integrity verified
//! }
//! ```
//!
//! ## Example - Multi-Registry Configuration
//!
//! ```rust,ignore
//! use dashflow::packages::{RegistryConfig, RegistrySource};
//!
//! // Load configuration from ~/.dashflow/registries.toml
//! let config = RegistryConfig::load_default()?;
//!
//! // Or create a custom configuration
//! let mut config = RegistryConfig::default();
//! config.insert_registry(0, RegistrySource::http("https://company.com/packages"));
//! ```
//!
//! ## Implementation Phases
//!
//!: Package manifest, local registry, basic types
//!: Central registry client, search, download
//!: Trust system: signatures, verification, keys
//!: AI discovery: introspection integration, suggestions
//!: Colony sharing: P2P package distribution
//!: Contributions: bug reports, improvements, requests
//!: Semantic search, dashswarm.com API

mod cache;
mod client;
mod config;
mod contributions;
mod dashswarm;
mod discovery;
mod manifest;
mod prompts;
mod registry;
mod semantic;
mod sharing;
mod trust;
mod types;

// Re-export manifest types
pub use manifest::{
    AuditStatus, Author, Capability, Contributor, Dependency, DerivationStep, Lineage, Maintainer,
    OptionalDependency, PackageManifest, PackageManifestBuilder, PackageRef, Permission,
};

// Re-export core types
pub use types::{
    HashAlgorithm, PackageId, PackageType, Signature, SignatureAlgorithm, SignedContent,
    TrustLevel, Version, VersionOp, VersionReq,
};

// Re-export local registry
pub use registry::{
    InstalledPackage, LocalRegistry, OutdatedPackage, PackageEntry, PackageIndex, RegistryError,
    RegistryResult,
};

// Re-export HTTP client
pub use client::{
    AdvisorySeverity, ClientError, ClientResult, HttpAuth, PackageDownload, PackageInfo,
    PackageSearchResult, PackageVersionInfo, RegistryClient, RegistryClientConfig, SearchOptions,
    SecurityAdvisory, SemanticSearchResult, SortOrder, VersionInfo,
};

// Re-export configuration
pub use config::{
    CacheConfig, ConfigError, GitAuth, RegistryConfig, RegistrySource, RequiredSignatures,
    TrustConfig,
};

// Re-export package cache (M-200)
pub use cache::{
    CacheEntry, CacheError, CacheIndex, CacheResult, CacheStats, CachedMetadata, PackageCache,
};

// Re-export trust system
pub use trust::{
    compute_key_fingerprint, generate_ecdsa_p256_keypair, generate_ed25519_keypair, Hasher,
    KeyStore, PackageSigner, PackageVerifier, TrustError, TrustResult, TrustedKey,
    VerificationResult,
};

// Re-export discovery system
pub use discovery::{
    CapabilityGapCategory,
    // Integration types
    CapabilityGapInfo,
    DiscoveryConfig,
    DiscoveryError,
    DiscoveryResult,
    EnhancementPoint,
    EnhancementType,
    GapCategoryRef,
    // Graph analysis types
    GraphAnalysis,
    GraphPattern,
    IntoCapabilityGapInfo,
    // Discovery service
    PackageDiscovery,
    // Recommendation types
    PackageRecommendation,
    PackageRecommendationExt,
    // Suggestion types
    PackageSuggestion,
    RecommendationPriority,
    RecommendedImpact,
    SuggestedPackage,
    SuggestionReason,
    SuggestionSource,
};

// Re-export sharing system
pub use sharing::{
    ColonyPackageEntry,
    // Registry
    ColonyPackageRegistry,
    ColonyPackageSource,
    ColonyPackageStats,
    PackageAdvertisement,
    PackageMessage,
    // Request/response
    PackageRequest,
    PackageResponse,
    PackageSharingPolicy,
    // Shared package types
    SharedPackage,
    // Errors
    SharingError,
    SharingResult,
    TransferMethod,
    PACKAGES_BROADCAST_INTERVAL,
    // Channel constants
    PACKAGES_CHANNEL,
};

// Re-export contributions system
pub use contributions::{
    BugSeverity,
    // Contribution wrapper
    Contribution,
    // Client
    ContributionClient,
    ContributionClientConfig,
    // Error types
    ContributionError,
    // Package reference for contributions
    ContributionPackageRef,
    ContributionResult,
    ContributionState,
    ContributionStatus,
    DiscoveryMethod,
    // Evidence
    Evidence,
    ExpectedImpact,
    ImpactLevel,
    ImpactMetric,
    Implementation,
    ImprovementPriority,
    ImprovementType,
    NewFile,
    // Package request types
    NewPackageRequest,
    // Bug report types
    PackageBugReport,
    // Fix types
    PackageFix,
    // Improvement types
    PackageImprovement,
    // Identity
    ReporterIdentity,
    ReproductionSteps,
    RequestPriority,
    ReviewerComment,
    SimilarPackage,
    SuggestedFix,
    TestCase,
};

// Re-export semantic search system
pub use semantic::{
    DefaultSemanticSearch,
    // Embedding types
    Embedding,
    EmbeddingModel,
    InMemoryVectorDb,
    IndexingReport,
    MockEmbeddingModel,
    // Package metadata for indexing
    PackageMetadata,
    // Search filter
    SearchFilter,
    // Search query and result
    SearchQuery,
    SearchResult,
    // Error types
    SemanticError,
    SemanticResult,
    SemanticSearchConfig,
    // Search service
    SemanticSearchService,
    VectorDb,
    // Vector database types
    VectorEntry,
    VectorSearchResult,
};

// Re-export prompt types (Package→Config)
pub use prompts::{
    // Utility
    parse_prompt_id,
    // Prompt template type
    PackagePromptTemplate,
    // Error types
    PromptError,
    // Prompt library
    PromptLibrary,
    PromptResult,
    PromptTestCase,
    // Supporting types
    VariableDefinition,
};

// Re-export DashSwarm API client
pub use dashswarm::{
    // Configuration
    DashSwarmAuth,
    // Client
    DashSwarmClient,
    DashSwarmConfig,
    // Error types
    DashSwarmError,
    DashSwarmResult,
    // Response types
    KeyVerificationResponse,
    PublicKeyInfo,
    PublishRequest,
    PublishResponse,
    SignatureData,
    SubmissionResponse,
    // Configuration constants (M-207: Make placeholder URL discoverable)
    DASHSWARM_DEFAULT_URL,
    DASHSWARM_REGISTRY_URL_ENV,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify core types are exported
        let _id: PackageId = PackageId::new("dashflow", "test");
        assert_eq!(_id.namespace(), "dashflow");
        assert_eq!(_id.name(), "test");
    }

    #[test]
    fn test_package_type_variants() {
        let types = vec![
            PackageType::GraphTemplate,
            PackageType::NodeLibrary,
            PackageType::ToolPack,
            PackageType::CheckpointerBackend,
            PackageType::ModelConnector,
            PackageType::PromptLibrary,
        ];
        assert_eq!(types.len(), 6);
    }

    #[test]
    fn test_client_types_exported() {
        // Verify client types are accessible
        let _config = RegistryClientConfig::default();
        assert!(RegistryClientConfig::official().official);
    }

    #[test]
    fn test_config_types_exported() {
        // Verify config types are accessible
        let config = RegistryConfig::default();
        assert!(!config.registries.is_empty());
    }

    #[test]
    fn test_search_options_exported() {
        let options = SearchOptions::new("test")
            .with_type(PackageType::NodeLibrary)
            .verified();
        assert_eq!(options.query, Some("test".to_string()));
    }

    #[test]
    fn test_version_req_exported() {
        let req = VersionReq::caret(Version::new(1, 0, 0));
        assert!(req.matches(&Version::new(1, 5, 0)));
        assert!(!req.matches(&Version::new(2, 0, 0)));
    }

    #[test]
    fn test_trust_types_exported() {
        // Verify trust system types are accessible
        let store = KeyStore::new();
        assert!(store.is_empty());

        // Hash function
        let hash = Hasher::hash_bytes(b"test", HashAlgorithm::Sha256);
        assert_eq!(hash.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
    }

    #[test]
    fn test_trust_keypair_generation() {
        // Can generate Ed25519 keypairs
        let (private, public) = generate_ed25519_keypair().unwrap();
        assert!(private.contains("PRIVATE KEY"));
        assert!(public.contains("PUBLIC KEY"));

        // Can generate ECDSA keypairs
        let (private, public) = generate_ecdsa_p256_keypair().unwrap();
        assert!(private.contains("PRIVATE KEY"));
        assert!(public.contains("PUBLIC KEY"));
    }

    #[test]
    fn test_discovery_types_exported() {
        // Verify discovery types are accessible
        let config = DiscoveryConfig::default();
        assert!(config.min_confidence > 0.0);
        assert!(config.max_suggestions > 0);

        // Create a discovery service
        let discovery = PackageDiscovery::new();
        assert!(!discovery.is_colony_available(&PackageId::new("test", "pkg")));
    }

    #[test]
    fn test_suggestion_types_exported() {
        // Verify suggestion types are accessible
        let suggestion = PackageSuggestion::new(
            "test query",
            SuggestionReason::MissingTool {
                tool_description: "A test tool".to_string(),
            },
            0.8,
        );
        assert_eq!(suggestion.search_query, "test query");
        assert!((suggestion.confidence - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_recommendation_types_exported() {
        // Verify recommendation types are accessible
        let rec = PackageRecommendation::new(
            "Test gap",
            "test search",
            PackageType::ToolPack,
            GapCategoryRef::MissingTool,
        )
        .with_priority(RecommendationPriority::High);

        assert_eq!(rec.gap_description, "Test gap");
        assert_eq!(rec.priority, RecommendationPriority::High);
    }

    #[test]
    fn test_graph_analysis_types_exported() {
        // Verify graph analysis types are accessible
        let mut analysis = GraphAnalysis::new();
        analysis.add_node_type("TestNode");
        analysis.add_enhancement_point(EnhancementPoint {
            location: "input".to_string(),
            enhancement_type: EnhancementType::AddValidation,
            suggestion: "Add validation".to_string(),
            confidence: 0.7,
        });

        assert_eq!(analysis.node_types.len(), 1);
        assert_eq!(analysis.enhancement_points.len(), 1);
    }

    #[test]
    fn test_capability_gap_info_exported() {
        // Verify capability gap info types are accessible
        let gap = CapabilityGapInfo::new(
            "Missing tool",
            CapabilityGapCategory::MissingTool {
                tool_description: "A missing tool".to_string(),
            },
        )
        .with_confidence(0.9);

        assert_eq!(gap.description, "Missing tool");
        assert!((gap.confidence - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sharing_types_exported() {
        // Verify sharing types are accessible
        let registry = ColonyPackageRegistry::new();
        let stats = registry.stats();
        assert_eq!(stats.local_packages, 0);

        // Create a shared package
        let pkg = SharedPackage::new(PackageId::new("dashflow", "test"), Version::new(1, 0, 0))
            .shareable(true);
        assert!(pkg.shareable);

        // Create an advertisement
        let peer_id = uuid::Uuid::new_v4();
        let advert = PackageAdvertisement::new(peer_id, "TestApp")
            .with_package(pkg)
            .with_policy(PackageSharingPolicy::ShareAll);
        assert!(!advert.is_expired());

        // Check channel constant
        assert_eq!(PACKAGES_CHANNEL, "_packages");
    }

    #[test]
    fn test_sharing_policy_exported() {
        let pkg_id = PackageId::new("dashflow", "test");

        // Test different policies
        assert!(PackageSharingPolicy::ShareAll.should_share(&pkg_id, true));
        assert!(PackageSharingPolicy::ShareOfficial.should_share(&pkg_id, true));
        assert!(!PackageSharingPolicy::NoSharing.should_share(&pkg_id, true));
    }

    #[test]
    fn test_package_request_response_exported() {
        let requester = uuid::Uuid::new_v4();
        let request = PackageRequest::new(
            requester,
            "RequesterApp",
            PackageId::new("dashflow", "test"),
            Version::new(1, 0, 0),
        );
        assert_eq!(request.requester, requester);

        // Test responses
        let ref_response = PackageResponse::reference("https://example.com/pkg.tar.gz");
        assert!(ref_response.is_success());

        let denied = PackageResponse::denied("Not allowed");
        assert!(!denied.is_success());
    }

    #[test]
    fn test_colony_package_source_exported() {
        let peer_id = uuid::Uuid::new_v4();
        let source = ColonyPackageSource::new(
            peer_id,
            "PeerApp",
            PackageId::new("dashflow", "test"),
            Version::new(1, 0, 0),
        )
        .with_transfer_method(TransferMethod::DirectTransfer)
        .with_latency(100);

        assert!(source.supports_direct_transfer());
        assert_eq!(source.latency_ms, Some(100));
    }

    #[test]
    fn test_contribution_types_exported() {
        // Verify contribution types are accessible
        let reporter = ReporterIdentity::ai("TestApp", None);
        assert!(reporter.is_ai);
        assert_eq!(reporter.app_name, "TestApp");

        // Create a package reference
        let pkg_ref = ContributionPackageRef::new("dashflow/test", "1.0.0");
        assert_eq!(pkg_ref.id.namespace(), "dashflow");

        // Create evidence
        let evidence = Evidence::citation("log", "Error message");
        assert!(matches!(evidence, Evidence::Citation { .. }));
    }

    #[test]
    fn test_bug_report_exported() {
        let pkg = ContributionPackageRef::new("dashflow/test", "1.0.0");
        let reporter = ReporterIdentity::ai("TestApp", None);

        let report = PackageBugReport::new(pkg, reporter)
            .with_title("Test bug")
            .with_description("Bug description")
            .with_severity(BugSeverity::High);

        assert_eq!(report.title, "Test bug");
        assert_eq!(report.severity, BugSeverity::High);
        assert!(report.validate().is_ok());
    }

    #[test]
    fn test_improvement_exported() {
        let pkg = ContributionPackageRef::new("dashflow/test", "1.0.0");
        let suggester = ReporterIdentity::ai("TestApp", None);

        let improvement = PackageImprovement::new(pkg, suggester)
            .with_title("Test improvement")
            .with_description("Improvement description")
            .with_priority(ImprovementPriority::High);

        assert_eq!(improvement.title, "Test improvement");
        assert_eq!(improvement.priority, ImprovementPriority::High);
        assert!(improvement.validate().is_ok());
    }

    #[test]
    fn test_new_package_request_exported() {
        let requester = ReporterIdentity::ai("TestApp", None);

        let request = NewPackageRequest::new(requester, "New functionality")
            .with_title("New package request")
            .with_use_case("Use case description")
            .with_priority(RequestPriority::High);

        assert_eq!(request.title, "New package request");
        assert_eq!(request.priority, RequestPriority::High);
        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_contribution_client_exported() {
        let reporter = ReporterIdentity::ai("TestApp", None);
        let client = ContributionClient::official(reporter);

        assert!(client.registry_url().contains("dashswarm.com"));
        assert!(client.submitted_ids().is_empty());
    }

    #[test]
    fn test_semantic_types_exported() {
        // Verify semantic search types are accessible
        let model = MockEmbeddingModel::new(384);
        assert_eq!(model.dimensions(), 384);
        assert_eq!(model.model_name(), "mock-embedding-model");

        // Create embedding
        let embedding: Embedding = model.embed("test query").unwrap();
        assert_eq!(embedding.len(), 384);
    }

    #[test]
    fn test_vector_db_exported() {
        // Create in-memory vector database
        let db = InMemoryVectorDb::new();
        assert!(db.is_empty());

        // Insert and search
        let entry = VectorEntry {
            id: "test/pkg".to_string(),
            vector: vec![1.0, 0.0, 0.0],
            metadata: PackageMetadata::new("test/pkg", "Test Package"),
        };
        db.upsert(entry).unwrap();
        assert_eq!(db.len(), 1);
    }

    #[test]
    fn test_semantic_search_service_exported() {
        // Create default service for testing
        let service = DefaultSemanticSearch::default_for_testing();
        assert!(service.is_empty());
        assert_eq!(service.dimensions(), 384);

        // Index a package
        let metadata = PackageMetadata::new("dashflow/sentiment", "Sentiment Analysis")
            .with_description("Production-grade sentiment analysis")
            .with_keywords(vec!["sentiment", "nlp", "text"]);
        service.index_package(&metadata).unwrap();
        assert_eq!(service.indexed_count(), 1);
    }

    #[test]
    fn test_search_query_exported() {
        let query = SearchQuery::new("sentiment analysis")
            .limit(20)
            .min_score(0.5)
            .with_type(PackageType::NodeLibrary)
            .verified();

        assert_eq!(query.query, "sentiment analysis");
        assert_eq!(query.limit, 20);
        assert!(query.filter.verified_only);
    }

    #[test]
    fn test_search_filter_exported() {
        let filter = SearchFilter::new()
            .with_type(PackageType::NodeLibrary)
            .with_category("nlp")
            .with_min_trust(TrustLevel::Verified)
            .verified();

        let matching = PackageMetadata::new("test/pkg", "Test")
            .with_type(PackageType::NodeLibrary)
            .with_categories(vec!["nlp"])
            .with_trust_level(TrustLevel::Official)
            .verified(true);

        assert!(filter.matches(&matching));
    }

    #[test]
    fn test_indexing_report_exported() {
        let report = IndexingReport {
            total: 100,
            indexed: 100,
            failed: 0,
            errors: Vec::new(),
        };
        assert!(report.is_success());
        assert!((report.success_rate() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_prompt_types_exported() {
        // Verify prompt types are accessible
        let prompt = PackagePromptTemplate::new("analyzer", "You are a sentiment analyzer.")
            .with_description("Analyzes sentiment in text")
            .with_user_template("Analyze: {text}")
            .with_temperature(0.7)
            .with_max_tokens(1000);

        assert_eq!(prompt.name, "analyzer");
        assert_eq!(prompt.recommended_temperature, Some(0.7));
    }

    #[test]
    fn test_prompt_to_node_config_exported() {
        let prompt = PackagePromptTemplate::new("test", "System prompt")
            .with_temperature(0.5)
            .with_max_tokens(500);

        let config = prompt.to_node_config();

        assert_eq!(config["system_prompt"], "System prompt");
        assert_eq!(config["temperature"], 0.5);
        assert_eq!(config["max_tokens"], 500);
    }

    #[test]
    fn test_prompt_library_exported() {
        let lib = PromptLibrary::new()
            .with_description("Test prompts")
            .with_prompt(PackagePromptTemplate::new("v1", "System v1"))
            .with_variable(VariableDefinition::new("text"));

        assert_eq!(lib.len(), 1);
        assert!(!lib.is_empty());
        assert!(lib.get_prompt("v1").is_some());
    }

    #[test]
    fn test_parse_prompt_id_exported() {
        // Qualified ID
        let (pkg, prompt) = parse_prompt_id("pkg/prompt").unwrap();
        assert_eq!(pkg, Some("pkg".to_string()));
        assert_eq!(prompt, "prompt");

        // Unqualified ID
        let (pkg, prompt) = parse_prompt_id("prompt").unwrap();
        assert!(pkg.is_none());
        assert_eq!(prompt, "prompt");
    }

    #[test]
    fn test_dashswarm_config_exported() {
        // Verify DashSwarm types are accessible
        let config = DashSwarmConfig::default();
        assert!(config.base_url.contains("dashswarm.com"));
        assert!(config.auth.is_none());
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_dashswarm_config_with_auth_exported() {
        let config = DashSwarmConfig::official()
            .with_token("test-token")
            .with_timeout(60)
            .with_max_retries(5);

        assert!(matches!(config.auth, Some(DashSwarmAuth::Bearer { .. })));
        assert_eq!(config.timeout_secs, 60);
        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn test_dashswarm_error_types_exported() {
        // Verify error types are accessible
        let err = DashSwarmError::RateLimited {
            retry_after_secs: Some(60),
            message: "Too many requests".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("Rate limited"));

        let err = DashSwarmError::AuthError("Token expired".to_string());
        assert!(matches!(err, DashSwarmError::AuthError(_)));
    }

    #[test]
    fn test_dashswarm_response_types_exported() {
        use uuid::Uuid;

        // SubmissionResponse
        let response = SubmissionResponse {
            contribution_id: Uuid::new_v4(),
            message: "Success".to_string(),
            status: ContributionState::Pending,
        };
        assert_eq!(response.message, "Success");

        // KeyVerificationResponse
        let key_response = KeyVerificationResponse {
            valid: true,
            key_id: Some("key-123".to_string()),
            trust_level: Some("official".to_string()),
            message: "Verified".to_string(),
        };
        assert!(key_response.valid);

        // PublicKeyInfo
        let key_info = PublicKeyInfo {
            key_id: "dashflow-official".to_string(),
            public_key: "key".to_string(),
            owner: "DashFlow".to_string(),
            trust_level: "official".to_string(),
            algorithm: "Ed25519".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            expires_at: None,
            revoked: false,
        };
        assert_eq!(key_info.key_id, "dashflow-official");
    }

    #[test]
    fn test_dashswarm_publish_types_exported() {
        // PublishRequest
        let request = PublishRequest {
            manifest: "[package]".to_string(),
            tarball: "base64data".to_string(),
            hash: "sha256hash".to_string(),
            hash_algorithm: HashAlgorithm::Sha256,
            signature: None,
        };
        assert_eq!(request.hash_algorithm, HashAlgorithm::Sha256);

        // SignatureData
        let sig = SignatureData {
            key_id: "key-123".to_string(),
            algorithm: SignatureAlgorithm::Ed25519,
            signature: "base64sig".to_string(),
        };
        assert_eq!(sig.algorithm, SignatureAlgorithm::Ed25519);

        // PublishResponse
        let response = PublishResponse {
            package_id: "test/pkg".to_string(),
            version: "1.0.0".to_string(),
            message: "Published".to_string(),
            url: "https://registry.dashswarm.com/packages/test/pkg".to_string(),
        };
        assert_eq!(response.package_id, "test/pkg");
    }
}
