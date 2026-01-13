//! Trust Routes
//!
//! Handlers for trust operations: signature verification, key management, lineage.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;

use crate::api::{
    types::{error_codes, ApiError, VerifyRequest},
    AppState,
};
use crate::{ContentHash, TrustLevel};

/// Trust routes
pub fn routes() -> Router<AppState> {
    Router::new()
        // POST /trust/verify - Verify a signature
        .route("/verify", post(verify_signature))
        // GET /trust/keys - List trusted keys
        .route("/keys", get(list_keys))
        // GET /trust/keys/:id - Get specific key
        .route("/keys/:id", get(get_key))
        // GET /trust/lineage/:hash - Get derivation chain for a package
        .route("/lineage/:hash", get(get_lineage))
}

/// Key information response
#[derive(Debug, Clone, Serialize)]
pub struct KeyInfo {
    pub key_id: String,
    pub owner: String,
    pub trust_level: TrustLevel,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Keys list response
#[derive(Debug, Clone, Serialize)]
pub struct KeysResponse {
    pub keys: Vec<KeyInfo>,
    pub total: usize,
    /// Maximum number of keys returned per request
    pub limit: u32,
    /// Number of keys skipped
    pub offset: u32,
}

/// Query parameters for list_keys
#[derive(Debug, serde::Deserialize)]
pub struct ListKeysParams {
    /// Maximum number of keys to return (default: 100, max: 1000)
    #[serde(default = "default_keys_limit")]
    pub limit: u32,
    /// Number of keys to skip for pagination (default: 0)
    #[serde(default)]
    pub offset: u32,
}

fn default_keys_limit() -> u32 {
    100
}

const MAX_KEYS_LIMIT: u32 = 1000;

/// Verification response
#[derive(Debug, Clone, Serialize)]
pub struct VerifyResponse {
    pub valid: bool,
    pub key_id: Option<String>,
    pub key_owner: Option<String>,
    pub trust_level: Option<TrustLevel>,
    #[serde(default)]
    pub errors: Vec<String>,
}

/// Lineage info response
#[derive(Debug, Clone, Serialize)]
pub struct LineageInfo {
    pub original_hash: Option<String>,
    pub steps: Vec<LineageStepInfo>,
}

/// Lineage step info
#[derive(Debug, Clone, Serialize)]
pub struct LineageStepInfo {
    pub from_hash: String,
    pub to_hash: String,
    pub transformation: String,
    pub performed_by: String,
    pub signature: Option<String>,
}

/// Verify a signature
async fn verify_signature(
    State(state): State<AppState>,
    Json(request): Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, (StatusCode, Json<ApiError>)> {
    // Decode content (could be base64 or raw hash)
    use base64::Engine;
    let content_bytes = if request.content.starts_with("sha256:") {
        // It's a hash reference
        hex::decode(request.content.trim_start_matches("sha256:")).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    error_codes::INVALID_REQUEST,
                    format!("Invalid hash format: {e}"),
                )),
            )
        })?
    } else {
        // Try base64 first, then raw
        base64::engine::general_purpose::STANDARD
            .decode(&request.content)
            .unwrap_or_else(|_| request.content.as_bytes().to_vec())
    };

    // If public key provided, verify directly
    if let Some(ref public_key) = request.public_key {
        let valid = state
            .trust
            .verify_data_signature(&content_bytes, &request.signature, public_key)
            .unwrap_or(false);

        return Ok(Json(VerifyResponse {
            valid,
            key_id: Some(public_key.key_id.clone()),
            key_owner: Some(public_key.owner.clone()),
            trust_level: None,
            errors: if valid {
                vec![]
            } else {
                vec!["Signature verification failed".to_string()]
            },
        }));
    }

    // Otherwise, look up the key in keyring
    Ok(Json(VerifyResponse {
        valid: false,
        key_id: None,
        key_owner: None,
        trust_level: None,
        errors: vec![
            "No public key provided (key lookup deferred - requires key registry)".to_string(),
        ],
    }))
}

/// List trusted keys with pagination
///
/// Query parameters:
/// - `limit`: Maximum number of keys to return (default: 100, max: 1000)
/// - `offset`: Number of keys to skip for pagination (default: 0)
async fn list_keys(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<ListKeysParams>,
) -> Result<Json<KeysResponse>, (StatusCode, Json<ApiError>)> {
    // Clamp limit to prevent unbounded memory growth
    let limit = params.limit.min(MAX_KEYS_LIMIT);
    let offset = params.offset as usize;

    let all_keys = state.trust.list_keys();
    let total = all_keys.len();

    // Apply pagination
    let key_infos: Vec<KeyInfo> = all_keys
        .into_iter()
        .skip(offset)
        .take(limit as usize)
        .map(|entry| KeyInfo {
            key_id: entry.key.key_id.clone(),
            owner: entry.key.owner.clone(),
            trust_level: entry.trust_level,
            created_at: entry.added_at,
            expires_at: entry.expires_at,
        })
        .collect();

    Ok(Json(KeysResponse {
        total,
        keys: key_infos,
        limit,
        offset: params.offset,
    }))
}

/// Get a specific key
async fn get_key(
    State(state): State<AppState>,
    Path(key_id): Path<String>,
) -> Result<Json<KeyInfo>, (StatusCode, Json<ApiError>)> {
    let entry = state.trust.get_key(&key_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ApiError::new(
                error_codes::NOT_FOUND,
                format!("Key {} not found", key_id),
            )),
        )
    })?;

    Ok(Json(KeyInfo {
        key_id: entry.key.key_id.clone(),
        owner: entry.key.owner.clone(),
        trust_level: entry.trust_level,
        created_at: entry.added_at,
        expires_at: entry.expires_at,
    }))
}

/// Get package lineage (derivation chain)
///
/// Returns the derivation history of a package, including all transformation
/// steps from the original source to the current version.
async fn get_lineage(
    State(state): State<AppState>,
    Path(hash_str): Path<String>,
) -> Result<Json<LineageInfo>, (StatusCode, Json<ApiError>)> {
    // Parse content hash to validate format
    let hash = ContentHash::from_string(&hash_str).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                error_codes::INVALID_REQUEST,
                format!("Invalid content hash format: {e}"),
            )),
        )
    })?;

    // Look up package info from metadata store
    let package_info = state.metadata.get_by_hash(&hash).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                error_codes::INTERNAL_ERROR,
                format!("Failed to fetch package: {}", e),
            )),
        )
    })?;

    let package_info = match package_info {
        Some(info) => info,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiError::new(error_codes::NOT_FOUND, "Package not found")),
            ));
        }
    };

    // Convert Lineage to LineageInfo if present
    let lineage_info = match package_info.lineage {
        Some(lineage) => LineageInfo {
            original_hash: lineage.derived_from.map(|h| h.to_string()),
            steps: lineage
                .chain
                .into_iter()
                .map(|step| LineageStepInfo {
                    from_hash: step.source_hash.to_string(),
                    to_hash: step.result_hash.to_string(),
                    transformation: format!("{:?}", step.derivation_type),
                    performed_by: step.actor,
                    signature: Some(step.signature),
                })
                .collect(),
        },
        None => LineageInfo {
            original_hash: None,
            steps: vec![],
        },
    };

    Ok(Json(lineage_info))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_hash_decoding() {
        let hash_str = "sha256:abcdef1234567890";
        let hex_part = hash_str.trim_start_matches("sha256:");
        assert_eq!(hex_part, "abcdef1234567890");
    }
}
