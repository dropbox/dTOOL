//! Caching Layer
//!
//! Provides a unified caching interface for the registry with multiple backends:
//! - `InMemoryCacheStore`: Simple in-memory cache with TTL (for testing/single-node)
//! - `RedisCacheStore`: Redis-backed cache for production (feature-gated)
//!
//! # Caching Strategy
//!
//! The registry caches these data types:
//!
//! | Data Type | Cache Key | TTL | Invalidation |
//! |-----------|-----------|-----|--------------|
//! | API Key Verification | `apikey:{hash}` | 60s | On revoke |
//! | Package Resolution | `resolve:{name}:{version}` | 5min | On publish |
//! | Search Results | `search:{query_hash}` | 2min | On index update |
//! | Package Metadata | `pkg:{hash}` | 30min | Immutable (never) |
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_registry::cache::{CacheStore, InMemoryCacheStore, CacheConfig};
//!
//! let cache = InMemoryCacheStore::new(CacheConfig::default());
//!
//! // Cache an API key verification result
//! cache.set("apikey:abc123", b"verified", Some(Duration::from_secs(60))).await?;
//!
//! // Retrieve from cache
//! if let Some(data) = cache.get("apikey:abc123").await? {
//!     println!("Cache hit!");
//! }
//! ```

use async_trait::async_trait;
use dashflow::core::config_loader::env_vars::{
    env_bool, env_duration_secs, env_usize, CACHE_API_KEY_TTL_SECS, CACHE_MAX_ENTRIES,
    CACHE_METADATA_TTL_SECS, CACHE_RESOLUTION_TTL_SECS, CACHE_SEARCH_TTL_SECS, CACHE_TRACK_STATS,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::Result;

// ============================================================================
// Cache Configuration
// ============================================================================

/// Configuration for cache TTLs and behavior
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// TTL for API key verification results
    pub api_key_ttl: Duration,
    /// TTL for package resolution results
    pub resolution_ttl: Duration,
    /// TTL for search results
    pub search_ttl: Duration,
    /// TTL for package metadata (immutable, so long TTL)
    pub metadata_ttl: Duration,
    /// Maximum entries in in-memory cache (0 = unlimited)
    pub max_entries: usize,
    /// Enable cache statistics tracking
    pub track_stats: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            api_key_ttl: Duration::from_secs(60),     // 1 minute
            resolution_ttl: Duration::from_secs(300), // 5 minutes
            search_ttl: Duration::from_secs(120),     // 2 minutes
            metadata_ttl: Duration::from_secs(1800),  // 30 minutes
            max_entries: 10_000,
            track_stats: true,
        }
    }
}

impl CacheConfig {
    /// Create from environment variables
    pub fn from_env() -> Self {
        Self {
            api_key_ttl: env_duration_secs(CACHE_API_KEY_TTL_SECS, Duration::from_secs(60)),
            resolution_ttl: env_duration_secs(CACHE_RESOLUTION_TTL_SECS, Duration::from_secs(300)),
            search_ttl: env_duration_secs(CACHE_SEARCH_TTL_SECS, Duration::from_secs(120)),
            metadata_ttl: env_duration_secs(CACHE_METADATA_TTL_SECS, Duration::from_secs(1800)),
            max_entries: env_usize(CACHE_MAX_ENTRIES, 10_000),
            track_stats: env_bool(CACHE_TRACK_STATS, true),
        }
    }
}

// ============================================================================
// Cache Key Utilities
// ============================================================================

/// Cache key prefixes for different data types
pub mod keys {
    /// Generate cache key for API key verification
    pub fn api_key(key_hash: &str) -> String {
        format!("apikey:{}", key_hash)
    }

    /// Generate cache key for package resolution
    pub fn resolution(name: &str, version_req: &str) -> String {
        format!("resolve:{}:{}", name, version_req)
    }

    /// Generate cache key for package resolution (latest version)
    pub fn resolution_latest(name: &str) -> String {
        format!("resolve:{}:latest", name)
    }

    /// Generate cache key for search results
    pub fn search(query: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(query.as_bytes());
        let hash = hex::encode(&hasher.finalize()[..8]); // Use first 8 bytes
        format!("search:{}", hash)
    }

    /// Generate cache key for package metadata by hash
    pub fn package_metadata(content_hash: &str) -> String {
        format!("pkg:{}", content_hash)
    }

    /// Generate cache key for version list
    pub fn versions(name: &str) -> String {
        format!("versions:{}", name)
    }

    /// Pattern for invalidating all resolutions of a package
    pub fn resolution_pattern(name: &str) -> String {
        format!("resolve:{}:*", name)
    }

    /// Pattern for invalidating all search results
    pub fn search_pattern() -> String {
        "search:*".to_string()
    }
}

// ============================================================================
// Cache Statistics
// ============================================================================

/// Cache statistics for monitoring
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total cache hits
    pub hits: u64,
    /// Total cache misses
    pub misses: u64,
    /// Total entries currently in cache
    pub entries: u64,
    /// Total bytes stored (approximate)
    pub bytes: u64,
    /// Number of entries evicted
    pub evictions: u64,
    /// Number of entries expired
    pub expirations: u64,
}

impl CacheStats {
    /// Calculate hit rate (0.0 to 1.0)
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

// ============================================================================
// Cache Store Trait
// ============================================================================

/// Abstract cache store interface
///
/// All cache operations are async and fallible. Implementations should:
/// - Handle TTL expiration
/// - Support pattern-based invalidation
/// - Be thread-safe (Send + Sync)
#[async_trait]
pub trait CacheStore: Send + Sync {
    /// Get a cached value by key
    ///
    /// Returns `None` if key doesn't exist or is expired
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Set a cached value with optional TTL
    ///
    /// If `ttl` is `None`, the value never expires.
    async fn set(&self, key: &str, value: &[u8], ttl: Option<Duration>) -> Result<()>;

    /// Delete a cached value
    ///
    /// Returns `true` if the key existed and was deleted
    async fn delete(&self, key: &str) -> Result<bool>;

    /// Delete all keys matching a glob pattern
    ///
    /// Supports `*` wildcard. Returns number of keys deleted.
    async fn delete_pattern(&self, pattern: &str) -> Result<usize>;

    /// Check if a key exists (and is not expired)
    async fn exists(&self, key: &str) -> Result<bool>;

    /// Get cache statistics
    async fn stats(&self) -> Result<CacheStats>;

    /// Clear all entries from the cache
    async fn clear(&self) -> Result<()>;
}

// ============================================================================
// Cache Extension Functions (for dyn CacheStore compatibility)
// ============================================================================

/// Get a typed value from cache (deserialize from JSON)
pub async fn cache_get_json<T: DeserializeOwned>(
    cache: &dyn CacheStore,
    key: &str,
) -> Result<Option<T>> {
    match cache.get(key).await? {
        Some(bytes) => {
            let value: T = serde_json::from_slice(&bytes)?;
            Ok(Some(value))
        }
        None => Ok(None),
    }
}

/// Set a typed value in cache (serialize to JSON)
pub async fn cache_set_json<T: Serialize>(
    cache: &dyn CacheStore,
    key: &str,
    value: &T,
    ttl: Option<Duration>,
) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    cache.set(key, &bytes, ttl).await
}

// ============================================================================
// In-Memory Cache Store
// ============================================================================

/// Entry in the in-memory cache
struct CacheEntry {
    /// Cached data
    data: Vec<u8>,
    /// When this entry was created
    created_at: Instant,
    /// Time-to-live (None = never expires)
    ttl: Option<Duration>,
    /// Last access time (for LRU eviction)
    last_accessed: Instant,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            self.created_at.elapsed() > ttl
        } else {
            false
        }
    }
}

/// In-memory cache store with TTL support
///
/// Features:
/// - Automatic TTL expiration on access
/// - LRU eviction when max entries exceeded
/// - Pattern-based invalidation
/// - Statistics tracking
pub struct InMemoryCacheStore {
    /// Cache entries
    entries: RwLock<HashMap<String, CacheEntry>>,
    /// Configuration
    config: CacheConfig,
    /// Statistics
    stats: RwLock<CacheStats>,
}

impl InMemoryCacheStore {
    /// Create a new in-memory cache store
    pub fn new(config: CacheConfig) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            config,
            stats: RwLock::new(CacheStats::default()),
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(CacheConfig::default())
    }

    /// Evict expired entries and LRU entries if over capacity
    async fn maybe_evict(&self) {
        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        // First, remove expired entries
        let before_count = entries.len();
        entries.retain(|_, entry| !entry.is_expired());
        let expired_count = before_count - entries.len();
        stats.expirations += expired_count as u64;

        // If still over capacity, evict LRU entries
        if self.config.max_entries > 0 && entries.len() > self.config.max_entries {
            let to_evict = entries.len() - self.config.max_entries;

            // Find LRU entries
            let mut by_access: Vec<_> = entries
                .iter()
                .map(|(k, v)| (k.clone(), v.last_accessed))
                .collect();
            by_access.sort_by_key(|(_, accessed)| *accessed);

            // Evict oldest
            for (key, _) in by_access.into_iter().take(to_evict) {
                entries.remove(&key);
                stats.evictions += 1;
            }
        }
    }

    /// Check if a pattern matches a key
    fn pattern_matches(pattern: &str, key: &str) -> bool {
        // Simple glob matching with * wildcard
        if pattern == "*" {
            return true;
        }

        if let Some(prefix) = pattern.strip_suffix('*') {
            return key.starts_with(prefix);
        }

        if let Some(suffix) = pattern.strip_prefix('*') {
            return key.ends_with(suffix);
        }

        pattern == key
    }
}

#[async_trait]
impl CacheStore for InMemoryCacheStore {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let mut entries = self.entries.write().await;

        if let Some(entry) = entries.get_mut(key) {
            if entry.is_expired() {
                entries.remove(key);
                if self.config.track_stats {
                    let mut stats = self.stats.write().await;
                    stats.misses += 1;
                    stats.expirations += 1;
                }
                return Ok(None);
            }

            // Update last accessed time
            entry.last_accessed = Instant::now();

            if self.config.track_stats {
                let mut stats = self.stats.write().await;
                stats.hits += 1;
            }

            return Ok(Some(entry.data.clone()));
        }

        if self.config.track_stats {
            let mut stats = self.stats.write().await;
            stats.misses += 1;
        }

        Ok(None)
    }

    async fn set(&self, key: &str, value: &[u8], ttl: Option<Duration>) -> Result<()> {
        // Maybe evict before adding new entry
        self.maybe_evict().await;

        let now = Instant::now();
        let entry = CacheEntry {
            data: value.to_vec(),
            created_at: now,
            ttl,
            last_accessed: now,
        };

        let mut entries = self.entries.write().await;
        entries.insert(key.to_string(), entry);

        if self.config.track_stats {
            let mut stats = self.stats.write().await;
            stats.entries = entries.len() as u64;
            stats.bytes = entries.values().map(|e| e.data.len() as u64).sum();
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        let mut entries = self.entries.write().await;
        let existed = entries.remove(key).is_some();

        if self.config.track_stats && existed {
            let mut stats = self.stats.write().await;
            stats.entries = entries.len() as u64;
            stats.bytes = entries.values().map(|e| e.data.len() as u64).sum();
        }

        Ok(existed)
    }

    async fn delete_pattern(&self, pattern: &str) -> Result<usize> {
        let mut entries = self.entries.write().await;

        let keys_to_delete: Vec<_> = entries
            .keys()
            .filter(|k| Self::pattern_matches(pattern, k))
            .cloned()
            .collect();

        let count = keys_to_delete.len();
        for key in keys_to_delete {
            entries.remove(&key);
        }

        if self.config.track_stats && count > 0 {
            let mut stats = self.stats.write().await;
            stats.entries = entries.len() as u64;
            stats.bytes = entries.values().map(|e| e.data.len() as u64).sum();
        }

        Ok(count)
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let entries = self.entries.read().await;
        if let Some(entry) = entries.get(key) {
            Ok(!entry.is_expired())
        } else {
            Ok(false)
        }
    }

    async fn stats(&self) -> Result<CacheStats> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }

    async fn clear(&self) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.clear();

        if self.config.track_stats {
            let mut stats = self.stats.write().await;
            stats.entries = 0;
            stats.bytes = 0;
        }

        Ok(())
    }
}

// ============================================================================
// No-Op Cache Store (for testing/disabled caching)
// ============================================================================

/// A cache store that does nothing (always misses)
///
/// Useful for testing or when caching should be disabled.
pub struct NoOpCacheStore;

#[async_trait]
impl CacheStore for NoOpCacheStore {
    async fn get(&self, _key: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    async fn set(&self, _key: &str, _value: &[u8], _ttl: Option<Duration>) -> Result<()> {
        Ok(())
    }

    async fn delete(&self, _key: &str) -> Result<bool> {
        Ok(false)
    }

    async fn delete_pattern(&self, _pattern: &str) -> Result<usize> {
        Ok(0)
    }

    async fn exists(&self, _key: &str) -> Result<bool> {
        Ok(false)
    }

    async fn stats(&self) -> Result<CacheStats> {
        Ok(CacheStats::default())
    }

    async fn clear(&self) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// Redis Cache Store (Feature-Gated)
// ============================================================================

#[cfg(feature = "redis")]
pub mod redis_store {
    //! Redis-backed cache store implementation
    //!
    //! Requires the `redis` feature to be enabled.

    use super::*;
    use ::redis::{aio::MultiplexedConnection, AsyncCommands, Client};
    use dashflow::core::config_loader::env_vars::{
        env_duration_secs, env_string_or_default, REDIS_CONNECT_TIMEOUT_SECS,
        REDIS_OPERATION_TIMEOUT_SECS, REDIS_PREFIX, REDIS_URL,
    };

    /// Redis connection configuration
    #[derive(Debug, Clone)]
    pub struct RedisConfig {
        /// Redis connection URL (e.g., "redis://localhost:6379")
        pub url: String,
        /// Key prefix for all cache keys
        pub prefix: String,
        /// Connection timeout
        pub connect_timeout: Duration,
        /// Operation timeout
        pub operation_timeout: Duration,
    }

    impl Default for RedisConfig {
        fn default() -> Self {
            Self {
                url: "redis://localhost:6379".to_string(),
                prefix: "dashflow:cache:".to_string(),
                connect_timeout: Duration::from_secs(5),
                operation_timeout: Duration::from_secs(2),
            }
        }
    }

    impl RedisConfig {
        /// Create from environment variables
        pub fn from_env() -> Self {
            Self {
                url: env_string_or_default(REDIS_URL, "redis://localhost:6379"),
                prefix: env_string_or_default(REDIS_PREFIX, "dashflow:cache:"),
                connect_timeout: env_duration_secs(
                    REDIS_CONNECT_TIMEOUT_SECS,
                    Duration::from_secs(5),
                ),
                operation_timeout: env_duration_secs(
                    REDIS_OPERATION_TIMEOUT_SECS,
                    Duration::from_secs(2),
                ),
            }
        }
    }

    /// Redis-backed cache store
    ///
    /// Features:
    /// - Connection pooling via multiplexed connection
    /// - TTL support via Redis SETEX
    /// - Pattern deletion via SCAN + DEL
    /// - Statistics from Redis INFO
    pub struct RedisCacheStore {
        /// Redis connection
        conn: MultiplexedConnection,
        /// Configuration
        config: RedisConfig,
    }

    impl RedisCacheStore {
        /// Create a new Redis cache store
        pub async fn new(config: RedisConfig) -> crate::Result<Self> {
            let client = Client::open(config.url.as_str())
                .map_err(|e| crate::RegistryError::Cache(format!("Redis client error: {}", e)))?;

            let conn = client
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| {
                    crate::RegistryError::Cache(format!("Redis connection error: {}", e))
                })?;

            Ok(Self { conn, config })
        }

        /// Create from environment variables
        pub async fn from_env() -> crate::Result<Self> {
            Self::new(RedisConfig::from_env()).await
        }

        /// Get prefixed key
        fn prefixed_key(&self, key: &str) -> String {
            format!("{}{}", self.config.prefix, key)
        }
    }

    #[async_trait]
    impl CacheStore for RedisCacheStore {
        async fn get(&self, key: &str) -> crate::Result<Option<Vec<u8>>> {
            let mut conn = self.conn.clone();
            let prefixed = self.prefixed_key(key);

            let result: Option<Vec<u8>> = conn
                .get(&prefixed)
                .await
                .map_err(|e| crate::RegistryError::Cache(format!("Redis GET error: {}", e)))?;

            Ok(result)
        }

        async fn set(&self, key: &str, value: &[u8], ttl: Option<Duration>) -> crate::Result<()> {
            let mut conn = self.conn.clone();
            let prefixed = self.prefixed_key(key);

            if let Some(ttl) = ttl {
                let _: () = conn
                    .set_ex(&prefixed, value, ttl.as_secs())
                    .await
                    .map_err(|e| {
                        crate::RegistryError::Cache(format!("Redis SETEX error: {}", e))
                    })?;
            } else {
                let _: () = conn
                    .set(&prefixed, value)
                    .await
                    .map_err(|e| crate::RegistryError::Cache(format!("Redis SET error: {}", e)))?;
            }

            Ok(())
        }

        async fn delete(&self, key: &str) -> crate::Result<bool> {
            let mut conn = self.conn.clone();
            let prefixed = self.prefixed_key(key);

            let deleted: i64 = conn
                .del(&prefixed)
                .await
                .map_err(|e| crate::RegistryError::Cache(format!("Redis DEL error: {}", e)))?;

            Ok(deleted > 0)
        }

        async fn delete_pattern(&self, pattern: &str) -> crate::Result<usize> {
            let mut conn = self.conn.clone();
            let prefixed = self.prefixed_key(pattern);

            // Use SCAN to find matching keys, then DEL
            let keys: Vec<String> = conn
                .keys(&prefixed)
                .await
                .map_err(|e| crate::RegistryError::Cache(format!("Redis KEYS error: {}", e)))?;

            if keys.is_empty() {
                return Ok(0);
            }

            let deleted: i64 = conn
                .del(&keys)
                .await
                .map_err(|e| crate::RegistryError::Cache(format!("Redis DEL error: {}", e)))?;

            Ok(deleted as usize)
        }

        async fn exists(&self, key: &str) -> crate::Result<bool> {
            let mut conn = self.conn.clone();
            let prefixed = self.prefixed_key(key);

            let exists: bool = conn
                .exists(&prefixed)
                .await
                .map_err(|e| crate::RegistryError::Cache(format!("Redis EXISTS error: {}", e)))?;

            Ok(exists)
        }

        async fn stats(&self) -> crate::Result<CacheStats> {
            let mut conn = self.conn.clone();

            // Get key count for our prefix
            let pattern = format!("{}*", self.config.prefix);
            let keys: Vec<String> = conn
                .keys(&pattern)
                .await
                .map_err(|e| crate::RegistryError::Cache(format!("Redis KEYS error: {}", e)))?;

            Ok(CacheStats {
                entries: keys.len() as u64,
                // Note: accurate hits/misses would require Redis server-side tracking
                ..Default::default()
            })
        }

        async fn clear(&self) -> crate::Result<()> {
            let pattern = format!("{}*", self.config.prefix);
            self.delete_pattern(&pattern).await?;
            Ok(())
        }
    }
}

#[cfg(feature = "redis")]
pub use redis_store::{RedisCacheStore, RedisConfig};

// ============================================================================
// Cached Wrapper for Stores
// ============================================================================

/// Extension trait for wrapping any function with caching
pub trait Cacheable {
    /// Cache key for this item
    fn cache_key(&self) -> String;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_cache_basic() {
        let cache = InMemoryCacheStore::new(CacheConfig::default());

        // Test set and get
        cache.set("key1", b"value1", None).await.unwrap();
        let result = cache.get("key1").await.unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));

        // Test non-existent key
        let result = cache.get("nonexistent").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_in_memory_cache_ttl() {
        let cache = InMemoryCacheStore::new(CacheConfig::default());

        // Set with short but reliable TTL (50ms with 150ms margin for slow CI systems)
        cache
            .set("key1", b"value1", Some(Duration::from_millis(50)))
            .await
            .unwrap();

        // Should exist immediately
        assert!(cache.exists("key1").await.unwrap());

        // Wait for expiration with adequate margin (3x TTL)
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Should be expired
        let result = cache.get("key1").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_in_memory_cache_delete() {
        let cache = InMemoryCacheStore::new(CacheConfig::default());

        cache.set("key1", b"value1", None).await.unwrap();
        cache.set("key2", b"value2", None).await.unwrap();

        // Delete existing key
        let deleted = cache.delete("key1").await.unwrap();
        assert!(deleted);

        // Key should be gone
        let result = cache.get("key1").await.unwrap();
        assert_eq!(result, None);

        // Delete non-existent key
        let deleted = cache.delete("nonexistent").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_in_memory_cache_pattern_delete() {
        let cache = InMemoryCacheStore::new(CacheConfig::default());

        cache.set("resolve:pkg1:1.0", b"v1", None).await.unwrap();
        cache.set("resolve:pkg1:2.0", b"v2", None).await.unwrap();
        cache.set("resolve:pkg2:1.0", b"v3", None).await.unwrap();
        cache.set("other:key", b"v4", None).await.unwrap();

        // Delete all pkg1 resolutions
        let count = cache.delete_pattern("resolve:pkg1:*").await.unwrap();
        assert_eq!(count, 2);

        // pkg2 should still exist
        assert!(cache.exists("resolve:pkg2:1.0").await.unwrap());
        // other:key should still exist
        assert!(cache.exists("other:key").await.unwrap());
        // pkg1 should be gone
        assert!(!cache.exists("resolve:pkg1:1.0").await.unwrap());
        assert!(!cache.exists("resolve:pkg1:2.0").await.unwrap());
    }

    #[tokio::test]
    async fn test_in_memory_cache_stats() {
        let config = CacheConfig {
            track_stats: true,
            ..Default::default()
        };
        let cache = InMemoryCacheStore::new(config);

        cache.set("key1", b"value1", None).await.unwrap();

        // Miss
        let _ = cache.get("nonexistent").await.unwrap();

        // Hit
        let _ = cache.get("key1").await.unwrap();
        let _ = cache.get("key1").await.unwrap();

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entries, 1);
    }

    #[tokio::test]
    async fn test_in_memory_cache_json() {
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct TestData {
            name: String,
            value: i32,
        }

        let cache = InMemoryCacheStore::new(CacheConfig::default());

        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        cache_set_json(&cache, "key1", &data, None).await.unwrap();

        let result: Option<TestData> = cache_get_json(&cache, "key1").await.unwrap();
        assert_eq!(result, Some(data));
    }

    #[tokio::test]
    async fn test_in_memory_cache_eviction() {
        let config = CacheConfig {
            max_entries: 3,
            track_stats: true,
            ..Default::default()
        };
        let cache = InMemoryCacheStore::new(config);

        // Add entries up to capacity (all created nearly simultaneously)
        cache.set("key1", b"value1", None).await.unwrap();
        cache.set("key2", b"value2", None).await.unwrap();
        cache.set("key3", b"value3", None).await.unwrap();

        // Establish deterministic LRU ordering via explicit access patterns with small delays
        // to ensure distinct Instant timestamps. 5ms is enough for timestamp distinction
        // while being fast enough for CI (vs original 1ms which was unreliable).
        tokio::time::sleep(Duration::from_millis(5)).await;
        let _ = cache.get("key3").await; // Update key3's last_accessed
        tokio::time::sleep(Duration::from_millis(5)).await;
        let _ = cache.get("key1").await; // Update key1's last_accessed (most recent)
        // Now: key2=LRU (never accessed after creation), key3=middle, key1=MRU

        // Add key4 - eviction runs BEFORE insert with len=3, max=3, so no eviction yet.
        cache.set("key4", b"value4", None).await.unwrap();
        // Now we have 4 entries, over capacity.

        // Add key5 - this triggers eviction because len(4) > max(3).
        // Eviction removes LRU (key2) to get back to 3 entries, then key5 is added.
        cache.set("key5", b"value5", None).await.unwrap();

        // key1 should still exist (recently accessed before key4/key5 were added)
        assert!(cache.exists("key1").await.unwrap());
        // key5 should exist (just added)
        assert!(cache.exists("key5").await.unwrap());
        // key3 should exist (accessed more recently than key2)
        assert!(cache.exists("key3").await.unwrap());
        // key4 should exist (added recently)
        assert!(cache.exists("key4").await.unwrap());
        // key2 should be evicted (LRU - never accessed after creation)
        assert!(!cache.exists("key2").await.unwrap());

        // Verify cache has correct number of entries (max_entries + 1 because eviction
        // runs BEFORE insert, so we end up with max+1 entries)
        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.entries, 4); // key1, key3, key4, key5
        assert!(stats.evictions >= 1); // At least key2 was evicted
    }

    #[tokio::test]
    async fn test_cache_keys() {
        assert_eq!(keys::api_key("abc123"), "apikey:abc123");
        assert_eq!(keys::resolution("pkg", "1.0"), "resolve:pkg:1.0");
        assert_eq!(keys::resolution_latest("pkg"), "resolve:pkg:latest");
        assert_eq!(keys::package_metadata("sha256:abc"), "pkg:sha256:abc");
        assert_eq!(keys::versions("pkg"), "versions:pkg");
        assert_eq!(keys::resolution_pattern("pkg"), "resolve:pkg:*");

        // Search key should be deterministic
        let key1 = keys::search("test query");
        let key2 = keys::search("test query");
        assert_eq!(key1, key2);
        assert!(key1.starts_with("search:"));
    }

    #[tokio::test]
    async fn test_noop_cache() {
        let cache = NoOpCacheStore;

        cache.set("key1", b"value1", None).await.unwrap();
        let result = cache.get("key1").await.unwrap();
        assert_eq!(result, None); // Always misses

        assert!(!cache.exists("key1").await.unwrap());
        assert!(!cache.delete("key1").await.unwrap());
        assert_eq!(cache.delete_pattern("*").await.unwrap(), 0);
    }
}
