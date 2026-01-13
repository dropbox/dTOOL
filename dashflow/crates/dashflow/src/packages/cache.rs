// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Package cache with CacheConfig enforcement (M-200).
//!
//! This module implements a package cache that enforces:
//! - Maximum cache size with LRU eviction
//! - Metadata TTL with expiry checking
//! - Offline mode that blocks network fetches
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::packages::{CacheConfig, PackageCache, PackageId, Version};
//!
//! let config = CacheConfig {
//!     max_size_mb: 1000,
//!     metadata_ttl_secs: 3600,
//!     offline: false,
//!     ..Default::default()
//! };
//!
//! let cache = PackageCache::new(config)?;
//!
//! // Store a package
//! cache.store_package(&id, &version, &data)?;
//!
//! // Retrieve a package (None if expired or not cached)
//! let data = cache.get_package(&id, &version)?;
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, info, warn};

use super::config::CacheConfig;
use super::types::{PackageId, Version};

/// Result type for cache operations.
pub type CacheResult<T> = Result<T, CacheError>;

/// Errors that can occur during cache operations.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum CacheError {
    /// IO error
    #[error("IO error: {0}")]
    Io(String),
    /// Cache is in offline mode and requested data not cached
    #[error("Offline mode: package {package}@{version} not in cache")]
    OfflineNotCached {
        /// Package identifier that was requested.
        package: String,
        /// Version string that was requested.
        version: String,
    },
    /// Metadata has expired
    #[error("Metadata expired for {0}")]
    MetadataExpired(String),
    /// Package not found in cache
    #[error("Package {package}@{version} not found in cache")]
    NotFound {
        /// Package identifier that was not found.
        package: String,
        /// Version string that was not found.
        version: String,
    },
    /// Cache is full and eviction failed
    #[error("Cache full: cannot store {size_bytes} bytes (limit: {max_bytes} bytes)")]
    CacheFull {
        /// Size in bytes of the item that could not be stored.
        size_bytes: u64,
        /// Maximum cache size in bytes.
        max_bytes: u64,
    },
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Cached metadata entry with timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedMetadata<T> {
    /// The cached data
    pub data: T,
    /// Unix timestamp when this entry was cached
    pub cached_at: u64,
    /// TTL in seconds (from config at cache time)
    pub ttl_secs: u64,
}

impl<T> CachedMetadata<T> {
    /// Create a new cached metadata entry.
    pub fn new(data: T, ttl_secs: u64) -> Self {
        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        Self {
            data,
            cached_at,
            ttl_secs,
        }
    }

    /// Check if this entry has expired.
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        now > self.cached_at + self.ttl_secs
    }

    /// Get the data if not expired.
    pub fn get_if_valid(&self) -> Option<&T> {
        if self.is_expired() {
            None
        } else {
            Some(&self.data)
        }
    }

    /// Get time remaining before expiry (0 if expired).
    pub fn time_remaining(&self) -> Duration {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        let expiry = self.cached_at + self.ttl_secs;
        if now >= expiry {
            Duration::ZERO
        } else {
            Duration::from_secs(expiry - now)
        }
    }
}

/// Cache index entry tracking package metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Package ID string
    pub package_id: String,
    /// Version string
    pub version: String,
    /// Size of the cached tarball in bytes
    pub size_bytes: u64,
    /// Unix timestamp of last access in milliseconds (for LRU eviction)
    pub last_accessed: u64,
    /// Unix timestamp when cached in milliseconds
    pub cached_at: u64,
    /// Relative path to the cached file
    pub file_path: String,
}

impl CacheEntry {
    /// Create a new cache entry.
    pub fn new(id: &PackageId, version: &Version, size_bytes: u64, file_path: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis() as u64;
        Self {
            package_id: id.to_string(),
            version: version.to_string(),
            size_bytes,
            last_accessed: now,
            cached_at: now,
            file_path,
        }
    }

    /// Update last accessed timestamp.
    pub fn touch(&mut self) {
        self.last_accessed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis() as u64;
    }
}

/// Cache index tracking all cached packages.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheIndex {
    /// Map of "package_id@version" to cache entry
    pub entries: HashMap<String, CacheEntry>,
    /// Total size of all cached packages in bytes
    pub total_size_bytes: u64,
    /// Index version for migrations
    pub version: u32,
}

impl CacheIndex {
    /// Get a cache entry key.
    fn key(id: &PackageId, version: &Version) -> String {
        format!("{}@{}", id, version)
    }

    /// Get an entry.
    pub fn get(&self, id: &PackageId, version: &Version) -> Option<&CacheEntry> {
        self.entries.get(&Self::key(id, version))
    }

    /// Get a mutable entry.
    pub fn get_mut(&mut self, id: &PackageId, version: &Version) -> Option<&mut CacheEntry> {
        self.entries.get_mut(&Self::key(id, version))
    }

    /// Insert an entry.
    pub fn insert(&mut self, entry: CacheEntry) {
        let size = entry.size_bytes;
        let key = format!("{}@{}", entry.package_id, entry.version);

        // Remove old entry's size if it exists
        if let Some(old) = self.entries.get(&key) {
            self.total_size_bytes = self.total_size_bytes.saturating_sub(old.size_bytes);
        }

        self.entries.insert(key, entry);
        self.total_size_bytes = self.total_size_bytes.saturating_add(size);
    }

    /// Remove an entry and return it.
    pub fn remove(&mut self, id: &PackageId, version: &Version) -> Option<CacheEntry> {
        let key = Self::key(id, version);
        if let Some(entry) = self.entries.remove(&key) {
            self.total_size_bytes = self.total_size_bytes.saturating_sub(entry.size_bytes);
            Some(entry)
        } else {
            None
        }
    }

    /// Get entries sorted by last access time (oldest first) for LRU eviction.
    pub fn lru_order(&self) -> Vec<(&String, &CacheEntry)> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by_key(|(_, e)| e.last_accessed);
        entries
    }
}

/// Package cache with CacheConfig enforcement.
///
/// Provides caching for downloaded packages with:
/// - Maximum size enforcement via LRU eviction
/// - Metadata TTL expiry
/// - Offline mode support
pub struct PackageCache {
    /// Cache configuration
    config: CacheConfig,
    /// Cache directory path
    cache_dir: PathBuf,
    /// Cache index (in-memory, persisted to disk)
    index: CacheIndex,
    /// Maximum cache size in bytes
    max_size_bytes: u64,
}

impl PackageCache {
    /// Create a new package cache with the given configuration.
    pub fn new(config: CacheConfig) -> CacheResult<Self> {
        let cache_dir = config
            .cache_path()
            .ok_or_else(|| CacheError::Io("Could not determine cache directory".to_string()))?;

        // Create cache directory if needed
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)
                .map_err(|e| CacheError::Io(format!("Failed to create cache directory: {}", e)))?;
        }

        // Load or create index
        let index_path = cache_dir.join("cache_index.json");
        let index = if index_path.exists() {
            let content = fs::read_to_string(&index_path)
                .map_err(|e| CacheError::Io(format!("Failed to read cache index: {}", e)))?;
            serde_json::from_str(&content).unwrap_or_else(|e| {
                warn!("Failed to parse cache index, creating new: {}", e);
                CacheIndex::default()
            })
        } else {
            CacheIndex::default()
        };

        let max_size_bytes = config.max_size_mb * 1024 * 1024;

        let mut cache = Self {
            config,
            cache_dir,
            index,
            max_size_bytes,
        };

        // Enforce size limit on startup (in case config changed)
        cache.enforce_size_limit()?;

        Ok(cache)
    }

    /// Create a cache using the default configuration.
    pub fn default_config() -> CacheResult<Self> {
        Self::new(CacheConfig::default())
    }

    /// Get the cache directory path.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get the current total cache size in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.index.total_size_bytes
    }

    /// Get the maximum cache size in bytes.
    pub fn max_size_bytes(&self) -> u64 {
        self.max_size_bytes
    }

    /// Check if offline mode is enabled.
    pub fn is_offline(&self) -> bool {
        self.config.offline
    }

    /// Get the metadata TTL.
    pub fn metadata_ttl(&self) -> Duration {
        Duration::from_secs(self.config.metadata_ttl_secs)
    }

    /// Get the number of cached packages.
    pub fn entry_count(&self) -> usize {
        self.index.entries.len()
    }

    /// Save the cache index to disk.
    pub fn save_index(&self) -> CacheResult<()> {
        let index_path = self.cache_dir.join("cache_index.json");
        let content = serde_json::to_string_pretty(&self.index)
            .map_err(|e| CacheError::Serialization(format!("Failed to serialize index: {}", e)))?;
        fs::write(&index_path, content)
            .map_err(|e| CacheError::Io(format!("Failed to write cache index: {}", e)))?;
        Ok(())
    }

    /// Store a package in the cache.
    ///
    /// Enforces the maximum cache size by evicting LRU entries if needed.
    pub fn store_package(
        &mut self,
        id: &PackageId,
        version: &Version,
        data: &[u8],
    ) -> CacheResult<()> {
        let size = data.len() as u64;

        // Check if this single package exceeds max cache size
        if size > self.max_size_bytes {
            return Err(CacheError::CacheFull {
                size_bytes: size,
                max_bytes: self.max_size_bytes,
            });
        }

        // Evict entries until we have room
        self.make_room_for(size)?;

        // Create package cache directory
        let pkg_dir = self.cache_dir.join(id.namespace()).join(id.name());
        fs::create_dir_all(&pkg_dir)
            .map_err(|e| CacheError::Io(format!("Failed to create package cache dir: {}", e)))?;

        // Write the tarball
        let file_path = format!("{}/{}/{}.tar.gz", id.namespace(), id.name(), version);
        let full_path = self.cache_dir.join(&file_path);
        fs::write(&full_path, data)
            .map_err(|e| CacheError::Io(format!("Failed to write cache file: {}", e)))?;

        // Update index
        let entry = CacheEntry::new(id, version, size, file_path);
        self.index.insert(entry);
        self.save_index()?;

        debug!("Cached package {}@{} ({} bytes)", id, version, size);

        Ok(())
    }

    /// Get a cached package.
    ///
    /// Returns None if not cached. In offline mode, returns an error if not cached.
    pub fn get_package(
        &mut self,
        id: &PackageId,
        version: &Version,
    ) -> CacheResult<Option<Vec<u8>>> {
        // Check if entry exists
        let entry = match self.index.get_mut(id, version) {
            Some(e) => {
                e.touch(); // Update LRU timestamp
                e.clone()
            }
            None => {
                if self.config.offline {
                    return Err(CacheError::OfflineNotCached {
                        package: id.to_string(),
                        version: version.to_string(),
                    });
                }
                return Ok(None);
            }
        };

        // Read the file
        let full_path = self.cache_dir.join(&entry.file_path);
        match fs::read(&full_path) {
            Ok(data) => {
                self.save_index()?; // Persist updated last_accessed
                Ok(Some(data))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // File missing, remove from index
                warn!(
                    "Cache file missing for {}@{}, removing from index",
                    id, version
                );
                self.index.remove(id, version);
                self.save_index()?;

                if self.config.offline {
                    return Err(CacheError::OfflineNotCached {
                        package: id.to_string(),
                        version: version.to_string(),
                    });
                }
                Ok(None)
            }
            Err(e) => Err(CacheError::Io(format!("Failed to read cache file: {}", e))),
        }
    }

    /// Check if a package is cached (without updating LRU).
    pub fn has_package(&self, id: &PackageId, version: &Version) -> bool {
        if let Some(entry) = self.index.get(id, version) {
            let full_path = self.cache_dir.join(&entry.file_path);
            full_path.exists()
        } else {
            false
        }
    }

    /// Remove a package from the cache.
    pub fn remove_package(&mut self, id: &PackageId, version: &Version) -> CacheResult<bool> {
        if let Some(entry) = self.index.remove(id, version) {
            let full_path = self.cache_dir.join(&entry.file_path);
            if full_path.exists() {
                fs::remove_file(&full_path)
                    .map_err(|e| CacheError::Io(format!("Failed to remove cache file: {}", e)))?;
            }
            self.save_index()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Parse a cache key back to (PackageId, Version).
    fn parse_cache_key(key: &str) -> Option<(super::types::PackageId, super::types::Version)> {
        let (pkg_str, ver_str) = key.rsplit_once('@')?;
        let id = super::types::PackageId::parse(pkg_str)?;
        let version = super::types::Version::parse(ver_str)?;
        Some((id, version))
    }

    /// Evict packages to make room for a new entry.
    fn make_room_for(&mut self, needed_bytes: u64) -> CacheResult<()> {
        // Check if we need to evict anything
        while self.index.total_size_bytes + needed_bytes > self.max_size_bytes {
            // Get the LRU entry
            let lru_entries = self.index.lru_order();
            if lru_entries.is_empty() {
                // No entries to evict, but still over limit
                return Err(CacheError::CacheFull {
                    size_bytes: needed_bytes,
                    max_bytes: self.max_size_bytes,
                });
            }

            // Evict the oldest entry
            let (key, _) = lru_entries[0];
            let key = key.clone();

            // Parse the key and evict
            if let Some((id, version)) = Self::parse_cache_key(&key) {
                info!(
                    "Evicting {}@{} from cache (LRU, size: {} bytes)",
                    id,
                    version,
                    self.index
                        .get(&id, &version)
                        .map(|e| e.size_bytes)
                        .unwrap_or(0)
                );
                self.remove_package(&id, &version)?;
                continue;
            }

            // Fallback: remove entry directly if parsing fails
            if let Some(entry) = self.index.entries.remove(&key) {
                self.index.total_size_bytes =
                    self.index.total_size_bytes.saturating_sub(entry.size_bytes);
                let full_path = self.cache_dir.join(&entry.file_path);
                if full_path.exists() {
                    let _ = fs::remove_file(&full_path);
                }
            }
        }

        Ok(())
    }

    /// Enforce the size limit (called on startup if config changed).
    fn enforce_size_limit(&mut self) -> CacheResult<()> {
        // Evict until under limit
        while self.index.total_size_bytes > self.max_size_bytes {
            let lru_entries = self.index.lru_order();
            if lru_entries.is_empty() {
                break;
            }

            let (key, _) = lru_entries[0];
            let key = key.clone();

            if let Some((id, version)) = Self::parse_cache_key(&key) {
                info!("Evicting {}@{} from cache (over limit)", id, version);
                self.remove_package(&id, &version)?;
                continue;
            }

            // Fallback
            if let Some(entry) = self.index.entries.remove(&key) {
                self.index.total_size_bytes =
                    self.index.total_size_bytes.saturating_sub(entry.size_bytes);
                let full_path = self.cache_dir.join(&entry.file_path);
                if full_path.exists() {
                    let _ = fs::remove_file(&full_path);
                }
            }
        }

        Ok(())
    }

    /// Clear all cached packages.
    pub fn clear(&mut self) -> CacheResult<()> {
        // Remove all package files
        for entry in self.index.entries.values() {
            let full_path = self.cache_dir.join(&entry.file_path);
            if full_path.exists() {
                let _ = fs::remove_file(&full_path);
            }
        }

        // Clear index
        self.index.entries.clear();
        self.index.total_size_bytes = 0;
        self.save_index()?;

        info!("Cleared package cache");
        Ok(())
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.index.entries.len(),
            total_size_bytes: self.index.total_size_bytes,
            max_size_bytes: self.max_size_bytes,
            utilization_percent: if self.max_size_bytes > 0 {
                (self.index.total_size_bytes as f64 / self.max_size_bytes as f64) * 100.0
            } else {
                0.0
            },
            offline_mode: self.config.offline,
            metadata_ttl_secs: self.config.metadata_ttl_secs,
        }
    }

    /// Validate that network fetches are allowed (not in offline mode).
    ///
    /// Call this before making network requests. Returns Ok(()) if network
    /// access is allowed, or an appropriate error if in offline mode.
    pub fn validate_network_access(&self, id: &PackageId, version: &Version) -> CacheResult<()> {
        if self.config.offline {
            // Check if we have it cached
            if self.has_package(id, version) {
                Ok(()) // Can serve from cache
            } else {
                Err(CacheError::OfflineNotCached {
                    package: id.to_string(),
                    version: version.to_string(),
                })
            }
        } else {
            Ok(()) // Network access allowed
        }
    }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of cached packages
    pub entry_count: usize,
    /// Total size of cached packages in bytes
    pub total_size_bytes: u64,
    /// Maximum cache size in bytes
    pub max_size_bytes: u64,
    /// Cache utilization percentage (0-100)
    pub utilization_percent: f64,
    /// Whether offline mode is enabled
    pub offline_mode: bool,
    /// Metadata TTL in seconds
    pub metadata_ttl_secs: u64,
}

impl std::fmt::Display for CacheStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Cache: {} packages, {:.1} MB / {:.1} MB ({:.1}%), offline={}",
            self.entry_count,
            self.total_size_bytes as f64 / (1024.0 * 1024.0),
            self.max_size_bytes as f64 / (1024.0 * 1024.0),
            self.utilization_percent,
            self.offline_mode
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_cache_config(temp_path: PathBuf, max_size_mb: u64) -> CacheConfig {
        CacheConfig {
            path: Some(temp_path),
            max_size_mb,
            metadata_ttl_secs: 3600,
            offline: false,
        }
    }

    #[test]
    fn test_package_cache_new() {
        let temp_dir = tempdir().unwrap();
        let config = test_cache_config(temp_dir.path().to_path_buf(), 100);
        let cache = PackageCache::new(config).unwrap();

        assert!(cache.cache_dir().exists());
        assert_eq!(cache.entry_count(), 0);
        assert_eq!(cache.total_size_bytes(), 0);
    }

    #[test]
    fn test_store_and_get_package() {
        let temp_dir = tempdir().unwrap();
        let config = test_cache_config(temp_dir.path().to_path_buf(), 100);
        let mut cache = PackageCache::new(config).unwrap();

        let id = PackageId::new("test", "pkg");
        let version = Version::new(1, 0, 0);
        let data = b"test package data";

        // Store
        cache.store_package(&id, &version, data).unwrap();
        assert!(cache.has_package(&id, &version));
        assert_eq!(cache.entry_count(), 1);
        assert_eq!(cache.total_size_bytes(), data.len() as u64);

        // Get
        let retrieved = cache.get_package(&id, &version).unwrap().unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn test_remove_package() {
        let temp_dir = tempdir().unwrap();
        let config = test_cache_config(temp_dir.path().to_path_buf(), 100);
        let mut cache = PackageCache::new(config).unwrap();

        let id = PackageId::new("test", "remove");
        let version = Version::new(1, 0, 0);
        let data = b"data to remove";

        cache.store_package(&id, &version, data).unwrap();
        assert!(cache.has_package(&id, &version));

        cache.remove_package(&id, &version).unwrap();
        assert!(!cache.has_package(&id, &version));
        assert_eq!(cache.entry_count(), 0);
    }

    #[test]
    fn test_lru_eviction() {
        let temp_dir = tempdir().unwrap();
        // Small cache for eviction testing.
        let config = CacheConfig {
            path: Some(temp_dir.path().to_path_buf()),
            max_size_mb: 0, // Will be converted to bytes below
            metadata_ttl_secs: 3600,
            offline: false,
        };

        // Create cache with very small max size (1KB) for testing
        let mut cache = PackageCache::new(config).unwrap();
        cache.max_size_bytes = 1024; // 1KB for testing

        let id1 = PackageId::new("test", "pkg1");
        let id2 = PackageId::new("test", "pkg2");
        let id3 = PackageId::new("test", "pkg3");
        let version = Version::new(1, 0, 0);

        // Each package is 400 bytes
        let data = vec![0u8; 400];

        // Store pkg1, pkg2 (800 bytes total, under 1KB limit)
        cache.store_package(&id1, &version, &data).unwrap();
        cache.store_package(&id2, &version, &data).unwrap();

        // Make ordering deterministic: set pkg1 older than pkg2.
        cache
            .index
            .get_mut(&id1, &version)
            .expect("pkg1 should be in cache index")
            .last_accessed = 1;
        cache
            .index
            .get_mut(&id2, &version)
            .expect("pkg2 should be in cache index")
            .last_accessed = 2;

        assert!(cache.has_package(&id1, &version));
        assert!(cache.has_package(&id2, &version));
        assert_eq!(cache.entry_count(), 2);

        // Access pkg1 to make it more recently used
        let _ = cache.get_package(&id1, &version).unwrap();
        assert!(
            cache
                .index
                .get(&id1, &version)
                .expect("pkg1 should be in cache index")
                .last_accessed
                > 2
        );

        // Store pkg3 - should evict pkg2 (oldest) to make room
        cache.store_package(&id3, &version, &data).unwrap();

        // pkg2 should be evicted (it was accessed before pkg1's get())
        assert!(cache.has_package(&id1, &version)); // Most recently accessed
        assert!(!cache.has_package(&id2, &version)); // Evicted
        assert!(cache.has_package(&id3, &version)); // Just added
    }

    #[test]
    fn test_offline_mode() {
        let temp_dir = tempdir().unwrap();
        let config = CacheConfig {
            path: Some(temp_dir.path().to_path_buf()),
            max_size_mb: 100,
            metadata_ttl_secs: 3600,
            offline: true, // Offline mode enabled
        };
        let mut cache = PackageCache::new(config).unwrap();

        let id = PackageId::new("test", "offline");
        let version = Version::new(1, 0, 0);

        // Try to get non-cached package in offline mode
        let result = cache.get_package(&id, &version);
        assert!(matches!(result, Err(CacheError::OfflineNotCached { .. })));

        // Store package, then get should work
        cache.store_package(&id, &version, b"offline data").unwrap();
        let result = cache.get_package(&id, &version);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn test_validate_network_access() {
        let temp_dir = tempdir().unwrap();

        // Online mode
        let config = CacheConfig {
            path: Some(temp_dir.path().to_path_buf()),
            max_size_mb: 100,
            metadata_ttl_secs: 3600,
            offline: false,
        };
        let cache = PackageCache::new(config).unwrap();

        let id = PackageId::new("test", "network");
        let version = Version::new(1, 0, 0);

        // Should always allow network access when online
        assert!(cache.validate_network_access(&id, &version).is_ok());

        // Offline mode
        let temp_dir2 = tempdir().unwrap();
        let config2 = CacheConfig {
            path: Some(temp_dir2.path().to_path_buf()),
            max_size_mb: 100,
            metadata_ttl_secs: 3600,
            offline: true,
        };
        let mut cache2 = PackageCache::new(config2).unwrap();

        // Should fail for non-cached package
        assert!(matches!(
            cache2.validate_network_access(&id, &version),
            Err(CacheError::OfflineNotCached { .. })
        ));

        // Should succeed for cached package
        cache2.store_package(&id, &version, b"data").unwrap();
        assert!(cache2.validate_network_access(&id, &version).is_ok());
    }

    #[test]
    fn test_cached_metadata_expiry() {
        let metadata = CachedMetadata::new("test data", 60); // Large TTL so this test never needs to sleep.

        // Should be valid immediately
        assert!(!metadata.is_expired());
        assert!(metadata.get_if_valid().is_some());

        // Force an expired entry without relying on wall-clock sleeps.
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        let expired = CachedMetadata {
            data: "test data",
            cached_at: now.saturating_sub(2),
            ttl_secs: 1,
        };

        // Should be expired now
        assert!(expired.is_expired());
        assert!(expired.get_if_valid().is_none());
        assert_eq!(expired.time_remaining(), Duration::ZERO);
    }

    #[test]
    fn test_cache_stats() {
        let temp_dir = tempdir().unwrap();
        let config = test_cache_config(temp_dir.path().to_path_buf(), 100);
        let mut cache = PackageCache::new(config).unwrap();

        let id = PackageId::new("test", "stats");
        let version = Version::new(1, 0, 0);
        let data = vec![0u8; 1024 * 1024]; // 1 MB

        cache.store_package(&id, &version, &data).unwrap();

        let stats = cache.stats();
        assert_eq!(stats.entry_count, 1);
        assert_eq!(stats.total_size_bytes, 1024 * 1024);
        assert!(!stats.offline_mode);

        // Display format
        let display = format!("{}", stats);
        assert!(display.contains("1 packages"));
        assert!(display.contains("1.0 MB"));
    }

    #[test]
    fn test_clear_cache() {
        let temp_dir = tempdir().unwrap();
        let config = test_cache_config(temp_dir.path().to_path_buf(), 100);
        let mut cache = PackageCache::new(config).unwrap();

        // Add some packages
        for i in 0..5 {
            let id = PackageId::new("test", &format!("pkg{}", i));
            let version = Version::new(1, 0, 0);
            cache.store_package(&id, &version, b"test data").unwrap();
        }
        assert_eq!(cache.entry_count(), 5);

        // Clear
        cache.clear().unwrap();
        assert_eq!(cache.entry_count(), 0);
        assert_eq!(cache.total_size_bytes(), 0);
    }

    #[test]
    fn test_cache_persistence() {
        let temp_dir = tempdir().unwrap();

        // Create and populate cache
        {
            let config = test_cache_config(temp_dir.path().to_path_buf(), 100);
            let mut cache = PackageCache::new(config).unwrap();

            let id = PackageId::new("test", "persist");
            let version = Version::new(1, 0, 0);
            cache
                .store_package(&id, &version, b"persistent data")
                .unwrap();
        }

        // Reopen cache and verify data persisted
        {
            let config = test_cache_config(temp_dir.path().to_path_buf(), 100);
            let mut cache = PackageCache::new(config).unwrap();

            let id = PackageId::new("test", "persist");
            let version = Version::new(1, 0, 0);
            assert!(cache.has_package(&id, &version));

            let data = cache.get_package(&id, &version).unwrap().unwrap();
            assert_eq!(data, b"persistent data");
        }
    }

    #[test]
    fn test_package_too_large() {
        let temp_dir = tempdir().unwrap();
        let config = CacheConfig {
            path: Some(temp_dir.path().to_path_buf()),
            max_size_mb: 0, // 0 MB = effectively disabled
            metadata_ttl_secs: 3600,
            offline: false,
        };
        let mut cache = PackageCache::new(config).unwrap();
        cache.max_size_bytes = 100; // 100 bytes max for testing

        let id = PackageId::new("test", "toolarge");
        let version = Version::new(1, 0, 0);
        let data = vec![0u8; 200]; // 200 bytes, exceeds 100 byte limit

        let result = cache.store_package(&id, &version, &data);
        assert!(matches!(result, Err(CacheError::CacheFull { .. })));
    }

    #[test]
    fn test_cache_error_display() {
        let err = CacheError::OfflineNotCached {
            package: "test/pkg".to_string(),
            version: "1.0.0".to_string(),
        };
        assert!(err.to_string().contains("Offline mode"));
        assert!(err.to_string().contains("test/pkg@1.0.0"));

        let err = CacheError::CacheFull {
            size_bytes: 1000,
            max_bytes: 500,
        };
        assert!(err.to_string().contains("Cache full"));
    }
}
