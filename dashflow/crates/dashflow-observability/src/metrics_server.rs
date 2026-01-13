//! HTTP server for Prometheus metrics scraping
//!
//! This module provides a simple HTTP server that exposes a `/metrics` endpoint
//! for Prometheus to scrape metrics from DashFlow applications.
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_observability::metrics_server::serve_metrics;
//! use dashflow_observability::metrics::init_default_recorder;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Initialize metrics recorder
//! init_default_recorder()?;
//!
//! // Start metrics server on port 9090
//! serve_metrics(9090).await?;
//! # Ok(())
//! # }
//! ```

use crate::error::Result;
use crate::metrics::MetricsRegistry;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;
use tokio::net::TcpListener;

/// Start an HTTP server to serve Prometheus metrics
///
/// This starts a simple HTTP server on the specified port that exposes a `/metrics`
/// endpoint in Prometheus text format. The server will run until the process is terminated.
///
/// # Arguments
///
/// * `port` - Port to listen on (e.g., 9090). Use 0 to let the OS assign a port.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_observability::metrics_server::serve_metrics;
/// use dashflow_observability::metrics::init_default_recorder;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Initialize metrics
/// init_default_recorder()?;
///
/// // Start server
/// serve_metrics(9090).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Prometheus Scrape Config
///
/// ```yaml
/// scrape_configs:
///   - job_name: 'dashflow'
///     static_configs:
///       - targets: ['localhost:9090']
/// ```
pub async fn serve_metrics(port: u16) -> Result<()> {
    let (_, server_future) = serve_metrics_with_addr(port).await?;
    server_future.await
}

/// Start an HTTP server and return the actual bound address
///
/// This is useful when port 0 is specified to let the OS assign a port.
/// Returns the actual bound socket address and a future that runs the server.
pub async fn serve_metrics_with_addr(
    port: u16,
) -> Result<(
    std::net::SocketAddr,
    impl std::future::Future<Output = Result<()>>,
)> {
    let registry = MetricsRegistry::global();

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .with_state(registry);

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| crate::error::Error::Metrics(format!("Failed to bind to {}: {}", addr, e)))?;

    let local_addr = listener
        .local_addr()
        .map_err(|e| crate::error::Error::Metrics(format!("Failed to get local addr: {}", e)))?;

    tracing::info!("Metrics server listening on http://{}/metrics", local_addr);

    let server_future = async move {
        axum::serve(listener, app)
            .await
            .map_err(|e| crate::error::Error::Metrics(format!("Server error: {}", e)))?;
        Ok(())
    };

    Ok((local_addr, server_future))
}

/// Handler for the /metrics endpoint
async fn metrics_handler(State(registry): State<Arc<MetricsRegistry>>) -> Response {
    match registry.export() {
        Ok(metrics) => (StatusCode::OK, metrics).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to export metrics: {}", e),
        )
            .into_response(),
    }
}

/// Handler for the /health endpoint
async fn health_handler() -> Response {
    (StatusCode::OK, "OK").into_response()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // Tests: unwrap() is idiomatic for test code
mod tests {
    use super::*;
    use crate::metrics::init_default_recorder;
    use tokio::task::JoinHandle;
    use tokio::time::{timeout, Duration};

    /// M-583: Guard that aborts server task on drop (prevents leaks on panic)
    ///
    /// This ensures the spawned metrics server is properly cleaned up even if the test panics.
    struct ServerGuard<T> {
        handle: JoinHandle<T>,
    }

    impl<T> Drop for ServerGuard<T> {
        fn drop(&mut self) {
            self.handle.abort();
        }
    }

    /// M-579: Helper to wait for HTTP server readiness with retry
    async fn wait_for_server_ready(port: u16, max_retries: u32) -> bool {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()
            .unwrap();

        for attempt in 0..max_retries {
            match client
                .get(format!("http://localhost:{}/health", port))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => return true,
                _ => {
                    // Exponential backoff: 10ms, 20ms, 40ms, 80ms, 160ms
                    let delay = Duration::from_millis(10 * (1 << attempt.min(4)));
                    tokio::time::sleep(delay).await;
                }
            }
        }
        false
    }

    #[tokio::test]
    async fn test_metrics_server_starts() {
        // Initialize recorder
        let _ = init_default_recorder();

        // M-570: Use port 0 for OS-assigned port to avoid test flakiness
        let (addr, server_future) = serve_metrics_with_addr(0).await.unwrap();
        let port = addr.port();

        // Start server in background with guard for cleanup on panic (M-583)
        let _guard = ServerGuard {
            handle: tokio::spawn(server_future),
        };

        // M-579: Use readiness check with retry instead of fixed 100ms sleep
        let ready = wait_for_server_ready(port, 10).await;
        assert!(ready, "Server failed to start within timeout");

        // Test /metrics endpoint
        let client = reqwest::Client::new();
        let response = timeout(
            Duration::from_secs(2),
            client
                .get(format!("http://localhost:{}/metrics", port))
                .send(),
        )
        .await;

        assert!(response.is_ok(), "Server should respond");
        let response = response.unwrap();
        assert!(response.is_ok(), "Request should succeed");
        let response = response.unwrap();
        assert_eq!(response.status(), 200);

        // Test /health endpoint
        let response = timeout(
            Duration::from_secs(2),
            client
                .get(format!("http://localhost:{}/health", port))
                .send(),
        )
        .await;

        assert!(response.is_ok(), "Health endpoint should respond");
        let response = response.unwrap();
        assert!(response.is_ok(), "Health request should succeed");
        let response = response.unwrap();
        assert_eq!(response.status(), 200);

        // Server is automatically aborted when _guard is dropped
    }

    #[tokio::test]
    async fn test_metrics_endpoint_returns_prometheus_format() {
        // Initialize recorder
        let _ = init_default_recorder();

        // M-570: Use port 0 for OS-assigned port to avoid test flakiness
        let (addr, server_future) = serve_metrics_with_addr(0).await.unwrap();
        let port = addr.port();

        // Start server in background with guard for cleanup on panic (M-583)
        let _guard = ServerGuard {
            handle: tokio::spawn(server_future),
        };

        // M-579: Use readiness check with retry instead of fixed 100ms sleep
        let ready = wait_for_server_ready(port, 10).await;
        assert!(ready, "Server failed to start within timeout");

        // Test /metrics endpoint
        let client = reqwest::Client::new();
        let response = timeout(
            Duration::from_secs(2),
            client
                .get(format!("http://localhost:{}/metrics", port))
                .send(),
        )
        .await;

        assert!(response.is_ok());
        let response = response.unwrap().unwrap();
        let body = response.text().await.unwrap();

        // Check for Prometheus format markers
        assert!(
            body.contains("# HELP") || body.is_empty(),
            "Should be Prometheus format or empty"
        );

        // Server is automatically aborted when _guard is dropped
    }
}
