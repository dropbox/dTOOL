//! Nomic AI embeddings implementation.
//!
//! This module provides embeddings using Nomic's embedding API.
//! Nomic provides high-quality text embeddings with models:
//! - nomic-embed-text-v1: Original model (768 dimensions)
//! - nomic-embed-text-v1.5: Latest model (768 dimensions, default)
//!
//! # Example
//!
//! ```rust
//! use dashflow_nomic::embeddings::NomicEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = NomicEmbeddings::new()
//!     .with_model("nomic-embed-text-v1.5");
//!
//! // Embed a single query
//! let query_vector = embedder.embed_query("What is the meaning of life?").await?;
//! assert!(!query_vector.is_empty());
//!
//! // Embed multiple documents
//! let docs = vec![
//!     "The quick brown fox".to_string(),
//!     "jumps over the lazy dog".to_string(),
//! ];
//! let doc_vectors = embedder.embed_documents(&docs).await?;
//! assert_eq!(doc_vectors.len(), 2);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use dashflow::core::{
    config_loader::env_vars::{env_string, NOMIC_API_KEY},
    embeddings::Embeddings,
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
    Error as DashFlowError,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Default model for Nomic embeddings
const DEFAULT_MODEL: &str = "nomic-embed-text-v1.5";

/// Nomic API base URL
const NOMIC_API_BASE: &str = "https://api-atlas.nomic.ai";

/// Nomic embedding model integration.
///
/// Nomic provides high-quality text embeddings optimized for semantic search
/// and retrieval tasks. Supports different task types for optimal performance:
/// - `search_document`: For embedding documents in a corpus
/// - `search_query`: For embedding search queries
/// - `classification`: For classification tasks
/// - `clustering`: For clustering tasks
///
/// # Configuration
///
/// The API key can be set via:
/// - Constructor: `NomicEmbeddings::new().with_api_key("nk_...")`
/// - Environment: `NOMIC_API_KEY`
///
/// # Models
///
/// - `nomic-embed-text-v1.5` (default): 768 dimensions, latest model
/// - `nomic-embed-text-v1`: 768 dimensions, original model
///
/// # Task Types
///
/// Nomic embeddings support different task types:
/// - Documents use `search_document` task type
/// - Queries use `search_query` task type
/// - This improves retrieval quality compared to generic embeddings
pub struct NomicEmbeddings {
    /// HTTP client for API requests
    client: Client,
    /// Model name (e.g., "nomic-embed-text-v1.5")
    model: String,
    /// Nomic API key
    api_key: Option<String>,
    /// Embedding dimensionality (optional, for Matryoshka-capable models)
    dimensionality: Option<u32>,
    /// Retry policy for handling transient errors
    retry_policy: RetryPolicy,
    /// Optional rate limiter for controlling request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl NomicEmbeddings {
    /// Try to create a new Nomic embeddings instance with default settings.
    ///
    /// Defaults:
    /// - Model: `nomic-embed-text-v1.5`
    /// - API key: from `NOMIC_API_KEY` environment variable
    ///
    /// # Errors
    ///
    /// Returns an error if `NOMIC_API_KEY` environment variable is not set.
    /// Use `with_api_key()` to set the key explicitly after creation.
    pub fn try_new() -> Result<Self, DashFlowError> {
        let api_key = env_string(NOMIC_API_KEY).ok_or_else(|| {
            DashFlowError::config(format!("{NOMIC_API_KEY} environment variable must be set"))
        })?;

        Ok(Self {
            client: Client::new(),
            model: DEFAULT_MODEL.to_string(),
            api_key: Some(api_key),
            dimensionality: None,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        })
    }

    /// Create a new Nomic embeddings instance with default settings.
    ///
    /// Defaults:
    /// - Model: `nomic-embed-text-v1.5`
    /// - API key: from `NOMIC_API_KEY` environment variable
    ///
    /// # Panics
    ///
    /// Panics if `NOMIC_API_KEY` environment variable is not set.
    /// Use `try_new()` for a fallible constructor, or `with_api_key()` to set the key explicitly.
    #[must_use]
    #[allow(clippy::expect_used)] // Documented panic with try_new() fallible alternative
    pub fn new() -> Self {
        Self::try_new().expect("NOMIC_API_KEY environment variable must be set")
    }

    /// Create a new instance without requiring environment variables.
    ///
    /// You must call `with_api_key()` before using this instance.
    ///
    /// Defaults:
    /// - Model: `nomic-embed-text-v1.5`
    #[must_use]
    pub fn new_without_api_key() -> Self {
        Self {
            client: Client::new(),
            model: DEFAULT_MODEL.to_string(),
            api_key: None,
            dimensionality: None,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Set the API key explicitly instead of using environment variable.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_nomic::embeddings::NomicEmbeddings;
    ///
    /// let embedder = NomicEmbeddings::new_without_api_key()
    ///     .with_api_key("nk_your_api_key_here");
    /// ```
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the model to use for embeddings.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_nomic::embeddings::NomicEmbeddings;
    ///
    /// let embedder = NomicEmbeddings::new_without_api_key()
    ///     .with_model("nomic-embed-text-v1");
    /// ```
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the embedding dimensionality (for Matryoshka-capable models).
    ///
    /// This allows you to reduce the embedding size for storage efficiency
    /// while maintaining most of the semantic information.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_nomic::embeddings::NomicEmbeddings;
    ///
    /// let embedder = NomicEmbeddings::new_without_api_key()
    ///     .with_dimensionality(512);  // Reduce from 768 to 512
    /// ```
    #[must_use]
    pub fn with_dimensionality(mut self, dimensionality: u32) -> Self {
        self.dimensionality = Some(dimensionality);
        self
    }

    /// Set custom retry policy for handling transient errors.
    ///
    /// By default, uses exponential backoff with 3 retries. This method allows
    /// you to customize the retry behavior.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_nomic::embeddings::NomicEmbeddings;
    /// use dashflow::core::retry::RetryPolicy;
    ///
    /// // Custom retry policy: 5 retries with exponential backoff
    /// let embedder = NomicEmbeddings::new_without_api_key()
    ///     .with_api_key("nk_your_api_key")
    ///     .with_retry_policy(RetryPolicy::exponential(5));
    ///
    /// // Disable retries
    /// let embedder_no_retry = NomicEmbeddings::new_without_api_key()
    ///     .with_api_key("nk_your_api_key")
    ///     .with_retry_policy(RetryPolicy::no_retry());
    /// ```
    #[must_use]
    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    /// Set a rate limiter to control request rate to the Nomic API.
    ///
    /// This is useful to avoid hitting rate limits, especially with free-tier
    /// API keys that have strict rate limits.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_nomic::embeddings::NomicEmbeddings;
    /// use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// use std::sync::Arc;
    /// use std::time::Duration;
    ///
    /// // Limit to 10 requests per second
    /// let rate_limiter = Arc::new(InMemoryRateLimiter::new(
    ///     10.0,
    ///     Duration::from_millis(100),
    ///     20.0,
    /// ));
    ///
    /// let embedder = NomicEmbeddings::new_without_api_key()
    ///     .with_api_key("nk_your_api_key")
    ///     .with_rate_limiter(rate_limiter);
    /// ```
    #[must_use]
    pub fn with_rate_limiter(mut self, rate_limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(rate_limiter);
        self
    }

    /// Internal method to call the Nomic API for embeddings.
    async fn embed_with_task_type(
        &self,
        texts: &[String],
        task_type: &str,
    ) -> Result<Vec<Vec<f32>>, DashFlowError> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| DashFlowError::invalid_input("API key not set"))?;

        // Acquire rate limiter token if configured
        if let Some(ref limiter) = self.rate_limiter {
            limiter.acquire().await;
        }

        let url = format!("{NOMIC_API_BASE}/v1/embedding/text");

        let request_body = EmbedRequest {
            texts: texts.to_vec(),
            model: self.model.clone(),
            task_type: task_type.to_string(),
            dimensionality: self.dimensionality,
        };

        // Wrap API call with retry logic
        let client = self.client.clone();
        let api_key = api_key.clone();
        let response = with_retry(&self.retry_policy, || async {
            let response = client
                .post(&url)
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&request_body)
                .send()
                .await
                .map_err(|e| DashFlowError::http(e.to_string()))?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                return Err(DashFlowError::http(format!(
                    "Nomic API error ({status}): {error_text}"
                )));
            }

            Ok(response)
        })
        .await?;

        let embed_response: EmbedResponse = response
            .json()
            .await
            .map_err(|e| DashFlowError::api_format(format!("Failed to parse response: {e}")))?;

        Ok(embed_response.embeddings)
    }
}

impl Default for NomicEmbeddings {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Embeddings for NomicEmbeddings {
    /// Embed a list of documents.
    ///
    /// Uses the `search_document` task type for optimal document embedding.
    ///
    /// # Arguments
    ///
    /// * `texts` - Slice of strings to embed
    ///
    /// # Returns
    ///
    /// A vector of embeddings, one per input text.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, DashFlowError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        self.embed_with_task_type(texts, "search_document").await
    }

    /// Embed a single query string.
    ///
    /// Uses the `search_query` task type for optimal query embedding.
    ///
    /// # Arguments
    ///
    /// * `text` - The query string to embed
    ///
    /// # Returns
    ///
    /// A single embedding vector.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, DashFlowError> {
        let texts = vec![text.to_string()];
        let mut embeddings = self.embed_with_task_type(&texts, "search_query").await?;
        embeddings
            .pop()
            .ok_or_else(|| DashFlowError::api_format("No embedding returned from API"))
    }
}

/// Request body for Nomic embedding API
#[derive(Debug, Serialize)]
struct EmbedRequest {
    texts: Vec<String>,
    model: String,
    task_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensionality: Option<u32>,
}

/// Response from Nomic embedding API
#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json;

    // ============================================================================
    // Constants Tests
    // ============================================================================

    #[test]
    fn test_default_model_constant() {
        assert_eq!(DEFAULT_MODEL, "nomic-embed-text-v1.5");
    }

    #[test]
    fn test_api_base_constant() {
        assert_eq!(NOMIC_API_BASE, "https://api-atlas.nomic.ai");
        assert!(NOMIC_API_BASE.starts_with("https://"));
    }

    // ============================================================================
    // Builder Pattern Tests
    // ============================================================================

    #[test]
    fn test_builder_pattern() {
        let embedder = NomicEmbeddings::new_without_api_key()
            .with_api_key("test_api_key")
            .with_model("nomic-embed-text-v1")
            .with_dimensionality(512);

        assert_eq!(embedder.model, "nomic-embed-text-v1");
        assert_eq!(embedder.dimensionality, Some(512));
    }

    #[test]
    fn test_default_values() {
        let embedder = NomicEmbeddings::new_without_api_key();

        assert_eq!(embedder.model, "nomic-embed-text-v1.5");
        assert_eq!(embedder.dimensionality, None);
    }

    #[test]
    fn test_builder_with_api_key_only() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("my_api_key");

        assert_eq!(embedder.api_key, Some("my_api_key".to_string()));
        assert_eq!(embedder.model, DEFAULT_MODEL);
        assert_eq!(embedder.dimensionality, None);
    }

    #[test]
    fn test_builder_with_model_only() {
        let embedder = NomicEmbeddings::new_without_api_key().with_model("custom-model");

        assert_eq!(embedder.model, "custom-model");
        assert!(embedder.api_key.is_none());
    }

    #[test]
    fn test_builder_with_dimensionality_only() {
        let embedder = NomicEmbeddings::new_without_api_key().with_dimensionality(256);

        assert_eq!(embedder.dimensionality, Some(256));
    }

    #[test]
    fn test_builder_chaining_order_irrelevant() {
        // Order 1: api_key -> model -> dimensionality
        let e1 = NomicEmbeddings::new_without_api_key()
            .with_api_key("key")
            .with_model("model")
            .with_dimensionality(128);

        // Order 2: dimensionality -> model -> api_key
        let e2 = NomicEmbeddings::new_without_api_key()
            .with_dimensionality(128)
            .with_model("model")
            .with_api_key("key");

        assert_eq!(e1.api_key, e2.api_key);
        assert_eq!(e1.model, e2.model);
        assert_eq!(e1.dimensionality, e2.dimensionality);
    }

    #[test]
    fn test_builder_overwrite_values() {
        let embedder = NomicEmbeddings::new_without_api_key()
            .with_api_key("first_key")
            .with_api_key("second_key")
            .with_model("first_model")
            .with_model("second_model");

        assert_eq!(embedder.api_key, Some("second_key".to_string()));
        assert_eq!(embedder.model, "second_model");
    }

    #[test]
    fn test_builder_empty_api_key() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("");

        assert_eq!(embedder.api_key, Some(String::new()));
    }

    #[test]
    fn test_builder_empty_model() {
        let embedder = NomicEmbeddings::new_without_api_key().with_model("");

        assert_eq!(embedder.model, "");
    }

    #[test]
    fn test_builder_zero_dimensionality() {
        let embedder = NomicEmbeddings::new_without_api_key().with_dimensionality(0);

        assert_eq!(embedder.dimensionality, Some(0));
    }

    #[test]
    fn test_builder_large_dimensionality() {
        let embedder = NomicEmbeddings::new_without_api_key().with_dimensionality(u32::MAX);

        assert_eq!(embedder.dimensionality, Some(u32::MAX));
    }

    #[test]
    fn test_builder_unicode_api_key() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("nk_🔑_key_emoji");

        assert_eq!(embedder.api_key, Some("nk_🔑_key_emoji".to_string()));
    }

    #[test]
    fn test_builder_unicode_model() {
        let embedder = NomicEmbeddings::new_without_api_key().with_model("モデル_v1");

        assert_eq!(embedder.model, "モデル_v1");
    }

    // ============================================================================
    // Retry Policy Tests
    // ============================================================================

    #[test]
    fn test_with_retry_policy() {
        use dashflow::core::retry::RetryPolicy;

        let embedder = NomicEmbeddings::new_without_api_key()
            .with_api_key("test_key")
            .with_retry_policy(RetryPolicy::exponential(5));

        // Test passes if no panic (builder pattern works)
        assert_eq!(embedder.model, DEFAULT_MODEL);
    }

    #[test]
    fn test_with_retry_policy_no_retry() {
        use dashflow::core::retry::RetryPolicy;

        let embedder = NomicEmbeddings::new_without_api_key()
            .with_api_key("test_key")
            .with_retry_policy(RetryPolicy::no_retry());

        assert_eq!(embedder.model, DEFAULT_MODEL);
    }

    #[test]
    fn test_with_retry_policy_zero_retries() {
        use dashflow::core::retry::RetryPolicy;

        let embedder = NomicEmbeddings::new_without_api_key()
            .with_api_key("test_key")
            .with_retry_policy(RetryPolicy::exponential(0));

        assert_eq!(embedder.model, DEFAULT_MODEL);
    }

    // ============================================================================
    // Rate Limiter Tests
    // ============================================================================

    #[test]
    fn test_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::sync::Arc;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));
        let embedder = NomicEmbeddings::new_without_api_key()
            .with_api_key("test_key")
            .with_rate_limiter(rate_limiter);

        // Test passes if no panic (builder pattern works)
        assert_eq!(embedder.model, DEFAULT_MODEL);
        assert!(embedder.rate_limiter.is_some());
    }

    #[test]
    fn test_without_rate_limiter() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("test_key");

        assert!(embedder.rate_limiter.is_none());
    }

    // ============================================================================
    // Async Tests - Deterministic Cases
    // ============================================================================

    #[tokio::test]
    async fn test_empty_input() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("test_api_key");
        let result = embedder._embed_documents(&[]).await.unwrap();
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_embed_documents_empty_returns_empty_vec() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("test_key");
        let result = embedder._embed_documents(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_embed_without_api_key_fails() {
        let embedder = NomicEmbeddings::new_without_api_key();
        let result = embedder._embed_query("test").await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("API key") || err_msg.contains("not set"),
            "Error should mention API key: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_embed_documents_without_api_key_fails() {
        let embedder = NomicEmbeddings::new_without_api_key();
        let texts = vec!["test document".to_string()];
        let result = embedder._embed_documents(&texts).await;
        assert!(result.is_err());
    }

    // ============================================================================
    // Serialization Tests - EmbedRequest
    // ============================================================================

    #[test]
    fn test_embed_request_serialization_minimal() {
        let request = EmbedRequest {
            texts: vec!["hello".to_string()],
            model: "nomic-embed-text-v1.5".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"texts\""));
        assert!(json.contains("\"model\""));
        assert!(json.contains("\"task_type\""));
        // dimensionality should be skipped when None
        assert!(!json.contains("\"dimensionality\""));
    }

    #[test]
    fn test_embed_request_serialization_with_dimensionality() {
        let request = EmbedRequest {
            texts: vec!["hello".to_string()],
            model: "nomic-embed-text-v1.5".to_string(),
            task_type: "search_document".to_string(),
            dimensionality: Some(512),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"dimensionality\":512"));
    }

    #[test]
    fn test_embed_request_multiple_texts() {
        let request = EmbedRequest {
            texts: vec![
                "first".to_string(),
                "second".to_string(),
                "third".to_string(),
            ],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"first\""));
        assert!(json.contains("\"second\""));
        assert!(json.contains("\"third\""));
    }

    #[test]
    fn test_embed_request_empty_texts() {
        let request = EmbedRequest {
            texts: vec![],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"texts\":[]"));
    }

    #[test]
    fn test_embed_request_unicode_texts() {
        let request = EmbedRequest {
            texts: vec!["你好世界".to_string(), "مرحبا".to_string(), "🌍🌎🌏".to_string()],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("你好世界"));
        assert!(json.contains("مرحبا"));
        // Emojis are serialized as unicode escapes or raw, both valid
        assert!(json.contains("🌍") || json.contains("\\u"));
    }

    #[test]
    fn test_embed_request_task_types() {
        let task_types = ["search_query", "search_document", "classification", "clustering"];

        for task_type in task_types {
            let request = EmbedRequest {
                texts: vec!["test".to_string()],
                model: "test-model".to_string(),
                task_type: task_type.to_string(),
                dimensionality: None,
            };

            let json = serde_json::to_string(&request).unwrap();
            assert!(json.contains(task_type));
        }
    }

    // ============================================================================
    // Serialization Tests - EmbedResponse
    // ============================================================================

    #[test]
    fn test_embed_response_deserialization() {
        let json = r#"{"embeddings":[[0.1,0.2,0.3],[0.4,0.5,0.6]]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.embeddings.len(), 2);
        assert_eq!(response.embeddings[0], vec![0.1, 0.2, 0.3]);
        assert_eq!(response.embeddings[1], vec![0.4, 0.5, 0.6]);
    }

    #[test]
    fn test_embed_response_empty_embeddings() {
        let json = r#"{"embeddings":[]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert!(response.embeddings.is_empty());
    }

    #[test]
    fn test_embed_response_single_embedding() {
        let json = r#"{"embeddings":[[1.0,2.0,3.0,4.0]]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.embeddings.len(), 1);
        assert_eq!(response.embeddings[0].len(), 4);
    }

    #[test]
    fn test_embed_response_high_dimensional() {
        // Simulate a 768-dimensional embedding (typical for Nomic)
        let values: Vec<f32> = (0..768).map(|i| i as f32 * 0.001).collect();
        let json = format!("{{\"embeddings\":[{}]}}", serde_json::to_string(&values).unwrap());
        let response: EmbedResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(response.embeddings.len(), 1);
        assert_eq!(response.embeddings[0].len(), 768);
    }

    #[test]
    fn test_embed_response_special_float_values() {
        let json = r#"{"embeddings":[[0.0,-0.0,1e-10,-1e10]]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.embeddings[0][0], 0.0);
        assert_eq!(response.embeddings[0][1], -0.0);
        assert!((response.embeddings[0][2] - 1e-10).abs() < 1e-15);
    }

    // ============================================================================
    // try_new Tests
    // ============================================================================

    #[test]
    fn test_try_new_without_env_var_fails() {
        // Temporarily ensure the env var is not set
        std::env::remove_var("NOMIC_API_KEY");

        let result = NomicEmbeddings::try_new();
        assert!(result.is_err());
        // Use match instead of unwrap_err() since NomicEmbeddings doesn't impl Debug
        match result {
            Ok(_) => panic!("Expected error but got Ok"),
            Err(e) => {
                let err_msg = e.to_string();
                assert!(
                    err_msg.contains("NOMIC_API_KEY"),
                    "Error should mention NOMIC_API_KEY: {}",
                    err_msg
                );
            }
        }
    }

    // ============================================================================
    // Debug Trait Tests
    // ============================================================================

    #[test]
    fn test_embed_request_debug() {
        let request = EmbedRequest {
            texts: vec!["test".to_string()],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: Some(256),
        };

        let debug_str = format!("{:?}", request);
        assert!(debug_str.contains("EmbedRequest"));
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("test-model"));
    }

    #[test]
    fn test_embed_response_debug() {
        let response = EmbedResponse {
            embeddings: vec![vec![1.0, 2.0, 3.0]],
        };

        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("EmbedResponse"));
        assert!(debug_str.contains("embeddings"));
    }

    // ============================================================================
    // Integration Tests (require API key - ignored by default)
    // ============================================================================

    #[tokio::test]
    #[ignore = "requires NOMIC_API_KEY"]
    async fn test_embed_query() {
        let embedder = NomicEmbeddings::new();
        let embedding = embedder._embed_query("Hello, world!").await.unwrap();

        // nomic-embed-text-v1.5 produces 768-dimensional embeddings
        assert_eq!(embedding.len(), 768);

        // Check that embeddings contain non-zero values
        let sum: f32 = embedding.iter().sum();
        assert!(sum.abs() > 0.0, "Embedding should contain non-zero values");
    }

    #[tokio::test]
    #[ignore = "requires NOMIC_API_KEY"]
    async fn test_embed_documents() {
        let embedder = NomicEmbeddings::new();
        let texts = vec![
            "The quick brown fox".to_string(),
            "jumps over the lazy dog".to_string(),
        ];

        let embeddings = embedder._embed_documents(&texts).await.unwrap();

        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 768);
        assert_eq!(embeddings[1].len(), 768);

        // Check that embeddings are different
        let are_different = embeddings[0]
            .iter()
            .zip(embeddings[1].iter())
            .any(|(a, b)| (a - b).abs() > 1e-6);
        assert!(
            are_different,
            "Embeddings should be different for different texts"
        );
    }

    #[tokio::test]
    #[ignore = "requires NOMIC_API_KEY"]
    async fn test_different_model() {
        let embedder = NomicEmbeddings::new().with_model("nomic-embed-text-v1");

        let embedding = embedder._embed_query("Test").await.unwrap();

        // nomic-embed-text-v1 also produces 768-dimensional embeddings
        assert_eq!(embedding.len(), 768);
    }

    #[tokio::test]
    #[ignore = "requires NOMIC_API_KEY"]
    async fn test_dimensionality_reduction() {
        let embedder = NomicEmbeddings::new().with_dimensionality(512);

        let embedding = embedder._embed_query("Test").await.unwrap();

        // Should produce 512-dimensional embeddings
        assert_eq!(embedding.len(), 512);
    }

    // ============================================================================
    // Extended Builder Pattern Tests
    // ============================================================================

    #[test]
    fn test_builder_with_special_chars_api_key() {
        let embedder =
            NomicEmbeddings::new_without_api_key().with_api_key("nk_!@#$%^&*()_+-=[]{}|;':\",./<>?");
        assert!(embedder.api_key.is_some());
    }

    #[test]
    fn test_builder_with_whitespace_api_key() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("nk_key with spaces");
        assert_eq!(
            embedder.api_key,
            Some("nk_key with spaces".to_string())
        );
    }

    #[test]
    fn test_builder_with_newline_api_key() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("nk_key\nwith\nnewlines");
        assert_eq!(
            embedder.api_key,
            Some("nk_key\nwith\nnewlines".to_string())
        );
    }

    #[test]
    fn test_builder_with_tab_api_key() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("nk_key\twith\ttabs");
        assert_eq!(
            embedder.api_key,
            Some("nk_key\twith\ttabs".to_string())
        );
    }

    #[test]
    fn test_builder_with_long_api_key() {
        let long_key = "nk_".to_string() + &"x".repeat(10000);
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key(long_key.clone());
        assert_eq!(embedder.api_key, Some(long_key));
    }

    #[test]
    fn test_builder_model_with_version_numbers() {
        let embedder = NomicEmbeddings::new_without_api_key().with_model("nomic-embed-text-v2.0.1");
        assert_eq!(embedder.model, "nomic-embed-text-v2.0.1");
    }

    #[test]
    fn test_builder_model_with_hyphens() {
        let embedder = NomicEmbeddings::new_without_api_key().with_model("my-custom-model-v1-beta");
        assert_eq!(embedder.model, "my-custom-model-v1-beta");
    }

    #[test]
    fn test_builder_model_with_underscores() {
        let embedder = NomicEmbeddings::new_without_api_key().with_model("my_custom_model_v1");
        assert_eq!(embedder.model, "my_custom_model_v1");
    }

    #[test]
    fn test_builder_dimensionality_boundary_values() {
        // Test dimensionality at various boundaries
        for dim in [1, 64, 128, 256, 384, 512, 768, 1024, 1536, 2048, 4096] {
            let embedder = NomicEmbeddings::new_without_api_key().with_dimensionality(dim);
            assert_eq!(embedder.dimensionality, Some(dim));
        }
    }

    #[test]
    fn test_builder_multiple_rate_limiter_overwrites() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::sync::Arc;
        use std::time::Duration;

        let limiter1 = Arc::new(InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0));
        let limiter2 = Arc::new(InMemoryRateLimiter::new(20.0, Duration::from_millis(50), 40.0));

        let embedder = NomicEmbeddings::new_without_api_key()
            .with_rate_limiter(limiter1)
            .with_rate_limiter(limiter2.clone());

        // Should have the second rate limiter
        assert!(embedder.rate_limiter.is_some());
    }

    #[test]
    fn test_builder_api_key_from_string() {
        let key = String::from("nk_string_key");
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key(key);
        assert_eq!(embedder.api_key, Some("nk_string_key".to_string()));
    }

    #[test]
    fn test_builder_api_key_from_str() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("nk_str_key");
        assert_eq!(embedder.api_key, Some("nk_str_key".to_string()));
    }

    #[test]
    fn test_builder_model_from_string() {
        let model = String::from("custom-model");
        let embedder = NomicEmbeddings::new_without_api_key().with_model(model);
        assert_eq!(embedder.model, "custom-model");
    }

    // ============================================================================
    // Extended EmbedRequest Serialization Tests
    // ============================================================================

    #[test]
    fn test_embed_request_special_chars_in_texts() {
        let request = EmbedRequest {
            texts: vec![
                "text with \"quotes\"".to_string(),
                "text with \\backslash".to_string(),
                "text with /forward/slash".to_string(),
            ],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        // JSON should properly escape these characters
        assert!(json.contains("\\\"quotes\\\""));
        assert!(json.contains("\\\\backslash"));
    }

    #[test]
    fn test_embed_request_newlines_in_texts() {
        let request = EmbedRequest {
            texts: vec!["line1\nline2\nline3".to_string()],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        // Newlines should be escaped as \n in JSON
        assert!(json.contains("\\n"));
    }

    #[test]
    fn test_embed_request_tabs_in_texts() {
        let request = EmbedRequest {
            texts: vec!["col1\tcol2\tcol3".to_string()],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        // Tabs should be escaped as \t in JSON
        assert!(json.contains("\\t"));
    }

    #[test]
    fn test_embed_request_very_long_text() {
        let long_text = "x".repeat(100_000);
        let request = EmbedRequest {
            texts: vec![long_text.clone()],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.len() > 100_000);
    }

    #[test]
    fn test_embed_request_many_texts() {
        let texts: Vec<String> = (0..1000).map(|i| format!("text_{}", i)).collect();
        let request = EmbedRequest {
            texts,
            model: "test-model".to_string(),
            task_type: "search_document".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("text_0"));
        assert!(json.contains("text_999"));
    }

    #[test]
    fn test_embed_request_mixed_unicode_and_ascii() {
        let request = EmbedRequest {
            texts: vec!["Hello 世界 🌍 Привет مرحبا".to_string()],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Hello"));
        assert!(json.contains("世界"));
    }

    #[test]
    fn test_embed_request_null_byte_in_text() {
        // Null bytes should be handled (may be escaped or cause issues)
        let request = EmbedRequest {
            texts: vec!["text\x00with\x00nulls".to_string()],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        // Should not panic during serialization
        let result = serde_json::to_string(&request);
        // Null bytes in JSON strings should be escaped
        assert!(result.is_ok());
    }

    #[test]
    fn test_embed_request_control_characters() {
        let request = EmbedRequest {
            texts: vec![
                "text\x01\x02\x03\x04".to_string(),
                "more\x1f\x1e\x1d".to_string(),
            ],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let result = serde_json::to_string(&request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_embed_request_whitespace_only_texts() {
        let request = EmbedRequest {
            texts: vec![
                "   ".to_string(),
                "\t\t\t".to_string(),
                "\n\n\n".to_string(),
            ],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("texts"));
    }

    #[test]
    fn test_embed_request_dimensionality_zero() {
        let request = EmbedRequest {
            texts: vec!["test".to_string()],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: Some(0),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"dimensionality\":0"));
    }

    #[test]
    fn test_embed_request_dimensionality_max() {
        let request = EmbedRequest {
            texts: vec!["test".to_string()],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: Some(u32::MAX),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(&u32::MAX.to_string()));
    }

    // ============================================================================
    // Extended EmbedResponse Deserialization Tests
    // ============================================================================

    #[test]
    fn test_embed_response_negative_floats() {
        let json = r#"{"embeddings":[[-0.5,-1.0,-100.0]]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.embeddings[0][0], -0.5);
        assert_eq!(response.embeddings[0][1], -1.0);
        assert_eq!(response.embeddings[0][2], -100.0);
    }

    #[test]
    fn test_embed_response_scientific_notation() {
        let json = r#"{"embeddings":[[1e5,-2.5e-3,3.14159e0]]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert!((response.embeddings[0][0] - 100000.0).abs() < 1e-10);
        assert!((response.embeddings[0][1] - (-0.0025)).abs() < 1e-10);
    }

    #[test]
    fn test_embed_response_very_small_floats() {
        let json = r#"{"embeddings":[[1e-38,1e-30,1e-20]]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert!(response.embeddings[0][0] > 0.0);
        assert!(response.embeddings[0][0] < 1e-37);
    }

    #[test]
    fn test_embed_response_very_large_floats() {
        let json = r#"{"embeddings":[[1e30,1e35,1e38]]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert!(response.embeddings[0][0] > 1e29);
    }

    #[test]
    fn test_embed_response_mixed_positive_negative() {
        let json = r#"{"embeddings":[[-0.1,0.2,-0.3,0.4,-0.5]]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert!(response.embeddings[0][0] < 0.0);
        assert!(response.embeddings[0][1] > 0.0);
        assert!(response.embeddings[0][2] < 0.0);
        assert!(response.embeddings[0][3] > 0.0);
        assert!(response.embeddings[0][4] < 0.0);
    }

    #[test]
    fn test_embed_response_multiple_embeddings_different_sizes() {
        // Nomic API typically returns same size, but test parser flexibility
        let json = r#"{"embeddings":[[1.0,2.0],[3.0,4.0,5.0],[6.0]]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.embeddings.len(), 3);
        assert_eq!(response.embeddings[0].len(), 2);
        assert_eq!(response.embeddings[1].len(), 3);
        assert_eq!(response.embeddings[2].len(), 1);
    }

    #[test]
    fn test_embed_response_single_value_embeddings() {
        let json = r#"{"embeddings":[[42.0]]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.embeddings.len(), 1);
        assert_eq!(response.embeddings[0].len(), 1);
        assert_eq!(response.embeddings[0][0], 42.0);
    }

    #[test]
    fn test_embed_response_integer_values_as_floats() {
        // JSON integers should be parsed as f32
        let json = r#"{"embeddings":[[1,2,3,4,5]]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.embeddings[0], vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn test_embed_response_extra_fields_ignored() {
        // Parser should ignore unknown fields
        let json = r#"{"embeddings":[[1.0,2.0]],"unknown_field":"value","count":42}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.embeddings.len(), 1);
    }

    #[test]
    fn test_embed_response_whitespace_tolerant() {
        let json = r#"{
            "embeddings": [
                [ 1.0, 2.0, 3.0 ],
                [ 4.0, 5.0, 6.0 ]
            ]
        }"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.embeddings.len(), 2);
    }

    // ============================================================================
    // Error Handling Tests
    // ============================================================================

    #[tokio::test]
    async fn test_embed_query_empty_string() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("test_key");
        // Empty string should still call API (behavior depends on API)
        // Without real API, this tests internal handling
        let result = embedder._embed_query("").await;
        // Should fail due to mock/no real API, but shouldn't panic
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_embed_documents_single_empty_string() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("test_key");
        let result = embedder._embed_documents(&["".to_string()]).await;
        // Network error expected without real API
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_embed_query_whitespace_only() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("test_key");
        let result = embedder._embed_query("   \t\n   ").await;
        // Should attempt API call (whitespace is valid input)
        assert!(result.is_err()); // Network error without real API
    }

    #[tokio::test]
    async fn test_embed_documents_mixed_empty_and_content() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("test_key");
        let texts = vec![
            "".to_string(),
            "real content".to_string(),
            "".to_string(),
        ];
        let result = embedder._embed_documents(&texts).await;
        assert!(result.is_err()); // Network error without real API
    }

    // ============================================================================
    // Constants and URL Construction Tests
    // ============================================================================

    #[test]
    fn test_nomic_api_base_is_https() {
        assert!(NOMIC_API_BASE.starts_with("https://"));
    }

    #[test]
    fn test_nomic_api_base_no_trailing_slash() {
        assert!(!NOMIC_API_BASE.ends_with('/'));
    }

    #[test]
    fn test_default_model_follows_naming_convention() {
        // Nomic models follow pattern: nomic-embed-text-vX.Y
        assert!(DEFAULT_MODEL.starts_with("nomic-embed-text-v"));
        assert!(DEFAULT_MODEL.contains('.'));
    }

    #[test]
    fn test_api_url_construction() {
        let expected = format!("{}/v1/embedding/text", NOMIC_API_BASE);
        assert_eq!(expected, "https://api-atlas.nomic.ai/v1/embedding/text");
    }

    // ============================================================================
    // Struct Field Access Tests
    // ============================================================================

    #[test]
    fn test_embedder_fields_accessible() {
        let embedder = NomicEmbeddings::new_without_api_key()
            .with_api_key("test_key")
            .with_model("custom-model")
            .with_dimensionality(512);

        assert_eq!(embedder.model, "custom-model");
        assert_eq!(embedder.dimensionality, Some(512));
        assert_eq!(embedder.api_key, Some("test_key".to_string()));
    }

    #[test]
    fn test_embedder_client_created() {
        let embedder = NomicEmbeddings::new_without_api_key();
        // Client is private but should be initialized (no panic on creation)
        let _ = embedder.model; // Just access another field to confirm construction
    }

    // ============================================================================
    // Task Type Tests
    // ============================================================================

    #[test]
    fn test_all_task_types_serializable() {
        let task_types = [
            "search_query",
            "search_document",
            "classification",
            "clustering",
        ];

        for task_type in task_types {
            let request = EmbedRequest {
                texts: vec!["test".to_string()],
                model: "test-model".to_string(),
                task_type: task_type.to_string(),
                dimensionality: None,
            };

            let json = serde_json::to_string(&request);
            assert!(json.is_ok(), "Failed to serialize task_type: {}", task_type);
        }
    }

    #[test]
    fn test_custom_task_type_allowed() {
        // API may support custom task types in future
        let request = EmbedRequest {
            texts: vec!["test".to_string()],
            model: "test-model".to_string(),
            task_type: "custom_task_type_v2".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("custom_task_type_v2"));
    }

    // ============================================================================
    // Clone and Equality Tests for Internal Structs
    // ============================================================================

    #[test]
    fn test_embed_request_fields_match() {
        let request1 = EmbedRequest {
            texts: vec!["a".to_string(), "b".to_string()],
            model: "model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: Some(256),
        };

        let request2 = EmbedRequest {
            texts: vec!["a".to_string(), "b".to_string()],
            model: "model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: Some(256),
        };

        // Compare via serialization
        let json1 = serde_json::to_string(&request1).unwrap();
        let json2 = serde_json::to_string(&request2).unwrap();
        assert_eq!(json1, json2);
    }

    #[test]
    fn test_embed_response_field_access() {
        let response = EmbedResponse {
            embeddings: vec![vec![1.0, 2.0], vec![3.0, 4.0]],
        };

        assert_eq!(response.embeddings.len(), 2);
        assert_eq!(response.embeddings[0][0], 1.0);
        assert_eq!(response.embeddings[1][1], 4.0);
    }

    // ============================================================================
    // Retry Policy Variations Tests
    // ============================================================================

    #[test]
    fn test_retry_policy_exponential_various_counts() {
        use dashflow::core::retry::RetryPolicy;

        for count in [1, 2, 3, 5, 10] {
            let embedder = NomicEmbeddings::new_without_api_key()
                .with_retry_policy(RetryPolicy::exponential(count));
            // Should not panic
            assert_eq!(embedder.model, DEFAULT_MODEL);
        }
    }

    // ============================================================================
    // Integration-style Tests (API key required but deterministic setup)
    // ============================================================================

    #[tokio::test]
    async fn test_full_builder_chain() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use dashflow::core::retry::RetryPolicy;
        use std::sync::Arc;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0));

        let embedder = NomicEmbeddings::new_without_api_key()
            .with_api_key("test_key")
            .with_model("nomic-embed-text-v1.5")
            .with_dimensionality(512)
            .with_retry_policy(RetryPolicy::exponential(5))
            .with_rate_limiter(rate_limiter);

        // Verify all settings applied
        assert_eq!(embedder.api_key, Some("test_key".to_string()));
        assert_eq!(embedder.model, "nomic-embed-text-v1.5");
        assert_eq!(embedder.dimensionality, Some(512));
        assert!(embedder.rate_limiter.is_some());
    }

    #[tokio::test]
    async fn test_embedder_multiple_empty_document_calls() {
        let embedder = NomicEmbeddings::new_without_api_key().with_api_key("test_key");

        // Multiple calls with empty input should all succeed
        for _ in 0..5 {
            let result = embedder._embed_documents(&[]).await;
            assert!(result.is_ok());
            assert!(result.unwrap().is_empty());
        }
    }

    // ============================================================================
    // Unicode Edge Cases
    // ============================================================================

    #[test]
    fn test_embed_request_rtl_text() {
        // Right-to-left text (Arabic/Hebrew)
        let request = EmbedRequest {
            texts: vec!["مرحبا بالعالم".to_string(), "שלום עולם".to_string()],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("مرحبا"));
        assert!(json.contains("שלום"));
    }

    #[test]
    fn test_embed_request_mixed_scripts() {
        let request = EmbedRequest {
            texts: vec![
                "English 中文 日本語 한국어 ไทย العربية".to_string(),
            ],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("English"));
        assert!(json.contains("中文"));
    }

    #[test]
    fn test_embed_request_emoji_sequences() {
        let request = EmbedRequest {
            texts: vec![
                "Family: 👨‍👩‍👧‍👦".to_string(),
                "Flag: 🇺🇸".to_string(),
                "Skin tone: 👋🏽".to_string(),
            ],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let result = serde_json::to_string(&request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_embed_request_zero_width_chars() {
        // Zero-width joiner and non-joiner
        let request = EmbedRequest {
            texts: vec!["test\u{200B}word".to_string(), "test\u{200D}word".to_string()],
            model: "test-model".to_string(),
            task_type: "search_query".to_string(),
            dimensionality: None,
        };

        let result = serde_json::to_string(&request);
        assert!(result.is_ok());
    }

    // ============================================================================
    // Boundary Value Tests for Dimensionality
    // ============================================================================

    #[test]
    fn test_dimensionality_common_values() {
        // Common embedding dimensions used in practice
        let common_dims = [64, 128, 256, 384, 512, 768, 1024, 1536, 2048, 3072, 4096];

        for dim in common_dims {
            let embedder = NomicEmbeddings::new_without_api_key().with_dimensionality(dim);
            assert_eq!(embedder.dimensionality, Some(dim));

            let request = EmbedRequest {
                texts: vec!["test".to_string()],
                model: "test-model".to_string(),
                task_type: "search_query".to_string(),
                dimensionality: Some(dim),
            };

            let json = serde_json::to_string(&request).unwrap();
            assert!(json.contains(&format!("\"dimensionality\":{}", dim)));
        }
    }

    // ============================================================================
    // Error Message Content Tests
    // ============================================================================

    #[tokio::test]
    async fn test_error_message_mentions_api_key() {
        let embedder = NomicEmbeddings::new_without_api_key();
        let result = embedder._embed_query("test").await;

        match result {
            Err(e) => {
                let msg = e.to_string().to_lowercase();
                assert!(
                    msg.contains("api") || msg.contains("key"),
                    "Error should mention API key: {}",
                    msg
                );
            }
            Ok(_) => panic!("Expected error without API key"),
        }
    }

    // ============================================================================
    // Model Name Validation Tests
    // ============================================================================

    #[test]
    fn test_model_name_variations() {
        let model_names = [
            "nomic-embed-text-v1",
            "nomic-embed-text-v1.5",
            "nomic-embed-text-v2",
            "custom-model-name",
            "my_model_v1.0.0",
            "MODEL-WITH-CAPS",
            "model.with.dots",
        ];

        for name in model_names {
            let embedder = NomicEmbeddings::new_without_api_key().with_model(name);
            assert_eq!(embedder.model, name);
        }
    }

    // ============================================================================
    // JSON Round-Trip Tests
    // ============================================================================

    #[test]
    fn test_embed_request_roundtrip() {
        let request = EmbedRequest {
            texts: vec!["hello".to_string(), "world".to_string()],
            model: "nomic-embed-text-v1.5".to_string(),
            task_type: "search_document".to_string(),
            dimensionality: Some(512),
        };

        let json = serde_json::to_string(&request).unwrap();
        // Note: EmbedRequest has Serialize but not Deserialize, so we can't roundtrip
        // But we can verify the JSON structure
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["texts"][0], "hello");
        assert_eq!(parsed["texts"][1], "world");
        assert_eq!(parsed["model"], "nomic-embed-text-v1.5");
        assert_eq!(parsed["dimensionality"], 512);
    }

    #[test]
    fn test_embed_response_roundtrip() {
        let original = EmbedResponse {
            embeddings: vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
        };

        // Serialize (requires Serialize which isn't derived, so use manual JSON)
        let json = r#"{"embeddings":[[0.1,0.2,0.3],[0.4,0.5,0.6]]}"#;
        let parsed: EmbedResponse = serde_json::from_str(json).unwrap();

        assert_eq!(parsed.embeddings.len(), original.embeddings.len());
        assert_eq!(parsed.embeddings[0], original.embeddings[0]);
        assert_eq!(parsed.embeddings[1], original.embeddings[1]);
    }
}
