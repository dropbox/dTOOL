//! API Integration Tests
//!
//! Tests the registry HTTP API routes using the router directly.

#![cfg(feature = "server")]

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use tower::util::ServiceExt;

use dashflow_registry::{ApiConfig, ApiServer};

/// Helper to create a test server and get its router
async fn test_router() -> axum::Router {
    let config = ApiConfig::default();
    let server = ApiServer::new(config).await.unwrap();
    server.router()
}

// ============================================================================
// Health & Status Endpoints
// ============================================================================

#[tokio::test]
async fn test_health_endpoint() {
    let router = test_router().await;

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "healthy");
}

#[tokio::test]
async fn test_ready_endpoint() {
    let router = test_router().await;

    let request = Request::builder()
        .uri("/ready")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ready"], true);
}

#[tokio::test]
async fn test_root_endpoint() {
    let router = test_router().await;

    let request = Request::builder().uri("/").body(Body::empty()).unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["name"], "dashflow-registry");
}

// ============================================================================
// Package Endpoints
// ============================================================================

#[tokio::test]
async fn test_package_not_found() {
    let router = test_router().await;

    let fake_hash = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let request = Request::builder()
        .uri(format!("/api/v1/packages/{}", fake_hash))
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_resolve_not_found() {
    let router = test_router().await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/packages/resolve")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"name": "nonexistent-package", "version": "*"}"#,
        ))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// Search Endpoints
// ============================================================================

#[tokio::test]
async fn test_keyword_search_empty() {
    let router = test_router().await;

    let request = Request::builder()
        .uri("/api/v1/search/keyword?q=test")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["results"].is_array());
}

#[tokio::test]
async fn test_unified_search() {
    let router = test_router().await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/search")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"query": "sentiment analysis", "limit": 10}"#,
        ))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["results"].is_array());
    assert!(json["total"].is_number());
}

// ============================================================================
// Batch Endpoints
// ============================================================================

#[tokio::test]
async fn test_batch_resolve_empty() {
    let router = test_router().await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/batch/resolve")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"packages": []}"#))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["resolved"].is_array());
    assert!(json["failed"].is_array());
}

#[tokio::test]
async fn test_batch_resolve_too_many() {
    let router = test_router().await;

    // Create a request with 101 packages (over the limit)
    let packages: Vec<serde_json::Value> = (0..101)
        .map(|i| serde_json::json!({"name": format!("pkg-{}", i), "version": "*"}))
        .collect();
    let body = serde_json::json!({"packages": packages});

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/batch/resolve")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_batch_download_empty() {
    let router = test_router().await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/batch/download")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"hashes": []}"#))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["downloads"].is_array());
}

// ============================================================================
// Trust Endpoints
// ============================================================================

#[tokio::test]
async fn test_trust_keys_list() {
    let router = test_router().await;

    let request = Request::builder()
        .uri("/api/v1/trust/keys")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["keys"].is_array());
}

// ============================================================================
// Error Handling
// ============================================================================

#[tokio::test]
async fn test_404_for_unknown_route() {
    let router = test_router().await;

    let request = Request::builder()
        .uri("/api/v1/nonexistent")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_invalid_json_body() {
    let router = test_router().await;

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/search")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("this is not json"))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    // Should get a 4xx error for invalid JSON
    assert!(response.status().is_client_error());
}
