//! E2E Integration Tests
//!
//! Tests the full flow of:
//! - API key creation and verification
//! - Contribution submission with authenticated keys
//! - Rate limiting with key-based limits
//!
//! These tests use in-memory stores for CI but can be extended for PostgreSQL integration.

#![cfg(feature = "server")]

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use chrono::Utc;
use tower::util::ServiceExt;
use uuid::Uuid;

use dashflow_registry::{
    generate_api_key, ApiConfig, ApiKeyTrustLevel, ApiServer, AppState, KeyPair, StoredApiKey,
};
// State module for custom configuration in rate limit tests
use dashflow_registry::api::state::ServerConfig;

// ============================================================================
// Test Helpers
// ============================================================================

/// Helper to create a test server with a fresh AppState
async fn test_state() -> AppState {
    AppState::new().await.unwrap()
}

/// Helper to create a test router
async fn test_router() -> axum::Router {
    let config = ApiConfig::default();
    let server = ApiServer::new(config).await.unwrap();
    server.router()
}

/// Helper to create and store a test API key, returning the full key and stored key info
async fn create_test_api_key(
    state: &AppState,
    name: &str,
    trust_level: ApiKeyTrustLevel,
    scopes: Vec<String>,
) -> (String, StoredApiKey) {
    let (full_key, key_prefix, key_hash) = generate_api_key("dk_test");
    let stored_key = StoredApiKey {
        id: Uuid::new_v4(),
        key_hash,
        key_prefix,
        agent_id: Some(Uuid::new_v4()),
        name: name.to_string(),
        trust_level,
        scopes,
        rate_limit_rpm: None,
        active: true,
        expires_at: None,
        last_used_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    state.api_keys.store_api_key(&stored_key).await.unwrap();
    (full_key, stored_key)
}

/// Helper to create a test contributor request JSON with valid signature
fn create_contributor_json(
    keypair: &KeyPair,
    name: &str,
    is_ai: bool,
) -> (serde_json::Value, String) {
    let public_key = &keypair.public_key;
    let public_key_json = serde_json::to_value(public_key).unwrap();

    let contributor = serde_json::json!({
        "app_id": Uuid::new_v4().to_string(),
        "name": name,
        "public_key": public_key_json,
        "is_ai": is_ai
    });

    (contributor, keypair.public_key.key_id.clone())
}

/// Helper to create a valid bug report request body
fn create_bug_report_body(keypair: &KeyPair) -> String {
    let (contributor, _) = create_contributor_json(keypair, "TestAgent", true);

    // Sign the title for the signature
    let title = "Test Bug: Emoji handling issue";
    let signature = keypair.sign(title.as_bytes());
    let signature_json = serde_json::to_value(&signature).unwrap();

    let body = serde_json::json!({
        "package": "sha256:0000000000000000000000000000000000000000000000000000000000001234",
        "title": title,
        "description": "The package fails when processing emoji characters",
        "category": "runtime_error",
        "severity": "medium",
        "error_messages": ["ParseError: unexpected character at position 47"],
        "reproduction_steps": [{
            "action": "invoke",
            "params": { "input": "Hello 😊" }
        }],
        "occurrence_rate": 0.03,
        "sample_count": 10000,
        "reporter": contributor,
        "signature": signature_json
    });

    serde_json::to_string(&body).unwrap()
}

/// Helper to create a valid improvement request body
fn create_improvement_body(keypair: &KeyPair) -> String {
    let (contributor, _) = create_contributor_json(keypair, "ImprovementBot", true);

    let title = "Add caching support";
    let signature = keypair.sign(title.as_bytes());
    let signature_json = serde_json::to_value(&signature).unwrap();

    let body = serde_json::json!({
        "package": "sha256:0000000000000000000000000000000000000000000000000000000000001234",
        "title": title,
        "description": "Adding a caching layer would improve performance",
        "category": "performance",
        "impact_level": "moderate",
        "effort_estimate": "medium",
        "rationale": "Cache frequently accessed data to reduce latency",
        "proposed_changes": ["Add cache trait", "Implement in-memory cache"],
        "alternatives": [],
        "reporter": contributor,
        "signature": signature_json
    });

    serde_json::to_string(&body).unwrap()
}

/// Helper to create a valid package request body
fn create_package_request_body(keypair: &KeyPair) -> String {
    let (contributor, _) = create_contributor_json(keypair, "RequestBot", true);

    let title = "PDF Processing Package";
    let signature = keypair.sign(title.as_bytes());
    let signature_json = serde_json::to_value(&signature).unwrap();

    let body = serde_json::json!({
        "title": title,
        "description": "A package for processing PDF documents",
        "priority": "medium",
        "use_cases": ["Extract text from PDFs", "Parse PDF forms"],
        "required_capabilities": [],
        "similar_packages": [],
        "suggested_name": "pdf-processor",
        "reporter": contributor,
        "signature": signature_json
    });

    serde_json::to_string(&body).unwrap()
}

/// Helper to create a valid fix request body
fn create_fix_body(keypair: &KeyPair) -> String {
    let (contributor, _) = create_contributor_json(keypair, "FixBot", true);

    let diff = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +1,2 @@\n-old code\n+new fixed code";
    let signature = keypair.sign(diff.as_bytes());
    let signature_json = serde_json::to_value(&signature).unwrap();

    let body = serde_json::json!({
        "package": "sha256:0000000000000000000000000000000000000000000000000000000000001234",
        "title": "Fix emoji parsing",
        "description": "This fix handles emoji characters properly",
        "fix_type": "bug_fix",
        "fixes_issues": [],
        "diff": diff,
        "test_cases": ["test_emoji_parsing"],
        "reporter": contributor,
        "signature": signature_json
    });

    serde_json::to_string(&body).unwrap()
}

// ============================================================================
// API Key Verification Flow Tests
// ============================================================================

#[tokio::test]
async fn test_api_key_generation_and_verification() {
    let state = test_state().await;

    // Generate API key
    let (full_key, key_prefix, key_hash) = generate_api_key("dk_test");

    // Key should start with prefix
    assert!(full_key.starts_with("dk_test_"));

    // Key prefix should include full prefix plus some characters
    assert!(key_prefix.len() >= 12); // dk_test_ + additional chars

    // Hash should be different from key
    assert_ne!(full_key, key_hash);

    // Store the key
    let stored_key = StoredApiKey {
        id: Uuid::new_v4(),
        key_hash: key_hash.clone(),
        key_prefix: key_prefix.clone(),
        agent_id: Some(Uuid::new_v4()),
        name: "Test Key".to_string(),
        trust_level: ApiKeyTrustLevel::Verified,
        scopes: vec!["read".to_string(), "write".to_string()],
        rate_limit_rpm: Some(100),
        active: true,
        expires_at: None,
        last_used_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    state.api_keys.store_api_key(&stored_key).await.unwrap();

    // Verify the key
    let verification = state.api_keys.verify_api_key(&key_hash).await.unwrap();
    assert!(verification.valid);
    assert_eq!(verification.key.trust_level, ApiKeyTrustLevel::Verified);
    assert_eq!(verification.key.scopes, vec!["read", "write"]);
}

#[tokio::test]
async fn test_api_key_revocation() {
    let state = test_state().await;

    // Create and store active key
    let (_full_key, stored_key) = create_test_api_key(
        &state,
        "Revokable Key",
        ApiKeyTrustLevel::Basic,
        vec!["read".to_string()],
    )
    .await;

    // Verify it works initially
    let verification = state
        .api_keys
        .verify_api_key(&stored_key.key_hash)
        .await
        .unwrap();
    assert!(verification.valid);

    // Revoke the key by its ID
    state.api_keys.revoke_api_key(stored_key.id).await.unwrap();

    // Verify it's now invalid
    let verification = state
        .api_keys
        .verify_api_key(&stored_key.key_hash)
        .await
        .unwrap();
    assert!(!verification.valid);
}

#[tokio::test]
async fn test_api_key_list_by_agent() {
    let state = test_state().await;

    let agent_id = Uuid::new_v4();

    // Create multiple keys for the same agent
    for i in 0..3 {
        let (_full_key, key_prefix, key_hash) = generate_api_key("dk_test");
        let stored_key = StoredApiKey {
            id: Uuid::new_v4(),
            key_hash,
            key_prefix,
            agent_id: Some(agent_id),
            name: format!("Key {}", i),
            trust_level: ApiKeyTrustLevel::Basic,
            scopes: vec![],
            rate_limit_rpm: None,
            active: true,
            expires_at: None,
            last_used_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        state.api_keys.store_api_key(&stored_key).await.unwrap();
    }

    // List keys for this agent
    let keys = state
        .api_keys
        .list_api_keys(Some(agent_id), 100, 0)
        .await
        .unwrap();
    assert_eq!(keys.len(), 3);
}

#[tokio::test]
async fn test_api_key_last_used_update() {
    let state = test_state().await;

    let (_full_key, stored_key) =
        create_test_api_key(&state, "Touchable Key", ApiKeyTrustLevel::Basic, vec![]).await;

    // Initially last_used_at is None
    let key = state
        .api_keys
        .get_api_key_by_hash(&stored_key.key_hash)
        .await
        .unwrap()
        .unwrap();
    assert!(key.last_used_at.is_none());

    // Touch the key
    state
        .api_keys
        .touch_api_key(&stored_key.key_hash)
        .await
        .unwrap();

    // Now last_used_at should be set
    let key = state
        .api_keys
        .get_api_key_by_hash(&stored_key.key_hash)
        .await
        .unwrap()
        .unwrap();
    assert!(key.last_used_at.is_some());
}

// ============================================================================
// Authenticated API Route Tests
// ============================================================================

#[tokio::test]
async fn test_authenticated_request_with_valid_key() {
    // Create state and store an API key
    let state = AppState::new().await.unwrap();
    let (full_key, _stored_key) = create_test_api_key(
        &state,
        "Valid Key",
        ApiKeyTrustLevel::Verified,
        vec!["read".to_string()],
    )
    .await;

    // Create server with this state
    let config = ApiConfig::default();
    let server = ApiServer::with_state(state, config);
    let router = server.router();

    // Make authenticated request
    let request = Request::builder()
        .uri("/health")
        .header("x-api-key", full_key)
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_request_with_invalid_key() {
    let router = test_router().await;

    // Make request with invalid API key
    let request = Request::builder()
        .uri("/health")
        .header("x-api-key", "invalid-key-not-in-db")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    // Health endpoint doesn't require auth, so it should still work
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_request_with_revoked_key() {
    let state = AppState::new().await.unwrap();

    // Create and immediately revoke a key
    let (full_key, stored_key) =
        create_test_api_key(&state, "Revoked Key", ApiKeyTrustLevel::Basic, vec![]).await;
    state.api_keys.revoke_api_key(stored_key.id).await.unwrap();

    let config = ApiConfig::default();
    let server = ApiServer::with_state(state, config);
    let router = server.router();

    // Request with revoked key should be treated as anonymous
    let request = Request::builder()
        .uri("/health")
        .header("x-api-key", full_key)
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    // Health doesn't require auth, but auth context will show Anonymous
    assert_eq!(response.status(), StatusCode::OK);
}

// ============================================================================
// Contribution Submission Tests
// ============================================================================

#[tokio::test]
async fn test_submit_bug_report_with_auth() {
    let state = AppState::new().await.unwrap();
    let (api_key, _) = create_test_api_key(
        &state,
        "Bug Reporter",
        ApiKeyTrustLevel::Verified,
        vec!["write".to_string()],
    )
    .await;

    let config = ApiConfig::default();
    let server = ApiServer::with_state(state, config);
    let router = server.router();

    // Create signed bug report
    let keypair = KeyPair::generate("TestAgent".to_string());
    let body = create_bug_report_body(&keypair);

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/contributions/bug")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-api-key", api_key)
        .body(Body::from(body))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["contribution_id"].is_string());
    assert_eq!(json["status"], "submitted");
    assert!(json["validation"]["schema_valid"].as_bool().unwrap());
}

#[tokio::test]
async fn test_submit_improvement_with_auth() {
    let state = AppState::new().await.unwrap();
    let (api_key, _) = create_test_api_key(
        &state,
        "Improver",
        ApiKeyTrustLevel::Basic,
        vec!["write".to_string()],
    )
    .await;

    let config = ApiConfig::default();
    let server = ApiServer::with_state(state, config);
    let router = server.router();

    let keypair = KeyPair::generate("ImprovementBot".to_string());
    let body = create_improvement_body(&keypair);

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/contributions/improvement")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-api-key", api_key)
        .body(Body::from(body))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["contribution_id"].is_string());
    assert_eq!(json["status"], "submitted");
}

#[tokio::test]
async fn test_submit_package_request_with_auth() {
    let state = AppState::new().await.unwrap();
    let (api_key, _) = create_test_api_key(
        &state,
        "Requester",
        ApiKeyTrustLevel::Basic,
        vec!["write".to_string()],
    )
    .await;

    let config = ApiConfig::default();
    let server = ApiServer::with_state(state, config);
    let router = server.router();

    let keypair = KeyPair::generate("RequestBot".to_string());
    let body = create_package_request_body(&keypair);

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/contributions/request")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-api-key", api_key)
        .body(Body::from(body))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["contribution_id"].is_string());
}

#[tokio::test]
async fn test_submit_fix_with_auth() {
    let state = AppState::new().await.unwrap();
    let (api_key, _) = create_test_api_key(
        &state,
        "Fixer",
        ApiKeyTrustLevel::Trusted,
        vec!["write".to_string()],
    )
    .await;

    let config = ApiConfig::default();
    let server = ApiServer::with_state(state, config);
    let router = server.router();

    let keypair = KeyPair::generate("FixBot".to_string());
    let body = create_fix_body(&keypair);

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/contributions/fix")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-api-key", api_key)
        .body(Body::from(body))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["contribution_id"].is_string());
}

#[tokio::test]
async fn test_get_contribution_after_submission() {
    let state = AppState::new().await.unwrap();
    let (api_key, _) = create_test_api_key(
        &state,
        "E2E Tester",
        ApiKeyTrustLevel::Verified,
        vec!["read".to_string(), "write".to_string()],
    )
    .await;

    let config = ApiConfig::default();
    let server = ApiServer::with_state(state.clone(), config);

    // Submit a bug report
    let keypair = KeyPair::generate("TestAgent".to_string());
    let body = create_bug_report_body(&keypair);

    let router = server.router();
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/contributions/bug")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-api-key", &api_key)
        .body(Body::from(body))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let contribution_id = json["contribution_id"].as_str().unwrap();

    // Now get the contribution
    let router = server.router();
    let request = Request::builder()
        .uri(format!("/api/v1/contributions/{}", contribution_id))
        .header("x-api-key", &api_key)
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["id"], contribution_id);
    assert_eq!(json["status"], "submitted");
    assert_eq!(json["contribution_type"], "bug");
}

#[tokio::test]
async fn test_list_contributions() {
    let state = AppState::new().await.unwrap();
    let (api_key, _) = create_test_api_key(
        &state,
        "Lister",
        ApiKeyTrustLevel::Basic,
        vec!["read".to_string(), "write".to_string()],
    )
    .await;

    let config = ApiConfig::default();
    let server = ApiServer::with_state(state.clone(), config);

    // Submit multiple contributions
    let keypair = KeyPair::generate("TestAgent".to_string());

    for _ in 0..3 {
        let body = create_bug_report_body(&keypair);
        let router = server.router();
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/contributions/bug")
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-api-key", &api_key)
            .body(Body::from(body))
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // List contributions
    let router = server.router();
    let request = Request::builder()
        .uri("/api/v1/contributions")
        .header("x-api-key", &api_key)
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["contributions"].as_array().unwrap().len() >= 3);
    assert!(json["total"].as_u64().unwrap() >= 3);
}

// ============================================================================
// Review Submission Tests
// ============================================================================

#[tokio::test]
async fn test_submit_review_for_contribution() {
    let state = AppState::new().await.unwrap();
    let (api_key, _) = create_test_api_key(
        &state,
        "Reviewer",
        ApiKeyTrustLevel::Verified,
        vec!["read".to_string(), "write".to_string()],
    )
    .await;

    let config = ApiConfig::default();
    let server = ApiServer::with_state(state.clone(), config);

    // First, submit a contribution
    let keypair = KeyPair::generate("TestAgent".to_string());
    let body = create_bug_report_body(&keypair);

    let router = server.router();
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/contributions/bug")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-api-key", &api_key)
        .body(Body::from(body))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let contribution_id = json["contribution_id"].as_str().unwrap();

    // Now submit a review
    let reviewer_keypair = KeyPair::generate("ReviewerAgent".to_string());
    let (reviewer, _) = create_contributor_json(&reviewer_keypair, "ReviewerAgent", true);

    let verdict_str = "Approve";
    let signature = reviewer_keypair.sign(verdict_str.as_bytes());
    let signature_json = serde_json::to_value(&signature).unwrap();

    let review_body = serde_json::json!({
        "contribution_id": contribution_id,
        "verdict": "approve",
        "confidence": 0.9,
        "comments": ["LGTM", "Good fix for the emoji issue"],
        "concerns": [],
        "suggestions": [],
        "reviewer": reviewer,
        "signature": signature_json
    });

    let router = server.router();
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/contributions/{}/review", contribution_id))
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-api-key", &api_key)
        .body(Body::from(review_body.to_string()))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["review_id"].is_string());
    assert_eq!(json["contribution_status"], "under_review");
}

// ============================================================================
// Rate Limiting Tests
// ============================================================================

#[tokio::test]
async fn test_rate_limit_headers_present() {
    let router = test_router().await;

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Rate limit headers should be present
    assert!(response.headers().contains_key("x-ratelimit-remaining"));
    assert!(response.headers().contains_key("x-ratelimit-limit"));
}

#[tokio::test]
async fn test_rate_limiting_triggers() {
    // Create server with very low rate limit for testing
    let mut config = ServerConfig::default();
    config.rate_limit_rpm = 3; // Very low for testing

    let state = AppState::with_config(config).await.unwrap();
    let api_config = ApiConfig::default();
    let server = ApiServer::with_state(state, api_config);

    // Make requests until rate limited
    for i in 0..5 {
        let router = server.router();
        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();

        if i < 3 {
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "Request {} should succeed",
                i
            );
        } else {
            assert_eq!(
                response.status(),
                StatusCode::TOO_MANY_REQUESTS,
                "Request {} should be rate limited",
                i
            );
            assert!(response.headers().contains_key("retry-after"));
        }
    }
}

#[tokio::test]
async fn test_rate_limit_per_api_key() {
    // Different API keys should have separate rate limit buckets
    let state = AppState::new().await.unwrap();

    let (api_key_1, _) =
        create_test_api_key(&state, "User 1", ApiKeyTrustLevel::Basic, vec![]).await;

    let (api_key_2, _) =
        create_test_api_key(&state, "User 2", ApiKeyTrustLevel::Basic, vec![]).await;

    let config = ApiConfig::default();
    let server = ApiServer::with_state(state, config);

    // Make requests with key 1
    for _ in 0..3 {
        let router = server.router();
        let request = Request::builder()
            .uri("/health")
            .header("x-api-key", &api_key_1)
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Key 2 should still have full quota
    let router = server.router();
    let request = Request::builder()
        .uri("/health")
        .header("x-api-key", &api_key_2)
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Check remaining is full (60 - 1 = 59 for default)
    let remaining: u32 = response
        .headers()
        .get("x-ratelimit-remaining")
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(remaining >= 50); // Should have most of quota left
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_contribution_validation_errors() {
    let router = test_router().await;

    // Submit bug report with missing required fields
    let invalid_body = serde_json::json!({
        "package": "sha256:1234",
        "title": "", // Empty title should fail validation
        "description": "",
        "category": "runtime_error",
        "severity": "low",
        "reporter": {
            "app_id": Uuid::new_v4().to_string(),
            "name": "Test",
            "public_key": {
                "key_id": "test",
                "bytes": "0".repeat(64),
                "owner": "test",
                "registered_at": "2024-01-01T00:00:00Z",
                "active": true
            },
            "is_ai": true
        },
        "signature": {
            "key_id": "test",
            "signature": "0".repeat(128),
            "timestamp": "2024-01-01T00:00:00Z"
        }
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/contributions/bug")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(invalid_body.to_string()))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["code"], "VALIDATION_FAILED");
}

#[tokio::test]
async fn test_contribution_not_found() {
    let router = test_router().await;

    let fake_id = Uuid::new_v4();
    let request = Request::builder()
        .uri(format!("/api/v1/contributions/{}", fake_id))
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test]
async fn test_review_for_nonexistent_contribution() {
    let router = test_router().await;

    let fake_id = Uuid::new_v4();
    let keypair = KeyPair::generate("Reviewer".to_string());
    let (reviewer, _) = create_contributor_json(&keypair, "Reviewer", true);

    let signature = keypair.sign(b"Approve");
    let signature_json = serde_json::to_value(&signature).unwrap();

    let review_body = serde_json::json!({
        "contribution_id": fake_id.to_string(),
        "verdict": "approve",
        "confidence": 0.8,
        "comments": [],
        "reviewer": reviewer,
        "signature": signature_json
    });

    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/contributions/{}/review", fake_id))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(review_body.to_string()))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// Full E2E Flow Test
// ============================================================================

#[tokio::test]
async fn test_full_contribution_flow() {
    // This test simulates the complete flow:
    // 1. Create API key
    // 2. Submit bug report
    // 3. Submit review
    // 4. Check contribution status

    let state = AppState::new().await.unwrap();

    // 1. Create API key
    let (api_key, _) = create_test_api_key(
        &state,
        "E2E Test Key",
        ApiKeyTrustLevel::Verified,
        vec!["read".to_string(), "write".to_string()],
    )
    .await;

    let config = ApiConfig::default();
    let server = ApiServer::with_state(state.clone(), config);

    // 2. Submit bug report
    let reporter_keypair = KeyPair::generate("BugReporter".to_string());
    let bug_body = create_bug_report_body(&reporter_keypair);

    let router = server.router();
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/contributions/bug")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-api-key", &api_key)
        .body(Body::from(bug_body))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let contribution_id = json["contribution_id"].as_str().unwrap().to_string();

    // Verify initial status
    assert_eq!(json["status"], "submitted");

    // 3. Submit review
    let reviewer_keypair = KeyPair::generate("Reviewer1".to_string());
    let (reviewer, _) = create_contributor_json(&reviewer_keypair, "Reviewer1", false);

    let verdict_str = "Approve";
    let signature = reviewer_keypair.sign(verdict_str.as_bytes());
    let signature_json = serde_json::to_value(&signature).unwrap();

    let review_body = serde_json::json!({
        "contribution_id": contribution_id,
        "verdict": "approve",
        "confidence": 0.95,
        "comments": ["Excellent fix!", "Well documented reproduction steps"],
        "concerns": [],
        "suggestions": [],
        "reviewer": reviewer,
        "signature": signature_json
    });

    let router = server.router();
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/contributions/{}/review", contribution_id))
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-api-key", &api_key)
        .body(Body::from(review_body.to_string()))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let review_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(review_json["review_id"].is_string());
    assert_eq!(review_json["contribution_status"], "under_review");

    // 4. Check contribution status - should show the review
    let router = server.router();
    let request = Request::builder()
        .uri(format!("/api/v1/contributions/{}", contribution_id))
        .header("x-api-key", &api_key)
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(detail_json["id"], contribution_id);
    assert_eq!(detail_json["status"], "under_review");
    assert!(!detail_json["reviews"].as_array().unwrap().is_empty());

    // Verify the review details
    let reviews = detail_json["reviews"].as_array().unwrap();
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0]["verdict"], "approve");
}
