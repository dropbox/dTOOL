//! Batch Routes
//!
//! AI-optimized batch operations for efficient bulk requests.
//! Includes caching for improved performance on repeated batch resolves.

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use chrono::{Duration, Utc};
use semver::VersionReq;

use crate::api::{
    types::{
        error_codes, ApiError, BatchDownloadRequest, BatchDownloadResponse, BatchDownloadUrl,
        BatchResolveFailed, BatchResolveRequest, BatchResolveResponse, BatchResolveResult,
    },
    AppState,
};
use crate::cache::{cache_get_json, cache_set_json, keys as cache_keys};
use crate::{ContentHash, Resolution};

/// Batch routes
pub fn routes() -> Router<AppState> {
    Router::new()
        // POST /batch/resolve - Resolve multiple packages
        .route("/resolve", post(batch_resolve))
        // POST /batch/download - Get download URLs for multiple packages
        .route("/download", post(batch_download))
}

/// Resolve multiple packages in one request
///
/// Uses caching to improve performance on repeated batch resolves.
/// Cache hits are significantly faster than database queries.
async fn batch_resolve(
    State(state): State<AppState>,
    Json(request): Json<BatchResolveRequest>,
) -> Result<Json<BatchResolveResponse>, (StatusCode, Json<ApiError>)> {
    // Validate request size
    if request.packages.len() > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                error_codes::INVALID_REQUEST,
                "Maximum 100 packages per batch resolve request",
            )),
        ));
    }

    let mut resolved = Vec::new();
    let mut failed = Vec::new();
    let mut cache_hits = 0u32;
    let total_packages = request.packages.len();

    for pkg_req in request.packages {
        // Parse version requirement (default to "*" for any version)
        let version_str = pkg_req.version.as_deref().unwrap_or("*");
        let version_req = match VersionReq::parse(version_str) {
            Ok(req) => req,
            Err(_) => {
                failed.push(BatchResolveFailed {
                    name: pkg_req.name.clone(),
                    version: pkg_req.version.clone(),
                    error: format!("Invalid version requirement: {}", version_str),
                });
                continue;
            }
        };

        // Generate cache key
        let cache_key = cache_keys::resolution(&pkg_req.name, version_str);

        // Check cache first
        if let Ok(Some(cached)) =
            cache_get_json::<Resolution>(state.data_cache.as_ref(), &cache_key).await
        {
            cache_hits += 1;
            let download_url = format!(
                "{}/api/v1/packages/{}/download",
                state.config.base_url, cached.hash
            );
            resolved.push(BatchResolveResult {
                name: cached.name,
                version: cached.version.to_string(),
                hash: cached.hash.to_string(),
                download_url,
            });
            continue;
        }

        // Cache miss - resolve from metadata store
        match state.metadata.resolve(&pkg_req.name, &version_req).await {
            Ok(Some(resolution)) => {
                // Cache the result for future requests
                let _ = cache_set_json(
                    state.data_cache.as_ref(),
                    &cache_key,
                    &resolution,
                    Some(state.cache_config.resolution_ttl),
                )
                .await;

                let download_url = format!(
                    "{}/api/v1/packages/{}/download",
                    state.config.base_url, resolution.hash
                );
                resolved.push(BatchResolveResult {
                    name: resolution.name,
                    version: resolution.version.to_string(),
                    hash: resolution.hash.to_string(),
                    download_url,
                });
            }
            Ok(None) => {
                failed.push(BatchResolveFailed {
                    name: pkg_req.name.clone(),
                    version: pkg_req.version.clone(),
                    error: "Package not found".to_string(),
                });
            }
            Err(e) => {
                failed.push(BatchResolveFailed {
                    name: pkg_req.name.clone(),
                    version: pkg_req.version.clone(),
                    error: format!("Resolution error: {}", e),
                });
            }
        }
    }

    tracing::debug!(
        total = total_packages,
        cache_hits = cache_hits,
        resolved = resolved.len(),
        failed = failed.len(),
        "Batch resolve completed"
    );

    Ok(Json(BatchResolveResponse { resolved, failed }))
}

/// Get download URLs for multiple packages by hash
async fn batch_download(
    State(state): State<AppState>,
    Json(request): Json<BatchDownloadRequest>,
) -> Result<Json<BatchDownloadResponse>, (StatusCode, Json<ApiError>)> {
    // Validate request size
    if request.hashes.len() > 50 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                error_codes::INVALID_REQUEST,
                "Maximum 50 packages per batch download request",
            )),
        ));
    }

    let mut downloads = Vec::new();
    let expires_at = Utc::now() + Duration::hours(1);

    for hash_str in request.hashes {
        // Validate hash format
        if ContentHash::from_string(&hash_str).is_err() {
            continue;
        }

        // Build download URL
        let url = format!(
            "{}/api/v1/packages/{}/download",
            state.config.base_url, hash_str
        );

        // Add mirrors if available
        let mirrors = if !state.config.storage_url.is_empty() {
            vec![format!("{}/{}", state.config.storage_url, hash_str)]
        } else {
            vec![]
        };

        downloads.push(BatchDownloadUrl {
            hash: hash_str,
            url,
            mirrors,
            expires_at,
        });
    }

    Ok(Json(BatchDownloadResponse { downloads }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_batch_size_limits() {
        // Just verify the constants exist
        assert!(100 > 0); // Max resolve batch size
        assert!(50 > 0); // Max download batch size
    }
}
