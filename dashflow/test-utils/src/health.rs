//! Health check utilities for services

use std::time::Duration;

use reqwest::Client;
use tokio::time::sleep;

use crate::{Result, TestError};

/// Service health checker
pub struct HealthChecker {
    client: Client,
    max_retries: u32,
    retry_delay: Duration,
}

impl HealthChecker {
    /// Create a new health checker
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            max_retries: 30,
            retry_delay: Duration::from_secs(2),
        }
    }

    /// Set maximum retries
    #[must_use]
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Set retry delay
    #[must_use]
    pub fn with_retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = delay;
        self
    }

    /// Check if a service is healthy by URL
    pub async fn check_http(&self, url: &str) -> Result<()> {
        for attempt in 1..=self.max_retries {
            match self.client.get(url).send().await {
                Ok(response) if response.status().is_success() => {
                    tracing::info!("Service healthy at {}", url);
                    return Ok(());
                }
                Ok(response) => {
                    tracing::debug!(
                        "Service {} returned status {} (attempt {}/{})",
                        url,
                        response.status(),
                        attempt,
                        self.max_retries
                    );
                }
                Err(e) => {
                    tracing::debug!(
                        "Service {} not ready: {} (attempt {}/{})",
                        url,
                        e,
                        attempt,
                        self.max_retries
                    );
                }
            }

            if attempt < self.max_retries {
                sleep(self.retry_delay).await;
            }
        }

        Err(TestError::ServiceUnhealthy(url.to_string()))
    }

    /// Check multiple services in parallel
    pub async fn check_all(&self, urls: &[&str]) -> Result<()> {
        let mut handles = Vec::new();

        for url in urls {
            let url = (*url).to_string();
            let checker = self.clone();
            handles.push(tokio::spawn(async move { checker.check_http(&url).await }));
        }

        for handle in handles {
            handle
                .await
                .map_err(|e| TestError::ServiceUnhealthy(format!("Join error: {e}")))??;
        }

        Ok(())
    }
}

impl Clone for HealthChecker {
    fn clone(&self) -> Self {
        Self {
            client: Client::new(),
            max_retries: self.max_retries,
            retry_delay: self.retry_delay,
        }
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Predefined health checks for common services
///
/// Check Chroma health
pub async fn check_chroma() -> Result<()> {
    let url = std::env::var("CHROMA_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());
    HealthChecker::new()
        .check_http(&format!("{url}/api/v1/heartbeat"))
        .await
}

/// Check Qdrant health
pub async fn check_qdrant() -> Result<()> {
    let url = std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".to_string());
    HealthChecker::new()
        .check_http(&format!("{url}/health"))
        .await
}

/// Check Weaviate health
pub async fn check_weaviate() -> Result<()> {
    let url = std::env::var("WEAVIATE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    HealthChecker::new()
        .check_http(&format!("{url}/v1/.well-known/ready"))
        .await
}

/// Check Elasticsearch health
pub async fn check_elasticsearch() -> Result<()> {
    let url =
        std::env::var("ELASTICSEARCH_URL").unwrap_or_else(|_| "http://localhost:9200".to_string());
    HealthChecker::new()
        .check_http(&format!("{url}/_cluster/health"))
        .await
}

/// Check `MongoDB` health by verifying TCP connectivity to the MongoDB port.
///
/// Uses `MONGODB_URL` env var (default: `localhost:27017`).
/// This verifies the port is accepting connections, which is a real health check.
pub async fn check_mongodb() -> Result<()> {
    let host = std::env::var("MONGODB_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port: u16 = std::env::var("MONGODB_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(27017);

    check_tcp_port(&host, port, "MongoDB").await
}

/// Check `PostgreSQL` health by verifying TCP connectivity to the PostgreSQL port.
///
/// Uses `POSTGRES_HOST` and `POSTGRES_PORT` env vars (default: `localhost:5432`).
/// This verifies the port is accepting connections, which is a real health check.
pub async fn check_postgres() -> Result<()> {
    let host = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port: u16 = std::env::var("POSTGRES_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5432);

    check_tcp_port(&host, port, "PostgreSQL").await
}

/// Check Redis health by verifying TCP connectivity to the Redis port.
///
/// Uses `REDIS_HOST` and `REDIS_PORT` env vars (default: `localhost:6379`).
/// This verifies the port is accepting connections, which is a real health check.
pub async fn check_redis() -> Result<()> {
    let host = std::env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port: u16 = std::env::var("REDIS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(6379);

    check_tcp_port(&host, port, "Redis").await
}

/// Check TCP connectivity to a host:port with retries.
///
/// This is the minimum viable health check - verifies the service is accepting connections.
async fn check_tcp_port(host: &str, port: u16, service_name: &str) -> Result<()> {
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    let addr = format!("{host}:{port}");
    let max_retries = 5;
    let retry_delay = Duration::from_millis(500);
    let connect_timeout = Duration::from_secs(5);

    for attempt in 1..=max_retries {
        match timeout(connect_timeout, TcpStream::connect(&addr)).await {
            Ok(Ok(_stream)) => {
                tracing::info!("{} healthy at {}", service_name, addr);
                return Ok(());
            }
            Ok(Err(e)) => {
                tracing::debug!(
                    "{} connection failed to {}: {} (attempt {}/{})",
                    service_name,
                    addr,
                    e,
                    attempt,
                    max_retries
                );
            }
            Err(_) => {
                tracing::debug!(
                    "{} connection timed out to {} (attempt {}/{})",
                    service_name,
                    addr,
                    attempt,
                    max_retries
                );
            }
        }

        if attempt < max_retries {
            sleep(retry_delay).await;
        }
    }

    Err(TestError::ServiceUnhealthy(format!(
        "{service_name} at {addr}"
    )))
}

/// Check all docker services
///
/// Uses environment variables for service URLs with localhost fallbacks:
/// - `CHROMA_URL` (default: http://localhost:8000)
/// - `QDRANT_URL` (default: http://localhost:6333)
/// - `WEAVIATE_URL` (default: http://localhost:8080)
/// - `ELASTICSEARCH_URL` (default: http://localhost:9200)
pub async fn check_all_docker_services() -> Result<()> {
    let checker = HealthChecker::new()
        .with_max_retries(60)
        .with_retry_delay(Duration::from_secs(2));

    // Use env vars with localhost fallbacks (same pattern as individual check functions)
    let chroma_url =
        std::env::var("CHROMA_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());
    let qdrant_url =
        std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".to_string());
    let weaviate_url =
        std::env::var("WEAVIATE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let elasticsearch_url =
        std::env::var("ELASTICSEARCH_URL").unwrap_or_else(|_| "http://localhost:9200".to_string());

    let http_services: Vec<String> = vec![
        format!("{chroma_url}/api/v1/heartbeat"),
        format!("{qdrant_url}/health"),
        format!("{weaviate_url}/v1/.well-known/ready"),
        format!("{elasticsearch_url}/_cluster/health"),
    ];
    // Convert Vec<String> to Vec<&str> for check_all
    let http_services_refs: Vec<&str> = http_services.iter().map(|s| s.as_str()).collect();

    tracing::info!("Checking health of docker services...");
    checker.check_all(&http_services_refs).await?;
    tracing::info!("All docker services are healthy");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_checker_invalid_url() {
        let checker = HealthChecker::new()
            .with_max_retries(3)
            .with_retry_delay(Duration::from_millis(100));

        let result = checker.check_http("http://localhost:99999/invalid").await;

        assert!(result.is_err());
    }
}
