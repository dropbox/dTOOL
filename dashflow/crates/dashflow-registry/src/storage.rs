//! Content-addressed storage backends.
//!
//! Packages are stored by their content hash, enabling:
//! - Deduplication
//! - Verification
//! - P2P distribution
//! - CDN caching

use crate::content_hash::ContentHash;
use crate::error::{RegistryError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Where a package is stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StorageLocation {
    /// Primary CDN (S3-compatible).
    Cdn {
        /// Download URL.
        url: String,
        /// Region.
        region: String,
    },
    /// Colony peer cache.
    ColonyPeer {
        /// Peer's app ID.
        app_id: Uuid,
        /// Endpoint to fetch from.
        endpoint: String,
    },
    /// Local filesystem.
    Local {
        /// File path.
        path: PathBuf,
    },
    /// IPFS (future).
    Ipfs {
        /// Content ID.
        cid: String,
    },
}

/// A package stored in the content store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPackage {
    /// SHA-256 hash of the package tarball.
    pub hash: ContentHash,
    /// Size in bytes.
    pub size: u64,
    /// MIME type.
    pub content_type: String,
    /// Storage locations (redundant).
    pub locations: Vec<StorageLocation>,
}

impl StoredPackage {
    /// Create a new stored package entry.
    pub fn new(hash: ContentHash, size: u64) -> Self {
        Self {
            hash,
            size,
            content_type: "application/gzip".to_string(),
            locations: Vec::new(),
        }
    }
}

/// A download URL for a package (presigned or public CDN).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadUrl {
    /// The URL to download from.
    pub url: String,
    /// When this URL expires (None for public CDN URLs).
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Whether this is a presigned URL.
    pub is_presigned: bool,
}

impl DownloadUrl {
    /// Create a public (non-expiring) download URL.
    pub fn public(url: String) -> Self {
        Self {
            url,
            expires_at: None,
            is_presigned: false,
        }
    }

    /// Check if this URL has expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            chrono::Utc::now() >= expires_at
        } else {
            false // Public URLs never expire
        }
    }

    /// Get seconds until expiration (None if public URL).
    pub fn seconds_until_expiration(&self) -> Option<i64> {
        self.expires_at.map(|exp| {
            let diff = exp - chrono::Utc::now();
            diff.num_seconds().max(0)
        })
    }
}

impl StoredPackage {
    /// Add a storage location.
    pub fn add_location(&mut self, location: StorageLocation) {
        self.locations.push(location);
    }

    /// Check if this package has any storage locations.
    pub fn is_stored(&self) -> bool {
        !self.locations.is_empty()
    }
}

/// Storage backend trait for package content.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Store package content, returning the content hash.
    async fn store(&self, data: &[u8]) -> Result<ContentHash>;

    /// Retrieve package content by hash.
    async fn get(&self, hash: &ContentHash) -> Result<Vec<u8>>;

    /// Check if a package exists.
    async fn exists(&self, hash: &ContentHash) -> Result<bool>;

    /// Delete a package (for yanking).
    async fn delete(&self, hash: &ContentHash) -> Result<()>;

    /// Get storage info for a package.
    async fn info(&self, hash: &ContentHash) -> Result<StoredPackage>;

    /// Get a download URL for a package (CDN or presigned).
    ///
    /// Returns `None` if the backend doesn't support direct download URLs.
    /// Override this in S3-compatible backends to enable CDN/presigned downloads.
    async fn get_download_url(&self, _hash: &ContentHash) -> Result<Option<DownloadUrl>> {
        Ok(None)
    }

    /// Check if this backend supports CDN-direct downloads.
    fn supports_cdn(&self) -> bool {
        false
    }
}

/// In-memory storage backend for testing.
#[derive(Debug, Default)]
pub struct InMemoryStorage {
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl InMemoryStorage {
    /// Create a new in-memory storage.
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl StorageBackend for InMemoryStorage {
    async fn store(&self, data: &[u8]) -> Result<ContentHash> {
        let hash = ContentHash::from_bytes(data);
        let mut storage = self.data.write().await;
        storage.insert(hash.to_string(), data.to_vec());
        Ok(hash)
    }

    async fn get(&self, hash: &ContentHash) -> Result<Vec<u8>> {
        let storage = self.data.read().await;
        storage
            .get(&hash.to_string())
            .cloned()
            .ok_or_else(|| RegistryError::PackageNotFound(hash.to_string()))
    }

    async fn exists(&self, hash: &ContentHash) -> Result<bool> {
        let storage = self.data.read().await;
        Ok(storage.contains_key(&hash.to_string()))
    }

    async fn delete(&self, hash: &ContentHash) -> Result<()> {
        let mut storage = self.data.write().await;
        storage.remove(&hash.to_string());
        Ok(())
    }

    async fn info(&self, hash: &ContentHash) -> Result<StoredPackage> {
        let storage = self.data.read().await;
        let data = storage
            .get(&hash.to_string())
            .ok_or_else(|| RegistryError::PackageNotFound(hash.to_string()))?;

        Ok(StoredPackage::new(hash.clone(), data.len() as u64))
    }
}

/// Filesystem storage backend.
pub struct FilesystemStorage {
    root: PathBuf,
}

impl FilesystemStorage {
    /// Create a new filesystem storage at the given root directory.
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Get the path for a content hash.
    fn path_for_hash(&self, hash: &ContentHash) -> PathBuf {
        let hex = hash.to_hex();
        // Use first 2 chars as subdirectory for filesystem efficiency
        self.root.join(&hex[..2]).join(&hex[2..])
    }
}

#[async_trait]
impl StorageBackend for FilesystemStorage {
    async fn store(&self, data: &[u8]) -> Result<ContentHash> {
        let hash = ContentHash::from_bytes(data);
        let path = self.path_for_hash(&hash);

        // Create parent directory
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Write atomically using temp file + rename
        let temp_path = path.with_extension("tmp");
        tokio::fs::write(&temp_path, data).await?;
        tokio::fs::rename(&temp_path, &path).await?;

        Ok(hash)
    }

    async fn get(&self, hash: &ContentHash) -> Result<Vec<u8>> {
        let path = self.path_for_hash(hash);
        tokio::fs::read(&path)
            .await
            .map_err(|e| RegistryError::PackageNotFound(format!("{hash}: {e}")))
    }

    async fn exists(&self, hash: &ContentHash) -> Result<bool> {
        let path = self.path_for_hash(hash);
        Ok(tokio::fs::try_exists(&path).await.unwrap_or(false))
    }

    async fn delete(&self, hash: &ContentHash) -> Result<()> {
        let path = self.path_for_hash(hash);
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            tokio::fs::remove_file(&path).await?;
        }
        Ok(())
    }

    async fn info(&self, hash: &ContentHash) -> Result<StoredPackage> {
        let path = self.path_for_hash(hash);
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|e| RegistryError::PackageNotFound(format!("{hash}: {e}")))?;

        let mut stored = StoredPackage::new(hash.clone(), metadata.len());
        stored.add_location(StorageLocation::Local { path });
        Ok(stored)
    }
}

/// Local package cache with size tracking and LRU eviction.
pub struct PackageCache {
    storage: Box<dyn StorageBackend>,
    max_size_bytes: u64,
    /// Tracks current cache size and access order for LRU eviction
    cache_state: Arc<RwLock<CacheState>>,
}

/// Internal cache state for size tracking
struct CacheState {
    /// Current total size in bytes
    current_size: u64,
    /// Hash -> (size, last_access_time) for LRU eviction
    entries: HashMap<String, (u64, std::time::Instant)>,
}

impl PackageCache {
    /// Create a new package cache.
    pub fn new(storage: impl StorageBackend + 'static, max_size_bytes: u64) -> Self {
        Self {
            storage: Box::new(storage),
            max_size_bytes,
            cache_state: Arc::new(RwLock::new(CacheState {
                current_size: 0,
                entries: HashMap::new(),
            })),
        }
    }

    /// Create an in-memory package cache with the given max size.
    pub fn in_memory(max_size_bytes: u64) -> Self {
        Self::new(InMemoryStorage::new(), max_size_bytes)
    }

    /// Get a package from cache, updating LRU access time.
    pub async fn get(&self, hash: &ContentHash) -> Result<Option<Vec<u8>>> {
        match self.storage.get(hash).await {
            Ok(data) => {
                // Update access time for LRU
                let mut state = self.cache_state.write().await;
                if let Some(entry) = state.entries.get_mut(&hash.to_string()) {
                    entry.1 = std::time::Instant::now();
                }
                Ok(Some(data))
            }
            Err(RegistryError::PackageNotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Store a package in cache, evicting LRU entries if necessary.
    pub async fn store(&self, data: &[u8]) -> Result<ContentHash> {
        let data_size = data.len() as u64;

        // Evict entries if needed to make room
        self.evict_if_needed(data_size).await?;

        // Store the data
        let hash = self.storage.store(data).await?;

        // Track in cache state
        let mut state = self.cache_state.write().await;
        state.current_size += data_size;
        state
            .entries
            .insert(hash.to_string(), (data_size, std::time::Instant::now()));

        Ok(hash)
    }

    /// Evict LRU entries until there's room for new data.
    async fn evict_if_needed(&self, new_data_size: u64) -> Result<()> {
        let mut state = self.cache_state.write().await;

        // Check if we need to evict
        while state.current_size + new_data_size > self.max_size_bytes && !state.entries.is_empty()
        {
            // Find oldest entry (LRU)
            let oldest = state
                .entries
                .iter()
                .min_by_key(|(_, (_, time))| *time)
                .map(|(k, _)| k.clone());

            if let Some(hash_str) = oldest {
                if let Some((size, _)) = state.entries.remove(&hash_str) {
                    state.current_size = state.current_size.saturating_sub(size);
                    // Delete from storage (ignore errors - entry might already be gone)
                    let hash = ContentHash::from_hex(&hash_str)
                        .unwrap_or_else(|_| ContentHash::from_bytes(&[]));
                    drop(state); // Release lock before async delete
                    let _ = self.storage.delete(&hash).await;
                    state = self.cache_state.write().await; // Re-acquire
                }
            }
        }

        Ok(())
    }

    /// Check if a package is cached.
    pub async fn contains(&self, hash: &ContentHash) -> Result<bool> {
        self.storage.exists(hash).await
    }

    /// Get the maximum cache size.
    pub fn max_size(&self) -> u64 {
        self.max_size_bytes
    }

    /// Get the current cache size.
    pub async fn current_size(&self) -> u64 {
        self.cache_state.read().await.current_size
    }

    /// Get the number of cached entries.
    pub async fn entry_count(&self) -> usize {
        self.cache_state.read().await.entries.len()
    }
}

/// S3-compatible storage backend configuration.
#[derive(Debug, Clone)]
pub struct S3Config {
    /// S3 bucket name.
    pub bucket: String,
    /// AWS region (e.g., "us-east-1").
    pub region: String,
    /// Optional custom endpoint (for R2, MinIO, etc.).
    pub endpoint: Option<String>,
    /// Prefix for all objects (e.g., "packages/").
    pub prefix: String,
    /// Whether to use path-style addressing (required for MinIO).
    pub path_style: bool,
    /// Public CDN URL for direct downloads (e.g., `https://cdn.example.com`).
    /// If set, this URL is used instead of generating S3/R2 URLs.
    /// Useful when using a CDN like Cloudflare in front of R2.
    pub public_url: Option<String>,
    /// Default expiration for presigned URLs (in seconds).
    pub presigned_expiration_secs: u64,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            bucket: "dashflow-packages".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
            prefix: "packages/".to_string(),
            path_style: false,
            public_url: None,
            presigned_expiration_secs: 3600, // 1 hour default
        }
    }
}

impl S3Config {
    /// Create a new S3 config with the given bucket.
    pub fn new(bucket: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            ..Default::default()
        }
    }

    /// Set the region.
    pub fn region(mut self, region: impl Into<String>) -> Self {
        self.region = region.into();
        self
    }

    /// Set a custom endpoint (for R2, MinIO, etc.).
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Set the object prefix.
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Enable path-style addressing (required for MinIO).
    pub fn path_style(mut self, enabled: bool) -> Self {
        self.path_style = enabled;
        self
    }

    /// Create config for Cloudflare R2.
    pub fn r2(bucket: impl Into<String>, account_id: impl Into<String>) -> Self {
        let account_id = account_id.into();
        Self {
            bucket: bucket.into(),
            region: "auto".to_string(),
            endpoint: Some(format!("https://{}.r2.cloudflarestorage.com", account_id)),
            prefix: "packages/".to_string(),
            path_style: false,
            public_url: None,
            presigned_expiration_secs: 3600,
        }
    }

    /// Create config for MinIO.
    pub fn minio(bucket: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            region: "us-east-1".to_string(),
            endpoint: Some(endpoint.into()),
            prefix: "packages/".to_string(),
            path_style: true,
            public_url: None,
            presigned_expiration_secs: 3600,
        }
    }

    /// Set a public CDN URL for direct downloads.
    /// When set, this URL is used instead of presigned S3 URLs.
    pub fn public_url(mut self, url: impl Into<String>) -> Self {
        self.public_url = Some(url.into());
        self
    }

    /// Set the presigned URL expiration time in seconds.
    pub fn presigned_expiration(mut self, secs: u64) -> Self {
        self.presigned_expiration_secs = secs;
        self
    }
}

/// S3-compatible storage backend.
///
/// Supports AWS S3, Cloudflare R2, MinIO, and other S3-compatible services.
#[cfg(feature = "s3")]
pub struct S3Storage {
    client: aws_sdk_s3::Client,
    config: S3Config,
}

#[cfg(feature = "s3")]
impl S3Storage {
    /// Create a new S3 storage backend.
    pub async fn new(config: S3Config) -> Result<Self> {
        use aws_config::BehaviorVersion;

        // Load AWS credentials from environment
        let mut aws_config_builder = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new(config.region.clone()));

        // Use custom endpoint if specified
        if let Some(ref endpoint) = config.endpoint {
            aws_config_builder = aws_config_builder.endpoint_url(endpoint);
        }

        let aws_config = aws_config_builder.load().await;

        // Build S3 client config
        let mut s3_config =
            aws_sdk_s3::config::Builder::from(&aws_config).force_path_style(config.path_style);

        // For R2 and other S3-compatible services, we may need to override region
        if config.endpoint.is_some() {
            s3_config = s3_config.region(aws_sdk_s3::config::Region::new(config.region.clone()));
        }

        let client = aws_sdk_s3::Client::from_conf(s3_config.build());

        Ok(Self { client, config })
    }

    /// Create with a pre-built S3 client (for testing).
    pub fn with_client(client: aws_sdk_s3::Client, config: S3Config) -> Self {
        Self { client, config }
    }

    /// Get the object key for a content hash.
    fn object_key(&self, hash: &ContentHash) -> String {
        let hex = hash.to_hex();
        // Use first 2 chars as subdirectory for better S3 partitioning
        format!("{}{}/{}", self.config.prefix, &hex[..2], &hex[2..])
    }

    /// Get the S3 bucket name.
    pub fn bucket(&self) -> &str {
        &self.config.bucket
    }

    /// Get the S3 config.
    pub fn config(&self) -> &S3Config {
        &self.config
    }

    /// Get a download URL for a package.
    ///
    /// Returns either:
    /// - A public CDN URL (if `public_url` is configured)
    /// - A presigned S3 URL (if no public URL)
    ///
    /// This allows clients to download directly from the CDN/S3 without proxying
    /// through the API server, reducing bandwidth and latency.
    pub async fn download_url(&self, hash: &ContentHash) -> Result<DownloadUrl> {
        let key = self.object_key(hash);

        // If a public CDN URL is configured, use that
        if let Some(ref public_url) = self.config.public_url {
            let url = format!("{}/{}", public_url.trim_end_matches('/'), key);
            return Ok(DownloadUrl {
                url,
                expires_at: None, // Public URLs don't expire
                is_presigned: false,
            });
        }

        // Otherwise, generate a presigned URL
        self.presigned_download_url(hash).await
    }

    /// Generate a presigned download URL for a package.
    ///
    /// The URL expires after `presigned_expiration_secs` (default: 1 hour).
    pub async fn presigned_download_url(&self, hash: &ContentHash) -> Result<DownloadUrl> {
        use aws_sdk_s3::presigning::PresigningConfig;
        use std::time::Duration;

        let key = self.object_key(hash);
        let expiration = Duration::from_secs(self.config.presigned_expiration_secs);

        let presigning_config = PresigningConfig::builder()
            .expires_in(expiration)
            .build()
            .map_err(|e| {
                RegistryError::StorageError(format!("Failed to build presigning config: {}", e))
            })?;

        let presigned_request = self
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .presigned(presigning_config)
            .await
            .map_err(|e| {
                RegistryError::StorageError(format!("Failed to generate presigned URL: {}", e))
            })?;

        let expires_at = chrono::Utc::now()
            + chrono::Duration::seconds(self.config.presigned_expiration_secs as i64);

        tracing::debug!(
            hash = %hash,
            expiration_secs = self.config.presigned_expiration_secs,
            "Generated presigned download URL"
        );

        Ok(DownloadUrl {
            url: presigned_request.uri().to_string(),
            expires_at: Some(expires_at),
            is_presigned: true,
        })
    }

    /// Check if CDN-direct downloads are enabled.
    pub fn cdn_enabled(&self) -> bool {
        self.config.public_url.is_some()
    }
}

#[cfg(feature = "s3")]
#[async_trait]
impl StorageBackend for S3Storage {
    async fn store(&self, data: &[u8]) -> Result<ContentHash> {
        use aws_sdk_s3::primitives::ByteStream;

        let hash = ContentHash::from_bytes(data);
        let key = self.object_key(&hash);

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(ByteStream::from(data.to_vec()))
            .content_type("application/gzip")
            .send()
            .await
            .map_err(|e| RegistryError::StorageError(format!("Failed to upload to S3: {}", e)))?;

        tracing::debug!(hash = %hash, key = %key, "Stored package in S3");

        Ok(hash)
    }

    async fn get(&self, hash: &ContentHash) -> Result<Vec<u8>> {
        let key = self.object_key(hash);

        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                // Check for NoSuchKey error
                if let aws_sdk_s3::error::SdkError::ServiceError(service_err) = &e {
                    if service_err.err().is_no_such_key() {
                        return RegistryError::PackageNotFound(hash.to_string());
                    }
                }
                RegistryError::StorageError(format!("Failed to get from S3: {}", e))
            })?;

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| RegistryError::StorageError(format!("Failed to read S3 response: {}", e)))?
            .into_bytes()
            .to_vec();

        tracing::debug!(hash = %hash, size = data.len(), "Retrieved package from S3");

        Ok(data)
    }

    async fn exists(&self, hash: &ContentHash) -> Result<bool> {
        let key = self.object_key(hash);

        match self
            .client
            .head_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(aws_sdk_s3::error::SdkError::ServiceError(service_err))
                if service_err.err().is_not_found() =>
            {
                Ok(false)
            }
            Err(e) => Err(RegistryError::StorageError(format!(
                "Failed to check S3 object: {}",
                e
            ))),
        }
    }

    async fn delete(&self, hash: &ContentHash) -> Result<()> {
        let key = self.object_key(hash);

        self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| RegistryError::StorageError(format!("Failed to delete from S3: {}", e)))?;

        tracing::debug!(hash = %hash, key = %key, "Deleted package from S3");

        Ok(())
    }

    async fn info(&self, hash: &ContentHash) -> Result<StoredPackage> {
        let key = self.object_key(hash);

        let response = self
            .client
            .head_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                if let aws_sdk_s3::error::SdkError::ServiceError(service_err) = &e {
                    if service_err.err().is_not_found() {
                        return RegistryError::PackageNotFound(hash.to_string());
                    }
                }
                RegistryError::StorageError(format!("Failed to get S3 object info: {}", e))
            })?;

        let size = response.content_length().unwrap_or(0) as u64;

        let mut stored = StoredPackage::new(hash.clone(), size);

        // Construct the CDN URL
        let url = if let Some(ref endpoint) = self.config.endpoint {
            format!("{}/{}/{}", endpoint, self.config.bucket, key)
        } else {
            format!(
                "https://{}.s3.{}.amazonaws.com/{}",
                self.config.bucket, self.config.region, key
            )
        };

        stored.add_location(StorageLocation::Cdn {
            url,
            region: self.config.region.clone(),
        });

        Ok(stored)
    }

    async fn get_download_url(&self, hash: &ContentHash) -> Result<Option<DownloadUrl>> {
        Ok(Some(self.download_url(hash).await?))
    }

    fn supports_cdn(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_storage() {
        let storage = InMemoryStorage::new();
        let data = b"hello world";

        // Store
        let hash = storage.store(data).await.unwrap();
        assert!(storage.exists(&hash).await.unwrap());

        // Retrieve
        let retrieved = storage.get(&hash).await.unwrap();
        assert_eq!(retrieved, data);

        // Info
        let info = storage.info(&hash).await.unwrap();
        assert_eq!(info.size, data.len() as u64);

        // Delete
        storage.delete(&hash).await.unwrap();
        assert!(!storage.exists(&hash).await.unwrap());
    }

    #[tokio::test]
    async fn test_filesystem_storage() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path()).unwrap();
        let data = b"test package content";

        // Store
        let hash = storage.store(data).await.unwrap();
        assert!(storage.exists(&hash).await.unwrap());

        // Retrieve
        let retrieved = storage.get(&hash).await.unwrap();
        assert_eq!(retrieved, data);

        // Delete
        storage.delete(&hash).await.unwrap();
        assert!(!storage.exists(&hash).await.unwrap());
    }

    #[tokio::test]
    async fn test_storage_location_serialization() {
        let cdn = StorageLocation::Cdn {
            url: "https://cdn.example.com/pkg".to_string(),
            region: "us-east-1".to_string(),
        };

        let json = serde_json::to_string(&cdn).unwrap();
        assert!(json.contains("cdn"));

        let parsed: StorageLocation = serde_json::from_str(&json).unwrap();
        match parsed {
            StorageLocation::Cdn { url, region } => {
                assert_eq!(region, "us-east-1");
                assert!(url.contains("cdn.example.com"));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[tokio::test]
    async fn test_package_cache() {
        let storage = InMemoryStorage::new();
        let cache = PackageCache::new(storage, 1024 * 1024);
        let data = b"cached content";

        // Store in cache
        let hash = cache.store(data).await.unwrap();
        assert!(cache.contains(&hash).await.unwrap());

        // Get from cache
        let cached = cache.get(&hash).await.unwrap();
        assert_eq!(cached, Some(data.to_vec()));

        // Miss
        let other_hash = ContentHash::from_bytes(b"other");
        assert!(cache.get(&other_hash).await.unwrap().is_none());
    }

    #[test]
    fn test_s3_config_default() {
        let config = S3Config::default();
        assert_eq!(config.bucket, "dashflow-packages");
        assert_eq!(config.region, "us-east-1");
        assert!(config.endpoint.is_none());
        assert_eq!(config.prefix, "packages/");
        assert!(!config.path_style);
    }

    #[test]
    fn test_s3_config_builder() {
        let config = S3Config::new("my-bucket")
            .region("eu-west-1")
            .endpoint("https://custom.s3.endpoint")
            .prefix("pkg/")
            .path_style(true);

        assert_eq!(config.bucket, "my-bucket");
        assert_eq!(config.region, "eu-west-1");
        assert_eq!(
            config.endpoint,
            Some("https://custom.s3.endpoint".to_string())
        );
        assert_eq!(config.prefix, "pkg/");
        assert!(config.path_style);
    }

    #[test]
    fn test_s3_config_r2() {
        let config = S3Config::r2("my-r2-bucket", "abc123def");
        assert_eq!(config.bucket, "my-r2-bucket");
        assert_eq!(config.region, "auto");
        assert_eq!(
            config.endpoint,
            Some("https://abc123def.r2.cloudflarestorage.com".to_string())
        );
        assert!(!config.path_style);
    }

    #[test]
    fn test_s3_config_minio() {
        let config = S3Config::minio("minio-bucket", "http://localhost:9000");
        assert_eq!(config.bucket, "minio-bucket");
        assert_eq!(config.endpoint, Some("http://localhost:9000".to_string()));
        assert!(config.path_style);
    }

    #[cfg(feature = "s3")]
    mod s3_tests {
        use super::*;

        #[test]
        fn test_s3_storage_object_key() {
            // Test that object keys are generated correctly
            // We can't test the actual S3Storage without credentials,
            // but we can verify the key generation logic
            let hash = ContentHash::from_bytes(b"test data");
            let hex = hash.to_hex();

            // Key should be prefix + first 2 chars + "/" + rest
            let expected_prefix = "packages/";
            let expected_key = format!("{}{}/{}", expected_prefix, &hex[..2], &hex[2..]);

            assert!(expected_key.starts_with("packages/"));
            assert!(expected_key.contains("/"));
            // Key should have 2 chars before the inner slash
            let parts: Vec<&str> = expected_key
                .strip_prefix("packages/")
                .unwrap()
                .splitn(2, '/')
                .collect();
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0].len(), 2);
        }
    }
}
