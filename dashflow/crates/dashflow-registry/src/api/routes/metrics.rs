//! Prometheus Metrics Endpoint
//!
//! Exposes Prometheus-format metrics at `/metrics` for scraping by Prometheus.
//!
//! # Metrics Exposed
//!
//! - HTTP request counts and latencies
//! - Cache hit/miss rates
//! - Storage operation metrics
//! - Search query metrics
//! - API key verification metrics
//! - Rate limiting events
//!
//! # Usage
//!
//! ```bash
//! curl http://localhost:3030/metrics
//! ```

#[cfg(feature = "metrics")]
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Router};

use crate::api::state::AppState;

/// Metrics routes (at root level)
#[cfg(feature = "metrics")]
pub fn metrics_routes() -> Router<AppState> {
    Router::new().route("/metrics", get(prometheus_metrics))
}

/// Prometheus metrics endpoint
///
/// Returns metrics in Prometheus text format for scraping.
#[cfg(feature = "metrics")]
async fn prometheus_metrics(State(state): State<AppState>) -> impl IntoResponse {
    match &state.metrics {
        Some(metrics) => match metrics.encode() {
            Ok(output) => (
                StatusCode::OK,
                [("Content-Type", "text/plain; version=0.0.4; charset=utf-8")],
                output,
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Content-Type", "text/plain; charset=utf-8")],
                format!("Failed to encode metrics: {}", e),
            ),
        },
        None => (
            StatusCode::NOT_FOUND,
            [("Content-Type", "text/plain; charset=utf-8")],
            "Metrics not enabled".to_string(),
        ),
    }
}

/// No-op metrics routes when feature is disabled
#[cfg(not(feature = "metrics"))]
pub fn metrics_routes() -> axum::Router<AppState> {
    use axum::{http::StatusCode, routing::get, Router};

    Router::new().route(
        "/metrics",
        get(|| async {
            (
                StatusCode::NOT_FOUND,
                "Metrics feature not enabled. Rebuild with --features metrics",
            )
        }),
    )
}

#[cfg(test)]
#[cfg(feature = "metrics")]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_metrics_endpoint() {
        let state = AppState::new().await.unwrap();
        let app = metrics_routes().with_state(state);

        let response = app
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        // Verify Prometheus format
        assert!(body_str.contains("dashflow_registry_"));
    }

    #[tokio::test]
    async fn test_metrics_content_type() {
        let state = AppState::new().await.unwrap();
        let app = metrics_routes().with_state(state);

        let response = app
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok());

        assert!(content_type
            .map(|ct| ct.contains("text/plain"))
            .unwrap_or(false));
    }
}
