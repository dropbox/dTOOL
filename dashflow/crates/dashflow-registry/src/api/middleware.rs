//! API Middleware
//!
//! Middleware layers for authentication, rate limiting, request tracing, and more.

use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use crate::api::{state::RateLimitResult, types::ApiError, types::error_codes, AppState};
use crate::{
    cache_get_json, cache_keys, cache_set_json, hash_api_key, ApiKeyTrustLevel, ApiKeyVerification,
};

// ============================================================================
// Request ID Middleware
// ============================================================================

/// Extract or generate request ID for tracing
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    // Get existing request ID or generate new one
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Store in request extensions
    request
        .extensions_mut()
        .insert(RequestId(request_id.clone()));

    // Continue with request
    let mut response = next.run(request).await;

    // Add request ID to response headers
    if let Ok(header_value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", header_value);
    }

    response
}

/// Request ID extension type
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

// ============================================================================
// Rate Limiting Middleware
// ============================================================================

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    // Get client identifier (API key or IP)
    let client_id = get_client_id(&headers);

    // Check rate limit
    match state.rate_limiter.check_and_increment(&client_id).await {
        RateLimitResult::Allowed { remaining } => {
            let mut response = next.run(request).await;

            // Add rate limit headers
            if let Ok(header_value) = HeaderValue::from_str(&remaining.to_string()) {
                response
                    .headers_mut()
                    .insert("x-ratelimit-remaining", header_value);
            }
            if let Ok(header_value) =
                HeaderValue::from_str(&state.config.rate_limit_rpm.to_string())
            {
                response
                    .headers_mut()
                    .insert("x-ratelimit-limit", header_value);
            }

            response
        }
        RateLimitResult::Limited { retry_after_secs } => {
            let error = ApiError::new(
                "RATE_LIMITED",
                format!(
                    "Rate limit exceeded. Retry after {} seconds.",
                    retry_after_secs
                ),
            );

            let mut response = (StatusCode::TOO_MANY_REQUESTS, Json(error)).into_response();
            if let Ok(header_value) = HeaderValue::from_str(&retry_after_secs.to_string()) {
                response.headers_mut().insert("retry-after", header_value);
            }
            response
        }
    }
}

fn get_client_id(headers: &HeaderMap) -> String {
    // Check for API key first
    if let Some(api_key) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        return format!("key:{}", api_key);
    }

    // Fall back to forwarded IP or direct IP
    if let Some(forwarded) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        // Take first IP from chain
        if let Some(ip) = forwarded.split(',').next() {
            return format!("ip:{}", ip.trim());
        }
    }

    // Default to unknown (will share rate limit bucket)
    "ip:unknown".to_string()
}

// ============================================================================
// Authentication Middleware
// ============================================================================

/// Authentication context
#[derive(Clone, Debug)]
pub struct AuthContext {
    /// API key (if authenticated) - stores the key prefix for logging, not the full key
    pub api_key_prefix: Option<String>,
    /// User/agent ID from API key
    pub agent_id: Option<Uuid>,
    /// Trust level
    pub trust_level: AuthTrustLevel,
    /// Scopes/permissions from API key
    pub scopes: Vec<String>,
    /// Custom rate limit from API key (overrides default)
    pub rate_limit_rpm: Option<u32>,
}

/// Authentication trust level
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthTrustLevel {
    /// No authentication
    Anonymous,
    /// Basic API key
    Basic,
    /// Verified key with signature
    Verified,
    /// Trusted/admin key
    Trusted,
}

impl From<ApiKeyTrustLevel> for AuthTrustLevel {
    fn from(level: ApiKeyTrustLevel) -> Self {
        match level {
            ApiKeyTrustLevel::Basic => AuthTrustLevel::Basic,
            ApiKeyTrustLevel::Verified => AuthTrustLevel::Verified,
            ApiKeyTrustLevel::Trusted => AuthTrustLevel::Trusted,
        }
    }
}

/// Extract authentication context from request with database verification
pub async fn auth_context_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Response {
    let auth_context = extract_and_verify_auth(&state, &headers).await;
    request.extensions_mut().insert(auth_context);
    next.run(request).await
}

async fn extract_and_verify_auth(state: &AppState, headers: &HeaderMap) -> AuthContext {
    // Check for API key
    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Check for agent ID header (can be overridden by key's agent_id)
    let header_agent_id = headers
        .get("x-agent-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok());

    if let Some(raw_key) = api_key {
        // Hash the key
        let key_hash = hash_api_key(&raw_key);

        // Try to get verification from cache first
        let cache_key = cache_keys::api_key(&key_hash);
        let verification =
            match cache_get_json::<ApiKeyVerification>(state.data_cache.as_ref(), &cache_key).await
            {
                Ok(Some(cached)) => {
                    // Cache hit
                    Some(cached)
                }
                _ => {
                    // Cache miss - look it up in the database
                    match state.api_keys.verify_api_key(&key_hash).await {
                        Ok(verification) => {
                            // Cache the verification result
                            let _ = cache_set_json(
                                state.data_cache.as_ref(),
                                &cache_key,
                                &verification,
                                Some(state.cache_config.api_key_ttl),
                            )
                            .await;
                            Some(verification)
                        }
                        Err(_) => None,
                    }
                }
            };

        if let Some(verification) = verification {
            if verification.valid {
                // Key is valid - update last_used_at in background (fire and forget)
                let api_keys = std::sync::Arc::clone(&state.api_keys);
                let key_hash_clone = key_hash.clone();
                tokio::spawn(async move {
                    let _ = api_keys.touch_api_key(&key_hash_clone).await;
                });

                return AuthContext {
                    api_key_prefix: Some(verification.key.key_prefix),
                    agent_id: verification.key.agent_id.or(header_agent_id),
                    trust_level: verification.key.trust_level.into(),
                    scopes: verification.key.scopes,
                    rate_limit_rpm: verification.key.rate_limit_rpm,
                };
            } else {
                // Key exists but is invalid (revoked or expired)
                // Return anonymous - the require_auth_middleware will reject if auth is needed
                return AuthContext {
                    api_key_prefix: Some(verification.key.key_prefix),
                    agent_id: header_agent_id,
                    trust_level: AuthTrustLevel::Anonymous,
                    scopes: Vec::new(),
                    rate_limit_rpm: None,
                };
            }
        }

        // Key not found - return anonymous
        return AuthContext {
            api_key_prefix: None,
            agent_id: header_agent_id,
            trust_level: AuthTrustLevel::Anonymous,
            scopes: Vec::new(),
            rate_limit_rpm: None,
        };
    }

    // No API key provided
    AuthContext {
        api_key_prefix: None,
        agent_id: header_agent_id,
        trust_level: AuthTrustLevel::Anonymous,
        scopes: Vec::new(),
        rate_limit_rpm: None,
    }
}

/// Require authentication middleware
pub async fn require_auth_middleware(request: Request, next: Next) -> Response {
    let auth = request
        .extensions()
        .get::<AuthContext>()
        .cloned()
        .unwrap_or(AuthContext {
            api_key_prefix: None,
            agent_id: None,
            trust_level: AuthTrustLevel::Anonymous,
            scopes: Vec::new(),
            rate_limit_rpm: None,
        });

    if auth.trust_level == AuthTrustLevel::Anonymous {
        let error = ApiError::new(
            "UNAUTHORIZED",
            "Authentication required. Provide a valid API key.",
        );
        return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
    }

    next.run(request).await
}

// ============================================================================
// Signature Verification Middleware
// ============================================================================

/// Result of request-level signature verification.
///
/// This is added to the request extensions to indicate whether the request
/// body signature was verified against a trusted key.
#[derive(Clone, Debug)]
pub struct SignatureVerificationResult {
    /// Whether a signature was present and verified
    pub verified: bool,
    /// Key ID that signed the request (if verified)
    pub key_id: Option<String>,
    /// Key owner name (if verified)
    pub key_owner: Option<String>,
    /// Trust level of the signing key (if verified)
    pub trust_level: Option<crate::TrustLevel>,
    /// Error message if verification failed
    pub error: Option<String>,
}

impl SignatureVerificationResult {
    fn unverified() -> Self {
        Self {
            verified: false,
            key_id: None,
            key_owner: None,
            trust_level: None,
            error: None,
        }
    }

    fn verified(key_id: String, key_owner: String, trust_level: crate::TrustLevel) -> Self {
        Self {
            verified: true,
            key_id: Some(key_id),
            key_owner: Some(key_owner),
            trust_level: Some(trust_level),
            error: None,
        }
    }

    /// Create a failed verification result with an error message.
    ///
    /// This can be used by handlers that want to record why verification failed
    /// without immediately rejecting the request.
    #[allow(dead_code)] // Architectural: Reserved for detailed verification failure tracking
    fn failed(error: impl Into<String>) -> Self {
        Self {
            verified: false,
            key_id: None,
            key_owner: None,
            trust_level: None,
            error: Some(error.into()),
        }
    }
}

/// Verify request signature header (M-553).
///
/// This middleware verifies Ed25519 signatures on request bodies when the
/// `x-signature` header is present. The header format is: `<key_id>:<hex_signature>`
///
/// For requests with valid signatures:
/// - The request continues with `SignatureVerificationResult::verified` in extensions
/// - Handler code can check this to enforce signature requirements
///
/// For requests with invalid signatures:
/// - Returns 401 Unauthorized with `SIGNATURE_INVALID` error code
///
/// For requests without signatures:
/// - GET/HEAD/OPTIONS requests continue without verification
/// - POST/PUT/DELETE/PATCH requests continue with `SignatureVerificationResult::unverified`
/// - Handlers can optionally require signatures by checking the extension
///
/// # Security
///
/// The signature is verified against the raw request body bytes using the
/// public key from the server's keyring. Only keys registered in the keyring
/// can produce valid signatures.
///
/// # Example Header
///
/// ```text
/// x-signature: a1b2c3d4:0123456789abcdef...
/// ```
///
/// Where `a1b2c3d4` is the key ID (first 8 bytes of public key, hex-encoded)
/// and the rest is the 64-byte Ed25519 signature (128 hex characters).
pub async fn verify_signature_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    use axum::body::Body;
    use http_body_util::BodyExt;

    // Check for signature header
    let signature_header = headers
        .get("x-signature")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let Some(signature_header) = signature_header else {
        // No signature header - mark as unverified and continue
        // GET/HEAD/OPTIONS don't typically need body signatures
        let method = request.method().clone();
        let mut request = request;

        if method == "GET" || method == "HEAD" || method == "OPTIONS" {
            // No signature needed for safe methods
            request
                .extensions_mut()
                .insert(SignatureVerificationResult::unverified());
        } else {
            // Mutable methods without signature - mark as unverified
            // Handlers can check this and enforce signature requirements
            request
                .extensions_mut()
                .insert(SignatureVerificationResult::unverified());
        }

        return next.run(request).await;
    };

    // Parse signature header: format is "key_id:hex_signature"
    let parts: Vec<&str> = signature_header.splitn(2, ':').collect();
    if parts.len() != 2 {
        let error = ApiError::new(
            error_codes::SIGNATURE_INVALID,
            "Invalid x-signature header format. Expected: key_id:hex_signature",
        );
        return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
    }

    let key_id = parts[0];
    let signature_hex = parts[1];

    // Validate signature hex format
    let signature_bytes = match hex::decode(signature_hex) {
        Ok(bytes) => bytes,
        Err(e) => {
            let error = ApiError::new(
                error_codes::SIGNATURE_INVALID,
                format!("Invalid signature hex encoding: {}", e),
            );
            return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
        }
    };

    // Validate signature length (Ed25519 signatures are 64 bytes)
    if signature_bytes.len() != 64 {
        let error = ApiError::new(
            error_codes::SIGNATURE_INVALID,
            format!(
                "Invalid signature length: expected 64 bytes, got {}",
                signature_bytes.len()
            ),
        );
        return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
    }

    // Look up the public key in the keyring
    let key_entry = match state.trust.get_key(key_id) {
        Some(entry) => entry,
        None => {
            let error = ApiError::new(
                error_codes::SIGNATURE_INVALID,
                format!("Unknown signing key: {}", key_id),
            );
            return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
        }
    };

    // Extract the request body for verification
    let (parts, body) = request.into_parts();
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to read request body for signature verification");
            let error = ApiError::new(
                error_codes::INTERNAL_ERROR,
                "Failed to read request body",
            );
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response();
        }
    };

    // Verify the signature
    let signature = crate::Signature {
        key_id: key_id.to_string(),
        signature: signature_bytes,
        timestamp: chrono::Utc::now(), // Timestamp from header could be added later
    };

    let verified = match state
        .trust
        .verify_data_signature(&body_bytes, &signature, &key_entry.key)
    {
        Ok(valid) => valid,
        Err(e) => {
            tracing::warn!(
                key_id = %key_id,
                error = %e,
                "Signature verification error"
            );
            false
        }
    };

    if !verified {
        let error = ApiError::new(
            error_codes::SIGNATURE_INVALID,
            "Request signature verification failed",
        );
        return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
    }

    // Signature verified - reconstruct request with body and add verification result
    let mut request = Request::from_parts(parts, Body::from(body_bytes.to_vec()));
    request.extensions_mut().insert(SignatureVerificationResult::verified(
        key_id.to_string(),
        key_entry.key.owner.clone(),
        key_entry.trust_level,
    ));

    tracing::debug!(
        key_id = %key_id,
        key_owner = %key_entry.key.owner,
        trust_level = ?key_entry.trust_level,
        "Request signature verified"
    );

    next.run(request).await
}

/// Middleware that requires requests to have a valid signature.
///
/// Use this on routes that must have verified signatures. It checks for
/// the `SignatureVerificationResult` extension added by `verify_signature_middleware`.
///
/// Returns 401 if no signature was provided or verification failed.
pub async fn require_signature_middleware(request: Request, next: Next) -> Response {
    let verification = request
        .extensions()
        .get::<SignatureVerificationResult>()
        .cloned();

    match verification {
        Some(result) if result.verified => {
            // Signature verified - continue
            next.run(request).await
        }
        Some(result) => {
            // Signature present but failed, or not present
            let message = result.error.unwrap_or_else(|| "Signature required".to_string());
            let error = ApiError::new(error_codes::SIGNATURE_INVALID, message);
            (StatusCode::UNAUTHORIZED, Json(error)).into_response()
        }
        None => {
            // No verification result - signature middleware not applied
            let error = ApiError::new(
                error_codes::SIGNATURE_INVALID,
                "Signature verification required but not configured",
            );
            (StatusCode::UNAUTHORIZED, Json(error)).into_response()
        }
    }
}

// ============================================================================
// CORS Headers
// ============================================================================

/// Add CORS headers to response
///
/// # Security (M-230)
/// CORS headers are only added if explicit origins are configured.
/// No wildcard "*" fallback - production must configure allowed origins.
pub async fn cors_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;

    // Only add CORS headers if enabled AND origins are explicitly configured
    // SECURITY (M-230): No wildcard fallback - empty origins means no CORS headers
    if state.config.cors_enabled && !state.config.cors_origins.is_empty() {
        // Get the first configured origin (production should use dynamic origin matching)
        if let Some(origin) = state.config.cors_origins.first() {
            // SECURITY: Warn if wildcard is explicitly configured (still allowed for dev)
            if origin == "*" {
                tracing::warn!(
                    "SECURITY WARNING: CORS wildcard '*' origin configured. \
                     This should not be used in production."
                );
            }

            if let Ok(header_value) = HeaderValue::from_str(origin) {
                response
                    .headers_mut()
                    .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, header_value);
            }
        }

        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"),
        );
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static(
                "Content-Type, Authorization, X-Api-Key, X-Request-Id, X-Signature",
            ),
        );
    }

    response
}

// ============================================================================
// Error Handling
// ============================================================================

/// Convert internal errors to API error responses
pub async fn error_handler_middleware(request: Request, next: Next) -> Response {
    let response = next.run(request).await;

    // If response is already an error, enhance it with request ID
    if response.status().is_server_error() {
        // Log error (in production, use proper logging)
        // For now, just return the response as-is
    }

    response
}

// ============================================================================
// Metrics Middleware
// ============================================================================

/// Track HTTP request metrics (timing, count, size)
///
/// This middleware records:
/// - Request count by method, path, status
/// - Request duration
/// - Request/response sizes
#[cfg(feature = "metrics")]
pub async fn metrics_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let start = std::time::Instant::now();
    let method = request.method().to_string();

    // Normalize path for metrics (remove dynamic segments)
    let path = normalize_path_for_metrics(request.uri().path());

    // Track request size (if known)
    let request_size = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    // Increment in-flight counter
    if let Some(ref metrics) = state.metrics {
        metrics.http_requests_in_flight.inc();
    }

    // Process request
    let response = next.run(request).await;

    // Calculate duration
    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16();

    // Get response size (if known)
    let response_size = response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    // Record metrics
    if let Some(ref metrics) = state.metrics {
        metrics.http_requests_in_flight.dec();
        metrics.record_http_request(
            &method,
            &path,
            status,
            duration,
            request_size,
            response_size,
        );
    }

    response
}

/// Normalize request path for metrics labels
///
/// Replaces dynamic path segments (UUIDs, hashes, etc.) with placeholders
/// to prevent cardinality explosion.
#[cfg(feature = "metrics")]
fn normalize_path_for_metrics(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let normalized: Vec<String> = parts
        .iter()
        .map(|part| {
            // UUID pattern
            if uuid::Uuid::parse_str(part).is_ok() {
                return ":id".to_string();
            }
            // SHA-256 hash pattern (64 hex chars)
            if part.len() == 64 && part.chars().all(|c| c.is_ascii_hexdigit()) {
                return ":hash".to_string();
            }
            // Content hash with prefix (sha256:...)
            if part.starts_with("sha256:") {
                return ":hash".to_string();
            }
            // Semantic version pattern (x.y.z)
            if part.contains('.') && part.chars().all(|c| c.is_ascii_digit() || c == '.') {
                return ":version".to_string();
            }
            part.to_string()
        })
        .collect();
    normalized.join("/")
}

/// No-op metrics middleware when feature is disabled
#[cfg(not(feature = "metrics"))]
pub async fn metrics_middleware(request: Request, next: Next) -> Response {
    next.run(request).await
}

#[cfg(test)]
#[cfg(feature = "metrics")]
mod metrics_tests {
    use super::*;

    #[test]
    fn test_normalize_path_uuid() {
        let path = "/api/v1/contributions/123e4567-e89b-12d3-a456-426614174000";
        let normalized = normalize_path_for_metrics(path);
        assert_eq!(normalized, "/api/v1/contributions/:id");
    }

    #[test]
    fn test_normalize_path_hash() {
        // 64-character hex hash
        let hash = "a".repeat(64);
        let path = format!("/api/v1/packages/{}", hash);
        let normalized = normalize_path_for_metrics(&path);
        assert_eq!(normalized, "/api/v1/packages/:hash");
    }

    #[test]
    fn test_normalize_path_sha256_prefix() {
        let path = "/api/v1/packages/sha256:abc123";
        let normalized = normalize_path_for_metrics(path);
        assert_eq!(normalized, "/api/v1/packages/:hash");
    }

    #[test]
    fn test_normalize_path_version() {
        let path = "/api/v1/packages/resolve/mypackage/1.2.3";
        let normalized = normalize_path_for_metrics(path);
        assert_eq!(normalized, "/api/v1/packages/resolve/mypackage/:version");
    }

    #[test]
    fn test_normalize_path_unchanged() {
        let path = "/api/v1/search/semantic";
        let normalized = normalize_path_for_metrics(path);
        assert_eq!(normalized, path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_client_id_api_key() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", "test-key-123".parse().unwrap());

        let id = get_client_id(&headers);
        assert_eq!(id, "key:test-key-123");
    }

    #[test]
    fn test_get_client_id_forwarded() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4, 5.6.7.8".parse().unwrap());

        let id = get_client_id(&headers);
        assert_eq!(id, "ip:1.2.3.4");
    }

    #[test]
    fn test_get_client_id_unknown() {
        let headers = HeaderMap::new();
        let id = get_client_id(&headers);
        assert_eq!(id, "ip:unknown");
    }

    #[tokio::test]
    async fn test_extract_auth_context_anonymous() {
        // Create state with in-memory store
        let state = AppState::new().await.unwrap();
        let headers = HeaderMap::new();

        let ctx = extract_and_verify_auth(&state, &headers).await;

        assert!(ctx.api_key_prefix.is_none());
        assert!(ctx.agent_id.is_none());
        assert_eq!(ctx.trust_level, AuthTrustLevel::Anonymous);
    }

    #[tokio::test]
    async fn test_extract_auth_context_with_valid_key() {
        use crate::{generate_api_key, ApiKeyTrustLevel as StoreKeyTrustLevel, StoredApiKey};
        use chrono::Utc;

        // Create state with in-memory store
        let state = AppState::new().await.unwrap();

        // Generate and store a test API key
        let (full_key, key_prefix, key_hash) = generate_api_key("dk_test");
        let stored_key = StoredApiKey {
            id: Uuid::new_v4(),
            key_hash: key_hash.clone(),
            key_prefix: key_prefix.clone(),
            agent_id: Some(Uuid::new_v4()),
            name: "Test Key".to_string(),
            trust_level: StoreKeyTrustLevel::Verified,
            scopes: vec!["read".to_string(), "write".to_string()],
            rate_limit_rpm: Some(100),
            active: true,
            expires_at: None,
            last_used_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        state.api_keys.store_api_key(&stored_key).await.unwrap();

        // Create headers with the API key
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", full_key.parse().unwrap());

        let ctx = extract_and_verify_auth(&state, &headers).await;

        assert_eq!(ctx.api_key_prefix, Some(key_prefix));
        assert_eq!(ctx.trust_level, AuthTrustLevel::Verified);
        assert_eq!(ctx.scopes, vec!["read".to_string(), "write".to_string()]);
        assert_eq!(ctx.rate_limit_rpm, Some(100));
    }

    #[tokio::test]
    async fn test_extract_auth_context_with_invalid_key() {
        // Create state with in-memory store
        let state = AppState::new().await.unwrap();

        // Create headers with an invalid API key (not in store)
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", "invalid-key-not-in-db".parse().unwrap());

        let ctx = extract_and_verify_auth(&state, &headers).await;

        // Key not found = Anonymous
        assert!(ctx.api_key_prefix.is_none());
        assert_eq!(ctx.trust_level, AuthTrustLevel::Anonymous);
    }

    #[tokio::test]
    async fn test_extract_auth_context_with_revoked_key() {
        use crate::{generate_api_key, ApiKeyTrustLevel as StoreKeyTrustLevel, StoredApiKey};
        use chrono::Utc;

        let state = AppState::new().await.unwrap();

        // Generate and store a revoked API key
        let (full_key, key_prefix, key_hash) = generate_api_key("dk_test");
        let stored_key = StoredApiKey {
            id: Uuid::new_v4(),
            key_hash: key_hash.clone(),
            key_prefix: key_prefix.clone(),
            agent_id: None,
            name: "Revoked Key".to_string(),
            trust_level: StoreKeyTrustLevel::Basic,
            scopes: vec![],
            rate_limit_rpm: None,
            active: false, // Revoked!
            expires_at: None,
            last_used_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        state.api_keys.store_api_key(&stored_key).await.unwrap();

        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", full_key.parse().unwrap());

        let ctx = extract_and_verify_auth(&state, &headers).await;

        // Revoked key = Anonymous (but we get the prefix for logging)
        assert_eq!(ctx.api_key_prefix, Some(key_prefix));
        assert_eq!(ctx.trust_level, AuthTrustLevel::Anonymous);
    }

    // ========================================================================
    // Signature Verification Middleware Tests (M-553)
    // ========================================================================

    #[test]
    fn test_signature_verification_result_states() {
        let unverified = SignatureVerificationResult::unverified();
        assert!(!unverified.verified);
        assert!(unverified.key_id.is_none());
        assert!(unverified.error.is_none());

        let verified = SignatureVerificationResult::verified(
            "abc123".to_string(),
            "Test Owner".to_string(),
            crate::TrustLevel::Community,
        );
        assert!(verified.verified);
        assert_eq!(verified.key_id, Some("abc123".to_string()));
        assert_eq!(verified.key_owner, Some("Test Owner".to_string()));
        assert_eq!(verified.trust_level, Some(crate::TrustLevel::Community));

        let failed = SignatureVerificationResult::failed("Bad signature");
        assert!(!failed.verified);
        assert_eq!(failed.error, Some("Bad signature".to_string()));
    }

    // Helper function to create a test Next handler for middleware tests.
    // This builds an axum router with the given handler and uses tower to
    // convert it into a service that can be called directly.
    async fn call_verify_middleware(
        state: AppState,
        headers: HeaderMap,
        request: Request,
    ) -> Response {
        use axum::{
            routing::post,
            Router,
        };
        use tower::ServiceExt;

        // Create a simple router with a handler that returns the verification result
        let app = Router::new()
            .route("/test", post(|req: Request| async move {
                let result = req.extensions().get::<SignatureVerificationResult>().cloned();
                match result {
                    Some(r) if r.verified => {
                        format!("VERIFIED:{}", r.key_owner.unwrap_or_default())
                    }
                    Some(_) => "UNVERIFIED".to_string(),
                    None => "NO_RESULT".to_string(),
                }
            }))
            .route("/get_test", axum::routing::get(|req: Request| async move {
                let result = req.extensions().get::<SignatureVerificationResult>().cloned();
                match result {
                    Some(r) if r.verified => "VERIFIED".to_string(),
                    Some(_) => "UNVERIFIED".to_string(),
                    None => "NO_RESULT".to_string(),
                }
            }))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                verify_signature_middleware,
            ))
            .with_state(state);

        // Build the request with headers
        let mut builder = http::Request::builder()
            .method(request.method().clone())
            .uri(request.uri().clone());

        for (key, value) in headers.iter() {
            builder = builder.header(key, value);
        }

        let (_parts, body) = request.into_parts();
        let req = builder.body(body).unwrap();

        // Call the service
        let response = app.oneshot(req).await.unwrap();
        response
    }

    #[tokio::test]
    async fn test_verify_signature_middleware_invalid_format() {
        use axum::body::Body;

        let state = AppState::new().await.unwrap();

        // Test invalid signature header format (no colon separator)
        let mut headers = HeaderMap::new();
        headers.insert("x-signature", "invalid-no-colon".parse().unwrap());

        let request = Request::builder()
            .method("POST")
            .uri("/test")
            .body(Body::from(r#"{"test": "data"}"#))
            .unwrap();

        let response = call_verify_middleware(state, headers, request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_verify_signature_middleware_invalid_hex() {
        use axum::body::Body;

        let state = AppState::new().await.unwrap();

        // Test invalid hex in signature
        let mut headers = HeaderMap::new();
        headers.insert("x-signature", "key123:not-valid-hex!@#".parse().unwrap());

        let request = Request::builder()
            .method("POST")
            .uri("/test")
            .body(Body::from(r#"{"test": "data"}"#))
            .unwrap();

        let response = call_verify_middleware(state, headers, request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_verify_signature_middleware_wrong_length() {
        use axum::body::Body;

        let state = AppState::new().await.unwrap();

        // Test signature with wrong length (not 64 bytes)
        let short_sig = "abcd".repeat(10); // 40 hex chars = 20 bytes
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-signature",
            format!("key123:{}", short_sig).parse().unwrap(),
        );

        let request = Request::builder()
            .method("POST")
            .uri("/test")
            .body(Body::from(r#"{"test": "data"}"#))
            .unwrap();

        let response = call_verify_middleware(state, headers, request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_verify_signature_middleware_unknown_key() {
        use axum::body::Body;

        let state = AppState::new().await.unwrap();

        // Test with unknown key ID (not in keyring)
        let fake_sig = "ab".repeat(64); // 128 hex chars = 64 bytes
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-signature",
            format!("unknown-key:{}", fake_sig).parse().unwrap(),
        );

        let request = Request::builder()
            .method("POST")
            .uri("/test")
            .body(Body::from(r#"{"test": "data"}"#))
            .unwrap();

        let response = call_verify_middleware(state, headers, request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_verify_signature_middleware_no_signature_get() {
        use axum::body::Body;
        use http_body_util::BodyExt;

        let state = AppState::new().await.unwrap();

        // GET request without signature should pass through
        let headers = HeaderMap::new();

        let request = Request::builder()
            .method("GET")
            .uri("/get_test")
            .body(Body::empty())
            .unwrap();

        let response = call_verify_middleware(state, headers, request).await;
        assert_eq!(response.status(), StatusCode::OK);

        // Check body shows unverified
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert_eq!(body_str, "UNVERIFIED");
    }

    #[tokio::test]
    async fn test_verify_signature_middleware_no_signature_post() {
        use axum::body::Body;
        use http_body_util::BodyExt;

        let state = AppState::new().await.unwrap();

        // POST request without signature should pass through but be marked unverified
        let headers = HeaderMap::new();

        let request = Request::builder()
            .method("POST")
            .uri("/test")
            .body(Body::from(r#"{"test": "data"}"#))
            .unwrap();

        let response = call_verify_middleware(state, headers, request).await;
        assert_eq!(response.status(), StatusCode::OK);

        // Check body shows unverified
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert_eq!(body_str, "UNVERIFIED");
    }

    #[tokio::test]
    async fn test_verify_signature_middleware_valid_signature() {
        use axum::body::Body;
        use crate::signature::KeyPair;
        use http_body_util::BodyExt;
        use std::sync::Arc;

        // Create a keypair and add to keyring
        let keypair = KeyPair::generate("test-signer".to_string());
        let mut keyring = crate::Keyring::new();
        keyring.add_key(&keypair.public_key, crate::TrustLevel::Community);

        // Create state with the keyring
        let trust = Arc::new(crate::TrustService::new(keyring));
        let mut state = AppState::new().await.unwrap();
        state.trust = trust;

        // Create body and sign it
        let body = r#"{"test": "data"}"#;
        let signature = keypair.sign(body.as_bytes());
        let sig_header = format!("{}:{}", signature.key_id, hex::encode(&signature.signature));

        let mut headers = HeaderMap::new();
        headers.insert("x-signature", sig_header.parse().unwrap());

        let request = Request::builder()
            .method("POST")
            .uri("/test")
            .body(Body::from(body))
            .unwrap();

        let response = call_verify_middleware(state, headers, request).await;
        assert_eq!(response.status(), StatusCode::OK);

        // Check body shows verified with owner name
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert_eq!(body_str, "VERIFIED:test-signer");
    }

    #[tokio::test]
    async fn test_verify_signature_middleware_wrong_signature() {
        use axum::body::Body;
        use crate::signature::KeyPair;
        use std::sync::Arc;

        // Create a keypair and add to keyring
        let keypair = KeyPair::generate("test-signer".to_string());
        let mut keyring = crate::Keyring::new();
        keyring.add_key(&keypair.public_key, crate::TrustLevel::Community);

        // Create state with the keyring
        let trust = Arc::new(crate::TrustService::new(keyring));
        let mut state = AppState::new().await.unwrap();
        state.trust = trust;

        // Sign different data than what we send
        let signature = keypair.sign(b"different data");
        let sig_header = format!("{}:{}", signature.key_id, hex::encode(&signature.signature));

        let mut headers = HeaderMap::new();
        headers.insert("x-signature", sig_header.parse().unwrap());

        let request = Request::builder()
            .method("POST")
            .uri("/test")
            .body(Body::from(r#"{"test": "actual data"}"#))
            .unwrap();

        let response = call_verify_middleware(state, headers, request).await;

        // Should reject because signature doesn't match body
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_require_signature_middleware_verified() {
        use axum::body::Body;
        use axum::{routing::post, Router};
        use tower::ServiceExt;

        // Create a keypair and sign the request
        let keypair = crate::signature::KeyPair::generate("test-signer".to_string());
        let mut keyring = crate::Keyring::new();
        keyring.add_key(&keypair.public_key, crate::TrustLevel::Community);

        let trust = std::sync::Arc::new(crate::TrustService::new(keyring));
        let mut state = AppState::new().await.unwrap();
        state.trust = trust;

        // Create a router with require_signature_middleware
        let app: Router = Router::new()
            .route("/test", post(|| async { "OK" }))
            .route_layer(axum::middleware::from_fn(require_signature_middleware))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                verify_signature_middleware,
            ))
            .with_state(state);

        let body = r#"{"test": "data"}"#;
        let signature = keypair.sign(body.as_bytes());
        let sig_header = format!("{}:{}", signature.key_id, hex::encode(&signature.signature));

        let request = http::Request::builder()
            .method("POST")
            .uri("/test")
            .header("x-signature", sig_header)
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_require_signature_middleware_unverified() {
        use axum::body::Body;
        use axum::{routing::post, Router};
        use tower::ServiceExt;

        // Create a router with require_signature_middleware
        let state = AppState::new().await.unwrap();

        let app: Router = Router::new()
            .route("/test", post(|| async { "OK" }))
            .route_layer(axum::middleware::from_fn(require_signature_middleware))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                verify_signature_middleware,
            ))
            .with_state(state);

        // Request without signature should be rejected
        let request = http::Request::builder()
            .method("POST")
            .uri("/test")
            .body(Body::from(r#"{"test": "data"}"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

// ============================================================================
// OpenTelemetry Tracing Middleware
// ============================================================================

/// OpenTelemetry distributed tracing middleware
///
/// This middleware:
/// 1. Extracts W3C Trace Context from incoming `traceparent` headers
/// 2. Creates a span for each HTTP request with standard attributes
/// 3. Injects trace context into response headers
///
/// # Example
///
/// When a request comes in with a `traceparent` header:
/// ```text
/// traceparent: 00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01
/// ```
///
/// The middleware will:
/// - Extract the trace ID and parent span ID
/// - Create a child span for this request
/// - Add attributes like method, path, status code
/// - Inject `traceresponse` header with the current span's context
#[cfg(feature = "opentelemetry")]
pub async fn tracing_middleware(request: Request, next: Next) -> Response {
    use opentelemetry::trace::SpanKind;
    use tracing::Instrument;

    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let version = format!("{:?}", request.version());

    // Extract trace context from headers
    let parent_ctx = extract_trace_context(request.headers());

    // Create span with OpenTelemetry attributes
    let span = tracing::info_span!(
        "http.request",
        http.method = %method,
        http.route = %path,
        http.flavor = %version,
        otel.kind = ?SpanKind::Server,
        http.status_code = tracing::field::Empty,
    );

    // Process request within the span
    let response = async { next.run(request).await }
        .instrument(span.clone())
        .await;

    // Record status code on span
    span.record("http.status_code", response.status().as_u16());

    // Inject trace context into response headers
    let mut response = response;
    inject_trace_context(&parent_ctx, response.headers_mut());

    response
}

/// Extract W3C Trace Context from request headers
#[cfg(feature = "opentelemetry")]
fn extract_trace_context(headers: &HeaderMap) -> opentelemetry::Context {
    use opentelemetry::propagation::TextMapPropagator;
    use opentelemetry_sdk::propagation::TraceContextPropagator;
    use std::collections::HashMap;

    // Convert headers to HashMap for extraction
    let mut carrier: HashMap<String, String> = HashMap::new();
    for (key, value) in headers.iter() {
        if let Ok(v) = value.to_str() {
            carrier.insert(key.as_str().to_lowercase(), v.to_string());
        }
    }

    // Extract using W3C Trace Context propagator
    let propagator = TraceContextPropagator::new();
    propagator.extract(&carrier)
}

/// Inject trace context into response headers
#[cfg(feature = "opentelemetry")]
fn inject_trace_context(ctx: &opentelemetry::Context, headers: &mut HeaderMap) {
    use opentelemetry::propagation::TextMapPropagator;
    use opentelemetry::trace::TraceContextExt;
    use opentelemetry_sdk::propagation::TraceContextPropagator;
    use std::collections::HashMap;

    // Only inject if there's an active span context
    if ctx.span().span_context().is_valid() {
        let propagator = TraceContextPropagator::new();
        let mut carrier: HashMap<String, String> = HashMap::new();
        propagator.inject_context(ctx, &mut carrier);

        // Add trace response header
        if let Some(traceparent) = carrier.get("traceparent") {
            if let Ok(value) = traceparent.parse() {
                headers.insert("traceresponse", value);
            }
        }
    }
}

/// No-op tracing middleware when OpenTelemetry feature is disabled
#[cfg(not(feature = "opentelemetry"))]
pub async fn tracing_middleware(request: Request, next: Next) -> Response {
    next.run(request).await
}
