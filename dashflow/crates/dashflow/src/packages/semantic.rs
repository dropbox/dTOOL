// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Semantic search for packages using embeddings and vector similarity.
//!
//! This module provides local semantic search capabilities for the package ecosystem.
//! It enables AI agents to find packages by describing functionality rather than
//! exact keyword matches.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    SemanticSearchService                             │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │                                                                      │
//! │   ┌──────────────────┐       ┌──────────────────┐                   │
//! │   │  EmbeddingModel  │       │     VectorDb     │                   │
//! │   │                  │       │                  │                   │
//! │   │  embed(text)     │       │  upsert(id, vec) │                   │
//! │   │  embed_batch()   │       │  search(vec, k)  │                   │
//! │   │  dimensions()    │       │  delete(id)      │                   │
//! │   └──────────────────┘       └──────────────────┘                   │
//! │                                                                      │
//! │   ┌──────────────────────────────────────────────────────────────┐  │
//! │   │                     PackageIndex                              │  │
//! │   │  • Package metadata stored alongside vectors                  │  │
//! │   │  • Enables filtering by type, category, trust level          │  │
//! │   └──────────────────────────────────────────────────────────────┘  │
//! │                                                                      │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::packages::semantic::{
//!     SemanticSearchService, InMemoryVectorDb, MockEmbeddingModel,
//!     SearchQuery, PackageMetadata,
//! };
//!
//! // Create service with mock embedding model (for testing)
//! let embedder = MockEmbeddingModel::new(384);
//! let vector_db = InMemoryVectorDb::new();
//! let service = SemanticSearchService::new(embedder, vector_db);
//!
//! // Index a package
//! let metadata = PackageMetadata::new("dashflow/sentiment", "Sentiment Analysis")
//!     .with_description("Production-grade sentiment analysis nodes")
//!     .with_keywords(vec!["sentiment", "nlp", "text-analysis"]);
//! service.index_package(&metadata)?;
//!
//! // Search semantically
//! let results = service.search("analyze customer emotions in text", 10)?;
//! for result in results {
//!     println!("{}: {:.3}", result.package_id, result.score);
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

use super::types::{PackageId, PackageType, TrustLevel};

/// Result type for semantic search operations.
pub type SemanticResult<T> = Result<T, SemanticError>;

/// Errors that can occur during semantic search operations.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum SemanticError {
    /// Embedding generation failed
    #[error("Embedding generation failed: {0}")]
    EmbeddingFailed(String),
    /// Vector database operation failed
    #[error("Vector database error: {0}")]
    VectorDbError(String),
    /// Package not found in index
    #[error("Package not found: {0}")]
    PackageNotFound(String),
    /// Invalid vector dimensions.
    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch {
        /// Expected number of dimensions.
        expected: usize,
        /// Actual number of dimensions.
        got: usize,
    },
    /// Index is empty
    #[error("Index is empty")]
    EmptyIndex,
    /// IO error
    #[error("IO error: {0}")]
    Io(String),
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
}

// ============================================================================
// Embedding Model Trait
// ============================================================================

/// An embedding vector.
pub type Embedding = Vec<f32>;

/// Trait for generating text embeddings.
///
/// Implementations can use local models (e.g., ONNX), remote APIs (e.g., OpenAI),
/// or mock implementations for testing.
pub trait EmbeddingModel: Send + Sync {
    /// Generate an embedding for a single text.
    fn embed(&self, text: &str) -> SemanticResult<Embedding>;

    /// Generate embeddings for multiple texts (batch processing).
    ///
    /// Default implementation calls `embed` for each text. Implementations
    /// should override this for more efficient batch processing.
    fn embed_batch(&self, texts: &[&str]) -> SemanticResult<Vec<Embedding>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Get the dimensionality of embeddings produced by this model.
    fn dimensions(&self) -> usize;

    /// Get the model name/identifier.
    fn model_name(&self) -> &str;

    /// Maximum input tokens (if applicable).
    fn max_tokens(&self) -> Option<usize> {
        None
    }
}

/// Mock embedding model for testing.
///
/// Generates deterministic embeddings based on text hash. Useful for testing
/// semantic search logic without requiring a real embedding model.
pub struct MockEmbeddingModel {
    dimensions: usize,
}

impl MockEmbeddingModel {
    /// Create a new mock embedding model.
    pub fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }

    /// Generate a deterministic embedding from text.
    fn hash_to_embedding(&self, text: &str) -> Embedding {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let base = hasher.finish();

        // Generate deterministic vector components
        let mut embedding = Vec::with_capacity(self.dimensions);
        let mut seed = base;
        for _ in 0..self.dimensions {
            // Simple LCG for deterministic random numbers
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let value = ((seed >> 32) as f32) / (u32::MAX as f32) * 2.0 - 1.0;
            embedding.push(value);
        }

        // Normalize to unit vector
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        embedding
    }
}

impl EmbeddingModel for MockEmbeddingModel {
    fn embed(&self, text: &str) -> SemanticResult<Embedding> {
        Ok(self.hash_to_embedding(text))
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_name(&self) -> &str {
        "mock-embedding-model"
    }
}

// ============================================================================
// Vector Database Trait
// ============================================================================

/// A vector with associated metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    /// Unique identifier
    pub id: String,
    /// The embedding vector
    pub vector: Embedding,
    /// Associated metadata
    pub metadata: PackageMetadata,
}

/// Search result from vector database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    /// Entry ID
    pub id: String,
    /// Similarity score (0.0 to 1.0, higher is better)
    pub score: f64,
    /// Associated metadata
    pub metadata: PackageMetadata,
}

/// Trait for vector database operations.
///
/// Implementations can use in-memory storage, local databases (e.g., SQLite with
/// vector extensions), or remote services (e.g., Pinecone, Qdrant).
pub trait VectorDb: Send + Sync {
    /// Insert or update a vector entry.
    fn upsert(&self, entry: VectorEntry) -> SemanticResult<()>;

    /// Insert or update multiple entries (batch).
    fn upsert_batch(&self, entries: Vec<VectorEntry>) -> SemanticResult<()> {
        for entry in entries {
            self.upsert(entry)?;
        }
        Ok(())
    }

    /// Search for similar vectors.
    ///
    /// Returns the top `k` most similar vectors, ordered by descending similarity.
    fn search(&self, query: &Embedding, k: usize) -> SemanticResult<Vec<VectorSearchResult>>;

    /// Search with filters.
    fn search_filtered(
        &self,
        query: &Embedding,
        k: usize,
        filter: &SearchFilter,
    ) -> SemanticResult<Vec<VectorSearchResult>>;

    /// Delete an entry by ID.
    fn delete(&self, id: &str) -> SemanticResult<bool>;

    /// Get an entry by ID.
    fn get(&self, id: &str) -> SemanticResult<Option<VectorEntry>>;

    /// Get the number of entries in the database.
    fn len(&self) -> usize;

    /// Check if the database is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get all entry IDs.
    fn ids(&self) -> SemanticResult<Vec<String>>;

    /// Clear all entries.
    fn clear(&self) -> SemanticResult<()>;
}

/// Filter criteria for vector search.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilter {
    /// Filter by package types (empty = no filter)
    pub package_types: Vec<PackageType>,
    /// Filter by categories (empty = no filter)
    pub categories: Vec<String>,
    /// Minimum trust level
    pub min_trust_level: Option<TrustLevel>,
    /// Only verified packages
    pub verified_only: bool,
    /// Exclude specific package IDs
    pub exclude_ids: Vec<String>,
}

impl SearchFilter {
    /// Create an empty filter (matches everything).
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by package type.
    #[must_use]
    pub fn with_type(mut self, pkg_type: PackageType) -> Self {
        self.package_types.push(pkg_type);
        self
    }

    /// Filter by category.
    #[must_use]
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.categories.push(category.into());
        self
    }

    /// Set minimum trust level.
    #[must_use]
    pub fn with_min_trust(mut self, level: TrustLevel) -> Self {
        self.min_trust_level = Some(level);
        self
    }

    /// Only return verified packages.
    pub fn verified(mut self) -> Self {
        self.verified_only = true;
        self
    }

    /// Exclude specific package IDs.
    pub fn exclude(mut self, id: impl Into<String>) -> Self {
        self.exclude_ids.push(id.into());
        self
    }

    /// Check if metadata matches this filter.
    pub fn matches(&self, metadata: &PackageMetadata) -> bool {
        // Package type filter
        if !self.package_types.is_empty() && !self.package_types.contains(&metadata.package_type) {
            return false;
        }

        // Category filter
        if !self.categories.is_empty() {
            let has_category = metadata
                .categories
                .iter()
                .any(|c| self.categories.contains(c));
            if !has_category {
                return false;
            }
        }

        // Trust level filter
        if let Some(min_trust) = &self.min_trust_level {
            if metadata.trust_level < *min_trust {
                return false;
            }
        }

        // Verified filter
        if self.verified_only && !metadata.verified {
            return false;
        }

        // Exclude filter
        if self.exclude_ids.contains(&metadata.package_id) {
            return false;
        }

        true
    }
}

// ============================================================================
// Package Metadata
// ============================================================================

/// Metadata about a package for semantic indexing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    /// Package ID (namespace/name)
    pub package_id: String,
    /// Human-readable name
    pub name: String,
    /// Short description
    pub description: String,
    /// Full readme/documentation (optional)
    pub readme: Option<String>,
    /// Keywords
    pub keywords: Vec<String>,
    /// Categories
    pub categories: Vec<String>,
    /// Package type
    pub package_type: PackageType,
    /// Trust level
    pub trust_level: TrustLevel,
    /// Is verified
    pub verified: bool,
    /// Last indexed timestamp
    pub indexed_at: Option<String>,
}

impl PackageMetadata {
    /// Create new package metadata.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            package_id: id.into(),
            name: name.into(),
            description: String::new(),
            readme: None,
            keywords: Vec::new(),
            categories: Vec::new(),
            package_type: PackageType::NodeLibrary,
            trust_level: TrustLevel::Community,
            verified: false,
            indexed_at: None,
        }
    }

    /// Set description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set readme.
    #[must_use]
    pub fn with_readme(mut self, readme: impl Into<String>) -> Self {
        self.readme = Some(readme.into());
        self
    }

    /// Set keywords.
    #[must_use]
    pub fn with_keywords(mut self, keywords: Vec<impl Into<String>>) -> Self {
        self.keywords = keywords.into_iter().map(|k| k.into()).collect();
        self
    }

    /// Add a keyword.
    #[must_use]
    pub fn add_keyword(mut self, keyword: impl Into<String>) -> Self {
        self.keywords.push(keyword.into());
        self
    }

    /// Set categories.
    #[must_use]
    pub fn with_categories(mut self, categories: Vec<impl Into<String>>) -> Self {
        self.categories = categories.into_iter().map(|c| c.into()).collect();
        self
    }

    /// Set package type.
    #[must_use]
    pub fn with_type(mut self, pkg_type: PackageType) -> Self {
        self.package_type = pkg_type;
        self
    }

    /// Set trust level.
    #[must_use]
    pub fn with_trust_level(mut self, level: TrustLevel) -> Self {
        self.trust_level = level;
        self
    }

    /// Set verified status.
    pub fn verified(mut self, is_verified: bool) -> Self {
        self.verified = is_verified;
        self
    }

    /// Generate text for embedding from metadata.
    ///
    /// Combines name, description, keywords, and readme into a single string
    /// optimized for semantic embedding.
    pub fn to_embedding_text(&self) -> String {
        let mut parts = Vec::new();

        // Name (weighted by repetition)
        parts.push(self.name.clone());

        // Description
        if !self.description.is_empty() {
            parts.push(self.description.clone());
        }

        // Keywords
        if !self.keywords.is_empty() {
            parts.push(self.keywords.join(" "));
        }

        // Categories
        if !self.categories.is_empty() {
            parts.push(self.categories.join(" "));
        }

        // Readme (truncated)
        if let Some(readme) = &self.readme {
            // Take first 1000 characters of readme
            let truncated: String = readme.chars().take(1000).collect();
            parts.push(truncated);
        }

        parts.join(" ")
    }

    /// Create from PackageId.
    pub fn from_package_id(id: &PackageId) -> Self {
        Self::new(id.to_string(), id.name().to_string())
    }
}

// ============================================================================
// In-Memory Vector Database
// ============================================================================

/// In-memory vector database implementation.
///
/// Uses brute-force cosine similarity for search. Suitable for small to medium
/// datasets (up to ~100k vectors). For larger datasets, consider using a
/// specialized vector database.
pub struct InMemoryVectorDb {
    entries: Arc<RwLock<HashMap<String, VectorEntry>>>,
    dimensions: Option<usize>,
}

impl InMemoryVectorDb {
    /// Create a new in-memory vector database.
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            dimensions: None,
        }
    }

    /// Create with pre-allocated capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::with_capacity(capacity))),
            dimensions: None,
        }
    }

    /// Compute cosine similarity between two vectors.
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
        if a.len() != b.len() {
            return 0.0;
        }

        let mut dot = 0.0_f64;
        let mut norm_a = 0.0_f64;
        let mut norm_b = 0.0_f64;

        for (x, y) in a.iter().zip(b.iter()) {
            let x = *x as f64;
            let y = *y as f64;
            dot += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a.sqrt() * norm_b.sqrt())
    }
}

impl Default for InMemoryVectorDb {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorDb for InMemoryVectorDb {
    fn upsert(&self, entry: VectorEntry) -> SemanticResult<()> {
        let mut entries = self
            .entries
            .write()
            .map_err(|e| SemanticError::VectorDbError(format!("Lock error: {}", e)))?;

        // Check dimensions consistency
        if let Some(dims) = self.dimensions {
            if entry.vector.len() != dims {
                return Err(SemanticError::DimensionMismatch {
                    expected: dims,
                    got: entry.vector.len(),
                });
            }
        }

        entries.insert(entry.id.clone(), entry);
        Ok(())
    }

    fn search(&self, query: &Embedding, k: usize) -> SemanticResult<Vec<VectorSearchResult>> {
        self.search_filtered(query, k, &SearchFilter::new())
    }

    fn search_filtered(
        &self,
        query: &Embedding,
        k: usize,
        filter: &SearchFilter,
    ) -> SemanticResult<Vec<VectorSearchResult>> {
        let entries = self
            .entries
            .read()
            .map_err(|e| SemanticError::VectorDbError(format!("Lock error: {}", e)))?;

        if entries.is_empty() {
            return Ok(Vec::new());
        }

        // Compute similarities for all entries that match filter
        let mut results: Vec<VectorSearchResult> = entries
            .values()
            .filter(|entry| filter.matches(&entry.metadata))
            .map(|entry| {
                let score = Self::cosine_similarity(query, &entry.vector);
                VectorSearchResult {
                    id: entry.id.clone(),
                    score,
                    metadata: entry.metadata.clone(),
                }
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top k
        results.truncate(k);

        Ok(results)
    }

    fn delete(&self, id: &str) -> SemanticResult<bool> {
        let mut entries = self
            .entries
            .write()
            .map_err(|e| SemanticError::VectorDbError(format!("Lock error: {}", e)))?;

        Ok(entries.remove(id).is_some())
    }

    fn get(&self, id: &str) -> SemanticResult<Option<VectorEntry>> {
        let entries = self
            .entries
            .read()
            .map_err(|e| SemanticError::VectorDbError(format!("Lock error: {}", e)))?;

        Ok(entries.get(id).cloned())
    }

    fn len(&self) -> usize {
        self.entries.read().map(|e| e.len()).unwrap_or(0)
    }

    fn ids(&self) -> SemanticResult<Vec<String>> {
        let entries = self
            .entries
            .read()
            .map_err(|e| SemanticError::VectorDbError(format!("Lock error: {}", e)))?;

        Ok(entries.keys().cloned().collect())
    }

    fn clear(&self) -> SemanticResult<()> {
        let mut entries = self
            .entries
            .write()
            .map_err(|e| SemanticError::VectorDbError(format!("Lock error: {}", e)))?;

        entries.clear();
        Ok(())
    }
}

// ============================================================================
// Semantic Search Service
// ============================================================================

/// Search query options.
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    /// The search query text
    pub query: String,
    /// Maximum results to return
    pub limit: usize,
    /// Minimum similarity score (0.0 to 1.0)
    pub min_score: Option<f64>,
    /// Filters to apply
    pub filter: SearchFilter,
}

impl SearchQuery {
    /// Create a new search query.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            limit: 10,
            min_score: None,
            filter: SearchFilter::default(),
        }
    }

    /// Set maximum results.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set minimum score threshold.
    pub fn min_score(mut self, score: f64) -> Self {
        self.min_score = Some(score);
        self
    }

    /// Set filter.
    #[must_use]
    pub fn with_filter(mut self, filter: SearchFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Filter by package type.
    #[must_use]
    pub fn with_type(mut self, pkg_type: PackageType) -> Self {
        self.filter = self.filter.with_type(pkg_type);
        self
    }

    /// Filter by category.
    #[must_use]
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.filter = self.filter.with_category(category);
        self
    }

    /// Only return verified packages.
    pub fn verified(mut self) -> Self {
        self.filter = self.filter.verified();
        self
    }
}

/// Semantic search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Package ID
    pub package_id: String,
    /// Package name
    pub name: String,
    /// Description
    pub description: String,
    /// Similarity score (0.0 to 1.0)
    pub score: f64,
    /// Package type
    pub package_type: PackageType,
    /// Trust level
    pub trust_level: TrustLevel,
    /// Is verified
    pub verified: bool,
    /// Highlighted matching parts (if available)
    pub highlights: Vec<String>,
}

impl SearchResult {
    /// Create from vector search result.
    pub fn from_vector_result(result: VectorSearchResult) -> Self {
        Self {
            package_id: result.metadata.package_id,
            name: result.metadata.name,
            description: result.metadata.description,
            score: result.score,
            package_type: result.metadata.package_type,
            trust_level: result.metadata.trust_level,
            verified: result.metadata.verified,
            highlights: Vec::new(),
        }
    }

    /// Add highlights.
    #[must_use]
    pub fn with_highlights(mut self, highlights: Vec<String>) -> Self {
        self.highlights = highlights;
        self
    }
}

/// Semantic search service for packages.
///
/// Combines an embedding model and vector database to provide semantic search
/// over packages. Handles indexing, searching, and metadata management.
pub struct SemanticSearchService<E: EmbeddingModel, V: VectorDb> {
    embedder: E,
    vector_db: V,
    config: SemanticSearchConfig,
}

/// Configuration for semantic search service.
#[derive(Debug, Clone)]
pub struct SemanticSearchConfig {
    /// Default number of results
    pub default_limit: usize,
    /// Default minimum score
    pub default_min_score: f64,
    /// Maximum query length (characters)
    pub max_query_length: usize,
    /// Batch size for indexing
    pub batch_size: usize,
}

impl Default for SemanticSearchConfig {
    fn default() -> Self {
        Self {
            default_limit: 10,
            default_min_score: 0.0,
            max_query_length: 1000,
            batch_size: 100,
        }
    }
}

impl<E: EmbeddingModel, V: VectorDb> SemanticSearchService<E, V> {
    /// Create a new semantic search service.
    pub fn new(embedder: E, vector_db: V) -> Self {
        Self {
            embedder,
            vector_db,
            config: SemanticSearchConfig::default(),
        }
    }

    /// Create with custom configuration.
    #[must_use]
    pub fn with_config(embedder: E, vector_db: V, config: SemanticSearchConfig) -> Self {
        Self {
            embedder,
            vector_db,
            config,
        }
    }

    /// Get the embedding model name.
    pub fn model_name(&self) -> &str {
        self.embedder.model_name()
    }

    /// Get the embedding dimensions.
    pub fn dimensions(&self) -> usize {
        self.embedder.dimensions()
    }

    /// Get the number of indexed packages.
    pub fn indexed_count(&self) -> usize {
        self.vector_db.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.vector_db.is_empty()
    }

    /// Index a single package.
    pub fn index_package(&self, metadata: &PackageMetadata) -> SemanticResult<()> {
        // Generate embedding from metadata
        let text = metadata.to_embedding_text();
        let embedding = self.embedder.embed(&text)?;

        // Create entry
        let entry = VectorEntry {
            id: metadata.package_id.clone(),
            vector: embedding,
            metadata: metadata.clone(),
        };

        // Store in vector database
        self.vector_db.upsert(entry)
    }

    /// Index multiple packages.
    pub fn index_packages(&self, packages: &[PackageMetadata]) -> SemanticResult<IndexingReport> {
        let mut report = IndexingReport {
            total: packages.len(),
            indexed: 0,
            failed: 0,
            errors: Vec::new(),
        };

        // Process in batches
        for chunk in packages.chunks(self.config.batch_size) {
            let texts: Vec<String> = chunk.iter().map(|p| p.to_embedding_text()).collect();
            let text_refs: Vec<&str> = texts.iter().map(String::as_str).collect();

            match self.embedder.embed_batch(&text_refs) {
                Ok(embeddings) => {
                    let entries: Vec<VectorEntry> = chunk
                        .iter()
                        .zip(embeddings)
                        .map(|(metadata, vector)| VectorEntry {
                            id: metadata.package_id.clone(),
                            vector,
                            metadata: metadata.clone(),
                        })
                        .collect();

                    match self.vector_db.upsert_batch(entries) {
                        Ok(()) => {
                            report.indexed += chunk.len();
                        }
                        Err(e) => {
                            report.failed += chunk.len();
                            report.errors.push(format!("Batch insert failed: {}", e));
                        }
                    }
                }
                Err(e) => {
                    report.failed += chunk.len();
                    report.errors.push(format!("Batch embedding failed: {}", e));
                }
            }
        }

        Ok(report)
    }

    /// Remove a package from the index.
    pub fn remove_package(&self, package_id: &str) -> SemanticResult<bool> {
        self.vector_db.delete(package_id)
    }

    /// Search for packages semantically.
    pub fn search(&self, query: &str, limit: usize) -> SemanticResult<Vec<SearchResult>> {
        self.search_with_query(&SearchQuery::new(query).limit(limit))
    }

    /// Search with full query options.
    pub fn search_with_query(&self, query: &SearchQuery) -> SemanticResult<Vec<SearchResult>> {
        // Truncate query if too long
        let query_text: String = query
            .query
            .chars()
            .take(self.config.max_query_length)
            .collect();

        // Generate query embedding
        let embedding = self.embedder.embed(&query_text)?;

        // Search vector database
        let results = self
            .vector_db
            .search_filtered(&embedding, query.limit, &query.filter)?;

        // Convert to search results
        let min_score = query.min_score.unwrap_or(self.config.default_min_score);
        let results: Vec<SearchResult> = results
            .into_iter()
            .filter(|r| r.score >= min_score)
            .map(|r| {
                let highlights = self.extract_highlights(&query.query, &r.metadata);
                SearchResult::from_vector_result(r).with_highlights(highlights)
            })
            .collect();

        Ok(results)
    }

    /// Get a package's metadata from the index.
    pub fn get_package(&self, package_id: &str) -> SemanticResult<Option<PackageMetadata>> {
        match self.vector_db.get(package_id)? {
            Some(entry) => Ok(Some(entry.metadata)),
            None => Ok(None),
        }
    }

    /// Get all indexed package IDs.
    pub fn get_indexed_ids(&self) -> SemanticResult<Vec<String>> {
        self.vector_db.ids()
    }

    /// Clear the entire index.
    pub fn clear(&self) -> SemanticResult<()> {
        self.vector_db.clear()
    }

    /// Extract highlighting snippets from metadata.
    fn extract_highlights(&self, query: &str, metadata: &PackageMetadata) -> Vec<String> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        let mut highlights = Vec::new();

        // Check name
        if query_words
            .iter()
            .any(|w| metadata.name.to_lowercase().contains(w))
        {
            highlights.push(format!("Name: {}", metadata.name));
        }

        // Check description
        if query_words
            .iter()
            .any(|w| metadata.description.to_lowercase().contains(w))
        {
            highlights.push(format!("Description: {}", metadata.description));
        }

        // Check keywords
        for keyword in &metadata.keywords {
            if query_words
                .iter()
                .any(|w| keyword.to_lowercase().contains(w))
            {
                highlights.push(format!("Keyword: {}", keyword));
            }
        }

        highlights
    }
}

/// Report from indexing operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexingReport {
    /// Total packages attempted
    pub total: usize,
    /// Successfully indexed
    pub indexed: usize,
    /// Failed to index
    pub failed: usize,
    /// Error messages
    pub errors: Vec<String>,
}

impl IndexingReport {
    /// Check if indexing was fully successful.
    pub fn is_success(&self) -> bool {
        self.failed == 0
    }

    /// Get success rate as percentage.
    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.indexed as f64 / self.total as f64) * 100.0
        }
    }
}

// ============================================================================
// Convenience Type Aliases
// ============================================================================

/// Default semantic search service using mock embeddings and in-memory storage.
pub type DefaultSemanticSearch = SemanticSearchService<MockEmbeddingModel, InMemoryVectorDb>;

impl DefaultSemanticSearch {
    /// Create a default semantic search service for testing.
    pub fn default_for_testing() -> Self {
        Self::new(MockEmbeddingModel::new(384), InMemoryVectorDb::new())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_embedding_model() {
        let model = MockEmbeddingModel::new(384);

        // Same text should produce same embedding
        let emb1 = model.embed("test query").unwrap();
        let emb2 = model.embed("test query").unwrap();
        assert_eq!(emb1, emb2);

        // Different text should produce different embedding
        let emb3 = model.embed("different query").unwrap();
        assert_ne!(emb1, emb3);

        // Correct dimensions
        assert_eq!(emb1.len(), 384);
        assert_eq!(model.dimensions(), 384);
    }

    #[test]
    fn test_mock_embedding_normalized() {
        let model = MockEmbeddingModel::new(128);
        let embedding = model.embed("test").unwrap();

        // Check unit vector (norm should be ~1.0)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_in_memory_vector_db() {
        let db = InMemoryVectorDb::new();
        assert!(db.is_empty());

        // Insert entry
        let entry = VectorEntry {
            id: "test".to_string(),
            vector: vec![1.0, 0.0, 0.0],
            metadata: PackageMetadata::new("test/pkg", "Test Package"),
        };
        db.upsert(entry.clone()).unwrap();
        assert_eq!(db.len(), 1);

        // Get entry
        let retrieved = db.get("test").unwrap().unwrap();
        assert_eq!(retrieved.id, "test");

        // Delete entry
        assert!(db.delete("test").unwrap());
        assert!(db.is_empty());
    }

    #[test]
    fn test_vector_search() {
        let db = InMemoryVectorDb::new();

        // Insert entries with different vectors
        for i in 0..5 {
            let mut vector = vec![0.0_f32; 3];
            vector[i % 3] = 1.0;
            let entry = VectorEntry {
                id: format!("pkg{}", i),
                vector,
                metadata: PackageMetadata::new(format!("test/pkg{}", i), format!("Package {}", i)),
            };
            db.upsert(entry).unwrap();
        }

        // Search for vector [1, 0, 0]
        let query = vec![1.0, 0.0, 0.0];
        let results = db.search(&query, 3).unwrap();
        assert!(!results.is_empty());

        // First result should have highest similarity
        assert!(results[0].score >= results.last().unwrap().score);
    }

    #[test]
    fn test_search_filter() {
        let filter = SearchFilter::new()
            .with_type(PackageType::NodeLibrary)
            .with_category("nlp")
            .verified();

        // Matching metadata
        let matching = PackageMetadata::new("test/pkg", "Test")
            .with_type(PackageType::NodeLibrary)
            .with_categories(vec!["nlp", "ai"])
            .verified(true);
        assert!(filter.matches(&matching));

        // Wrong type
        let wrong_type = PackageMetadata::new("test/pkg", "Test")
            .with_type(PackageType::ToolPack)
            .with_categories(vec!["nlp"])
            .verified(true);
        assert!(!filter.matches(&wrong_type));

        // Not verified
        let not_verified = PackageMetadata::new("test/pkg", "Test")
            .with_type(PackageType::NodeLibrary)
            .with_categories(vec!["nlp"])
            .verified(false);
        assert!(!filter.matches(&not_verified));
    }

    #[test]
    fn test_package_metadata_embedding_text() {
        let metadata = PackageMetadata::new("dashflow/sentiment", "Sentiment Analysis")
            .with_description("Analyze sentiment in text")
            .with_keywords(vec!["sentiment", "nlp", "text"])
            .with_categories(vec!["nlp", "analysis"]);

        let text = metadata.to_embedding_text();
        assert!(text.contains("Sentiment Analysis"));
        assert!(text.contains("Analyze sentiment"));
        assert!(text.contains("sentiment nlp text"));
    }

    #[test]
    fn test_semantic_search_service() {
        let service = DefaultSemanticSearch::default_for_testing();
        assert!(service.is_empty());

        // Index packages
        let packages = vec![
            PackageMetadata::new("test/sentiment", "Sentiment Analysis")
                .with_description("Analyze emotions in text")
                .with_keywords(vec!["sentiment", "emotion", "nlp"]),
            PackageMetadata::new("test/translation", "Translation Tools")
                .with_description("Translate between languages")
                .with_keywords(vec!["translation", "languages", "i18n"]),
            PackageMetadata::new("test/sql", "SQL Tools")
                .with_description("Database query tools")
                .with_keywords(vec!["sql", "database", "query"]),
        ];

        let report = service.index_packages(&packages).unwrap();
        assert!(report.is_success());
        assert_eq!(report.indexed, 3);
        assert_eq!(service.indexed_count(), 3);
    }

    #[test]
    fn test_semantic_search() {
        let service = DefaultSemanticSearch::default_for_testing();

        // Index packages
        service
            .index_package(
                &PackageMetadata::new("test/sentiment", "Sentiment Analysis")
                    .with_description("Analyze emotions and feelings in text")
                    .with_keywords(vec!["sentiment", "emotion", "nlp"]),
            )
            .unwrap();

        service
            .index_package(
                &PackageMetadata::new("test/translation", "Translation")
                    .with_description("Translate between different languages")
                    .with_keywords(vec!["translation", "language"]),
            )
            .unwrap();

        // Search
        let results = service.search("emotion analysis", 5).unwrap();
        assert!(!results.is_empty());

        // All results should have scores
        for result in &results {
            assert!(result.score > 0.0 || result.score == 0.0);
        }
    }

    #[test]
    fn test_search_query_builder() {
        let query = SearchQuery::new("sentiment analysis")
            .limit(20)
            .min_score(0.5)
            .with_type(PackageType::NodeLibrary)
            .verified();

        assert_eq!(query.query, "sentiment analysis");
        assert_eq!(query.limit, 20);
        assert_eq!(query.min_score, Some(0.5));
        assert!(query.filter.verified_only);
    }

    #[test]
    fn test_remove_package() {
        let service = DefaultSemanticSearch::default_for_testing();

        service
            .index_package(&PackageMetadata::new("test/pkg", "Test"))
            .unwrap();
        assert_eq!(service.indexed_count(), 1);

        assert!(service.remove_package("test/pkg").unwrap());
        assert_eq!(service.indexed_count(), 0);

        // Remove non-existent package
        assert!(!service.remove_package("non/existent").unwrap());
    }

    #[test]
    fn test_get_package() {
        let service = DefaultSemanticSearch::default_for_testing();

        let metadata =
            PackageMetadata::new("test/pkg", "Test Package").with_description("A test package");
        service.index_package(&metadata).unwrap();

        let retrieved = service.get_package("test/pkg").unwrap().unwrap();
        assert_eq!(retrieved.package_id, "test/pkg");
        assert_eq!(retrieved.description, "A test package");

        // Non-existent
        assert!(service.get_package("non/existent").unwrap().is_none());
    }

    #[test]
    fn test_clear_index() {
        let service = DefaultSemanticSearch::default_for_testing();

        for i in 0..10 {
            service
                .index_package(&PackageMetadata::new(
                    format!("test/pkg{}", i),
                    format!("Pkg {}", i),
                ))
                .unwrap();
        }
        assert_eq!(service.indexed_count(), 10);

        service.clear().unwrap();
        assert!(service.is_empty());
    }

    #[test]
    fn test_indexing_report() {
        let report = IndexingReport {
            total: 100,
            indexed: 95,
            failed: 5,
            errors: vec!["Error 1".to_string()],
        };

        assert!(!report.is_success());
        assert!((report.success_rate() - 95.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity() {
        // Same vector should have similarity 1.0
        let v1 = vec![1.0, 0.0, 0.0];
        let sim = InMemoryVectorDb::cosine_similarity(&v1, &v1);
        assert!((sim - 1.0).abs() < 0.001);

        // Orthogonal vectors should have similarity 0.0
        let v2 = vec![0.0, 1.0, 0.0];
        let sim = InMemoryVectorDb::cosine_similarity(&v1, &v2);
        assert!(sim.abs() < 0.001);

        // Opposite vectors should have similarity -1.0
        let v3 = vec![-1.0, 0.0, 0.0];
        let sim = InMemoryVectorDb::cosine_similarity(&v1, &v3);
        assert!((sim + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_search_with_min_score() {
        let service = DefaultSemanticSearch::default_for_testing();

        // Index packages
        for i in 0..5 {
            service
                .index_package(&PackageMetadata::new(
                    format!("test/pkg{}", i),
                    format!("Package {}", i),
                ))
                .unwrap();
        }

        // Search with high min score
        let query = SearchQuery::new("test").limit(10).min_score(0.99);
        let results = service.search_with_query(&query).unwrap();

        // Should filter out low-scoring results
        for result in &results {
            assert!(result.score >= 0.99);
        }
    }

    #[test]
    fn test_search_with_filter() {
        let service = DefaultSemanticSearch::default_for_testing();

        // Index packages with different types
        service
            .index_package(
                &PackageMetadata::new("test/nodes", "Node Library")
                    .with_type(PackageType::NodeLibrary),
            )
            .unwrap();
        service
            .index_package(
                &PackageMetadata::new("test/tools", "Tool Pack").with_type(PackageType::ToolPack),
            )
            .unwrap();

        // Search only for NodeLibrary
        let query = SearchQuery::new("test").with_type(PackageType::NodeLibrary);
        let results = service.search_with_query(&query).unwrap();

        for result in &results {
            assert_eq!(result.package_type, PackageType::NodeLibrary);
        }
    }

    #[test]
    fn test_batch_embedding() {
        let model = MockEmbeddingModel::new(128);

        let texts = vec!["text 1", "text 2", "text 3"];
        let embeddings = model.embed_batch(&texts).unwrap();

        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.len(), 128);
        }

        // Different texts should have different embeddings
        assert_ne!(embeddings[0], embeddings[1]);
        assert_ne!(embeddings[1], embeddings[2]);
    }

    #[test]
    fn test_vector_db_ids() {
        let db = InMemoryVectorDb::new();

        for i in 0..5 {
            let entry = VectorEntry {
                id: format!("pkg{}", i),
                vector: vec![1.0],
                metadata: PackageMetadata::new(format!("test/pkg{}", i), format!("Pkg {}", i)),
            };
            db.upsert(entry).unwrap();
        }

        let ids = db.ids().unwrap();
        assert_eq!(ids.len(), 5);
    }

    #[test]
    fn test_extract_highlights() {
        let service = DefaultSemanticSearch::default_for_testing();

        let metadata = PackageMetadata::new("test/sentiment", "Sentiment Analysis")
            .with_description("Analyze emotions in customer feedback")
            .with_keywords(vec!["sentiment", "emotion", "nlp"]);

        let highlights = service.extract_highlights("emotion analysis", &metadata);

        // Should find matches in name and keywords
        assert!(!highlights.is_empty());
        assert!(highlights.iter().any(|h| h.contains("Analysis")));
    }

    #[test]
    fn test_error_display() {
        let err = SemanticError::DimensionMismatch {
            expected: 384,
            got: 256,
        };
        assert!(err.to_string().contains("384"));
        assert!(err.to_string().contains("256"));

        let err = SemanticError::PackageNotFound("test/pkg".to_string());
        assert!(err.to_string().contains("test/pkg"));
    }

    #[test]
    fn test_search_result_from_vector_result() {
        let vector_result = VectorSearchResult {
            id: "test/pkg".to_string(),
            score: 0.85,
            metadata: PackageMetadata::new("test/pkg", "Test Package")
                .with_description("A test package")
                .with_type(PackageType::ToolPack)
                .verified(true),
        };

        let search_result = SearchResult::from_vector_result(vector_result);
        assert_eq!(search_result.package_id, "test/pkg");
        assert_eq!(search_result.name, "Test Package");
        assert!((search_result.score - 0.85).abs() < f64::EPSILON);
        assert_eq!(search_result.package_type, PackageType::ToolPack);
        assert!(search_result.verified);
    }

    #[test]
    fn test_semantic_search_config() {
        let config = SemanticSearchConfig {
            default_limit: 20,
            default_min_score: 0.1,
            max_query_length: 500,
            batch_size: 50,
        };

        let service = SemanticSearchService::with_config(
            MockEmbeddingModel::new(128),
            InMemoryVectorDb::new(),
            config,
        );

        assert_eq!(service.dimensions(), 128);
        assert_eq!(service.model_name(), "mock-embedding-model");
    }
}
