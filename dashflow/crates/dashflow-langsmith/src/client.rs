//! `LangSmith` HTTP client

use crate::error::{Error, Result};
use crate::run::{RunCreate, RunUpdate};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error};
use uuid::Uuid;

// Environment variable names (matching dashflow::core::config_loader::env_vars constants)
// Note: Cannot import from dashflow due to cyclic dependency
const LANGSMITH_API_KEY: &str = "LANGSMITH_API_KEY";
const LANGCHAIN_API_KEY: &str = "LANGCHAIN_API_KEY";
const LANGSMITH_ENDPOINT: &str = "LANGSMITH_ENDPOINT";
const LANGCHAIN_ENDPOINT: &str = "LANGCHAIN_ENDPOINT";
const LANGSMITH_PROJECT: &str = "LANGSMITH_PROJECT";
const LANGCHAIN_PROJECT: &str = "LANGCHAIN_PROJECT";

/// Helper to read a string from environment variable
fn env_string(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Default API URL for `LangSmith`
pub const DEFAULT_API_URL: &str = "https://api.smith.dashflow.com";

/// Default timeout for API requests (30 seconds)
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// `LangSmith` client for creating and managing runs
#[derive(Clone, Debug)]
pub struct Client {
    inner: Arc<ClientInner>,
}

#[derive(Debug)]
struct ClientInner {
    http_client: reqwest::Client,
    api_url: String,
    project_name: Option<String>,
}

impl Client {
    /// Create a new client builder
    #[must_use]
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Create a client from environment variables
    ///
    /// Reads:
    /// - `LANGSMITH_API_KEY` or `LANGCHAIN_API_KEY` for authentication
    /// - `LANGSMITH_ENDPOINT` or `LANGCHAIN_ENDPOINT` for API URL
    /// - `LANGSMITH_PROJECT` or `LANGCHAIN_PROJECT` for default project
    pub fn from_env() -> Result<Self> {
        ClientBuilder::default().from_env()?.build()
    }

    /// Create a run
    pub async fn create_run(&self, run: &RunCreate) -> Result<()> {
        let url = format!("{}/runs", self.inner.api_url);

        debug!("Creating run: {} ({})", run.name, run.id);

        let response = self.inner.http_client.post(&url).json(run).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            error!("Failed to create run: {} - {}", status, body);
            return Err(Error::api_error(status, body));
        }

        debug!("Successfully created run: {}", run.id);
        Ok(())
    }

    /// Update a run
    pub async fn update_run(&self, run_id: Uuid, update: &RunUpdate) -> Result<()> {
        let url = format!("{}/runs/{}", self.inner.api_url, run_id);

        debug!("Updating run: {}", run_id);

        let response = self
            .inner
            .http_client
            .patch(&url)
            .json(update)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            error!("Failed to update run: {} - {}", status, body);
            return Err(Error::api_error(status, body));
        }

        debug!("Successfully updated run: {}", run_id);
        Ok(())
    }

    /// Batch create/update runs
    pub async fn batch_ingest_runs(&self, batch: &BatchIngest) -> Result<()> {
        let url = format!("{}/runs/batch", self.inner.api_url);

        debug!(
            "Batch ingesting {} creates and {} updates",
            batch.post.len(),
            batch.patch.len()
        );

        let response = self.inner.http_client.post(&url).json(batch).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            error!("Failed to batch ingest runs: {} - {}", status, body);
            return Err(Error::api_error(status, body));
        }

        debug!(
            "Successfully batch ingested {} runs",
            batch.post.len() + batch.patch.len()
        );
        Ok(())
    }

    /// Get the default project name
    #[must_use]
    pub fn project_name(&self) -> Option<&str> {
        self.inner.project_name.as_deref()
    }

    /// Get the API URL
    #[must_use]
    pub fn api_url(&self) -> &str {
        &self.inner.api_url
    }
}

/// Batch ingestion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchIngest {
    /// Runs to create
    pub post: Vec<RunCreate>,
    /// Run updates (`run_id`, update)
    pub patch: Vec<(Uuid, RunUpdate)>,
}

impl BatchIngest {
    /// Create a new empty batch
    pub fn new() -> Self {
        Self {
            post: Vec::new(),
            patch: Vec::new(),
        }
    }

    /// Add a run creation to the batch
    pub fn add_create(&mut self, run: RunCreate) {
        self.post.push(run);
    }

    /// Add a run update to the batch
    pub fn add_update(&mut self, run_id: Uuid, update: RunUpdate) {
        self.patch.push((run_id, update));
    }

    /// Check if the batch is empty
    pub fn is_empty(&self) -> bool {
        self.post.is_empty() && self.patch.is_empty()
    }

    /// Get the total number of operations
    pub fn len(&self) -> usize {
        self.post.len() + self.patch.len()
    }
}

impl Default for BatchIngest {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for `LangSmith` client
#[derive(Default)]
pub struct ClientBuilder {
    api_url: Option<String>,
    api_key: Option<String>,
    project_name: Option<String>,
    timeout: Option<Duration>,
}

impl ClientBuilder {
    /// Set the API URL
    pub fn api_url(mut self, url: impl Into<String>) -> Self {
        self.api_url = Some(url.into());
        self
    }

    /// Set the API key
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the default project name
    pub fn project_name(mut self, name: impl Into<String>) -> Self {
        self.project_name = Some(name.into());
        self
    }

    /// Set the request timeout
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Load configuration from environment variables
    pub fn from_env(mut self) -> Result<Self> {
        // Try LANGSMITH_* variables first, then LANGCHAIN_* as fallback
        if self.api_key.is_none() {
            self.api_key = env_string(LANGSMITH_API_KEY).or_else(|| env_string(LANGCHAIN_API_KEY));
        }

        if self.api_url.is_none() {
            self.api_url =
                env_string(LANGSMITH_ENDPOINT).or_else(|| env_string(LANGCHAIN_ENDPOINT));
        }

        if self.project_name.is_none() {
            self.project_name =
                env_string(LANGSMITH_PROJECT).or_else(|| env_string(LANGCHAIN_PROJECT));
        }

        Ok(self)
    }

    /// Build the client
    pub fn build(self) -> Result<Client> {
        let api_key = self
            .api_key
            .ok_or_else(|| Error::config("API key is required"))?;

        let api_url = self.api_url.unwrap_or_else(|| DEFAULT_API_URL.to_string());
        let timeout = self.timeout.unwrap_or(DEFAULT_TIMEOUT);

        // Build headers
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {api_key}"))
                .map_err(|e| Error::config(format!("Invalid API key: {e}")))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // Build HTTP client with optimized connection pooling
        // Note: We apply HTTP client optimizations (pool size, keepalive, etc.) manually
        // because reqwest doesn't allow rebuilding a client with additional headers
        let http_client = reqwest::Client::builder()
            .pool_max_idle_per_host(32) // LLM-optimized connection pooling
            .pool_idle_timeout(Duration::from_secs(90)) // Longer connection reuse
            .tcp_keepalive(Duration::from_secs(60)) // Proactive broken connection detection
            .connect_timeout(Duration::from_secs(10)) // Connection establishment timeout
            .timeout(timeout)
            .default_headers(headers)
            .build()
            .map_err(|e| Error::config(format!("Failed to build HTTP client: {e}")))?;

        Ok(Client {
            inner: Arc::new(ClientInner {
                http_client,
                api_url,
                project_name: self.project_name,
            }),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::run::RunType;

    // ===== ClientBuilder Tests =====

    #[test]
    fn test_client_builder() {
        let client = Client::builder()
            .api_key("test-key")
            .api_url("https://test.example.com")
            .project_name("test-project")
            .timeout(Duration::from_secs(10))
            .build();

        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.api_url(), "https://test.example.com");
        assert_eq!(client.project_name(), Some("test-project"));
    }

    #[test]
    fn test_client_builder_missing_api_key() {
        let result = Client::builder().build();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Config(_)));
    }

    #[test]
    fn test_client_builder_api_key_only() {
        let result = Client::builder().api_key("test-key").build();
        assert!(result.is_ok());
        let client = result.unwrap();
        // Should use default API URL
        assert_eq!(client.api_url(), DEFAULT_API_URL);
        // No project name set
        assert!(client.project_name().is_none());
    }

    #[test]
    fn test_client_builder_api_url_method() {
        let builder = Client::builder().api_url("https://custom.example.com");
        let client = builder.api_key("test-key").build().unwrap();
        assert_eq!(client.api_url(), "https://custom.example.com");
    }

    #[test]
    fn test_client_builder_project_name_method() {
        let client = Client::builder()
            .api_key("test-key")
            .project_name("my-project")
            .build()
            .unwrap();
        assert_eq!(client.project_name(), Some("my-project"));
    }

    #[test]
    fn test_client_builder_timeout_method() {
        // Just ensure it doesn't panic with different timeout values
        let client = Client::builder()
            .api_key("test-key")
            .timeout(Duration::from_millis(500))
            .build();
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_builder_timeout_zero() {
        // Zero timeout should work (though impractical)
        let client = Client::builder()
            .api_key("test-key")
            .timeout(Duration::ZERO)
            .build();
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_builder_timeout_long() {
        let client = Client::builder()
            .api_key("test-key")
            .timeout(Duration::from_secs(3600))
            .build();
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_builder_chaining_order() {
        // Order of builder methods shouldn't matter
        let client1 = Client::builder()
            .api_key("key")
            .api_url("https://a.com")
            .project_name("proj")
            .build()
            .unwrap();

        let client2 = Client::builder()
            .project_name("proj")
            .api_url("https://a.com")
            .api_key("key")
            .build()
            .unwrap();

        assert_eq!(client1.api_url(), client2.api_url());
        assert_eq!(client1.project_name(), client2.project_name());
    }

    #[test]
    fn test_client_builder_overwrite_values() {
        let client = Client::builder()
            .api_key("old-key")
            .api_url("https://old.example.com")
            .project_name("old-project")
            .api_key("new-key")
            .api_url("https://new.example.com")
            .project_name("new-project")
            .build()
            .unwrap();

        assert_eq!(client.api_url(), "https://new.example.com");
        assert_eq!(client.project_name(), Some("new-project"));
    }

    #[test]
    fn test_client_builder_default() {
        let builder = ClientBuilder::default();
        // Should fail without API key
        let result = builder.build();
        assert!(result.is_err());
    }

    #[test]
    fn test_client_builder_from_env_no_env_vars() {
        // Clear relevant env vars (save and restore)
        let saved_key = std::env::var(LANGSMITH_API_KEY).ok();
        let saved_lc_key = std::env::var(LANGCHAIN_API_KEY).ok();
        std::env::remove_var(LANGSMITH_API_KEY);
        std::env::remove_var(LANGCHAIN_API_KEY);

        let result = Client::builder().from_env().unwrap().build();
        // Should fail - no API key
        assert!(result.is_err());

        // Restore
        if let Some(key) = saved_key {
            std::env::set_var(LANGSMITH_API_KEY, key);
        }
        if let Some(key) = saved_lc_key {
            std::env::set_var(LANGCHAIN_API_KEY, key);
        }
    }

    #[test]
    fn test_client_builder_from_env_with_langsmith_key() {
        let saved = std::env::var(LANGSMITH_API_KEY).ok();
        std::env::set_var(LANGSMITH_API_KEY, "test-env-key");

        let result = Client::builder().from_env().unwrap().build();
        assert!(result.is_ok());

        // Restore
        if let Some(key) = saved {
            std::env::set_var(LANGSMITH_API_KEY, key);
        } else {
            std::env::remove_var(LANGSMITH_API_KEY);
        }
    }

    #[test]
    fn test_client_builder_from_env_with_langchain_key_fallback() {
        let saved_ls = std::env::var(LANGSMITH_API_KEY).ok();
        let saved_lc = std::env::var(LANGCHAIN_API_KEY).ok();
        std::env::remove_var(LANGSMITH_API_KEY);
        std::env::set_var(LANGCHAIN_API_KEY, "langchain-key");

        let result = Client::builder().from_env().unwrap().build();
        assert!(result.is_ok());

        // Restore
        if let Some(key) = saved_ls {
            std::env::set_var(LANGSMITH_API_KEY, key);
        }
        if let Some(key) = saved_lc {
            std::env::set_var(LANGCHAIN_API_KEY, key);
        } else {
            std::env::remove_var(LANGCHAIN_API_KEY);
        }
    }

    #[test]
    fn test_client_builder_explicit_key_overrides_env() {
        let saved = std::env::var(LANGSMITH_API_KEY).ok();
        std::env::set_var(LANGSMITH_API_KEY, "env-key");

        // Explicit key set before from_env should not be overwritten
        let client = Client::builder()
            .api_key("explicit-key")
            .from_env()
            .unwrap()
            .build()
            .unwrap();
        // The client should be built (can't verify which key was used without mocking HTTP)
        assert_eq!(client.api_url(), DEFAULT_API_URL);

        // Restore
        if let Some(key) = saved {
            std::env::set_var(LANGSMITH_API_KEY, key);
        } else {
            std::env::remove_var(LANGSMITH_API_KEY);
        }
    }

    // ===== Client Accessor Tests =====

    #[test]
    fn test_client_api_url_accessor() {
        let client = Client::builder()
            .api_key("key")
            .api_url("https://api.test.com")
            .build()
            .unwrap();
        assert_eq!(client.api_url(), "https://api.test.com");
    }

    #[test]
    fn test_client_project_name_accessor_some() {
        let client = Client::builder()
            .api_key("key")
            .project_name("test-proj")
            .build()
            .unwrap();
        assert_eq!(client.project_name(), Some("test-proj"));
    }

    #[test]
    fn test_client_project_name_accessor_none() {
        let client = Client::builder().api_key("key").build().unwrap();
        assert!(client.project_name().is_none());
    }

    #[test]
    fn test_client_clone() {
        let client = Client::builder()
            .api_key("key")
            .api_url("https://clone.test.com")
            .project_name("clone-proj")
            .build()
            .unwrap();

        let cloned = client.clone();
        assert_eq!(client.api_url(), cloned.api_url());
        assert_eq!(client.project_name(), cloned.project_name());
    }

    #[test]
    fn test_client_debug() {
        let client = Client::builder()
            .api_key("key")
            .project_name("debug-proj")
            .build()
            .unwrap();

        let debug = format!("{:?}", client);
        assert!(debug.contains("Client"));
        // Should not contain the API key in debug output for security
    }

    // ===== BatchIngest Tests =====

    #[test]
    fn test_batch_ingest() {
        let mut batch = BatchIngest::new();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);

        let run = RunCreate::new(Uuid::new_v4(), "test", RunType::Chain);
        batch.add_create(run);

        assert!(!batch.is_empty());
        assert_eq!(batch.len(), 1);

        let update = RunUpdate::new();
        batch.add_update(Uuid::new_v4(), update);

        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn test_batch_ingest_new() {
        let batch = BatchIngest::new();
        assert!(batch.post.is_empty());
        assert!(batch.patch.is_empty());
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
    }

    #[test]
    fn test_batch_ingest_default() {
        let batch = BatchIngest::default();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
    }

    #[test]
    fn test_batch_ingest_add_create() {
        let mut batch = BatchIngest::new();
        let run1 = RunCreate::new(Uuid::new_v4(), "run1", RunType::Llm);
        let run2 = RunCreate::new(Uuid::new_v4(), "run2", RunType::Tool);

        batch.add_create(run1);
        assert_eq!(batch.post.len(), 1);
        assert_eq!(batch.len(), 1);

        batch.add_create(run2);
        assert_eq!(batch.post.len(), 2);
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn test_batch_ingest_add_update() {
        let mut batch = BatchIngest::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        batch.add_update(id1, RunUpdate::new());
        assert_eq!(batch.patch.len(), 1);
        assert_eq!(batch.len(), 1);

        batch.add_update(id2, RunUpdate::new().with_error("err"));
        assert_eq!(batch.patch.len(), 2);
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn test_batch_ingest_mixed_creates_and_updates() {
        let mut batch = BatchIngest::new();

        batch.add_create(RunCreate::new(Uuid::new_v4(), "run", RunType::Chain));
        batch.add_update(Uuid::new_v4(), RunUpdate::new());
        batch.add_create(RunCreate::new(Uuid::new_v4(), "run2", RunType::Llm));
        batch.add_update(Uuid::new_v4(), RunUpdate::new());

        assert_eq!(batch.post.len(), 2);
        assert_eq!(batch.patch.len(), 2);
        assert_eq!(batch.len(), 4);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_batch_ingest_is_empty() {
        let mut batch = BatchIngest::new();
        assert!(batch.is_empty());

        batch.add_create(RunCreate::new(Uuid::new_v4(), "run", RunType::Chain));
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_batch_ingest_len() {
        let mut batch = BatchIngest::new();
        assert_eq!(batch.len(), 0);

        for i in 0..5 {
            batch.add_create(RunCreate::new(Uuid::new_v4(), format!("run{i}"), RunType::Llm));
        }
        assert_eq!(batch.len(), 5);

        for _ in 0..3 {
            batch.add_update(Uuid::new_v4(), RunUpdate::new());
        }
        assert_eq!(batch.len(), 8);
    }

    #[test]
    fn test_batch_ingest_clone() {
        let mut batch = BatchIngest::new();
        batch.add_create(RunCreate::new(Uuid::new_v4(), "run", RunType::Chain));
        batch.add_update(Uuid::new_v4(), RunUpdate::new());

        let cloned = batch.clone();
        assert_eq!(batch.len(), cloned.len());
        assert_eq!(batch.post.len(), cloned.post.len());
        assert_eq!(batch.patch.len(), cloned.patch.len());
    }

    #[test]
    fn test_batch_ingest_debug() {
        let batch = BatchIngest::new();
        let debug = format!("{:?}", batch);
        assert!(debug.contains("BatchIngest"));
    }

    #[test]
    fn test_batch_ingest_serialization() {
        let mut batch = BatchIngest::new();
        let run_id = Uuid::new_v4();
        batch.add_create(RunCreate::new(run_id, "test-run", RunType::Llm));

        let json = serde_json::to_string(&batch).unwrap();
        assert!(json.contains("post"));
        assert!(json.contains("patch"));
        assert!(json.contains("test-run"));

        let deserialized: BatchIngest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.post.len(), 1);
        assert_eq!(deserialized.patch.len(), 0);
    }

    #[test]
    fn test_batch_ingest_deserialization() {
        let json = r#"{"post":[],"patch":[]}"#;
        let batch: BatchIngest = serde_json::from_str(json).unwrap();
        assert!(batch.is_empty());
    }

    // ===== Constants Tests =====

    #[test]
    fn test_default_api_url_constant() {
        assert!(!DEFAULT_API_URL.is_empty());
        assert!(DEFAULT_API_URL.starts_with("https://"));
    }

    #[test]
    fn test_default_timeout_constant() {
        assert!(DEFAULT_TIMEOUT > Duration::ZERO);
        assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(30));
    }

    // ===== env_string Helper Tests =====

    #[test]
    fn test_env_string_exists() {
        std::env::set_var("TEST_LANGSMITH_VAR", "test_value");
        let result = env_string("TEST_LANGSMITH_VAR");
        assert_eq!(result, Some("test_value".to_string()));
        std::env::remove_var("TEST_LANGSMITH_VAR");
    }

    #[test]
    fn test_env_string_not_exists() {
        std::env::remove_var("NONEXISTENT_VAR_12345");
        let result = env_string("NONEXISTENT_VAR_12345");
        assert!(result.is_none());
    }
}
