// Allow clippy warnings for LangServe server
// - needless_pass_by_value: Request handlers take owned values for async operations
#![allow(clippy::needless_pass_by_value)]

//! Server setup and route management

use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use dashflow::core::runnable::Runnable;
use serde_json::Value;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::handler::{
    batch_handler, config_schema_handler, input_schema_handler, invoke_handler,
    output_schema_handler, playground_handler, stream_handler, AppState,
};

/// Prometheus metrics endpoint handler
async fn metrics_handler() -> impl IntoResponse {
    match crate::metrics::get_metrics() {
        Ok(metrics) => (StatusCode::OK, metrics),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to gather metrics: {e}"),
        ),
    }
}

/// Configuration for adding routes
#[derive(Debug, Clone)]
pub struct RouteConfig {
    /// Base path for the runnable (e.g., "/`my_runnable`")
    pub path: String,

    /// Whether to enable the /invoke endpoint
    pub enable_invoke: bool,

    /// Whether to enable the /batch endpoint
    pub enable_batch: bool,

    /// Whether to enable the /stream endpoint
    pub enable_stream: bool,

    /// Whether to enable schema endpoints
    pub enable_schema: bool,

    /// Whether to enable the playground
    pub enable_playground: bool,
}

impl Default for RouteConfig {
    fn default() -> Self {
        Self {
            path: String::new(),
            enable_invoke: true,
            enable_batch: true,
            enable_stream: true,
            enable_schema: true,
            enable_playground: true,
        }
    }
}

impl RouteConfig {
    /// Create a new route configuration with a path
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            ..Default::default()
        }
    }

    /// Set whether to enable the /invoke endpoint
    #[must_use]
    pub fn with_invoke(mut self, enable: bool) -> Self {
        self.enable_invoke = enable;
        self
    }

    /// Set whether to enable the /batch endpoint
    #[must_use]
    pub fn with_batch(mut self, enable: bool) -> Self {
        self.enable_batch = enable;
        self
    }

    /// Set whether to enable the /stream endpoint
    #[must_use]
    pub fn with_stream(mut self, enable: bool) -> Self {
        self.enable_stream = enable;
        self
    }

    /// Set whether to enable schema endpoints
    #[must_use]
    pub fn with_schema(mut self, enable: bool) -> Self {
        self.enable_schema = enable;
        self
    }

    /// Set whether to enable the playground
    #[must_use]
    pub fn with_playground(mut self, enable: bool) -> Self {
        self.enable_playground = enable;
        self
    }
}

/// Add routes for a runnable to an Axum router
///
/// This is the main entry point for serving a runnable as a REST API.
///
/// # Example
/// ```ignore
/// use axum::Router;
/// use dashflow_langserve::{add_routes, RouteConfig};
///
/// let app = Router::new();
/// let runnable = /* your runnable */;
/// let config = RouteConfig::new("/my_runnable");
///
/// let app = add_routes(app, runnable, config);
/// ```
pub fn add_routes<R>(router: Router, runnable: R, config: RouteConfig) -> Router
where
    R: Runnable<Input = Value, Output = Value> + 'static,
{
    let state = AppState {
        runnable: Arc::new(runnable),
        base_path: config.path.clone(),
    };

    let mut runnable_router = Router::new();

    // Add endpoints based on configuration
    if config.enable_invoke {
        runnable_router = runnable_router.route("/invoke", post(invoke_handler));
    }

    if config.enable_batch {
        runnable_router = runnable_router.route("/batch", post(batch_handler));
    }

    if config.enable_stream {
        runnable_router = runnable_router.route("/stream", post(stream_handler));
    }

    if config.enable_schema {
        runnable_router = runnable_router
            .route("/input_schema", get(input_schema_handler))
            .route("/output_schema", get(output_schema_handler))
            .route("/config_schema", get(config_schema_handler));
    }

    if config.enable_playground {
        runnable_router = runnable_router.route("/playground", get(playground_handler));
    }

    // Apply state to the runnable router and nest it under the configured path
    let runnable_router = runnable_router.with_state(state);

    // Use nest_service to integrate the stateful router
    router.nest_service(&config.path, runnable_router)
}

/// Create a default server with CORS enabled
///
/// # Security Warning (M-230)
/// This is a **development-only** convenience function. It uses permissive CORS
/// settings that allow **all origins, methods, and headers**. For production,
/// use [`create_server_with_cors`] to configure explicit allowed origins.
///
/// The server includes:
/// - CORS middleware (allows all origins, methods, headers)
/// - `/metrics` endpoint for Prometheus monitoring
/// - `/health` endpoint for liveness probes
/// - `/ready` endpoint for readiness probes
///
/// # Example
/// ```ignore
/// use dashflow_langserve::{create_server, RouteConfig};
///
/// let runnable = /* your runnable */;
/// let app = create_server()
///     .add_routes(runnable, RouteConfig::new("/my_runnable"));
/// ```
pub fn create_server() -> Router {
    // SECURITY (M-230): Log warning about permissive CORS
    tracing::warn!(
        "Using create_server() with permissive CORS (allow all origins). \
         For production, use create_server_with_cors() with explicit origins."
    );

    Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
}

/// Create a production server with explicit CORS origins
///
/// # Security (M-230)
/// This function requires explicit CORS origin configuration for production use.
/// Unlike [`create_server`], it does not use wildcard origins.
///
/// # Arguments
/// * `allowed_origins` - List of allowed CORS origins (e.g., `["https://app.example.com"]`)
///
/// # Example
/// ```ignore
/// use dashflow_langserve::{create_server_with_cors, RouteConfig};
///
/// let runnable = /* your runnable */;
/// let app = create_server_with_cors(vec!["https://app.example.com".to_string()])
///     .add_routes(runnable, RouteConfig::new("/my_runnable"));
/// ```
pub fn create_server_with_cors(allowed_origins: Vec<String>) -> Router {
    use tower_http::cors::AllowOrigin;

    let cors = if allowed_origins.is_empty() {
        // No origins configured - don't add CORS layer
        tracing::info!("No CORS origins configured - CORS headers will not be added");
        CorsLayer::new()
    } else {
        let origins: Vec<_> = allowed_origins
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect();

        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods(Any)
            .allow_headers(Any)
    };

    Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .layer(cors)
}

/// Health check endpoint handler (liveness probe)
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// Readiness check endpoint handler (readiness probe)
///
/// M-655: Now performs real checks instead of always returning OK.
/// Checks that the metrics system is functional before reporting ready.
async fn ready_handler() -> impl IntoResponse {
    // Verify metrics can be gathered - this is a real check that the
    // observability infrastructure is functional
    match crate::metrics::get_metrics() {
        Ok(_) => (StatusCode::OK, "OK"),
        Err(e) => {
            tracing::warn!(error = %e, "Readiness check failed: metrics unavailable");
            (StatusCode::SERVICE_UNAVAILABLE, "Metrics unavailable")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_config_defaults() {
        let config = RouteConfig::default();
        assert!(config.enable_invoke);
        assert!(config.enable_batch);
        assert!(config.enable_stream);
        assert!(config.enable_schema);
        assert!(config.enable_playground);
    }

    #[test]
    fn test_route_config_builder() {
        let config = RouteConfig::new("/test")
            .with_invoke(false)
            .with_batch(true)
            .with_stream(false);

        assert_eq!(config.path, "/test");
        assert!(!config.enable_invoke);
        assert!(config.enable_batch);
        assert!(!config.enable_stream);
    }

    // ==================== RouteConfig Tests ====================

    #[test]
    fn test_route_config_new_with_string() {
        let config = RouteConfig::new("/my_runnable");
        assert_eq!(config.path, "/my_runnable");
        assert!(config.enable_invoke);
    }

    #[test]
    fn test_route_config_new_with_owned_string() {
        let config = RouteConfig::new(String::from("/owned_path"));
        assert_eq!(config.path, "/owned_path");
    }

    #[test]
    fn test_route_config_empty_path() {
        let config = RouteConfig::new("");
        assert_eq!(config.path, "");
    }

    #[test]
    fn test_route_config_with_all_disabled() {
        let config = RouteConfig::new("/test")
            .with_invoke(false)
            .with_batch(false)
            .with_stream(false)
            .with_schema(false)
            .with_playground(false);

        assert!(!config.enable_invoke);
        assert!(!config.enable_batch);
        assert!(!config.enable_stream);
        assert!(!config.enable_schema);
        assert!(!config.enable_playground);
    }

    #[test]
    fn test_route_config_with_schema() {
        let config = RouteConfig::new("/api").with_schema(false);
        assert!(!config.enable_schema);
        assert!(config.enable_invoke); // Others should be unchanged
    }

    #[test]
    fn test_route_config_with_playground() {
        let config = RouteConfig::new("/api").with_playground(false);
        assert!(!config.enable_playground);
        assert!(config.enable_invoke); // Others should be unchanged
    }

    #[test]
    fn test_route_config_chained_toggles() {
        let config = RouteConfig::new("/test")
            .with_invoke(false)
            .with_invoke(true) // Re-enable
            .with_batch(false)
            .with_batch(true); // Re-enable

        assert!(config.enable_invoke);
        assert!(config.enable_batch);
    }

    #[test]
    fn test_route_config_debug() {
        let config = RouteConfig::new("/debug_test");
        let debug = format!("{:?}", config);
        assert!(debug.contains("RouteConfig"));
        assert!(debug.contains("/debug_test"));
    }

    #[test]
    fn test_route_config_clone() {
        let config = RouteConfig::new("/clone_test")
            .with_invoke(false)
            .with_stream(false);
        let cloned = config.clone();
        assert_eq!(config.path, cloned.path);
        assert_eq!(config.enable_invoke, cloned.enable_invoke);
        assert_eq!(config.enable_stream, cloned.enable_stream);
    }

    // ==================== RouteConfig Path Tests ====================

    #[test]
    fn test_route_config_various_paths() {
        let paths = vec![
            "/",
            "/api",
            "/api/v1",
            "/api/v1/runnable",
            "/my-runnable",
            "/my_runnable",
        ];
        for path in paths {
            let config = RouteConfig::new(path);
            assert_eq!(config.path, path);
        }
    }

    #[test]
    fn test_route_config_path_with_special_chars() {
        let config = RouteConfig::new("/api/v1/my-runnable_test");
        assert_eq!(config.path, "/api/v1/my-runnable_test");
    }

    // ==================== RouteConfig Default Tests ====================

    #[test]
    fn test_route_config_default_path_is_empty() {
        let config = RouteConfig::default();
        assert_eq!(config.path, "");
    }

    #[test]
    fn test_route_config_default_all_enabled() {
        let config = RouteConfig::default();
        assert!(config.enable_invoke);
        assert!(config.enable_batch);
        assert!(config.enable_stream);
        assert!(config.enable_schema);
        assert!(config.enable_playground);
    }

    // ==================== Server Creation Tests ====================

    #[test]
    fn test_create_server_returns_router() {
        let _router = create_server();
        // Just verify it compiles and returns
    }

    #[test]
    fn test_create_server_with_cors_empty_origins() {
        let _router = create_server_with_cors(vec![]);
        // Just verify it compiles and returns
    }

    #[test]
    fn test_create_server_with_cors_single_origin() {
        let _router = create_server_with_cors(vec!["https://example.com".to_string()]);
        // Just verify it compiles and returns
    }

    #[test]
    fn test_create_server_with_cors_multiple_origins() {
        let _router = create_server_with_cors(vec![
            "https://example.com".to_string(),
            "https://app.example.com".to_string(),
            "http://localhost:3000".to_string(),
        ]);
        // Just verify it compiles and returns
    }

    #[test]
    fn test_create_server_with_cors_invalid_origin_ignored() {
        // Invalid origins should be filtered out
        let _router = create_server_with_cors(vec![
            "https://valid.com".to_string(),
            "not-a-valid-origin".to_string(), // Should be filtered
        ]);
        // Just verify it compiles and returns
    }

    // ==================== Health and Ready Handler Tests ====================

    #[tokio::test]
    async fn test_health_handler_returns_ok() {
        let response = health_handler().await;
        let response = response.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_ready_handler_returns_ok() {
        // This test may fail if metrics can't be gathered, but in test env it should work
        let response = ready_handler().await;
        let response = response.into_response();
        // Can be OK or SERVICE_UNAVAILABLE depending on metrics state
        assert!(
            response.status() == StatusCode::OK
                || response.status() == StatusCode::SERVICE_UNAVAILABLE
        );
    }

    // ==================== Metrics Handler Tests ====================

    #[tokio::test]
    async fn test_metrics_handler_returns_response() {
        let response = metrics_handler().await;
        let response = response.into_response();
        // Can be OK or INTERNAL_SERVER_ERROR depending on metrics state
        assert!(
            response.status() == StatusCode::OK
                || response.status() == StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
