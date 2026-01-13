//! Semantic and keyword search for packages.
//!
//! Provides unified search combining:
//! - Semantic search via vector embeddings
//! - Keyword/text search
//! - Capability matching
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_registry::{SearchService, SearchRequest, SearchFilters};
//!
//! let search_service = SearchService::new(embedder, vector_db);
//!
//! let results = search_service.search(SearchRequest {
//!     query: Some("sentiment analysis for customer reviews".to_string()),
//!     keywords: None,
//!     capabilities: None,
//!     filters: SearchFilters::default(),
//!     limit: 10,
//!     offset: 0,
//! }).await?;
//!
//! for result in results.results {
//!     println!("{}: {:.2}", result.package.manifest.name, result.score);
//! }
//! ```

// Note: All RwLock usages now use poison-safe patterns (unwrap_or_else with into_inner).
// The blanket #![allow(clippy::unwrap_used)] was removed; only test code uses .unwrap().

use std::collections::HashSet;

use crate::error::Result;
use crate::package::{Capability, PackageInfo, PackageType, TrustLevel};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

/// A search request combining multiple search modes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchRequest {
    /// Natural language query (semantic search).
    pub query: Option<String>,

    /// Keywords (keyword search).
    pub keywords: Option<Vec<String>>,

    /// Required capabilities (capability search).
    pub capabilities: Option<Vec<Capability>>,

    /// Filters to apply.
    #[serde(default)]
    pub filters: SearchFilters,

    /// Maximum number of results.
    #[serde(default = "default_limit")]
    pub limit: u32,

    /// Offset for pagination.
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    10
}

/// Filters for search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilters {
    /// Filter by package type.
    pub package_type: Option<PackageType>,

    /// Minimum download count.
    pub min_downloads: Option<u64>,

    /// Only return verified packages.
    #[serde(default)]
    pub verified_only: bool,

    /// Minimum trust level required.
    pub min_trust_level: Option<TrustLevel>,

    /// Only packages updated after this time.
    pub updated_after: Option<DateTime<Utc>>,

    /// Filter by namespace.
    pub namespace: Option<String>,

    /// Exclude yanked packages.
    #[serde(default = "default_true")]
    pub exclude_yanked: bool,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            package_type: None,
            min_downloads: None,
            verified_only: false,
            min_trust_level: None,
            updated_after: None,
            namespace: None,
            exclude_yanked: true,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Search response with results and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    /// Search results.
    pub results: Vec<SearchResult>,

    /// Total number of matching results (before pagination).
    pub total: u64,

    /// Which search methods contributed to results.
    pub sources: SearchSources,

    /// Time taken to execute search (milliseconds).
    pub search_time_ms: u64,
}

/// A single search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The matching package.
    pub package: PackageInfo,

    /// Combined relevance score (0-1).
    pub score: f64,

    /// Score breakdown by component.
    pub score_components: ScoreComponents,

    /// Why this result matched.
    pub match_reasons: Vec<MatchReason>,
}

/// Score breakdown by search component.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoreComponents {
    /// Semantic similarity score (vector search).
    pub semantic_score: Option<f64>,

    /// Keyword/text match score (BM25 or similar).
    pub keyword_score: Option<f64>,

    /// Capability match score.
    pub capability_score: Option<f64>,

    /// Popularity score (based on downloads, usage).
    pub popularity_score: f64,

    /// Trust score (based on verification status).
    pub trust_score: f64,
}

impl ScoreComponents {
    /// Calculate the combined score using weighted averaging.
    pub fn combined_score(&self, weights: &ScoreWeights) -> f64 {
        let mut total_weight = 0.0;
        let mut weighted_sum = 0.0;

        if let Some(semantic) = self.semantic_score {
            weighted_sum += semantic * weights.semantic;
            total_weight += weights.semantic;
        }

        if let Some(keyword) = self.keyword_score {
            weighted_sum += keyword * weights.keyword;
            total_weight += weights.keyword;
        }

        if let Some(capability) = self.capability_score {
            weighted_sum += capability * weights.capability;
            total_weight += weights.capability;
        }

        weighted_sum += self.popularity_score * weights.popularity;
        total_weight += weights.popularity;

        weighted_sum += self.trust_score * weights.trust;
        total_weight += weights.trust;

        if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        }
    }
}

/// Weights for combining score components.
#[derive(Debug, Clone)]
pub struct ScoreWeights {
    pub semantic: f64,
    pub keyword: f64,
    pub capability: f64,
    pub popularity: f64,
    pub trust: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            semantic: 0.4,
            keyword: 0.2,
            capability: 0.2,
            popularity: 0.1,
            trust: 0.1,
        }
    }
}

/// Reason why a result matched the search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchReason {
    /// Semantic similarity to query.
    SemanticMatch { similarity: f64 },

    /// Keyword matched in name.
    NameMatch { keyword: String },

    /// Keyword matched in description.
    DescriptionMatch { keyword: String },

    /// Keyword matched in keywords list.
    KeywordMatch { keyword: String },

    /// Capability matched.
    CapabilityMatch { capability: String },

    /// Namespace matched.
    NamespaceMatch,
}

/// Which search methods contributed to results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchSources {
    /// Semantic search was used.
    pub semantic: bool,

    /// Keyword search was used.
    pub keyword: bool,

    /// Capability matching was used.
    pub capability: bool,
}

/// Trait for text embedding models.
///
/// Implementations can use local models (ONNX) or remote APIs (OpenAI, Cohere).
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Generate an embedding for a single text.
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embeddings for multiple texts (batch).
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;

    /// Get the embedding dimension.
    fn dimension(&self) -> usize;

    /// Get the model name/identifier.
    fn model_name(&self) -> &str;
}

/// Trait for vector database operations.
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Upsert a vector with metadata.
    async fn upsert(&self, id: &str, vector: Vec<f32>, metadata: PackageMetadata) -> Result<()>;

    /// Search for similar vectors.
    async fn search(&self, vector: Vec<f32>, limit: usize) -> Result<Vec<VectorMatch>>;

    /// Delete a vector by ID.
    async fn delete(&self, id: &str) -> Result<bool>;

    /// Check if a vector exists.
    async fn exists(&self, id: &str) -> Result<bool>;
}

/// Metadata stored with package vectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    /// Content hash (unique identifier).
    pub hash: String,

    /// Package name.
    pub name: String,

    /// Namespace.
    pub namespace: Option<String>,

    /// Version string.
    pub version: String,

    /// Short description.
    pub description: String,

    /// Package type.
    pub package_type: PackageType,

    /// Keywords.
    pub keywords: Vec<String>,

    /// Capabilities provided.
    pub capabilities: Vec<String>,

    /// Trust level.
    pub trust_level: TrustLevel,

    /// Download count.
    pub downloads: u64,

    /// When indexed.
    pub indexed_at: DateTime<Utc>,
}

impl From<&PackageInfo> for PackageMetadata {
    fn from(info: &PackageInfo) -> Self {
        Self {
            hash: info.hash.to_string(),
            name: info.manifest.name.clone(),
            namespace: info.manifest.namespace.clone(),
            version: info.manifest.version.to_string(),
            description: info.manifest.description.clone(),
            package_type: info.manifest.package_type.clone(),
            keywords: info.manifest.keywords.clone(),
            capabilities: info
                .manifest
                .provides
                .iter()
                .map(|c| c.name.clone())
                .collect(),
            trust_level: info.trust_level,
            downloads: info.downloads,
            indexed_at: Utc::now(),
        }
    }
}

/// A match from vector search.
#[derive(Debug, Clone)]
pub struct VectorMatch {
    /// Package content hash (the ID).
    pub id: String,

    /// Similarity score (0-1).
    pub score: f64,

    /// Package metadata.
    pub metadata: PackageMetadata,
}

/// Mock embedder for testing.
pub struct MockEmbedder {
    dimension: usize,
}

impl MockEmbedder {
    /// Create a mock embedder with the given dimension.
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

#[async_trait]
impl Embedder for MockEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Generate a deterministic "embedding" based on text hash
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let hash = hasher.finish();

        let mut embedding = Vec::with_capacity(self.dimension);
        for i in 0..self.dimension {
            // Generate pseudo-random values from hash
            let val = ((hash.wrapping_mul(i as u64 + 1)) % 1000) as f32 / 1000.0;
            embedding.push(val * 2.0 - 1.0); // Range [-1, 1]
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        for v in &mut embedding {
            *v /= norm;
        }

        Ok(embedding)
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_name(&self) -> &str {
        "mock-embedder"
    }
}

/// In-memory vector store for testing.
pub struct InMemoryVectorStore {
    vectors: std::sync::RwLock<Vec<StoredVector>>,
}

struct StoredVector {
    id: String,
    vector: Vec<f32>,
    metadata: PackageMetadata,
}

impl InMemoryVectorStore {
    /// Create a new in-memory vector store.
    pub fn new() -> Self {
        Self {
            vectors: std::sync::RwLock::new(Vec::new()),
        }
    }

    /// Compute cosine similarity between two vectors.
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            (dot / (norm_a * norm_b)) as f64
        }
    }
}

impl Default for InMemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn upsert(&self, id: &str, vector: Vec<f32>, metadata: PackageMetadata) -> Result<()> {
        // SAFETY: Use poison-safe pattern - if a thread panicked while holding the lock,
        // we still want to be able to upsert rather than crash
        let mut vectors = self.vectors.write().unwrap_or_else(|e| e.into_inner());

        // Remove existing if present
        vectors.retain(|v| v.id != id);

        vectors.push(StoredVector {
            id: id.to_string(),
            vector,
            metadata,
        });

        Ok(())
    }

    async fn search(&self, vector: Vec<f32>, limit: usize) -> Result<Vec<VectorMatch>> {
        // SAFETY: Use poison-safe pattern - if a thread panicked while holding the lock,
        // we still want to be able to search rather than crash
        let vectors = self.vectors.read().unwrap_or_else(|e| e.into_inner());

        let mut matches: Vec<_> = vectors
            .iter()
            .map(|stored| VectorMatch {
                id: stored.id.clone(),
                score: Self::cosine_similarity(&vector, &stored.vector),
                metadata: stored.metadata.clone(),
            })
            .collect();

        // Sort by score descending
        matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top N
        matches.truncate(limit);

        Ok(matches)
    }

    async fn delete(&self, id: &str) -> Result<bool> {
        // SAFETY: Use poison-safe pattern - if a thread panicked while holding the lock,
        // we still want to be able to delete rather than crash
        let mut vectors = self.vectors.write().unwrap_or_else(|e| e.into_inner());
        let len_before = vectors.len();
        vectors.retain(|v| v.id != id);
        Ok(vectors.len() < len_before)
    }

    async fn exists(&self, id: &str) -> Result<bool> {
        // SAFETY: Use poison-safe pattern - if a thread panicked while holding the lock,
        // we still want to be able to check existence rather than crash
        let vectors = self.vectors.read().unwrap_or_else(|e| e.into_inner());
        Ok(vectors.iter().any(|v| v.id == id))
    }
}

/// Semantic search service.
///
/// Combines embedding generation and vector search for semantic package discovery.
pub struct SemanticSearchService<E: Embedder, V: VectorStore> {
    embedder: E,
    vector_store: V,
}

impl<E: Embedder, V: VectorStore> SemanticSearchService<E, V> {
    /// Create a new semantic search service.
    pub fn new(embedder: E, vector_store: V) -> Self {
        Self {
            embedder,
            vector_store,
        }
    }

    /// Index a package for semantic search.
    pub async fn index(&self, package: &PackageInfo) -> Result<()> {
        // Create rich text representation for embedding
        let text = self.create_searchable_text(package);

        // Generate embedding
        let embedding = self.embedder.embed(&text).await?;

        // Store in vector DB
        let metadata = PackageMetadata::from(package);
        self.vector_store
            .upsert(&package.hash.to_string(), embedding, metadata)
            .await
    }

    /// Search semantically.
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<VectorMatch>> {
        let query_embedding = self.embedder.embed(query).await?;
        self.vector_store.search(query_embedding, limit).await
    }

    /// Remove a package from the index.
    pub async fn remove(&self, hash: &str) -> Result<bool> {
        self.vector_store.delete(hash).await
    }

    /// Create searchable text from package info.
    fn create_searchable_text(&self, package: &PackageInfo) -> String {
        let mut parts = Vec::new();

        // Name and namespace
        if let Some(ns) = &package.manifest.namespace {
            parts.push(format!("{}/{}", ns, package.manifest.name));
        } else {
            parts.push(package.manifest.name.clone());
        }

        // Description
        parts.push(package.manifest.description.clone());

        // Keywords
        if !package.manifest.keywords.is_empty() {
            parts.push(format!(
                "Keywords: {}",
                package.manifest.keywords.join(", ")
            ));
        }

        // Capabilities
        if !package.manifest.provides.is_empty() {
            let caps: Vec<_> = package
                .manifest
                .provides
                .iter()
                .map(|c| c.name.as_str())
                .collect();
            parts.push(format!("Capabilities: {}", caps.join(", ")));
        }

        // README (truncated)
        if let Some(readme) = &package.manifest.readme {
            let truncated = if readme.len() > 1000 {
                &readme[..1000]
            } else {
                readme
            };
            parts.push(truncated.to_string());
        }

        parts.join("\n\n")
    }

    /// Get reference to the embedder.
    pub fn embedder(&self) -> &E {
        &self.embedder
    }

    /// Get reference to the vector store.
    pub fn vector_store(&self) -> &V {
        &self.vector_store
    }
}

/// Keyword search utilities.
pub struct KeywordSearch;

impl KeywordSearch {
    /// Check if a package matches the given keywords.
    pub fn matches(package: &PackageInfo, keywords: &[String]) -> (bool, Vec<MatchReason>) {
        let mut reasons = Vec::new();

        for keyword in keywords {
            let keyword_lower = keyword.to_lowercase();

            // Check name
            if package
                .manifest
                .name
                .to_lowercase()
                .contains(&keyword_lower)
            {
                reasons.push(MatchReason::NameMatch {
                    keyword: keyword.clone(),
                });
            }

            // Check description
            if package
                .manifest
                .description
                .to_lowercase()
                .contains(&keyword_lower)
            {
                reasons.push(MatchReason::DescriptionMatch {
                    keyword: keyword.clone(),
                });
            }

            // Check keywords list
            for pkg_kw in &package.manifest.keywords {
                if pkg_kw.to_lowercase().contains(&keyword_lower) {
                    reasons.push(MatchReason::KeywordMatch {
                        keyword: keyword.clone(),
                    });
                    break;
                }
            }
        }

        (!reasons.is_empty(), reasons)
    }

    /// Calculate keyword match score (simple TF-based).
    pub fn score(package: &PackageInfo, keywords: &[String]) -> f64 {
        if keywords.is_empty() {
            return 0.0;
        }

        let (matched, reasons) = Self::matches(package, keywords);
        if !matched {
            return 0.0;
        }

        // Count unique keywords that matched
        let matched_keywords: HashSet<_> = reasons
            .iter()
            .map(|r| match r {
                MatchReason::NameMatch { keyword } => keyword,
                MatchReason::DescriptionMatch { keyword } => keyword,
                MatchReason::KeywordMatch { keyword } => keyword,
                _ => "",
            })
            .filter(|k| !k.is_empty())
            .collect();

        // Score is proportion of keywords that matched, with bonus for name matches
        let base_score = matched_keywords.len() as f64 / keywords.len() as f64;

        // Bonus for name match
        let has_name_match = reasons
            .iter()
            .any(|r| matches!(r, MatchReason::NameMatch { .. }));
        let bonus = if has_name_match { 0.2 } else { 0.0 };

        (base_score + bonus).min(1.0)
    }
}

/// Capability matching utilities.
pub struct CapabilityMatch;

impl CapabilityMatch {
    /// Check if a package provides the required capabilities.
    pub fn matches(package: &PackageInfo, required: &[Capability]) -> (bool, Vec<MatchReason>) {
        let mut reasons = Vec::new();
        let mut all_matched = true;

        for req in required {
            let mut found = false;
            for provided in &package.manifest.provides {
                if Self::capability_matches(provided, req) {
                    reasons.push(MatchReason::CapabilityMatch {
                        capability: req.name.clone(),
                    });
                    found = true;
                    break;
                }
            }
            if !found {
                all_matched = false;
            }
        }

        (all_matched && !required.is_empty(), reasons)
    }

    /// Check if a provided capability matches a required one.
    ///
    /// Matching rules:
    /// 1. Names must match (case-insensitive)
    /// 2. If required has no version constraint, match on name only
    /// 3. If required has a version constraint:
    ///    - Provided must have a version
    ///    - Provided version must satisfy the version requirement
    fn capability_matches(provided: &Capability, required: &Capability) -> bool {
        // Names must match (case-insensitive)
        if provided.name.to_lowercase() != required.name.to_lowercase() {
            return false;
        }

        // If no version requirement, name match is sufficient
        let Some(version_req_str) = &required.version else {
            return true;
        };

        // Parse the version requirement
        let Ok(version_req) = VersionReq::parse(version_req_str) else {
            // Invalid version requirement - treat as no requirement
            tracing::warn!(
                capability = %required.name,
                version_req = %version_req_str,
                "Invalid version requirement, treating as no version constraint"
            );
            return true;
        };

        // If provided capability has no version, it can't satisfy a version requirement
        let Some(version_str) = &provided.version else {
            return false;
        };

        // Parse the provided version
        let Ok(version) = Version::parse(version_str) else {
            // Invalid provided version - can't match
            return false;
        };

        // Check if version satisfies the requirement
        version_req.matches(&version)
    }

    /// Calculate capability match score.
    pub fn score(package: &PackageInfo, required: &[Capability]) -> f64 {
        if required.is_empty() {
            return 0.0;
        }

        let mut matched = 0;
        for req in required {
            for provided in &package.manifest.provides {
                if Self::capability_matches(provided, req) {
                    matched += 1;
                    break;
                }
            }
        }

        matched as f64 / required.len() as f64
    }
}

// =============================================================================
// Production Embedder and VectorStore Adapters
// =============================================================================

/// OpenAI embedder adapter for the registry.
///
/// Wraps `dashflow_openai::OpenAIEmbeddings` to implement the registry's `Embedder` trait.
/// This enables semantic search using OpenAI's embedding models.
///
/// # Feature Flag
///
/// Requires the `openai-embeddings` feature flag.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_registry::search::{OpenAIRegistryEmbedder, SemanticSearchService, InMemoryVectorStore};
///
/// let embedder = OpenAIRegistryEmbedder::new()
///     .with_model("text-embedding-3-small");
///
/// let vector_store = InMemoryVectorStore::new();
/// let search = SemanticSearchService::new(embedder, vector_store);
/// ```
#[cfg(feature = "openai-embeddings")]
pub struct OpenAIRegistryEmbedder {
    inner: Option<dashflow_openai::embeddings::OpenAIEmbeddings>,
    model_name: String,
    dimension: usize,
}

#[cfg(feature = "openai-embeddings")]
impl OpenAIRegistryEmbedder {
    fn model_dimensions(model: &str) -> usize {
        match model {
            "text-embedding-3-large" => 3072,
            _ => 1536, // Default for small and ada-002
        }
    }

    fn try_new_with_secret(
        model: &str,
        api_key: dashflow::core::config_loader::SecretReference,
    ) -> Result<Self> {
        use dashflow::core::config_loader::EmbeddingConfig;

        let config = EmbeddingConfig::OpenAI {
            model: model.to_string(),
            api_key,
            batch_size: 512,
        };

        let inner =
            dashflow_openai::embeddings::OpenAIEmbeddings::from_config(&config).map_err(|e| {
                crate::error::RegistryError::Search(format!(
                    "Failed to build OpenAI embeddings from config: {e}",
                ))
            })?;

        Ok(Self {
            inner: Some(inner),
            model_name: model.to_string(),
            dimension: Self::model_dimensions(model),
        })
    }

    /// Try to create a new OpenAI embedder with default settings.
    ///
    /// Defaults:
    /// - Model: text-embedding-3-small
    /// - Dimension: 1536
    /// - Batch size: 512
    ///
    /// # Environment
    ///
    /// Requires `OPENAI_API_KEY` environment variable to be set.
    pub fn try_new() -> Result<Self> {
        use dashflow::core::config_loader::SecretReference;
        Self::try_new_with_secret(
            "text-embedding-3-small",
            SecretReference::from_env("OPENAI_API_KEY"),
        )
    }

    /// Try to create a new OpenAI embedder using an explicit API key.
    pub fn try_new_with_api_key(api_key: &str) -> Result<Self> {
        use dashflow::core::config_loader::SecretReference;
        Self::try_new_with_secret(
            "text-embedding-3-small",
            SecretReference::from_inline(api_key),
        )
    }

    /// Create a new OpenAI embedder with default settings.
    ///
    /// Defaults:
    /// - Model: text-embedding-3-small
    /// - Dimension: 1536
    ///
    /// # Environment
    ///
    /// Requires `OPENAI_API_KEY` environment variable to be set.
    pub fn new() -> Self {
        match Self::try_new() {
            Ok(embedder) => embedder,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "OpenAIRegistryEmbedder created without a valid configuration; embed calls will fail until configured"
                );
                Self {
                    inner: None,
                    model_name: "text-embedding-3-small".to_string(),
                    dimension: 1536,
                }
            }
        }
    }

    /// Set the model to use for embeddings.
    ///
    /// Supported models:
    /// - `text-embedding-3-small` (1536 dimensions, default)
    /// - `text-embedding-3-large` (3072 dimensions)
    /// - `text-embedding-ada-002` (1536 dimensions, legacy)
    #[must_use]
    pub fn with_model(mut self, model: &str) -> Self {
        self.model_name = model.to_string();
        self.dimension = Self::model_dimensions(model);
        if let Some(inner) = self.inner.take() {
            self.inner = Some(inner.with_model(model));
        }
        self
    }

    /// Set a custom dimension for the embeddings.
    ///
    /// Only supported by text-embedding-3 models.
    #[must_use]
    pub fn with_dimensions(mut self, dimensions: u32) -> Self {
        self.dimension = dimensions as usize;
        if let Some(inner) = self.inner.take() {
            self.inner = Some(inner.with_dimensions(dimensions));
        }
        self
    }

    /// Set the API key explicitly.
    #[must_use]
    pub fn with_api_key(mut self, api_key: &str) -> Self {
        if let Some(inner) = self.inner.take() {
            self.inner = Some(inner.with_api_key(api_key));
        }
        self
    }
}

#[cfg(feature = "openai-embeddings")]
impl Default for OpenAIRegistryEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "openai-embeddings")]
#[async_trait]
impl Embedder for OpenAIRegistryEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        use dashflow::core::embeddings::Embeddings;
        let inner = self.inner.as_ref().ok_or_else(|| {
            crate::error::RegistryError::Search(
                "OpenAIRegistryEmbedder is not configured (missing inner embedder)".to_string(),
            )
        })?;

        inner
            ._embed_query(text)
            .await
            .map_err(|e| crate::error::RegistryError::Search(format!("OpenAI embed error: {}", e)))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        use dashflow::core::embeddings::Embeddings;
        let inner = self.inner.as_ref().ok_or_else(|| {
            crate::error::RegistryError::Search(
                "OpenAIRegistryEmbedder is not configured (missing inner embedder)".to_string(),
            )
        })?;

        inner._embed_documents(texts).await.map_err(|e| {
            crate::error::RegistryError::Search(format!("OpenAI batch embed error: {}", e))
        })
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}

/// Convert a Qdrant Value to a serde_json::Value.
#[cfg(feature = "vector-search")]
fn qdrant_value_to_json(value: &qdrant_client::qdrant::Value) -> serde_json::Value {
    use qdrant_client::qdrant::value::Kind;
    match &value.kind {
        Some(Kind::NullValue(_)) => serde_json::Value::Null,
        Some(Kind::BoolValue(b)) => serde_json::Value::Bool(*b),
        Some(Kind::IntegerValue(i)) => serde_json::json!(*i),
        Some(Kind::DoubleValue(d)) => serde_json::json!(*d),
        Some(Kind::StringValue(s)) => serde_json::Value::String(s.clone()),
        Some(Kind::ListValue(list)) => {
            serde_json::Value::Array(list.values.iter().map(qdrant_value_to_json).collect())
        }
        Some(Kind::StructValue(s)) => {
            let obj: serde_json::Map<String, serde_json::Value> = s
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), qdrant_value_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
        None => serde_json::Value::Null,
    }
}

/// Qdrant vector store adapter for the registry.
///
/// Wraps `qdrant-client` to implement the registry's `VectorStore` trait.
/// This enables production vector search with Qdrant.
///
/// # Feature Flag
///
/// Requires the `vector-search` feature flag.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_registry::search::{QdrantRegistryStore, SemanticSearchService};
///
/// let vector_store = QdrantRegistryStore::new("http://localhost:6334", "packages").await?;
/// ```
#[cfg(feature = "vector-search")]
pub struct QdrantRegistryStore {
    client: qdrant_client::Qdrant,
    collection_name: String,
    dimension: usize,
}

#[cfg(feature = "vector-search")]
impl QdrantRegistryStore {
    /// Create a new Qdrant vector store.
    ///
    /// # Arguments
    ///
    /// * `url` - Qdrant server URL (e.g., "http://localhost:6334")
    /// * `collection_name` - Name of the collection to use
    /// * `dimension` - Dimension of the vectors (must match your embedder)
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails or collection creation fails.
    pub async fn new(url: &str, collection_name: &str, dimension: usize) -> Result<Self> {
        use qdrant_client::qdrant::{CreateCollectionBuilder, Distance, VectorParamsBuilder};

        let client = qdrant_client::Qdrant::from_url(url).build().map_err(|e| {
            crate::error::RegistryError::Search(format!("Qdrant connection error: {}", e))
        })?;

        // Check if collection exists, create if not
        let collections = client.list_collections().await.map_err(|e| {
            crate::error::RegistryError::Search(format!("Qdrant list collections error: {}", e))
        })?;

        let exists = collections
            .collections
            .iter()
            .any(|c| c.name == collection_name);

        if !exists {
            client
                .create_collection(
                    CreateCollectionBuilder::new(collection_name).vectors_config(
                        VectorParamsBuilder::new(dimension as u64, Distance::Cosine),
                    ),
                )
                .await
                .map_err(|e| {
                    crate::error::RegistryError::Search(format!(
                        "Qdrant create collection error: {}",
                        e
                    ))
                })?;
        }

        Ok(Self {
            client,
            collection_name: collection_name.to_string(),
            dimension,
        })
    }

    /// Connect to an existing Qdrant collection.
    ///
    /// Does not create the collection if it doesn't exist.
    pub async fn connect(url: &str, collection_name: &str) -> Result<Self> {
        let client = qdrant_client::Qdrant::from_url(url).build().map_err(|e| {
            crate::error::RegistryError::Search(format!("Qdrant connection error: {}", e))
        })?;

        // Get collection info to determine dimension
        let info = client.collection_info(collection_name).await.map_err(|e| {
            crate::error::RegistryError::Search(format!("Qdrant collection info error: {}", e))
        })?;

        let dimension = info
            .result
            .and_then(|r| r.config)
            .and_then(|c| c.params)
            .and_then(|p| p.vectors_config)
            .and_then(|vc| vc.config)
            .and_then(|config| match config {
                qdrant_client::qdrant::vectors_config::Config::Params(p) => Some(p.size as usize),
                qdrant_client::qdrant::vectors_config::Config::ParamsMap(_) => None,
            })
            .ok_or_else(|| {
                crate::error::RegistryError::Search(
                    "Could not determine collection dimension".to_string(),
                )
            })?;

        Ok(Self {
            client,
            collection_name: collection_name.to_string(),
            dimension,
        })
    }

    /// Get the collection name.
    pub fn collection_name(&self) -> &str {
        &self.collection_name
    }

    /// Get the vector dimension.
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(feature = "vector-search")]
#[async_trait]
impl VectorStore for QdrantRegistryStore {
    async fn upsert(&self, id: &str, vector: Vec<f32>, metadata: PackageMetadata) -> Result<()> {
        use qdrant_client::qdrant::{PointStruct, UpsertPointsBuilder};

        // Convert metadata to JSON payload
        let payload: qdrant_client::Payload = serde_json::to_value(&metadata)
            .map_err(|e| {
                crate::error::RegistryError::Search(format!("Metadata serialization error: {}", e))
            })?
            .try_into()
            .map_err(|e| {
                crate::error::RegistryError::Search(format!("Payload conversion error: {}", e))
            })?;

        // Create point with string ID (using hash as the ID)
        let point = PointStruct::new(id.to_string(), vector, payload);

        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection_name, vec![point]).wait(true))
            .await
            .map_err(|e| {
                crate::error::RegistryError::Search(format!("Qdrant upsert error: {}", e))
            })?;

        Ok(())
    }

    async fn search(&self, vector: Vec<f32>, limit: usize) -> Result<Vec<VectorMatch>> {
        use qdrant_client::qdrant::SearchPointsBuilder;

        let results = self
            .client
            .search_points(
                SearchPointsBuilder::new(&self.collection_name, vector, limit as u64)
                    .with_payload(true),
            )
            .await
            .map_err(|e| {
                crate::error::RegistryError::Search(format!("Qdrant search error: {}", e))
            })?;

        let matches = results
            .result
            .into_iter()
            .filter_map(|point| {
                // Extract ID from PointId
                let id = point
                    .id
                    .as_ref()
                    .and_then(|pid| match &pid.point_id_options {
                        Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(u)) => {
                            Some(u.clone())
                        }
                        Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(n)) => {
                            Some(n.to_string())
                        }
                        None => None,
                    })?;

                // Convert Qdrant payload to JSON, then to our metadata type
                let payload_map: std::collections::HashMap<String, serde_json::Value> = point
                    .payload
                    .into_iter()
                    .map(|(k, v)| (k, qdrant_value_to_json(&v)))
                    .collect();
                let payload_json = serde_json::Value::Object(payload_map.into_iter().collect());
                let metadata: PackageMetadata = serde_json::from_value(payload_json).ok()?;

                Some(VectorMatch {
                    id,
                    score: point.score as f64,
                    metadata,
                })
            })
            .collect();

        Ok(matches)
    }

    async fn delete(&self, id: &str) -> Result<bool> {
        use qdrant_client::qdrant::{DeletePointsBuilder, PointsIdsList};

        let result = self
            .client
            .delete_points(
                DeletePointsBuilder::new(&self.collection_name)
                    .points(PointsIdsList {
                        ids: vec![id.to_string().into()],
                    })
                    .wait(true),
            )
            .await
            .map_err(|e| {
                crate::error::RegistryError::Search(format!("Qdrant delete error: {}", e))
            })?;

        // Check if any points were deleted
        Ok(result.result.is_some_and(|r| r.status == 2)) // 2 = Completed
    }

    async fn exists(&self, id: &str) -> Result<bool> {
        use qdrant_client::qdrant::GetPointsBuilder;

        let result = self
            .client
            .get_points(
                GetPointsBuilder::new(&self.collection_name, vec![id.to_string().into()])
                    .with_payload(false)
                    .with_vectors(false),
            )
            .await
            .map_err(|e| crate::error::RegistryError::Search(format!("Qdrant get error: {}", e)))?;

        Ok(!result.result.is_empty())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // Test assertions should panic on errors
mod tests {
    use super::*;
    use crate::content_hash::ContentHash;
    use crate::package::PackageManifest;

    fn test_package_info(name: &str, description: &str, keywords: Vec<&str>) -> PackageInfo {
        let manifest = PackageManifest::builder()
            .name(name)
            .version("1.0.0")
            .description(description)
            .keywords(keywords.into_iter().map(String::from))
            .build()
            .unwrap();

        PackageInfo {
            hash: ContentHash::from_bytes(name.as_bytes()),
            manifest,
            published_at: Utc::now(),
            publisher_key_id: "test-key".to_string(),
            downloads: 100,
            trust_level: TrustLevel::Community,
            lineage: None,
            yanked: false,
        }
    }

    #[test]
    fn test_score_components_combined() {
        let components = ScoreComponents {
            semantic_score: Some(0.8),
            keyword_score: Some(0.6),
            capability_score: None,
            popularity_score: 0.5,
            trust_score: 0.9,
        };

        let weights = ScoreWeights::default();
        let score = components.combined_score(&weights);

        // Should be weighted average of present components
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_keyword_search_matches() {
        let package = test_package_info(
            "sentiment-analyzer",
            "Analyze sentiment in customer reviews",
            vec!["nlp", "sentiment", "analysis"],
        );

        let (matched, reasons) = KeywordSearch::matches(&package, &["sentiment".to_string()]);
        assert!(matched);
        assert!(!reasons.is_empty());

        let (matched, _) = KeywordSearch::matches(&package, &["nonexistent".to_string()]);
        assert!(!matched);
    }

    #[test]
    fn test_keyword_search_score() {
        let package = test_package_info(
            "sentiment-analyzer",
            "Analyze sentiment in customer reviews",
            vec!["nlp", "sentiment"],
        );

        let score = KeywordSearch::score(&package, &["sentiment".to_string()]);
        assert!(score > 0.0);

        let score = KeywordSearch::score(&package, &["nonexistent".to_string()]);
        assert!(score.abs() < f64::EPSILON);
    }

    #[test]
    fn test_capability_match() {
        let mut manifest = PackageManifest::builder()
            .name("test-agent")
            .version("1.0.0")
            .description("Test agent")
            .build()
            .unwrap();
        manifest
            .provides
            .push(Capability::new("sentiment_analysis"));

        let package = PackageInfo {
            hash: ContentHash::from_bytes(b"test"),
            manifest,
            published_at: Utc::now(),
            publisher_key_id: "key".to_string(),
            downloads: 50,
            trust_level: TrustLevel::Community,
            lineage: None,
            yanked: false,
        };

        let required = vec![Capability::new("sentiment_analysis")];
        let (matched, reasons) = CapabilityMatch::matches(&package, &required);

        assert!(matched);
        assert_eq!(reasons.len(), 1);
    }

    #[test]
    fn test_capability_match_with_version_requirement() {
        let mut manifest = PackageManifest::builder()
            .name("test-agent")
            .version("1.0.0")
            .description("Test agent with versioned capabilities")
            .build()
            .unwrap();

        // Add capability with version
        manifest.provides.push(Capability {
            name: "sentiment_analysis".to_string(),
            version: Some("2.1.0".to_string()),
            description: Some("Sentiment analysis v2".to_string()),
        });

        let package = PackageInfo {
            hash: ContentHash::from_bytes(b"test"),
            manifest,
            published_at: Utc::now(),
            publisher_key_id: "key".to_string(),
            downloads: 50,
            trust_level: TrustLevel::Community,
            lineage: None,
            yanked: false,
        };

        // Test 1: Version requirement satisfied (^2.0 matches 2.1.0)
        let required_compatible = vec![Capability {
            name: "sentiment_analysis".to_string(),
            version: Some("^2.0".to_string()),
            description: None,
        }];
        let (matched, _) = CapabilityMatch::matches(&package, &required_compatible);
        assert!(matched, "Version 2.1.0 should satisfy ^2.0");

        // Test 2: Version requirement NOT satisfied (>=3.0 doesn't match 2.1.0)
        let required_incompatible = vec![Capability {
            name: "sentiment_analysis".to_string(),
            version: Some(">=3.0".to_string()),
            description: None,
        }];
        let (matched, _) = CapabilityMatch::matches(&package, &required_incompatible);
        assert!(!matched, "Version 2.1.0 should not satisfy >=3.0");

        // Test 3: No version requirement should match
        let required_no_version = vec![Capability::new("sentiment_analysis")];
        let (matched, _) = CapabilityMatch::matches(&package, &required_no_version);
        assert!(matched, "No version requirement should match any version");
    }

    #[tokio::test]
    async fn test_mock_embedder() {
        let embedder = MockEmbedder::new(128);

        let embedding1 = embedder.embed("hello world").await.unwrap();
        assert_eq!(embedding1.len(), 128);

        let embedding2 = embedder.embed("hello world").await.unwrap();
        assert_eq!(embedding1, embedding2); // Deterministic

        let embedding3 = embedder.embed("different text").await.unwrap();
        assert_ne!(embedding1, embedding3); // Different text = different embedding
    }

    #[tokio::test]
    async fn test_in_memory_vector_store() {
        let store = InMemoryVectorStore::new();

        let metadata = PackageMetadata {
            hash: "sha256:test".to_string(),
            name: "test".to_string(),
            namespace: None,
            version: "1.0.0".to_string(),
            description: "Test package".to_string(),
            package_type: PackageType::Library,
            keywords: vec![],
            capabilities: vec![],
            trust_level: TrustLevel::Community,
            downloads: 0,
            indexed_at: Utc::now(),
        };

        // Upsert
        store
            .upsert("hash1", vec![1.0, 0.0, 0.0], metadata.clone())
            .await
            .unwrap();
        store
            .upsert("hash2", vec![0.0, 1.0, 0.0], metadata.clone())
            .await
            .unwrap();

        // Search
        let results = store.search(vec![0.9, 0.1, 0.0], 10).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "hash1"); // Most similar

        // Delete
        let deleted = store.delete("hash1").await.unwrap();
        assert!(deleted);

        // Verify deleted
        let exists = store.exists("hash1").await.unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn test_semantic_search_service() {
        let embedder = MockEmbedder::new(64);
        let store = InMemoryVectorStore::new();
        let service = SemanticSearchService::new(embedder, store);

        let package = test_package_info(
            "sentiment-analyzer",
            "Analyze customer sentiment from reviews",
            vec!["nlp", "sentiment"],
        );

        // Index
        service.index(&package).await.unwrap();

        // Search
        let results = service.search("sentiment analysis", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metadata.name, "sentiment-analyzer");

        // Remove
        let removed = service.remove(&package.hash.to_string()).await.unwrap();
        assert!(removed);
    }

    #[test]
    fn test_search_filters_default() {
        let filters = SearchFilters::default();
        assert!(!filters.verified_only);
        assert!(filters.exclude_yanked);
        assert!(filters.package_type.is_none());
    }
}
