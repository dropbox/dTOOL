//! Embeddings interface for text embedding models.
//!
//! Text embedding models map text to vectors (points in n-dimensional space).
//! Texts that are similar will usually be mapped to points that are close to each
//! other in this space.
//!
//! This module provides the core [`Embeddings`] trait that all embedding implementations
//! must implement, as well as a [`CachedEmbeddings`] wrapper for caching embeddings.

use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::core::Error;

/// Interface for embedding models.
///
/// Text embedding models are used to map text to a vector (a point in n-dimensional
/// space). Texts that are similar will usually be mapped to points that are close to
/// each other in this space. The exact details of what's considered "similar" and how
/// "distance" is measured in this space are dependent on the specific embedding model.
///
/// This trait contains methods for embedding a list of documents and a method
/// for embedding a query text. The embedding of a query text is expected to be a single
/// vector, while the embedding of a list of documents is expected to be a list of
/// vectors.
///
/// Usually the query embedding is identical to the document embedding, but the
/// abstraction allows treating them independently.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::embeddings::Embeddings;
///
/// async fn example(embedder: impl Embeddings) {
///     // Embed a single query
///     let query_vector = embedder.embed_query("What is the meaning of life?").await?;
///
///     // Embed multiple documents
///     let docs = vec![
///         "The quick brown fox".to_string(),
///         "jumps over the lazy dog".to_string(),
///     ];
///     let doc_vectors = embedder.embed_documents(&docs).await?;
/// }
/// ```
#[async_trait]
pub trait Embeddings: Send + Sync {
    /// Embed a list of documents (texts).
    ///
    /// **IMPORTANT: Application code should use [`dashflow::embed()`] instead of calling
    /// this method directly.** Direct calls bypass DashFlow's graph infrastructure and miss:
    /// - ExecutionTrace collection for optimizers
    /// - Streaming events for live progress
    /// - Introspection capabilities
    /// - Metrics collection (tokens, cost, latency)
    ///
    /// This method is intended for trait implementors. Applications should use:
    /// ```rust,ignore
    /// use dashflow::embed;
    /// let vectors = embed(embeddings, &texts).await?;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `texts` - List of text to embed
    ///
    /// # Returns
    ///
    /// List of embeddings, one for each input text. Each embedding is a vector
    /// of floats.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedding operation fails (e.g., network error,
    /// API error, invalid input).
    #[doc(hidden)]
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Error>;

    /// Embed a single query text.
    ///
    /// **IMPORTANT: Application code should use [`dashflow::embed_query()`] instead of calling
    /// this method directly.** Direct calls bypass DashFlow's graph infrastructure and miss:
    /// - ExecutionTrace collection for optimizers
    /// - Streaming events for live progress
    /// - Introspection capabilities
    /// - Metrics collection (tokens, cost, latency)
    ///
    /// This method is intended for trait implementors. Applications should use:
    /// ```rust,ignore
    /// use dashflow::embed_query;
    /// let vector = embed_query(embeddings, "query text").await?;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `text` - Text to embed
    ///
    /// # Returns
    ///
    /// A single embedding vector.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedding operation fails (e.g., network error,
    /// API error, invalid input).
    #[doc(hidden)]
    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, Error>;
}

/// Cache entry storing an embedding vector with optional expiration time.
#[derive(Clone, Debug)]
struct CacheEntry {
    /// The embedding vector
    vector: Vec<f32>,
    /// When this entry was created (for TTL calculation)
    created_at: Instant,
}

/// Metrics for tracking cache performance.
#[derive(Debug, Clone, Default)]
pub struct CacheMetrics {
    /// Number of cache hits
    pub hits: usize,
    /// Number of cache misses
    pub misses: usize,
    /// Current number of entries in cache
    pub size: usize,
}

/// Configuration for cached embeddings.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries in cache (default: 1000)
    pub max_size: usize,
    /// Time-to-live for cache entries (default: None = no expiration)
    pub ttl: Option<Duration>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size: 1000,
            ttl: None,
        }
    }
}

impl CacheConfig {
    /// Create a new cache configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum cache size.
    #[must_use]
    pub fn with_max_size(mut self, max_size: usize) -> Self {
        self.max_size = max_size;
        self
    }

    /// Set the time-to-live for cache entries.
    #[must_use]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }
}

/// Wrapper that adds caching to any Embeddings implementation.
///
/// This wrapper caches embedding vectors to avoid redundant API calls for the same text.
/// It uses a thread-safe `DashMap` for concurrent access and supports:
/// - Configurable cache size limit (LRU-like eviction when full)
/// - Optional TTL (time-to-live) for cache entries
/// - Cache metrics (hits, misses, size)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::embeddings::{CachedEmbeddings, CacheConfig};
/// use std::time::Duration;
///
/// // Wrap any embeddings implementation with caching
/// let embedder = OpenAIEmbeddings::default();
/// let cached = CachedEmbeddings::new(
///     embedder,
///     CacheConfig::new()
///         .with_max_size(5000)
///         .with_ttl(Duration::from_secs(3600))
/// );
///
/// // First call hits the API
/// let result1 = cached.embed_query("Hello world").await?;
/// // Second call returns cached result
/// let result2 = cached.embed_query("Hello world").await?;
///
/// // Check metrics
/// let metrics = cached.metrics();
/// println!("Cache hits: {}, misses: {}", metrics.hits, metrics.misses);
/// ```
pub struct CachedEmbeddings<E: Embeddings> {
    /// The underlying embeddings implementation
    inner: E,
    /// Cache mapping text to embedding vectors
    cache: Arc<DashMap<String, CacheEntry>>,
    /// Cache configuration
    config: CacheConfig,
    /// Cache hit counter
    hits: Arc<std::sync::atomic::AtomicUsize>,
    /// Cache miss counter
    misses: Arc<std::sync::atomic::AtomicUsize>,
}

impl<E: Embeddings> CachedEmbeddings<E> {
    /// Create a new cached embeddings wrapper with default configuration.
    pub fn new(inner: E, config: CacheConfig) -> Self {
        Self {
            inner,
            cache: Arc::new(DashMap::new()),
            config,
            hits: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            misses: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Create a cached embeddings wrapper with default configuration.
    #[must_use]
    pub fn with_default_config(inner: E) -> Self {
        Self::new(inner, CacheConfig::default())
    }

    /// Get cache metrics (hits, misses, current size).
    pub fn metrics(&self) -> CacheMetrics {
        CacheMetrics {
            hits: self.hits.load(std::sync::atomic::Ordering::Relaxed),
            misses: self.misses.load(std::sync::atomic::Ordering::Relaxed),
            size: self.cache.len(),
        }
    }

    /// Clear all cached entries.
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Check if a cache entry is expired based on TTL.
    fn is_expired(&self, entry: &CacheEntry) -> bool {
        if let Some(ttl) = self.config.ttl {
            entry.created_at.elapsed() > ttl
        } else {
            false
        }
    }

    /// Get a cached embedding if it exists and is not expired.
    fn get_cached(&self, text: &str) -> Option<Vec<f32>> {
        if let Some(entry) = self.cache.get(text) {
            if self.is_expired(&entry) {
                // Entry expired, remove it
                drop(entry);
                self.cache.remove(text);
                None
            } else {
                Some(entry.vector.clone())
            }
        } else {
            None
        }
    }

    /// Store an embedding in the cache, enforcing size limits.
    fn store_cached(&self, text: String, vector: Vec<f32>) {
        // Check if we're updating an existing entry
        let is_update = self.cache.contains_key(&text);

        // If cache is at max size and this is a new entry, remove an arbitrary entry
        if !is_update && self.cache.len() >= self.config.max_size {
            // Collect first key to remove (to avoid holding iterator while removing)
            let key_to_remove = self.cache.iter().next().map(|entry| entry.key().clone());
            if let Some(key) = key_to_remove {
                self.cache.remove(&key);
            }
        }

        self.cache.insert(
            text,
            CacheEntry {
                vector,
                created_at: Instant::now(),
            },
        );
    }
}

#[async_trait]
impl<E: Embeddings> Embeddings for CachedEmbeddings<E> {
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Error> {
        let mut results = Vec::with_capacity(texts.len());
        let mut uncached_indices = Vec::new();
        let mut uncached_texts = Vec::new();

        // Check cache for each text
        for (i, text) in texts.iter().enumerate() {
            if let Some(cached) = self.get_cached(text) {
                self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                results.push(Some(cached));
            } else {
                self.misses
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                results.push(None);
                uncached_indices.push(i);
                uncached_texts.push(text.clone());
            }
        }

        // If we have uncached texts, fetch them from the underlying embedder
        if !uncached_texts.is_empty() {
            let embeddings = self
                .inner
                ._embed_documents(&uncached_texts)
                .await
                .map_err(|e| {
                    Error::other(format!(
                        "Failed to embed {} uncached documents: {e}",
                        uncached_texts.len()
                    ))
                })?;

            // Store in cache and fill results
            for (idx_in_uncached, &original_idx) in uncached_indices.iter().enumerate() {
                let text = &texts[original_idx];
                let vector = embeddings[idx_in_uncached].clone();
                self.store_cached(text.clone(), vector.clone());
                results[original_idx] = Some(vector);
            }
        }

        // Collect all results (all positions guaranteed filled above)
        results
            .into_iter()
            .enumerate()
            .map(|(i, r)| {
                r.ok_or_else(|| {
                    Error::other(format!(
                        "Internal error: embedding result at index {} was not filled",
                        i
                    ))
                })
            })
            .collect()
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, Error> {
        // Check cache first
        if let Some(cached) = self.get_cached(text) {
            self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(cached);
        }

        // Cache miss - fetch from underlying embedder
        self.misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let vector = self
            .inner
            ._embed_query(text)
            .await
            .map_err(|e| Error::other(format!("Failed to embed query text: {e}")))?;

        // Store in cache
        self.store_cached(text.to_string(), vector.clone());

        Ok(vector)
    }
}

// ============================================================================
// Traced Embeddings Wrapper
// ============================================================================

/// Wrapper that adds tracing to any Embeddings implementation.
///
/// This wrapper instruments all embedding calls with OpenTelemetry spans,
/// enabling:
/// - Distributed tracing across service boundaries
/// - Performance monitoring and latency tracking
/// - Request/response metrics
/// - Error monitoring and alerting
///
/// # Span Attributes
///
/// Each traced call includes the following span attributes:
///
/// | Attribute | Description |
/// |-----------|-------------|
/// | `embeddings.operation` | "embed_documents" or "embed_query" |
/// | `embeddings.text_count` | Number of texts being embedded |
/// | `embeddings.duration_ms` | Call duration in milliseconds |
/// | `embeddings.success` | Whether the call succeeded |
/// | `embeddings.dimension` | Embedding dimension (if successful) |
/// | `service.name` | Service name (if configured) |
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::embeddings::{TracedEmbeddings, EmbeddingsTracedExt};
/// use dashflow_openai::OpenAIEmbeddings;
///
/// // Wrap any embeddings implementation with tracing
/// let embedder = OpenAIEmbeddings::default();
/// let traced = embedder.with_tracing();
///
/// // Or with a custom service name
/// let traced = embedder.with_tracing_named("document-processor");
///
/// // All calls are now automatically traced
/// let vector = traced.embed_query("Hello, world!").await?;
/// ```
pub struct TracedEmbeddings<E: Embeddings> {
    /// The underlying embeddings implementation
    inner: E,
    /// Optional service name for trace attribution
    service_name: Option<String>,
}

impl<E: Embeddings> TracedEmbeddings<E> {
    /// Create a new `TracedEmbeddings` wrapping the given embeddings.
    ///
    /// # Arguments
    ///
    /// * `inner` - The underlying embeddings implementation to wrap
    pub fn new(inner: E) -> Self {
        Self {
            inner,
            service_name: None,
        }
    }

    /// Create a new `TracedEmbeddings` with a service name.
    ///
    /// The service name is included in trace attributes for easier filtering
    /// and grouping in observability platforms.
    ///
    /// # Arguments
    ///
    /// * `inner` - The underlying embeddings implementation to wrap
    /// * `service_name` - Name to identify this service in traces
    #[must_use]
    pub fn with_service_name(inner: E, service_name: impl Into<String>) -> Self {
        Self {
            inner,
            service_name: Some(service_name.into()),
        }
    }

    /// Get a reference to the underlying embeddings.
    #[must_use]
    pub fn inner(&self) -> &E {
        &self.inner
    }

    /// Get the service name, if configured.
    #[must_use]
    pub fn service_name(&self) -> Option<&str> {
        self.service_name.as_deref()
    }
}

#[async_trait]
impl<E: Embeddings> Embeddings for TracedEmbeddings<E> {
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Error> {
        use tracing::{info_span, Instrument};

        let text_count = texts.len();
        let start = Instant::now();

        let span = if let Some(ref service) = self.service_name {
            info_span!(
                "embeddings.embed_documents",
                embeddings.operation = "embed_documents",
                embeddings.text_count = text_count,
                service.name = service.as_str(),
                embeddings.duration_ms = tracing::field::Empty,
                embeddings.success = tracing::field::Empty,
                embeddings.dimension = tracing::field::Empty,
            )
        } else {
            info_span!(
                "embeddings.embed_documents",
                embeddings.operation = "embed_documents",
                embeddings.text_count = text_count,
                embeddings.duration_ms = tracing::field::Empty,
                embeddings.success = tracing::field::Empty,
                embeddings.dimension = tracing::field::Empty,
            )
        };

        let result = async { self.inner._embed_documents(texts).await }
            .instrument(span.clone())
            .await;

        let duration_ms = start.elapsed().as_millis() as u64;
        let success = result.is_ok();

        span.record("embeddings.duration_ms", duration_ms);
        span.record("embeddings.success", success);

        if let Ok(ref embeddings) = result {
            if let Some(first) = embeddings.first() {
                span.record("embeddings.dimension", first.len() as u64);
            }
            tracing::info!(
                parent: &span,
                duration_ms = duration_ms,
                text_count = text_count,
                "Embedding documents completed"
            );
        } else {
            tracing::warn!(
                parent: &span,
                duration_ms = duration_ms,
                error = ?result.as_ref().err(),
                "Embedding documents failed"
            );
        }

        result
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, Error> {
        use tracing::{info_span, Instrument};

        let text_len = text.len();
        let start = Instant::now();

        let span = if let Some(ref service) = self.service_name {
            info_span!(
                "embeddings.embed_query",
                embeddings.operation = "embed_query",
                embeddings.text_len = text_len,
                service.name = service.as_str(),
                embeddings.duration_ms = tracing::field::Empty,
                embeddings.success = tracing::field::Empty,
                embeddings.dimension = tracing::field::Empty,
            )
        } else {
            info_span!(
                "embeddings.embed_query",
                embeddings.operation = "embed_query",
                embeddings.text_len = text_len,
                embeddings.duration_ms = tracing::field::Empty,
                embeddings.success = tracing::field::Empty,
                embeddings.dimension = tracing::field::Empty,
            )
        };

        let result = async { self.inner._embed_query(text).await }
            .instrument(span.clone())
            .await;

        let duration_ms = start.elapsed().as_millis() as u64;
        let success = result.is_ok();

        span.record("embeddings.duration_ms", duration_ms);
        span.record("embeddings.success", success);

        if let Ok(ref vector) = result {
            span.record("embeddings.dimension", vector.len() as u64);
            tracing::info!(
                parent: &span,
                duration_ms = duration_ms,
                "Embedding query completed"
            );
        } else {
            tracing::warn!(
                parent: &span,
                duration_ms = duration_ms,
                error = ?result.as_ref().err(),
                "Embedding query failed"
            );
        }

        result
    }
}

/// Extension trait adding tracing support to `Embeddings`.
///
/// This trait is automatically implemented for all types that implement `Embeddings`,
/// providing convenient methods to wrap embeddings with automatic tracing.
pub trait EmbeddingsTracedExt: Embeddings + Sized {
    /// Wrap this embeddings with automatic tracing.
    ///
    /// Returns a `TracedEmbeddings` that instruments all calls with OpenTelemetry spans.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let traced = embeddings.with_tracing();
    /// ```
    fn with_tracing(self) -> TracedEmbeddings<Self> {
        TracedEmbeddings::new(self)
    }

    /// Wrap this embeddings with automatic tracing and a custom service name.
    ///
    /// The service name is included in span attributes for easier filtering.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let traced = embeddings.with_tracing_named("document-processor");
    /// ```
    fn with_tracing_named(self, service_name: impl Into<String>) -> TracedEmbeddings<Self> {
        TracedEmbeddings::with_service_name(self, service_name)
    }
}

/// Blanket implementation of `EmbeddingsTracedExt` for all `Embeddings` implementations.
impl<E: Embeddings + Sized> EmbeddingsTracedExt for E {}

// ============================================================================
// Mock Embeddings
// ============================================================================

/// Mock embeddings implementation for testing and examples.
///
/// This embeddings implementation generates deterministic pseudo-random vectors
/// based on the input text. It's useful for testing vector stores and other
/// components without requiring a real embeddings service.
///
/// # Example
///
/// ```rust
/// use dashflow::core::embeddings::MockEmbeddings;
/// use dashflow::{embed, embed_query};
/// use std::sync::Arc;
///
/// # async fn example() -> dashflow::Result<()> {
/// // Create mock embeddings with 384 dimensions
/// let embeddings = Arc::new(MockEmbeddings::new(384));
///
/// // Embed a query
/// let vector = embed_query(embeddings.clone(), "Hello, world!").await?;
/// assert_eq!(vector.len(), 384);
///
/// // Embed multiple documents
/// let docs = vec!["doc1".to_string(), "doc2".to_string()];
/// let vectors = embed(embeddings, &docs).await?;
/// assert_eq!(vectors.len(), 2);
/// assert_eq!(vectors[0].len(), 384);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MockEmbeddings {
    /// Dimension of the embedding vectors
    dimension: usize,
}

impl MockEmbeddings {
    /// Create a new `MockEmbeddings` with the specified dimension.
    ///
    /// # Arguments
    ///
    /// * `dimension` - The dimension of the embedding vectors to generate
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow::core::embeddings::MockEmbeddings;
    ///
    /// let embeddings = MockEmbeddings::new(768);
    /// ```
    #[must_use]
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    /// Generate a deterministic pseudo-random vector for a given text.
    ///
    /// The vector is generated by using the text's bytes as a seed for a simple
    /// pseudo-random number generator. This ensures the same text always produces
    /// the same vector.
    fn generate_vector(&self, text: &str) -> Vec<f32> {
        let mut rng_state = text.bytes().fold(0u64, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(u64::from(b))
        });

        (0..self.dimension)
            .map(|_| {
                // Simple LCG (Linear Congruential Generator)
                rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                let value = (rng_state / 65536) % 32768;
                // Normalize to [-1, 1] range
                (value as f32 / 16384.0) - 1.0
            })
            .collect()
    }
}

#[async_trait]
impl Embeddings for MockEmbeddings {
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Error> {
        Ok(texts
            .iter()
            .map(|text| self.generate_vector(text))
            .collect())
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, Error> {
        Ok(self.generate_vector(text))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CacheConfig, CacheEntry, CacheMetrics, CachedEmbeddings, EmbeddingsTracedExt,
        MockEmbeddings, TracedEmbeddings,
    };
    use crate::test_prelude::*;

    /// Simple mock embeddings for internal cache tests
    struct SimpleMockEmbeddings {
        /// Counter for tracking how many times embed_documents was called
        call_count: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl SimpleMockEmbeddings {
        fn new() -> Self {
            Self {
                call_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }
        }

        fn calls(&self) -> usize {
            self.call_count.load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl Embeddings for SimpleMockEmbeddings {
        async fn _embed_documents(&self, texts: &[String]) -> StdResult<Vec<Vec<f32>>, Error> {
            self.call_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            // Return simple embeddings based on text length
            Ok(texts
                .iter()
                .map(|text| vec![text.len() as f32, text.chars().count() as f32])
                .collect())
        }

        async fn _embed_query(&self, text: &str) -> StdResult<Vec<f32>, Error> {
            self.call_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(vec![text.len() as f32, text.chars().count() as f32])
        }
    }

    #[tokio::test]
    async fn test_cache_hit_single_query() {
        let mock = SimpleMockEmbeddings::new();
        let cached = CachedEmbeddings::with_default_config(mock);

        // First call should miss cache and call underlying embedder
        let result1 = cached._embed_query("hello").await.unwrap();
        assert_eq!(result1, vec![5.0, 5.0]); // "hello" has 5 chars

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 0);
        assert_eq!(metrics.misses, 1);
        assert_eq!(metrics.size, 1);

        // Second call should hit cache
        let result2 = cached._embed_query("hello").await.unwrap();
        assert_eq!(result2, vec![5.0, 5.0]);

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.misses, 1);
        assert_eq!(metrics.size, 1);

        // Underlying embedder should only be called once
        assert_eq!(cached.inner.calls(), 1);
    }

    #[tokio::test]
    async fn test_cache_hit_batch_documents() {
        let mock = SimpleMockEmbeddings::new();
        let cached = CachedEmbeddings::with_default_config(mock);

        let texts = vec!["hello".to_string(), "world".to_string()];

        // First call should miss cache
        let result1 = cached._embed_documents(&texts).await.unwrap();
        assert_eq!(result1.len(), 2);
        assert_eq!(result1[0], vec![5.0, 5.0]); // "hello"
        assert_eq!(result1[1], vec![5.0, 5.0]); // "world"

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 0);
        assert_eq!(metrics.misses, 2);
        assert_eq!(metrics.size, 2);

        // Second call should hit cache for both
        let result2 = cached._embed_documents(&texts).await.unwrap();
        assert_eq!(result2.len(), 2);

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 2);
        assert_eq!(metrics.misses, 2);
        assert_eq!(metrics.size, 2);

        // Underlying embedder should only be called once
        assert_eq!(cached.inner.calls(), 1);
    }

    #[tokio::test]
    async fn test_cache_partial_hit() {
        let mock = SimpleMockEmbeddings::new();
        let cached = CachedEmbeddings::with_default_config(mock);

        // Cache "hello"
        let _ = cached._embed_query("hello").await.unwrap();

        // Now embed batch with one cached and one uncached
        let texts = vec!["hello".to_string(), "world".to_string()];
        let result = cached._embed_documents(&texts).await.unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], vec![5.0, 5.0]); // "hello" - cached
        assert_eq!(result[1], vec![5.0, 5.0]); // "world" - new

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 1); // "hello" was cached
        assert_eq!(metrics.misses, 2); // initial "hello" + "world"
        assert_eq!(metrics.size, 2);

        // Underlying embedder called twice: once for "hello", once for ["world"]
        assert_eq!(cached.inner.calls(), 2);
    }

    #[tokio::test]
    async fn test_cache_size_limit() {
        let mock = SimpleMockEmbeddings::new();
        let config = CacheConfig::new().with_max_size(2);
        let cached = CachedEmbeddings::new(mock, config);

        // Add 3 entries (more than max_size)
        let _ = cached._embed_query("first").await.unwrap();
        let _ = cached._embed_query("second").await.unwrap();
        let _ = cached._embed_query("third").await.unwrap();

        let metrics = cached.metrics();
        // Cache should evict oldest entry when full
        assert_eq!(metrics.size, 2); // Only 2 entries remain
        assert_eq!(metrics.misses, 3); // All 3 were misses initially
    }

    #[tokio::test]
    async fn test_cache_ttl_expiration() {
        let mock = SimpleMockEmbeddings::new();
        let config = CacheConfig::new().with_ttl(Duration::from_millis(50));
        let cached = CachedEmbeddings::new(mock, config);

        // Cache an entry
        let result1 = cached._embed_query("hello").await.unwrap();
        assert_eq!(result1, vec![5.0, 5.0]);

        let metrics = cached.metrics();
        assert_eq!(metrics.misses, 1);
        assert_eq!(metrics.size, 1);

        // Wait for TTL to expire
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should miss cache due to expiration
        let result2 = cached._embed_query("hello").await.unwrap();
        assert_eq!(result2, vec![5.0, 5.0]);

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 0); // No hits, expired
        assert_eq!(metrics.misses, 2); // Two misses
        assert_eq!(metrics.size, 1); // Re-cached after expiration

        // Underlying embedder called twice (initial + after expiration)
        assert_eq!(cached.inner.calls(), 2);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let mock = SimpleMockEmbeddings::new();
        let cached = CachedEmbeddings::with_default_config(mock);

        // Cache some entries
        let _ = cached._embed_query("hello").await.unwrap();
        let _ = cached._embed_query("world").await.unwrap();

        let metrics = cached.metrics();
        assert_eq!(metrics.size, 2);

        // Clear cache
        cached.clear_cache();

        let metrics = cached.metrics();
        assert_eq!(metrics.size, 0);

        // Next call should miss cache
        let _ = cached._embed_query("hello").await.unwrap();
        let metrics = cached.metrics();
        assert_eq!(metrics.misses, 3); // Original 2 misses + 1 after clear
    }

    #[tokio::test]
    async fn test_cache_with_different_texts() {
        let mock = SimpleMockEmbeddings::new();
        let cached = CachedEmbeddings::with_default_config(mock);

        // Each unique text should be cached separately
        let _ = cached._embed_query("a").await.unwrap();
        let _ = cached._embed_query("b").await.unwrap();
        let _ = cached._embed_query("a").await.unwrap(); // Cache hit

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.misses, 2);
        assert_eq!(metrics.size, 2);

        // Underlying embedder called twice (for "a" and "b")
        assert_eq!(cached.inner.calls(), 2);
    }

    #[test]
    fn test_cache_config_builder() {
        let config = CacheConfig::new()
            .with_max_size(5000)
            .with_ttl(Duration::from_secs(3600));

        assert_eq!(config.max_size, 5000);
        assert_eq!(config.ttl, Some(Duration::from_secs(3600)));
    }

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert_eq!(config.max_size, 1000);
        assert_eq!(config.ttl, None);
    }

    // ============================================================================
    // Additional comprehensive tests for embeddings module
    // ============================================================================

    #[tokio::test]
    async fn test_mock_embeddings_deterministic() {
        let embeddings = MockEmbeddings::new(128);

        // Same text should always produce same vector
        let v1 = embeddings._embed_query("test").await.unwrap();
        let v2 = embeddings._embed_query("test").await.unwrap();
        assert_eq!(v1, v2);

        // Different text should produce different vector
        let v3 = embeddings._embed_query("different").await.unwrap();
        assert_ne!(v1, v3);
    }

    #[tokio::test]
    async fn test_mock_embeddings_dimensions() {
        let embeddings = MockEmbeddings::new(768);
        let vector = embeddings._embed_query("hello").await.unwrap();
        assert_eq!(vector.len(), 768);

        // Test with different dimension
        let embeddings2 = MockEmbeddings::new(384);
        let vector2 = embeddings2._embed_query("hello").await.unwrap();
        assert_eq!(vector2.len(), 384);
    }

    #[tokio::test]
    async fn test_mock_embeddings_batch() {
        let embeddings = MockEmbeddings::new(256);
        let texts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let vectors = embeddings._embed_documents(&texts).await.unwrap();

        assert_eq!(vectors.len(), 3);
        assert_eq!(vectors[0].len(), 256);
        assert_eq!(vectors[1].len(), 256);
        assert_eq!(vectors[2].len(), 256);

        // Each should be different
        assert_ne!(vectors[0], vectors[1]);
        assert_ne!(vectors[1], vectors[2]);
    }

    #[tokio::test]
    async fn test_mock_embeddings_empty_string() {
        let embeddings = MockEmbeddings::new(128);
        let vector = embeddings._embed_query("").await.unwrap();
        assert_eq!(vector.len(), 128);
        // Should still generate valid vector
    }

    #[tokio::test]
    async fn test_mock_embeddings_very_long_text() {
        let embeddings = MockEmbeddings::new(128);
        let long_text = "a".repeat(10000);
        let vector = embeddings._embed_query(&long_text).await.unwrap();
        assert_eq!(vector.len(), 128);
    }

    #[tokio::test]
    async fn test_mock_embeddings_unicode() {
        let embeddings = MockEmbeddings::new(128);
        let texts = vec![
            "Hello 世界".to_string(),
            "🚀 Emoji test".to_string(),
            "Ñoño café".to_string(),
        ];
        let vectors = embeddings._embed_documents(&texts).await.unwrap();
        assert_eq!(vectors.len(), 3);
        for v in vectors {
            assert_eq!(v.len(), 128);
        }
    }

    #[tokio::test]
    async fn test_cache_empty_batch() {
        let mock = SimpleMockEmbeddings::new();
        let cached = CachedEmbeddings::with_default_config(mock);

        let texts: Vec<String> = vec![];
        let result = cached._embed_documents(&texts).await.unwrap();
        assert_eq!(result.len(), 0);

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 0);
        assert_eq!(metrics.misses, 0);
    }

    #[tokio::test]
    async fn test_cache_update_existing_entry() {
        let mock = SimpleMockEmbeddings::new();
        let config = CacheConfig::new().with_ttl(Duration::from_secs(10));
        let cached = CachedEmbeddings::new(mock, config);

        // Cache an entry
        let _ = cached._embed_query("test").await.unwrap();
        let metrics = cached.metrics();
        assert_eq!(metrics.size, 1);

        // Clear underlying call count
        cached
            .inner
            .call_count
            .store(0, std::sync::atomic::Ordering::Relaxed);

        // Wait a bit but not enough to expire
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Access again - should be a cache hit
        let _ = cached._embed_query("test").await.unwrap();
        assert_eq!(cached.inner.calls(), 0); // No new calls

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.size, 1); // Still 1 entry
    }

    #[tokio::test]
    async fn test_cache_concurrent_access() {
        use tokio::task;

        let mock = SimpleMockEmbeddings::new();
        let cached = Arc::new(CachedEmbeddings::with_default_config(mock));

        // Spawn multiple tasks accessing same text concurrently
        let mut handles = vec![];
        for _ in 0..10 {
            let cached_clone = Arc::clone(&cached);
            handles.push(task::spawn(async move {
                cached_clone._embed_query("concurrent").await.unwrap()
            }));
        }

        // Wait for all to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert_eq!(result.len(), 2);
        }

        // Should have high cache hit rate
        let metrics = cached.metrics();
        assert!(metrics.hits >= 5); // Most should be cache hits
    }

    #[tokio::test]
    async fn test_cache_max_size_eviction() {
        let mock = SimpleMockEmbeddings::new();
        let config = CacheConfig::new().with_max_size(3);
        let cached = CachedEmbeddings::new(mock, config);

        // Add exactly max_size entries
        let _ = cached._embed_query("one").await.unwrap();
        let _ = cached._embed_query("two").await.unwrap();
        let _ = cached._embed_query("three").await.unwrap();

        let metrics = cached.metrics();
        assert_eq!(metrics.size, 3);

        // Add one more - should evict oldest
        let _ = cached._embed_query("four").await.unwrap();

        let metrics = cached.metrics();
        assert_eq!(metrics.size, 3); // Still at max size
    }

    #[tokio::test]
    async fn test_cache_metrics_consistency() {
        let mock = SimpleMockEmbeddings::new();
        let cached = CachedEmbeddings::with_default_config(mock);

        // Perform operations
        let _ = cached._embed_query("a").await.unwrap(); // miss
        let _ = cached._embed_query("a").await.unwrap(); // hit
        let _ = cached._embed_query("b").await.unwrap(); // miss
        let _ = cached._embed_query("a").await.unwrap(); // hit
        let _ = cached._embed_query("b").await.unwrap(); // hit

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 3);
        assert_eq!(metrics.misses, 2);
        assert_eq!(metrics.size, 2);
        assert_eq!(metrics.hits + metrics.misses, 5); // Total accesses
    }

    #[tokio::test]
    async fn test_cache_query_vs_documents() {
        let mock = SimpleMockEmbeddings::new();
        let cached = CachedEmbeddings::with_default_config(mock);

        // Cache via query
        let query_result = cached._embed_query("shared").await.unwrap();

        // Access via documents - should hit cache
        let doc_result = cached
            ._embed_documents(&[String::from("shared")])
            .await
            .unwrap();

        assert_eq!(query_result, doc_result[0]);

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 1); // documents hit the cache from query
        assert_eq!(metrics.misses, 1); // only initial query was a miss
    }

    #[tokio::test]
    async fn test_cache_documents_vs_query() {
        let mock = SimpleMockEmbeddings::new();
        let cached = CachedEmbeddings::with_default_config(mock);

        // Cache via documents
        let doc_result = cached
            ._embed_documents(&[String::from("shared")])
            .await
            .unwrap();

        // Access via query - should hit cache
        let query_result = cached._embed_query("shared").await.unwrap();

        assert_eq!(query_result, doc_result[0]);

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 1); // query hit the cache from documents
        assert_eq!(metrics.misses, 1); // only initial documents was a miss
    }

    #[tokio::test]
    async fn test_cache_no_ttl() {
        let mock = SimpleMockEmbeddings::new();
        let config = CacheConfig::new().with_ttl(Duration::ZERO); // Zero TTL
        let cached = CachedEmbeddings::new(mock, config);

        // First call
        let _ = cached._embed_query("test").await.unwrap();

        // Should immediately expire
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Second call should miss (expired)
        let _ = cached._embed_query("test").await.unwrap();

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 0);
        assert_eq!(metrics.misses, 2);
    }

    #[tokio::test]
    async fn test_cache_large_batch_partial_hit() {
        let mock = SimpleMockEmbeddings::new();
        let cached = CachedEmbeddings::with_default_config(mock);

        // Cache some entries
        let _ = cached._embed_query("a").await.unwrap();
        let _ = cached._embed_query("c").await.unwrap();
        let _ = cached._embed_query("e").await.unwrap();

        // Batch with mixed cached and uncached
        let texts = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
        ];
        let result = cached._embed_documents(&texts).await.unwrap();

        assert_eq!(result.len(), 5);

        let metrics = cached.metrics();
        assert_eq!(metrics.hits, 3); // a, c, e
        assert_eq!(metrics.misses, 5); // initial a,c,e + batch b,d
    }

    #[tokio::test]
    async fn test_mock_embeddings_zero_dimension() {
        let embeddings = MockEmbeddings::new(0);
        let vector = embeddings._embed_query("test").await.unwrap();
        assert_eq!(vector.len(), 0);
    }

    #[tokio::test]
    async fn test_mock_embeddings_large_dimension() {
        let embeddings = MockEmbeddings::new(4096);
        let vector = embeddings._embed_query("test").await.unwrap();
        assert_eq!(vector.len(), 4096);
    }

    #[test]
    fn test_cache_entry_debug() {
        let entry = CacheEntry {
            vector: vec![1.0, 2.0, 3.0],
            created_at: Instant::now(),
        };
        let debug_str = format!("{:?}", entry);
        assert!(debug_str.contains("CacheEntry"));
    }

    #[test]
    fn test_cache_metrics_debug() {
        let metrics = CacheMetrics {
            hits: 10,
            misses: 5,
            size: 8,
        };
        let debug_str = format!("{:?}", metrics);
        assert!(debug_str.contains("hits"));
        assert!(debug_str.contains("10"));
    }

    #[test]
    fn test_cache_config_debug() {
        let config = CacheConfig::new()
            .with_max_size(100)
            .with_ttl(Duration::from_secs(60));
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("CacheConfig"));
        assert!(debug_str.contains("100"));
    }

    #[test]
    fn test_mock_embeddings_debug() {
        let embeddings = MockEmbeddings::new(256);
        let debug_str = format!("{:?}", embeddings);
        assert!(debug_str.contains("MockEmbeddings"));
        assert!(debug_str.contains("256"));
    }

    #[test]
    fn test_mock_embeddings_clone() {
        let embeddings1 = MockEmbeddings::new(128);
        let embeddings2 = embeddings1.clone();
        assert_eq!(embeddings1.dimension, embeddings2.dimension);
    }

    #[test]
    fn test_cache_config_clone() {
        let config1 = CacheConfig::new().with_max_size(500);
        let config2 = config1.clone();
        assert_eq!(config1.max_size, config2.max_size);
    }

    #[test]
    fn test_cache_metrics_clone() {
        let metrics1 = CacheMetrics {
            hits: 5,
            misses: 3,
            size: 2,
        };
        let metrics2 = metrics1.clone();
        assert_eq!(metrics1.hits, metrics2.hits);
        assert_eq!(metrics1.misses, metrics2.misses);
        assert_eq!(metrics1.size, metrics2.size);
    }

    #[test]
    fn test_cache_metrics_default() {
        let metrics = CacheMetrics::default();
        assert_eq!(metrics.hits, 0);
        assert_eq!(metrics.misses, 0);
        assert_eq!(metrics.size, 0);
    }

    #[tokio::test]
    async fn test_cache_same_text_different_case() {
        let mock = SimpleMockEmbeddings::new();
        let cached = CachedEmbeddings::with_default_config(mock);

        let _ = cached._embed_query("Hello").await.unwrap();
        let _ = cached._embed_query("hello").await.unwrap();

        let metrics = cached.metrics();
        // Different strings, should be 2 separate cache entries
        assert_eq!(metrics.size, 2);
        assert_eq!(metrics.misses, 2);
    }

    #[tokio::test]
    async fn test_cache_whitespace_matters() {
        let mock = SimpleMockEmbeddings::new();
        let cached = CachedEmbeddings::with_default_config(mock);

        let _ = cached._embed_query("hello").await.unwrap();
        let _ = cached._embed_query("hello ").await.unwrap();
        let _ = cached._embed_query(" hello").await.unwrap();

        let metrics = cached.metrics();
        // Whitespace differences create separate entries
        assert_eq!(metrics.size, 3);
        assert_eq!(metrics.misses, 3);
    }

    #[tokio::test]
    async fn test_cache_clear_resets_size_not_counters() {
        let mock = SimpleMockEmbeddings::new();
        let cached = CachedEmbeddings::with_default_config(mock);

        let _ = cached._embed_query("a").await.unwrap();
        let _ = cached._embed_query("a").await.unwrap(); // hit

        let metrics_before = cached.metrics();
        assert_eq!(metrics_before.hits, 1);
        assert_eq!(metrics_before.misses, 1);
        assert_eq!(metrics_before.size, 1);

        cached.clear_cache();

        let metrics_after = cached.metrics();
        assert_eq!(metrics_after.hits, 1); // Counters not reset
        assert_eq!(metrics_after.misses, 1);
        assert_eq!(metrics_after.size, 0); // Size reset to 0
    }

    #[tokio::test]
    async fn test_mock_embeddings_special_characters() {
        let embeddings = MockEmbeddings::new(128);
        let texts = vec![
            "\n\t\r".to_string(),
            "line1\nline2".to_string(),
            "\"quotes\"".to_string(),
        ];
        let vectors = embeddings._embed_documents(&texts).await.unwrap();
        assert_eq!(vectors.len(), 3);
        for v in vectors {
            assert_eq!(v.len(), 128);
        }
    }

    #[tokio::test]
    async fn test_cache_size_one() {
        let mock = SimpleMockEmbeddings::new();
        let config = CacheConfig::new().with_max_size(1);
        let cached = CachedEmbeddings::new(mock, config);

        let _ = cached._embed_query("first").await.unwrap();
        let metrics = cached.metrics();
        assert_eq!(metrics.size, 1);

        let _ = cached._embed_query("second").await.unwrap();
        let metrics = cached.metrics();
        assert_eq!(metrics.size, 1); // Still 1, evicted "first"
    }

    #[tokio::test]
    async fn test_cache_very_large_size() {
        let mock = SimpleMockEmbeddings::new();
        let config = CacheConfig::new().with_max_size(10000);
        let cached = CachedEmbeddings::new(mock, config);

        // Add many entries
        for i in 0..100 {
            let _ = cached._embed_query(&format!("text_{}", i)).await.unwrap();
        }

        let metrics = cached.metrics();
        assert_eq!(metrics.size, 100); // All fit in cache
        assert_eq!(metrics.misses, 100);
    }

    #[tokio::test]
    async fn test_mock_embeddings_numeric_strings() {
        let embeddings = MockEmbeddings::new(64);
        let texts = vec![
            "123".to_string(),
            "456".to_string(),
            "0".to_string(),
            "-1".to_string(),
        ];
        let vectors = embeddings._embed_documents(&texts).await.unwrap();
        assert_eq!(vectors.len(), 4);

        // Each numeric string should produce different vector
        assert_ne!(vectors[0], vectors[1]);
        assert_ne!(vectors[1], vectors[2]);
    }

    // ========================================================================
    // TracedEmbeddings Tests
    // ========================================================================

    #[tokio::test]
    async fn test_traced_embeddings_query() {
        // Initialize tracing for test (ignore errors if already initialized)
        let _ = tracing_subscriber::fmt::try_init();

        let embeddings = MockEmbeddings::new(128);
        let traced = TracedEmbeddings::new(embeddings);

        let result = traced._embed_query("test query").await;
        assert!(result.is_ok());
        let vector = result.unwrap();
        assert_eq!(vector.len(), 128);
    }

    #[tokio::test]
    async fn test_traced_embeddings_documents() {
        let _ = tracing_subscriber::fmt::try_init();

        let embeddings = MockEmbeddings::new(256);
        let traced = TracedEmbeddings::new(embeddings);

        let texts = vec![
            "document 1".to_string(),
            "document 2".to_string(),
            "document 3".to_string(),
        ];
        let result = traced._embed_documents(&texts).await;
        assert!(result.is_ok());
        let vectors = result.unwrap();
        assert_eq!(vectors.len(), 3);
        for v in vectors {
            assert_eq!(v.len(), 256);
        }
    }

    #[tokio::test]
    async fn test_traced_embeddings_with_service_name() {
        let _ = tracing_subscriber::fmt::try_init();

        let embeddings = MockEmbeddings::new(64);
        let traced = TracedEmbeddings::with_service_name(embeddings, "test-service");

        assert_eq!(traced.service_name(), Some("test-service"));

        let result = traced._embed_query("test").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_traced_embeddings_extension_trait() {
        let _ = tracing_subscriber::fmt::try_init();

        let embeddings = MockEmbeddings::new(64);
        let traced = embeddings.with_tracing();

        let result = traced._embed_query("extension trait test").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_traced_embeddings_extension_trait_named() {
        let _ = tracing_subscriber::fmt::try_init();

        let embeddings = MockEmbeddings::new(64);
        let traced = embeddings.with_tracing_named("named-service");

        assert_eq!(traced.service_name(), Some("named-service"));
        let result = traced._embed_query("named test").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_traced_embeddings_inner_access() {
        let embeddings = MockEmbeddings::new(512);
        let traced = TracedEmbeddings::new(embeddings);

        // Access inner embeddings
        let inner = traced.inner();
        assert_eq!(inner._embed_query("test").await.unwrap().len(), 512);
    }
}
