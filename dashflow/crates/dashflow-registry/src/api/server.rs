//! API Server
//!
//! Main server implementation that wires together routes, middleware, and state.

use axum::{middleware, Router};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::limit::RequestBodyLimitLayer;

use crate::api::{
    middleware as mw, routes,
    state::{AppState, ServerConfig},
};
use crate::Result;

/// API Server configuration
#[derive(Debug, Clone)]
pub struct ApiConfig {
    /// Address to bind to
    pub bind_addr: SocketAddr,
    /// Server configuration
    pub server: ServerConfig,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 3030)),
            server: ServerConfig::default(),
        }
    }
}

impl ApiConfig {
    /// Create config with custom bind address
    pub fn with_addr(mut self, addr: SocketAddr) -> Self {
        self.bind_addr = addr;
        self
    }

    /// Create config with custom port
    pub fn with_port(mut self, port: u16) -> Self {
        self.bind_addr.set_port(port);
        self
    }

    /// Create config with custom base URL
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.server.base_url = url.into();
        self
    }
}

/// API Server instance
pub struct ApiServer {
    config: ApiConfig,
    state: AppState,
    router: Router,
}

impl ApiServer {
    /// Create a new API server with the given configuration
    pub async fn new(config: ApiConfig) -> Result<Self> {
        let state = AppState::with_config(config.server.clone()).await?;
        let router = Self::build_router(&state);

        Ok(Self {
            config,
            state,
            router,
        })
    }

    /// Create a new API server with existing application state
    pub fn with_state(state: AppState, config: ApiConfig) -> Self {
        let router = Self::build_router(&state);
        Self {
            config,
            state,
            router,
        }
    }

    /// Create a new API server with default configuration
    pub async fn with_defaults() -> Result<Self> {
        Self::new(ApiConfig::default()).await
    }

    /// Build the complete router with all routes and middleware
    fn build_router(state: &AppState) -> Router {
        // Build API routes
        let api = routes::api_router(state.clone());

        // Apply middleware layers (in reverse order - last applied runs first)
        let router = api
            // Error handling (outermost - catches all errors)
            .layer(middleware::from_fn(mw::error_handler_middleware))
            // CORS headers
            .layer(middleware::from_fn_with_state(
                state.clone(),
                mw::cors_middleware,
            ))
            // Rate limiting
            .layer(middleware::from_fn_with_state(
                state.clone(),
                mw::rate_limit_middleware,
            ))
            // Authentication context extraction (verifies keys against database)
            .layer(middleware::from_fn_with_state(
                state.clone(),
                mw::auth_context_middleware,
            ))
            // Request ID generation/extraction
            .layer(middleware::from_fn(mw::request_id_middleware));

        // Add metrics middleware (innermost - records after request completes)
        #[cfg(feature = "metrics")]
        let router = router.layer(middleware::from_fn_with_state(
            state.clone(),
            mw::metrics_middleware,
        ));

        // Add OpenTelemetry tracing middleware (innermost - traces entire request)
        #[cfg(feature = "opentelemetry")]
        let router = router.layer(middleware::from_fn(mw::tracing_middleware));

        // Request body size limit (M-233: prevent oversized requests)
        // Applied last so it runs first - rejects oversized requests before any processing.
        // Returns 413 Payload Too Large if body exceeds max_body_size (default 50MB).
        router.layer(RequestBodyLimitLayer::new(state.config.max_body_size))
    }

    /// Get the server's bind address
    pub fn addr(&self) -> SocketAddr {
        self.config.bind_addr
    }

    /// Get the application state (for testing)
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Get the router (for testing)
    pub fn router(&self) -> Router {
        self.router.clone()
    }

    /// Run the server
    pub async fn run(self) -> Result<()> {
        let listener = TcpListener::bind(self.config.bind_addr)
            .await
            .map_err(|e| crate::RegistryError::StorageError(format!("Failed to bind: {}", e)))?;

        println!(
            "DashFlow Registry API starting on {}",
            self.config.bind_addr
        );
        println!("  Health:  http://{}/health", self.config.bind_addr);
        println!("  API v1:  http://{}/api/v1", self.config.bind_addr);
        #[cfg(feature = "metrics")]
        println!("  Metrics: http://{}/metrics", self.config.bind_addr);

        axum::serve(listener, self.router)
            .await
            .map_err(|e| crate::RegistryError::StorageError(format!("Server error: {}", e)))?;

        Ok(())
    }

    /// Run the server until the given signal is received
    pub async fn run_until<F>(self, shutdown_signal: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let listener = TcpListener::bind(self.config.bind_addr)
            .await
            .map_err(|e| crate::RegistryError::StorageError(format!("Failed to bind: {}", e)))?;

        println!(
            "DashFlow Registry API starting on {}",
            self.config.bind_addr
        );

        axum::serve(listener, self.router)
            .with_graceful_shutdown(shutdown_signal)
            .await
            .map_err(|e| crate::RegistryError::StorageError(format!("Server error: {}", e)))?;

        println!("Server shutdown complete");
        Ok(())
    }
}

/// Convenience function to run a server with default config
pub async fn run_server() -> Result<()> {
    let server = ApiServer::with_defaults().await?;
    server.run().await
}

/// Convenience function to run a server on a specific port
pub async fn run_server_on_port(port: u16) -> Result<()> {
    let config = ApiConfig::default().with_port(port);
    let server = ApiServer::new(config).await?;
    server.run().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn test_server_creation() {
        let config = ApiConfig::default();
        let server = ApiServer::new(config).await;
        assert!(server.is_ok());
        let server = server.unwrap();
        // Verify the server was created and can produce a router
        let _router = server.router();
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let config = ApiConfig::default();
        let server = ApiServer::new(config).await.unwrap();
        let router = server.router();

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = router.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_root_endpoint() {
        let config = ApiConfig::default();
        let server = ApiServer::new(config).await.unwrap();
        let router = server.router();

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();

        let response = router.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_body_size_limit_enforced() {
        // M-233: Test that request body size limits are enforced
        use crate::api::state::ServerConfig;

        // Create a config with a small body size limit (1KB)
        let mut server_config = ServerConfig::default();
        server_config.max_body_size = 1024; // 1KB limit

        let config = ApiConfig {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            server: server_config,
        };

        let server = ApiServer::new(config).await.unwrap();
        let router = server.router();

        // Create a body that exceeds the limit (2KB)
        let oversized_body = vec![b'x'; 2048];

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/packages") // Any POST endpoint
            .header("Content-Type", "application/json")
            .body(Body::from(oversized_body))
            .unwrap();

        let response = router.oneshot(request).await.unwrap();

        // Should get 413 Payload Too Large (or 411 Length Required)
        // tower-http's RequestBodyLimitLayer returns 413
        assert_eq!(
            response.status(),
            StatusCode::PAYLOAD_TOO_LARGE,
            "Oversized requests should be rejected with 413 Payload Too Large"
        );
    }

    #[tokio::test]
    async fn test_body_size_limit_allows_small_requests() {
        // M-233: Test that requests within limit are allowed
        use crate::api::state::ServerConfig;

        // Create a config with a 10KB body size limit
        let mut server_config = ServerConfig::default();
        server_config.max_body_size = 10 * 1024; // 10KB limit

        let config = ApiConfig {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            server: server_config,
        };

        let server = ApiServer::new(config).await.unwrap();
        let router = server.router();

        // Create a body within the limit (1KB)
        let small_body = vec![b'x'; 1024];

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/packages")
            .header("Content-Type", "application/json")
            .body(Body::from(small_body))
            .unwrap();

        let response = router.oneshot(request).await.unwrap();

        // Should NOT be 413 - the request should pass the body limit check
        // (it may fail for other reasons like invalid JSON, but not body size)
        assert_ne!(
            response.status(),
            StatusCode::PAYLOAD_TOO_LARGE,
            "Requests within size limit should not be rejected for body size"
        );
    }
}
