//! Package Routes
//!
//! Handlers for package operations: publish, get, resolve, yank.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use chrono::Utc;

use crate::api::{
    types::{
        error_codes, ApiError, PublishRequest, PublishResponse, ResolveRequest, ResolveResponse,
        SignatureInfo,
    },
    AppState,
};
use crate::cache::{cache_get_json, cache_set_json, keys as cache_keys};
use crate::{ContentHash, Resolution};

/// Package routes
pub fn routes() -> Router<AppState> {
    Router::new()
        // POST /packages - Publish a new package
        .route("/", post(publish_package))
        // GET /packages/:hash - Get package data by content hash
        .route("/:hash", get(get_package))
        // POST /packages/resolve - Resolve name@version to hash
        .route("/resolve", post(resolve_package))
        // DELETE /packages/:hash - Yank a package (mark unavailable)
        .route("/:hash", delete(yank_package))
}

/// Publish a new package
async fn publish_package(
    State(state): State<AppState>,
    Json(request): Json<PublishRequest>,
) -> Result<Json<PublishResponse>, (StatusCode, Json<ApiError>)> {
    // Decode the base64 content
    use base64::Engine;
    let content = base64::engine::general_purpose::STANDARD
        .decode(&request.content)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    error_codes::INVALID_REQUEST,
                    format!("Invalid base64 content: {}", e),
                )),
            )
        })?;

    // Calculate content hash
    let hash = ContentHash::from_bytes(&content);

    // Verify signature
    let signature_verified = state
        .trust
        .verify_data_signature(&content, &request.signature, &request.public_key)
        .unwrap_or(false);

    if !signature_verified {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                error_codes::SIGNATURE_INVALID,
                "Package signature verification failed",
            )),
        ));
    }

    // Store in primary storage (S3, filesystem, etc.)
    let stored_hash = state.storage.store(&content).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                error_codes::INTERNAL_ERROR,
                format!("Failed to store package: {}", e),
            )),
        )
    })?;

    // Verify the stored hash matches the calculated hash
    if stored_hash != hash {
        tracing::error!(
            expected = %hash,
            actual = %stored_hash,
            "Content hash mismatch after storage"
        );
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                error_codes::INTERNAL_ERROR,
                "Content hash mismatch after storage",
            )),
        ));
    }

    // Also store in local cache for faster reads
    {
        let cache = state.cache.write().await;
        if let Err(e) = cache.store(&content).await {
            tracing::debug!(hash = %hash, error = %e, "Failed to store package in local cache (non-fatal)");
        }
    }

    // Invalidate resolution cache for this package name
    // This ensures subsequent resolves get the new version
    // Policy: Log failures but don't fail the publish - the package is stored successfully
    // and cache will eventually expire. Warn level since stale cache affects consistency.
    let package_name = &request.manifest.name;
    let resolution_pattern = cache_keys::resolution_pattern(package_name);
    if let Err(e) = state.data_cache.delete_pattern(&resolution_pattern).await {
        tracing::warn!(
            package = %package_name,
            pattern = %resolution_pattern,
            error = %e,
            "Failed to invalidate resolution cache - clients may see stale data until TTL expires"
        );
    }

    // Also invalidate the "latest" cache entry specifically
    let latest_key = cache_keys::resolution_latest(package_name);
    if let Err(e) = state.data_cache.delete(&latest_key).await {
        tracing::warn!(
            package = %package_name,
            key = %latest_key,
            error = %e,
            "Failed to invalidate 'latest' resolution cache entry"
        );
    }

    // Invalidate search cache since new package affects search results
    let search_pattern = cache_keys::search_pattern();
    if let Err(e) = state.data_cache.delete_pattern(&search_pattern).await {
        tracing::warn!(
            pattern = %search_pattern,
            error = %e,
            "Failed to invalidate search cache - search results may be stale"
        );
    }

    Ok(Json(PublishResponse {
        hash: hash.to_string(),
        version: request.manifest.version.to_string(),
        signature_verified,
        published_at: Utc::now(),
    }))
}

/// Get package by content hash - returns the raw content
///
/// Uses a read-through cache pattern:
/// 1. Check local cache first (fast path)
/// 2. If cache miss, fetch from primary storage (S3, filesystem, etc.)
/// 3. Cache the result for future requests
async fn get_package(
    State(state): State<AppState>,
    Path(hash_str): Path<String>,
) -> Result<Vec<u8>, (StatusCode, Json<ApiError>)> {
    // Parse content hash
    let hash = ContentHash::from_string(&hash_str).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                error_codes::INVALID_REQUEST,
                format!("Invalid content hash format: {e}"),
            )),
        )
    })?;

    // Check local cache first (fast path)
    {
        let cache = state.cache.read().await;
        if let Ok(Some(data)) = cache.get(&hash).await {
            tracing::debug!(hash = %hash_str, "Cache hit for package");
            return Ok(data);
        }
    }

    // Cache miss - fetch from primary storage
    tracing::debug!(hash = %hash_str, "Cache miss, fetching from primary storage");

    let data = state.storage.get(&hash).await.map_err(|e| match e {
        crate::RegistryError::PackageNotFound(_) => (
            StatusCode::NOT_FOUND,
            Json(ApiError::new(error_codes::NOT_FOUND, "Package not found")),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                error_codes::INTERNAL_ERROR,
                format!("Failed to get package: {}", e),
            )),
        ),
    })?;

    // Cache the result for future requests
    // Policy: Log failures at debug level - caching is an optimization, not critical
    {
        let cache = state.cache.write().await;
        if let Err(e) = cache.store(&data).await {
            tracing::debug!(hash = %hash_str, error = %e, "Failed to cache package data (non-fatal)");
        }
    }

    Ok(data)
}

/// Resolve package name/version to content hash
async fn resolve_package(
    State(state): State<AppState>,
    Json(request): Json<ResolveRequest>,
) -> Result<Json<ResolveResponse>, (StatusCode, Json<ApiError>)> {
    use semver::VersionReq;

    // Generate cache key based on name and version
    let cache_key = if let Some(ref version_str) = request.version {
        cache_keys::resolution(&request.name, version_str)
    } else {
        cache_keys::resolution_latest(&request.name)
    };

    // Check cache first
    if let Ok(Some(cached)) =
        cache_get_json::<Resolution>(state.data_cache.as_ref(), &cache_key).await
    {
        let download_url = format!("{}/api/v1/packages/{}", state.config.base_url, cached.hash);

        // Get CDN URL if enabled
        let (cdn_url, cdn_expires_at) = if state.config.cdn_enabled && state.storage.supports_cdn()
        {
            match state.storage.get_download_url(&cached.hash).await {
                Ok(Some(download)) => (Some(download.url), download.expires_at),
                Ok(None) => (None, None),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to generate CDN download URL");
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        return Ok(Json(ResolveResponse {
            name: cached.name,
            version: cached.version.to_string(),
            hash: cached.hash.to_string(),
            download_url,
            cdn_url,
            cdn_expires_at,
            signatures: Vec::new(),
        }));
    }

    // Cache miss - resolve from metadata store
    let resolution = if let Some(version_str) = &request.version {
        // Parse version requirement
        let version_req = VersionReq::parse(version_str).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    error_codes::INVALID_REQUEST,
                    format!("Invalid version requirement '{}': {}", version_str, e),
                )),
            )
        })?;

        state.metadata.resolve(&request.name, &version_req).await
    } else {
        // No version specified - get latest
        state.metadata.resolve_latest(&request.name).await
    };

    // Handle result
    let resolution = resolution.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                error_codes::INTERNAL_ERROR,
                format!("Resolution failed: {}", e),
            )),
        )
    })?;

    match resolution {
        Some(res) => {
            // Cache the result for faster subsequent lookups
            // Policy: Log failures at debug level - caching is an optimization
            if let Err(e) = cache_set_json(
                state.data_cache.as_ref(),
                &cache_key,
                &res,
                Some(state.cache_config.resolution_ttl),
            )
            .await
            {
                tracing::debug!(
                    cache_key = %cache_key,
                    package = %res.name,
                    version = %res.version,
                    error = %e,
                    "Failed to cache resolution result (non-fatal)"
                );
            }

            let download_url = format!("{}/api/v1/packages/{}", state.config.base_url, res.hash);

            // Get CDN URL if enabled
            let (cdn_url, cdn_expires_at) =
                if state.config.cdn_enabled && state.storage.supports_cdn() {
                    match state.storage.get_download_url(&res.hash).await {
                        Ok(Some(download)) => (Some(download.url), download.expires_at),
                        Ok(None) => (None, None),
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to generate CDN download URL");
                            (None, None)
                        }
                    }
                } else {
                    (None, None)
                };

            // Fetch signature info from trust service using package metadata (M-225)
            // Include public_key_bytes to enable client-side verification
            let signatures = match state.metadata.get_by_hash(&res.hash).await {
                Ok(Some(pkg_info)) => {
                    // Look up the publisher key in the trust service
                    match state.trust.get_key(&pkg_info.publisher_key_id) {
                        Some(key_entry) => vec![SignatureInfo {
                            key_id: key_entry.key.key_id.clone(),
                            owner: key_entry.key.owner.clone(),
                            trust_level: key_entry.trust_level,
                            timestamp: pkg_info.published_at,
                            // M-225: Include public key bytes for client-side verification
                            // Future: Store actual signature bytes in registry metadata
                            // (requires schema change: PackageMetadata.signature_bytes field)
                            signature_bytes: None,
                            public_key_bytes: Some(hex::encode(key_entry.key.bytes)),
                        }],
                        None => Vec::new(),
                    }
                }
                Ok(None) | Err(_) => Vec::new(),
            };

            Ok(Json(ResolveResponse {
                name: res.name,
                version: res.version.to_string(),
                hash: res.hash.to_string(),
                download_url,
                cdn_url,
                cdn_expires_at,
                signatures,
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new(
                error_codes::NOT_FOUND,
                format!("Package '{}' not found", request.name),
            )),
        )),
    }
}

/// Yank (mark unavailable) a package
///
/// This removes the package from primary storage.
/// In production, you may want to only mark as yanked in metadata instead.
async fn yank_package(
    State(state): State<AppState>,
    Path(hash_str): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    // Parse content hash
    let hash = ContentHash::from_string(&hash_str).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                error_codes::INVALID_REQUEST,
                format!("Invalid content hash format: {e}"),
            )),
        )
    })?;

    // Check if package exists in primary storage
    let exists = state.storage.exists(&hash).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                error_codes::INTERNAL_ERROR,
                format!("Failed to check package: {}", e),
            )),
        )
    })?;

    if !exists {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new(error_codes::NOT_FOUND, "Package not found")),
        ));
    }

    // Delete from primary storage
    state.storage.delete(&hash).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                error_codes::INTERNAL_ERROR,
                format!("Failed to delete package: {}", e),
            )),
        )
    })?;

    tracing::info!(hash = %hash_str, "Package yanked from storage");

    // Mark as yanked in metadata store
    if let Err(e) = state.metadata.yank(&hash).await {
        tracing::warn!(hash = %hash_str, error = %e, "Failed to mark package as yanked in metadata store");
        // Continue anyway - the package is already deleted from storage
    }

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{CacheConfig, CacheStore, InMemoryCacheStore};
    use std::time::Duration;

    #[test]
    fn test_hash_parsing() {
        let hash_str = "sha256:b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let result = ContentHash::from_string(hash_str);
        assert!(result.is_ok());
        let hash = result.unwrap();
        // Verify the parsed hash has the expected digest
        assert_eq!(
            hash.to_hex(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[tokio::test]
    async fn test_resolution_cache_key_generation() {
        // Test that cache keys are generated correctly
        let key_with_version = cache_keys::resolution("my-package", "^1.0.0");
        assert_eq!(key_with_version, "resolve:my-package:^1.0.0");

        let key_latest = cache_keys::resolution_latest("my-package");
        assert_eq!(key_latest, "resolve:my-package:latest");
    }

    #[tokio::test]
    async fn test_resolution_cache_stores_and_retrieves() {
        use semver::Version;

        let cache = InMemoryCacheStore::new(CacheConfig::default());

        // Create a Resolution to cache
        let resolution = Resolution {
            name: "test-package".to_string(),
            version: Version::new(1, 2, 3),
            hash: ContentHash::from_bytes(b"test content"),
            published_at: Utc::now(),
            yanked: false,
        };

        let cache_key = cache_keys::resolution("test-package", "^1.0.0");

        // Store in cache
        cache_set_json(
            &cache,
            &cache_key,
            &resolution,
            Some(Duration::from_secs(60)),
        )
        .await
        .unwrap();

        // Retrieve from cache
        let cached: Option<Resolution> = cache_get_json(&cache, &cache_key).await.unwrap();
        assert!(cached.is_some());
        let cached = cached.unwrap();
        assert_eq!(cached.name, "test-package");
        assert_eq!(cached.version, Version::new(1, 2, 3));
    }

    #[tokio::test]
    async fn test_cache_invalidation_pattern() {
        let cache = InMemoryCacheStore::new(CacheConfig::default());

        // Store multiple resolutions for the same package
        cache
            .set("resolve:my-pkg:1.0.0", b"v1", None)
            .await
            .unwrap();
        cache
            .set("resolve:my-pkg:2.0.0", b"v2", None)
            .await
            .unwrap();
        cache
            .set("resolve:my-pkg:latest", b"v2", None)
            .await
            .unwrap();
        cache
            .set("resolve:other-pkg:1.0.0", b"other", None)
            .await
            .unwrap();

        // Invalidate all resolutions for my-pkg
        let pattern = cache_keys::resolution_pattern("my-pkg");
        let count = cache.delete_pattern(&pattern).await.unwrap();
        assert_eq!(count, 3);

        // other-pkg should still exist
        assert!(cache.exists("resolve:other-pkg:1.0.0").await.unwrap());

        // my-pkg should be gone
        assert!(!cache.exists("resolve:my-pkg:1.0.0").await.unwrap());
        assert!(!cache.exists("resolve:my-pkg:2.0.0").await.unwrap());
        assert!(!cache.exists("resolve:my-pkg:latest").await.unwrap());
    }

    #[tokio::test]
    async fn test_search_cache_invalidation() {
        let cache = InMemoryCacheStore::new(CacheConfig::default());

        // Store some search results
        cache.set("search:abc123", b"results1", None).await.unwrap();
        cache.set("search:def456", b"results2", None).await.unwrap();
        cache
            .set("resolve:pkg:1.0", b"resolution", None)
            .await
            .unwrap();

        // Invalidate all search results
        let pattern = cache_keys::search_pattern();
        let count = cache.delete_pattern(&pattern).await.unwrap();
        assert_eq!(count, 2);

        // Search results should be gone
        assert!(!cache.exists("search:abc123").await.unwrap());
        assert!(!cache.exists("search:def456").await.unwrap());

        // Resolution should still exist
        assert!(cache.exists("resolve:pkg:1.0").await.unwrap());
    }
}
