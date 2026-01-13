//! Application State
//!
//! Shared state for all API handlers, including services and configuration.

#[cfg(feature = "metrics")]
use crate::RegistryMetrics;
use crate::{
    ApiKeyStore, CacheConfig, CacheStore, ContributionStore, InMemoryCacheStore,
    InMemoryMetadataStore, InMemoryStorage, InMemoryVectorStore, Keyring, MetadataStore,
    MockEmbedder, PackageCache, PackageInfo, SemanticSearchService, StorageBackend, TrustService,
    VectorMatch,
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

// =============================================================================
// Environment Variable Constants
// =============================================================================
// M-153: Import centralized env var constants from dashflow core.
use dashflow::core::config_loader::env_vars::{
    env_string, EMBEDDING_DIMENSION, OPENAI_API_KEY, OPENAI_EMBEDDING_MODEL, QDRANT_COLLECTION,
    QDRANT_URL, STORAGE_PATH,
};
#[cfg(feature = "s3")]
use dashflow::core::config_loader::env_vars::{
    R2_ACCOUNT_ID, S3_BUCKET, S3_ENDPOINT, S3_PATH_STYLE, S3_PREFIX, S3_REGION, S3_STORAGE_TYPE,
};

/// Type-erased semantic search service for dynamic dispatch.
///
/// This allows the AppState to use either mock or production embeddings
/// without requiring generic type parameters on AppState.
#[async_trait]
pub trait SearchService: Send + Sync {
    /// Index a package for semantic search.
    async fn index(&self, package: &PackageInfo) -> crate::Result<()>;

    /// Search for packages semantically.
    async fn search(&self, query: &str, limit: usize) -> crate::Result<Vec<VectorMatch>>;

    /// Remove a package from the index.
    async fn remove(&self, hash: &str) -> crate::Result<bool>;
}

/// Wrapper to implement SearchService for SemanticSearchService<E, V>
pub struct SearchServiceWrapper<E, V>
where
    E: crate::Embedder + 'static,
    V: crate::VectorStore + 'static,
{
    inner: SemanticSearchService<E, V>,
}

impl<E, V> SearchServiceWrapper<E, V>
where
    E: crate::Embedder + 'static,
    V: crate::VectorStore + 'static,
{
    pub fn new(service: SemanticSearchService<E, V>) -> Self {
        Self { inner: service }
    }
}

#[async_trait]
impl<E, V> SearchService for SearchServiceWrapper<E, V>
where
    E: crate::Embedder + 'static,
    V: crate::VectorStore + 'static,
{
    async fn index(&self, package: &PackageInfo) -> crate::Result<()> {
        self.inner.index(package).await
    }

    async fn search(&self, query: &str, limit: usize) -> crate::Result<Vec<VectorMatch>> {
        self.inner.search(query, limit).await
    }

    async fn remove(&self, hash: &str) -> crate::Result<bool> {
        self.inner.remove(hash).await
    }
}

/// Shared application state for all handlers
#[derive(Clone)]
pub struct AppState {
    /// Trust service for signature verification
    pub trust: Arc<TrustService>,
    /// Primary storage backend (S3, filesystem, etc.)
    pub storage: Arc<dyn StorageBackend>,
    /// Package cache (binary content) - local read-through cache
    pub cache: Arc<RwLock<PackageCache>>,
    /// Data cache for API keys, resolutions, etc.
    pub data_cache: Arc<dyn CacheStore>,
    /// Semantic search service (type-erased for flexibility)
    pub search: Arc<dyn SearchService>,
    /// Metadata store for package info and name resolution
    pub metadata: Arc<dyn MetadataStore>,
    /// Contribution store for bug reports, improvements, reviews
    pub contributions: Arc<dyn ContributionStore>,
    /// API key store for authentication
    pub api_keys: Arc<dyn ApiKeyStore>,
    /// Server configuration
    pub config: Arc<ServerConfig>,
    /// Rate limiter state
    pub rate_limiter: Arc<RateLimiterState>,
    /// Cache configuration for TTLs
    pub cache_config: Arc<CacheConfig>,
    /// Prometheus metrics (optional, feature-gated)
    #[cfg(feature = "metrics")]
    pub metrics: Option<Arc<RegistryMetrics>>,
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Maximum request body size in bytes
    pub max_body_size: usize,
    /// Rate limit requests per minute
    pub rate_limit_rpm: u32,
    /// Enable CORS
    pub cors_enabled: bool,
    /// Allowed CORS origins
    pub cors_origins: Vec<String>,
    /// Base URL for download links
    pub base_url: String,
    /// Storage backend URL (S3, R2, etc.)
    pub storage_url: String,
    /// Enable CDN-direct downloads (redirect to S3/R2 instead of proxying)
    pub cdn_enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_body_size: 50 * 1024 * 1024, // 50MB
            rate_limit_rpm: 60,
            cors_enabled: true,
            // SECURITY (M-230): No wildcard "*" origin by default.
            // Production deployments must explicitly configure allowed origins.
            // Empty list means CORS headers won't be added.
            cors_origins: Vec::new(),
            base_url: "http://localhost:3030".to_string(),
            storage_url: "file:///tmp/dashflow-registry".to_string(),
            cdn_enabled: false, // Off by default - must be enabled explicitly
        }
    }
}

/// Rate limiter state (in-memory for now)
pub struct RateLimiterState {
    /// Per-IP request counts
    requests: RwLock<std::collections::HashMap<String, RequestCount>>,
    /// Config
    config: RateLimiterConfig,
}

#[derive(Clone)]
struct RequestCount {
    count: u32,
    window_start: std::time::Instant,
}

/// Rate limiter configuration
#[derive(Clone)]
pub struct RateLimiterConfig {
    /// Requests per window
    pub requests_per_window: u32,
    /// Window duration
    pub window_duration: std::time::Duration,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            requests_per_window: 60,
            window_duration: std::time::Duration::from_secs(60),
        }
    }
}

impl RateLimiterState {
    pub fn new(config: RateLimiterConfig) -> Self {
        Self {
            requests: RwLock::new(std::collections::HashMap::new()),
            config,
        }
    }

    /// Check if a request is allowed for the given key (IP, API key, etc.)
    pub async fn check_and_increment(&self, key: &str) -> RateLimitResult {
        let mut requests = self.requests.write().await;
        let now = std::time::Instant::now();

        let entry = requests.entry(key.to_string()).or_insert(RequestCount {
            count: 0,
            window_start: now,
        });

        // Reset window if expired
        if now.duration_since(entry.window_start) > self.config.window_duration {
            entry.count = 0;
            entry.window_start = now;
        }

        entry.count += 1;

        if entry.count > self.config.requests_per_window {
            let reset_at = entry.window_start + self.config.window_duration;
            let remaining_secs = reset_at.saturating_duration_since(now).as_secs();
            RateLimitResult::Limited {
                retry_after_secs: remaining_secs as u32,
            }
        } else {
            RateLimitResult::Allowed {
                remaining: self.config.requests_per_window - entry.count,
            }
        }
    }

    /// Clean up expired entries
    pub async fn cleanup(&self) {
        let mut requests = self.requests.write().await;
        let now = std::time::Instant::now();
        requests.retain(|_, v| now.duration_since(v.window_start) <= self.config.window_duration);
    }
}

/// Rate limit check result
pub enum RateLimitResult {
    Allowed { remaining: u32 },
    Limited { retry_after_secs: u32 },
}

impl AppState {
    /// Create new application state with default configuration
    pub async fn new() -> crate::Result<Self> {
        Self::with_config(ServerConfig::default()).await
    }

    /// Create new application state with custom configuration
    pub async fn with_config(config: ServerConfig) -> crate::Result<Self> {
        // Initialize trust service with empty keyring
        let keyring = Keyring::new();
        let trust = Arc::new(TrustService::new(keyring));

        // Initialize primary storage backend (in-memory by default, use with_storage for production)
        let storage: Arc<dyn StorageBackend> = Arc::new(InMemoryStorage::new());

        // Initialize package binary cache (local read-through cache)
        let cache = Arc::new(RwLock::new(
            PackageCache::in_memory(100 * 1024 * 1024), // 100MB cache
        ));

        // Initialize data cache (API keys, resolutions, etc.)
        let cache_config = CacheConfig::default();
        let data_cache: Arc<dyn CacheStore> =
            Arc::new(InMemoryCacheStore::new(cache_config.clone()));

        // Initialize search services with mock embedder (use with_search for production)
        let embedder = MockEmbedder::new(384); // Standard embedding dimension
        let vector_store = InMemoryVectorStore::new();
        let search: Arc<dyn SearchService> = Arc::new(SearchServiceWrapper::new(
            SemanticSearchService::new(embedder, vector_store),
        ));

        // Initialize metadata store, contribution store, and API key store (in-memory by default)
        // InMemoryMetadataStore implements all three traits
        let store = Arc::new(InMemoryMetadataStore::new());
        let metadata: Arc<dyn MetadataStore> = Arc::<InMemoryMetadataStore>::clone(&store);
        let contributions: Arc<dyn ContributionStore> = Arc::<InMemoryMetadataStore>::clone(&store);
        let api_keys: Arc<dyn ApiKeyStore> = store;

        // Initialize rate limiter
        let rate_limiter = Arc::new(RateLimiterState::new(RateLimiterConfig {
            requests_per_window: config.rate_limit_rpm,
            window_duration: std::time::Duration::from_secs(60),
        }));

        // Initialize metrics if feature enabled
        #[cfg(feature = "metrics")]
        let metrics = RegistryMetrics::new().map(Arc::new).ok();

        Ok(Self {
            trust,
            storage,
            cache,
            data_cache,
            search,
            metadata,
            contributions,
            api_keys,
            config: Arc::new(config),
            rate_limiter,
            cache_config: Arc::new(cache_config),
            #[cfg(feature = "metrics")]
            metrics,
        })
    }

    /// Create application state with a custom store that implements MetadataStore, ContributionStore, and ApiKeyStore
    pub fn with_store<S: MetadataStore + ContributionStore + ApiKeyStore + 'static>(
        mut self,
        store: S,
    ) -> Self {
        let store = Arc::new(store);
        self.metadata = Arc::<S>::clone(&store);
        self.contributions = Arc::<S>::clone(&store);
        self.api_keys = store;
        self
    }

    /// Create application state with a custom metadata store only
    pub fn with_metadata_store<M: MetadataStore + 'static>(mut self, metadata: M) -> Self {
        self.metadata = Arc::new(metadata);
        self
    }

    /// Create application state with a custom contribution store only
    pub fn with_contribution_store<C: ContributionStore + 'static>(
        mut self,
        contributions: C,
    ) -> Self {
        self.contributions = Arc::new(contributions);
        self
    }

    /// Create application state with a custom API key store only
    pub fn with_api_key_store<A: ApiKeyStore + 'static>(mut self, api_keys: A) -> Self {
        self.api_keys = Arc::new(api_keys);
        self
    }

    /// Create application state with a custom semantic search service
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_registry::{
    ///     OpenAIRegistryEmbedder, QdrantRegistryStore, SemanticSearchService,
    /// };
    /// use dashflow_registry::api::{AppState, SearchServiceWrapper};
    ///
    /// let embedder = OpenAIRegistryEmbedder::new();
    /// let vector_store = QdrantRegistryStore::new("http://localhost:6334", "packages", 1536).await?;
    /// let search = SemanticSearchService::new(embedder, vector_store);
    ///
    /// let state = AppState::new().await?
    ///     .with_search(SearchServiceWrapper::new(search));
    /// ```
    pub fn with_search<S: SearchService + 'static>(mut self, search: S) -> Self {
        self.search = Arc::new(search);
        self
    }

    /// Create application state with a custom data cache store
    ///
    /// # Example (Redis)
    ///
    /// ```rust,ignore
    /// use dashflow_registry::{RedisCacheStore, RedisConfig};
    ///
    /// let redis = RedisCacheStore::new(RedisConfig::from_env()).await?;
    /// let state = AppState::new().await?
    ///     .with_data_cache(redis);
    /// ```
    pub fn with_data_cache<C: CacheStore + 'static>(mut self, cache: C) -> Self {
        self.data_cache = Arc::new(cache);
        self
    }

    /// Create application state with custom cache configuration
    pub fn with_cache_config(mut self, config: CacheConfig) -> Self {
        self.cache_config = Arc::new(config);
        self
    }

    /// Create application state with a custom storage backend
    ///
    /// # Example (S3)
    ///
    /// ```rust,ignore
    /// use dashflow_registry::{S3Config, S3Storage};
    ///
    /// let s3_config = S3Config::new("my-packages")
    ///     .region("us-west-2");
    /// let storage = S3Storage::new(s3_config).await?;
    ///
    /// let state = AppState::new().await?
    ///     .with_storage(storage);
    /// ```
    ///
    /// # Example (Cloudflare R2)
    ///
    /// ```rust,ignore
    /// use dashflow_registry::{S3Config, S3Storage};
    ///
    /// let r2_config = S3Config::r2("packages", "ACCOUNT_ID");
    /// let storage = S3Storage::new(r2_config).await?;
    ///
    /// let state = AppState::new().await?
    ///     .with_storage(storage);
    /// ```
    pub fn with_storage<S: StorageBackend + 'static>(mut self, storage: S) -> Self {
        self.storage = Arc::new(storage);
        self
    }

    /// Create application state with custom Prometheus metrics
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use dashflow_registry::RegistryMetrics;
    ///
    /// let metrics = RegistryMetrics::new()?;
    /// let state = AppState::new().await?
    ///     .with_metrics(metrics);
    /// ```
    #[cfg(feature = "metrics")]
    pub fn with_metrics(mut self, metrics: RegistryMetrics) -> Self {
        self.metrics = Some(Arc::new(metrics));
        self
    }

    /// Get a reference to the metrics, if enabled
    #[cfg(feature = "metrics")]
    pub fn metrics(&self) -> Option<&Arc<RegistryMetrics>> {
        self.metrics.as_ref()
    }
}

/// Configuration for production semantic search
#[cfg(feature = "semantic-search")]
#[derive(Debug, Clone)]
pub struct SemanticSearchConfig {
    /// OpenAI API key (defaults to OPENAI_API_KEY env var)
    pub openai_api_key: Option<String>,
    /// OpenAI model (defaults to text-embedding-3-small)
    pub openai_model: String,
    /// Qdrant URL (defaults to http://localhost:6334)
    pub qdrant_url: String,
    /// Qdrant collection name
    pub qdrant_collection: String,
    /// Embedding dimension
    pub embedding_dimension: usize,
}

#[cfg(feature = "semantic-search")]
impl Default for SemanticSearchConfig {
    fn default() -> Self {
        Self {
            openai_api_key: None, // Will use OPENAI_API_KEY env var
            openai_model: "text-embedding-3-small".to_string(),
            qdrant_url: "http://localhost:6334".to_string(),
            qdrant_collection: "dashflow_packages".to_string(),
            embedding_dimension: 1536,
        }
    }
}

#[cfg(feature = "semantic-search")]
impl SemanticSearchConfig {
    /// Create config from environment variables
    pub fn from_env() -> Self {
        Self {
            openai_api_key: env_string(OPENAI_API_KEY),
            openai_model: env_string(OPENAI_EMBEDDING_MODEL)
                .unwrap_or_else(|| "text-embedding-3-small".to_string()),
            qdrant_url: env_string(QDRANT_URL)
                .unwrap_or_else(|| "http://localhost:6334".to_string()),
            qdrant_collection: env_string(QDRANT_COLLECTION)
                .unwrap_or_else(|| "dashflow_packages".to_string()),
            embedding_dimension: env_string(EMBEDDING_DIMENSION)
                .and_then(|s| s.parse().ok())
                .unwrap_or(1536),
        }
    }
}

/// Create production semantic search service
#[cfg(feature = "semantic-search")]
pub async fn create_production_search(
    config: SemanticSearchConfig,
) -> crate::Result<impl SearchService> {
    use crate::{OpenAIRegistryEmbedder, QdrantRegistryStore, SemanticSearchService};

    // Create OpenAI embedder
    let mut embedder = match config.openai_api_key {
        Some(api_key) => OpenAIRegistryEmbedder::try_new_with_api_key(&api_key)?,
        None => OpenAIRegistryEmbedder::try_new()?,
    };
    embedder = embedder.with_model(&config.openai_model);

    // Create Qdrant vector store
    let vector_store = QdrantRegistryStore::new(
        &config.qdrant_url,
        &config.qdrant_collection,
        config.embedding_dimension,
    )
    .await?;

    // Create semantic search service
    let search = SemanticSearchService::new(embedder, vector_store);

    Ok(SearchServiceWrapper::new(search))
}

/// Configuration for production S3-compatible storage
///
/// Supports AWS S3, Cloudflare R2, and MinIO.
#[cfg(feature = "s3")]
#[derive(Debug, Clone)]
pub struct S3StorageConfig {
    /// S3 bucket name (required)
    pub bucket: String,
    /// AWS region (defaults to us-east-1)
    pub region: String,
    /// Custom endpoint for R2/MinIO (optional)
    pub endpoint: Option<String>,
    /// Object key prefix (defaults to "packages/")
    pub prefix: String,
    /// Use path-style addressing (required for MinIO)
    pub path_style: bool,
    /// Storage type hint for configuration
    pub storage_type: S3StorageType,
}

/// Type of S3-compatible storage for configuration hints
#[cfg(feature = "s3")]
#[derive(Debug, Clone, PartialEq)]
pub enum S3StorageType {
    /// AWS S3
    AwsS3,
    /// Cloudflare R2
    CloudflareR2,
    /// MinIO (self-hosted)
    MinIO,
    /// Other S3-compatible storage
    Custom,
}

#[cfg(feature = "s3")]
impl Default for S3StorageConfig {
    fn default() -> Self {
        Self {
            bucket: "dashflow-packages".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
            prefix: "packages/".to_string(),
            path_style: false,
            storage_type: S3StorageType::AwsS3,
        }
    }
}

#[cfg(feature = "s3")]
impl S3StorageConfig {
    /// Create config from environment variables
    ///
    /// Environment variables:
    /// - `S3_BUCKET` - S3 bucket name (required)
    /// - `S3_REGION` - AWS region (default: us-east-1)
    /// - `S3_ENDPOINT` - Custom endpoint for R2/MinIO
    /// - `S3_PREFIX` - Object key prefix (default: packages/)
    /// - `S3_PATH_STYLE` - Use path-style addressing (default: false)
    /// - `S3_STORAGE_TYPE` - Storage type: aws, r2, minio (default: aws)
    /// - `R2_ACCOUNT_ID` - Cloudflare account ID (for R2)
    pub fn from_env() -> Option<Self> {
        let bucket = env_string(S3_BUCKET)?;

        let storage_type = match env_string(S3_STORAGE_TYPE)
            .unwrap_or_else(|| "aws".to_string())
            .to_lowercase()
            .as_str()
        {
            "r2" | "cloudflare" => S3StorageType::CloudflareR2,
            "minio" => S3StorageType::MinIO,
            "custom" => S3StorageType::Custom,
            _ => S3StorageType::AwsS3,
        };

        let (region, endpoint, path_style) = match storage_type {
            S3StorageType::CloudflareR2 => {
                let account_id = env_string(R2_ACCOUNT_ID)?;
                (
                    "auto".to_string(),
                    Some(format!("https://{}.r2.cloudflarestorage.com", account_id)),
                    false,
                )
            }
            S3StorageType::MinIO => {
                let endpoint = env_string(S3_ENDPOINT)?;
                (
                    env_string(S3_REGION).unwrap_or_else(|| "us-east-1".to_string()),
                    Some(endpoint),
                    true,
                )
            }
            _ => (
                env_string(S3_REGION).unwrap_or_else(|| "us-east-1".to_string()),
                env_string(S3_ENDPOINT),
                env_string(S3_PATH_STYLE)
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(false),
            ),
        };

        Some(Self {
            bucket,
            region,
            endpoint,
            prefix: env_string(S3_PREFIX).unwrap_or_else(|| "packages/".to_string()),
            path_style,
            storage_type,
        })
    }

    /// Convert to S3Config for creating S3Storage
    pub fn to_s3_config(&self) -> crate::S3Config {
        let mut config = crate::S3Config::new(&self.bucket)
            .region(&self.region)
            .prefix(&self.prefix)
            .path_style(self.path_style);

        if let Some(ref endpoint) = self.endpoint {
            config = config.endpoint(endpoint);
        }

        config
    }
}

/// Create production storage backend from environment variables
///
/// Returns S3Storage if S3_BUCKET is set, otherwise falls back to filesystem storage.
#[cfg(feature = "s3")]
pub async fn create_production_storage() -> crate::Result<Arc<dyn StorageBackend>> {
    use crate::{FilesystemStorage, S3Storage};

    if let Some(config) = S3StorageConfig::from_env() {
        tracing::info!(
            bucket = %config.bucket,
            region = %config.region,
            storage_type = ?config.storage_type,
            "Initializing S3 storage backend"
        );

        let s3_config = config.to_s3_config();
        let storage = S3Storage::new(s3_config).await?;
        Ok(Arc::new(storage))
    } else if let Some(storage_path) = env_string(STORAGE_PATH) {
        tracing::info!(path = %storage_path, "Initializing filesystem storage backend");
        let storage = FilesystemStorage::new(&storage_path)?;
        Ok(Arc::new(storage))
    } else {
        tracing::info!("Initializing in-memory storage backend (no {} or {} set)", S3_BUCKET, STORAGE_PATH);
        Ok(Arc::new(InMemoryStorage::new()))
    }
}

/// Create production storage backend from environment variables (non-S3 fallback)
#[cfg(not(feature = "s3"))]
pub fn create_production_storage() -> crate::Result<Arc<dyn StorageBackend>> {
    use crate::FilesystemStorage;

    if let Some(storage_path) = env_string(STORAGE_PATH) {
        tracing::info!(path = %storage_path, "Initializing filesystem storage backend");
        let storage = FilesystemStorage::new(&storage_path)?;
        Ok(Arc::new(storage))
    } else {
        tracing::info!("Initializing in-memory storage backend (no {} set)", STORAGE_PATH);
        Ok(Arc::new(InMemoryStorage::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiterState::new(RateLimiterConfig {
            requests_per_window: 3,
            window_duration: std::time::Duration::from_secs(60),
        });

        // First 3 requests should be allowed
        for i in 0..3 {
            match limiter.check_and_increment("test-key").await {
                RateLimitResult::Allowed { remaining } => {
                    assert_eq!(remaining, 2 - i);
                }
                RateLimitResult::Limited { .. } => panic!("Should be allowed"),
            }
        }

        // 4th request should be limited
        match limiter.check_and_increment("test-key").await {
            RateLimitResult::Limited { retry_after_secs } => {
                assert!(retry_after_secs > 0);
            }
            RateLimitResult::Allowed { .. } => panic!("Should be limited"),
        }

        // Different key should be allowed
        match limiter.check_and_increment("other-key").await {
            RateLimitResult::Allowed { remaining } => {
                assert_eq!(remaining, 2);
            }
            RateLimitResult::Limited { .. } => panic!("Different key should be allowed"),
        }
    }

    #[tokio::test]
    async fn test_app_state_creation() {
        let state = AppState::new().await;
        assert!(state.is_ok());
    }
}
