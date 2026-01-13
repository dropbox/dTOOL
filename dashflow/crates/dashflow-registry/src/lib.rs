//! DashFlow Package Registry
//!
//! AI-native package registry with content-addressed storage, semantic search,
//! and multi-model contribution review.
//!
//! # Architecture
//!
//! The registry is built on these core principles:
//!
//! 1. **Content-Addressed Storage (CAS)**: Packages are stored by SHA-256 hash,
//!    not by name+version. This enables deduplication, verification, and P2P distribution.
//!
//! 2. **Semantic-First Search**: Vector embeddings enable natural language package
//!    discovery alongside traditional keyword search.
//!
//! 3. **Trust-Native**: Every package operation verifies Ed25519 signatures.
//!    Lineage tracking provides full derivation chains.
//!
//! 4. **Colony-First Distribution**: Local P2P sharing before fetching from central registry.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_registry::{ContentHash, PackageManifest, RegistryClient};
//!
//! // Create a content hash from package data
//! let hash = ContentHash::from_bytes(package_data);
//! println!("Package hash: {}", hash);
//!
//! // Publish a package
//! let manifest = PackageManifest::builder()
//!     .name("my-agent")
//!     .version("1.0.0")
//!     .description("AI agent for customer support")
//!     .build()?;
//!
//! client.publish(&manifest, package_data, &signature).await?;
//! ```

pub mod cache;
pub mod client;
pub mod colony;
pub mod content_hash;
pub mod contribution;
pub mod error;
pub mod metadata;
pub mod package;
pub mod search;
pub mod signature;
pub mod storage;
pub mod trust;

// HTTP API server (feature-gated)
#[cfg(feature = "server")]
pub mod api;

// Re-exports for convenience
pub use content_hash::ContentHash;
pub use error::{RegistryError, Result};
pub use package::{
    Capability, Lineage, LineageStep, PackageInfo, PackageManifest, PackageType, TrustLevel,
};
pub use search::{
    CapabilityMatch, Embedder, InMemoryVectorStore, KeywordSearch, MatchReason, MockEmbedder,
    PackageMetadata, ScoreComponents, ScoreWeights, SearchFilters, SearchRequest, SearchResponse,
    SearchResult, SearchSources, SemanticSearchService, VectorMatch, VectorStore,
};
pub use signature::{KeyPair, PublicKey, Signature, SignedPackage};
#[cfg(feature = "s3")]
pub use storage::S3Storage;
pub use storage::{
    DownloadUrl, FilesystemStorage, InMemoryStorage, PackageCache, S3Config, StorageBackend,
    StorageLocation, StoredPackage,
};
pub use trust::{
    KeyEntry, Keyring, LineageStepBuilder, LineageStepVerification, LineageVerification,
    SignatureVerification, TrustService, VerificationResult,
};

// Production embedder and vector store adapters (feature-gated)
pub use colony::{
    ColonyConfig, ColonyNetwork, ColonyPackageResolver, FetchResult, FetchSource,
    MockColonyNetwork, PackageTransferRequest, PackageTransferResponse, PeerInfo, PeerPackageInfo,
    PeerTrustLevel, ResolverStats, PACKAGE_ANNOUNCE_CHANNEL, PACKAGE_TRANSFER_CHANNEL,
};
pub use contribution::{
    AlternativeSolution,
    // Configuration
    AutoApprovePolicy,
    BugCategory,
    BugEvidence,
    // Bug Report types
    BugReport,
    BugReportBuilder,
    BugSeverity,
    ConcernCategory,
    ConcernSeverity,
    // Consensus and Actions
    ConsensusResult,
    // Unified Contribution
    Contribution,
    ContributionReviewer,
    ContributionStatus,
    ContributionType,
    // Contributor
    Contributor,
    EffortEstimate,
    FileChange,
    FileChangeType,
    // Fix types
    FixSubmission,
    FixSubmissionBuilder,
    FixType,
    ImpactLevel,
    ImprovementCategory,
    // Improvement types
    ImprovementProposal,
    ImprovementProposalBuilder,
    MockModelReviewer,
    // Review types
    ModelReviewResult,
    // Reviewer traits and implementations
    ModelReviewer,
    // Package Request types
    PackageRequest,
    PackageRequestBuilder,
    ReproductionStep,
    RequestPriority,
    ReviewAction,
    ReviewConcern,
    ReviewConfig,
    ReviewResult,
    ReviewScores,
    ReviewVerdict,
    SignedContribution,
    SimilarPackage,
    SuggestedFix,
    TriggerCondition,
    TriggerOperator,
    Urgency,
};
#[cfg(feature = "openai-embeddings")]
pub use search::OpenAIRegistryEmbedder;
#[cfg(feature = "vector-search")]
pub use search::QdrantRegistryStore;

// Metadata store re-exports
#[cfg(feature = "postgres")]
pub use metadata::postgres::PostgresMetadataStore;
pub use metadata::{
    generate_api_key,
    hash_api_key,
    // API key store
    ApiKeyStore,
    ApiKeyTrustLevel,
    ApiKeyVerification,
    ContributionConsensus,
    // Contribution store
    ContributionStore,
    InMemoryMetadataStore,
    MetadataStore,
    Resolution,
    StoredApiKey,
    StoredContribution,
    StoredReview,
    VersionInfo,
};

// API server re-exports (feature-gated)
#[cfg(feature = "server")]
pub use api::{ApiConfig, ApiServer, AppState};

// Client re-exports
pub use client::{RegistryClient, RegistryClientConfig};

// Cache re-exports
pub use cache::{
    cache_get_json, cache_set_json, keys as cache_keys, CacheConfig, CacheStats, CacheStore,
    InMemoryCacheStore, NoOpCacheStore,
};
#[cfg(feature = "redis")]
pub use cache::{RedisCacheStore, RedisConfig};

// Prometheus metrics (feature-gated)
#[cfg(feature = "metrics")]
pub mod metrics;
#[cfg(feature = "metrics")]
pub use metrics::RegistryMetrics;
