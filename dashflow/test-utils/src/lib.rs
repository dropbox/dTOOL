//! Test utilities for `DashFlow` Rust integration testing
//!
//! This crate provides shared infrastructure for real integration tests:
//! - Credential loading and validation
//! - Service health checks
//! - Test setup/teardown automation
//! - Docker service management helpers
//! - Strict mock HTTP servers (M-240)

use std::env;
use std::time::Duration;

use thiserror::Error;

pub mod credentials;
pub mod docker;
pub mod health;
pub mod mock_embeddings;
pub mod observability;
#[cfg(feature = "mock-server")]
pub mod strict_mock;
pub mod test_cost;

pub use credentials::{
    anthropic_credentials, chroma_credentials, cohere_credentials, fireworks_credentials,
    groq_credentials, huggingface_credentials, mistral_credentials, mongodb_credentials,
    nomic_credentials, ollama_credentials, openai_credentials, pinecone_credentials,
    postgres_credentials, qdrant_credentials, redis_credentials, weaviate_credentials,
    xai_credentials, Credentials, CredentialsLoader,
};
pub use docker::DockerServices;
pub use health::HealthChecker;
pub use mock_embeddings::MockEmbeddings;
pub use observability::{
    check_expected_schema_api, check_grafana_has_data, count_quality_processed, get_container_logs,
    get_kafka_offset, is_container_running, is_kafka_healthy, is_prometheus_available,
    query_grafana_frames, query_prometheus, query_quality_score_in_range, verify_expected_schema_content,
    verify_grafana_data, verify_schema_roundtrip, wait_for_kafka_healthy, wait_for_kafka_messages,
    wait_for_prometheus_metric, wait_for_quality_processed, ExpectedSchemaEntry,
    ExpectedSchemaVerification, GrafanaAssertionResult, GrafanaFrame, GrafanaFrameAssertionResult,
    GrafanaFrameData, GrafanaFrameField, GrafanaFrameSchema, GrafanaRefResult, GrafanaTypedResponse,
    GrafanaValueAssertion, GrafanaVerificationResult, ObservabilityTestResult, PollingConfig,
    SchemaRoundtripResult, SetExpectedSchemaRequest, DASHSTREAM_TOPIC, KAFKA_CONTAINER,
    QUALITY_AGGREGATOR_CONTAINER,
};
pub use test_cost::{
    recommended_test_model, with_rate_limit_retry, RetryConfig, RetryError, TestCostReport,
    TestCostTracker, RECOMMENDED_TEST_MODEL,
};

// Re-export strict mock types when feature is enabled (M-240)
#[cfg(feature = "mock-server")]
pub use strict_mock::{StrictMock, StrictMockServer};

#[derive(Debug, Error)]
pub enum TestError {
    #[error("Missing required credential: {0}")]
    MissingCredential(String),

    #[error("Service not healthy: {0}")]
    ServiceUnhealthy(String),

    #[error("Docker service failed: {0}")]
    DockerError(String),

    #[error("Environment error: {0}")]
    EnvError(#[from] std::env::VarError),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, TestError>;

/// Initialize test environment
pub fn init_test_env() -> Result<()> {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    // Initialize tracing for test logging
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info,dashflow=debug".to_string()))
        .try_init();

    Ok(())
}

/// Get test timeout duration
#[must_use]
pub fn test_timeout() -> Duration {
    let timeout_secs = env::var("TEST_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    Duration::from_secs(timeout_secs)
}

/// Find the repository root directory (M-108: robust repo-root discovery)
///
/// This function walks up from `CARGO_MANIFEST_DIR` (or current directory as fallback)
/// looking for a `Cargo.toml` file that contains `[workspace]`, indicating the
/// workspace root.
///
/// This is more robust than using relative paths because tests can be run from
/// any working directory.
///
/// # Returns
/// - `Some(PathBuf)` - Path to the workspace root
/// - `None` - If no workspace root could be found
///
/// # Example
/// ```ignore
/// use dashflow_test_utils::find_repo_root;
///
/// let root = find_repo_root().expect("Could not find repo root");
/// let compose_file = root.join("docker-compose.dashstream.yml");
/// ```
#[must_use]
pub fn find_repo_root() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    // Start from CARGO_MANIFEST_DIR if available (set by cargo during test runs)
    // Otherwise fall back to current directory
    let start_dir = env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let mut current = start_dir.as_path();

    // Walk up the directory tree looking for workspace Cargo.toml
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            // Check if this Cargo.toml has [workspace]
            if let Ok(contents) = std::fs::read_to_string(&cargo_toml) {
                if contents.contains("[workspace]") {
                    return Some(current.to_path_buf());
                }
            }
        }

        // Also check for .git as a fallback indicator of repo root
        if current.join(".git").exists() {
            // Even if no [workspace], .git indicates repo root
            return Some(current.to_path_buf());
        }

        // Move to parent directory
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }

    None
}
