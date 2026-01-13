// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! `HuggingFace` Hub embeddings implementation.
//!
//! This module provides embeddings using `HuggingFace` Hub's Inference API for embedding models.
//!
//! # Example
//!
//! ```rust
//! use dashflow_huggingface::embeddings::HuggingFaceEmbeddings;
//! use dashflow::{embed, embed_query};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = Arc::new(
//!     HuggingFaceEmbeddings::new().with_model("sentence-transformers/all-mpnet-base-v2"),
//! );
//!
//! // Embed a single query
//! let query_vector = embed_query(embedder.clone(), "What is the meaning of life?").await?;
//! assert!(!query_vector.is_empty());
//!
//! // Embed multiple documents
//! let docs = vec![
//!     "The quick brown fox".to_string(),
//!     "jumps over the lazy dog".to_string(),
//! ];
//! let doc_vectors = embed(embedder, &docs).await?;
//! assert_eq!(doc_vectors.len(), 2);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use dashflow::core::config_loader::env_vars::{env_string, HF_TOKEN, HUGGINGFACEHUB_API_TOKEN};
use dashflow::core::{
    embeddings::Embeddings,
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
    Error as DashFlowError,
};
use reqwest::Client;
use std::sync::Arc;

/// Default model for `HuggingFace` embeddings
const DEFAULT_MODEL: &str = "sentence-transformers/all-mpnet-base-v2";

/// `HuggingFace` Hub Inference API base URL
const HUGGINGFACE_API_BASE: &str = "https://api-inference.huggingface.co";

/// `HuggingFace` Hub embedding model integration.
///
/// Uses `HuggingFace` Hub's Inference API to generate embeddings from text.
/// Requires a `HuggingFace` API token which can be obtained from <https://huggingface.co/settings/tokens>
///
/// # Configuration
///
/// The API token can be set via:
/// - Constructor: `HuggingFaceEmbeddings::new().with_api_token("hf_...")`
/// - Environment: `HUGGINGFACEHUB_API_TOKEN` or `HF_TOKEN`
///
/// # Supported Models
///
/// Any `HuggingFace` model that supports the "feature-extraction" task, including:
/// - `sentence-transformers/all-mpnet-base-v2` (default): 768-dim embeddings
/// - `sentence-transformers/all-MiniLM-L6-v2`: 384-dim, fast and efficient
/// - `BAAI/bge-large-en-v1.5`: 1024-dim, high-quality embeddings
/// - `thenlper/gte-large`: 1024-dim, General Text Embeddings
///
/// # Rate Limiting
///
/// The Inference API has rate limits for free tier users. Consider using a Pro subscription
/// or deploying your own inference endpoint for production use.
pub struct HuggingFaceEmbeddings {
    /// HTTP client for API requests
    client: Client,
    /// Model name (e.g., "sentence-transformers/all-mpnet-base-v2")
    model: String,
    /// `HuggingFace` API token
    api_token: Option<String>,
    /// Additional model parameters
    model_kwargs: Option<serde_json::Value>,
    /// Retry policy for API calls
    retry_policy: RetryPolicy,
    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl HuggingFaceEmbeddings {
    /// Create a new `HuggingFace` embeddings instance with default settings.
    ///
    /// Defaults:
    /// - Model: `sentence-transformers/all-mpnet-base-v2`
    /// - API token: from `HUGGINGFACEHUB_API_TOKEN` or `HF_TOKEN` environment variable
    ///
    /// # Panics
    ///
    /// Panics if neither `HUGGINGFACEHUB_API_TOKEN` nor `HF_TOKEN` environment variables are set.
    /// Use `with_api_token()` to set the token explicitly.
    #[must_use]
    pub fn new() -> Self {
        let api_token = env_string(HUGGINGFACEHUB_API_TOKEN)
            .or_else(|| env_string(HF_TOKEN))
            .expect("HUGGINGFACEHUB_API_TOKEN or HF_TOKEN environment variable must be set");

        Self {
            client: Client::new(),
            model: DEFAULT_MODEL.to_string(),
            api_token: Some(api_token),
            model_kwargs: None,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Create a new instance without requiring environment variables.
    ///
    /// You must call `with_api_token()` before using this instance.
    #[must_use]
    pub fn new_without_token() -> Self {
        Self {
            client: Client::new(),
            model: DEFAULT_MODEL.to_string(),
            api_token: None,
            model_kwargs: None,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Set the model to use.
    ///
    /// # Example
    /// ```
    /// use dashflow_huggingface::embeddings::HuggingFaceEmbeddings;
    ///
    /// let embedder = HuggingFaceEmbeddings::new_without_token()
    ///     .with_model("sentence-transformers/all-MiniLM-L6-v2");
    /// ```
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the API token.
    ///
    /// # Example
    /// ```
    /// use dashflow_huggingface::embeddings::HuggingFaceEmbeddings;
    ///
    /// let embedder = HuggingFaceEmbeddings::new_without_token()
    ///     .with_api_token("hf_...");
    /// ```
    pub fn with_api_token(mut self, token: impl Into<String>) -> Self {
        self.api_token = Some(token.into());
        self
    }

    /// Set additional model parameters.
    ///
    /// # Example
    /// ```
    /// use dashflow_huggingface::embeddings::HuggingFaceEmbeddings;
    /// use serde_json::json;
    ///
    /// let embedder = HuggingFaceEmbeddings::new_without_token()
    ///     .with_model_kwargs(json!({"wait_for_model": true}));
    /// ```
    #[must_use]
    pub fn with_model_kwargs(mut self, kwargs: serde_json::Value) -> Self {
        self.model_kwargs = Some(kwargs);
        self
    }

    /// Set the retry policy for API calls.
    ///
    /// Default is exponential backoff with 3 retries.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_huggingface::embeddings::HuggingFaceEmbeddings;
    /// # use dashflow::core::retry::RetryPolicy;
    /// let embeddings = HuggingFaceEmbeddings::new_without_token()
    ///     .with_api_token("hf_...")
    ///     .with_retry_policy(RetryPolicy::exponential(5));
    /// ```
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Set a rate limiter to control request rate.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_huggingface::embeddings::HuggingFaceEmbeddings;
    /// # use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// # use std::time::Duration;
    /// # use std::sync::Arc;
    /// let rate_limiter = InMemoryRateLimiter::new(
    ///     10.0,  // 10 requests per second
    ///     Duration::from_millis(100),  // Check every 100ms
    ///     20.0,  // Max burst of 20 requests
    /// );
    ///
    /// let embeddings = HuggingFaceEmbeddings::new_without_token()
    ///     .with_api_token("hf_...")
    ///     .with_rate_limiter(Arc::new(rate_limiter));
    /// ```
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Internal method to call the `HuggingFace` Inference API.
    async fn feature_extraction(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, DashFlowError> {
        let api_token = self
            .api_token
            .as_ref()
            .ok_or_else(|| DashFlowError::invalid_input("API token not set"))?
            .clone();

        // Acquire rate limiter token if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        // Replace newlines which can negatively affect performance
        let processed_texts: Vec<String> = texts.iter().map(|t| t.replace('\n', " ")).collect();

        let url = format!(
            "{}/pipeline/feature-extraction/{}",
            HUGGINGFACE_API_BASE, self.model
        );

        let mut request_body = serde_json::json!({
            "inputs": processed_texts,
        });

        // Merge model_kwargs if present
        if let Some(kwargs) = &self.model_kwargs {
            if let Some(obj) = request_body.as_object_mut() {
                if let Some(kwargs_obj) = kwargs.as_object() {
                    for (k, v) in kwargs_obj {
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        // Clone values for retry closure
        let client = self.client.clone();
        let url = url.clone();
        let request_body = request_body.clone();

        // Execute with retry logic
        let response = with_retry(&self.retry_policy, || async {
            let response = client
                .post(&url)
                .header("Authorization", format!("Bearer {api_token}"))
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
                    "HuggingFace API error ({status}): {error_text}"
                )));
            }

            Ok(response)
        })
        .await?;

        let embeddings: Vec<Vec<f32>> = response
            .json()
            .await
            .map_err(|e| DashFlowError::api_format(e.to_string()))?;

        Ok(embeddings)
    }
}

impl Default for HuggingFaceEmbeddings {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Embeddings for HuggingFaceEmbeddings {
    /// Embed multiple documents.
    ///
    /// # Arguments
    ///
    /// * `texts` - A slice of strings to embed
    ///
    /// # Returns
    ///
    /// A vector of embeddings, one for each input text.
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, DashFlowError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        self.feature_extraction(texts.to_vec()).await
    }

    /// Embed a single query string.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to embed
    ///
    /// # Returns
    ///
    /// An embedding vector for the query.
    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, DashFlowError> {
        let embeddings = self.feature_extraction(vec![text.to_string()]).await?;

        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| DashFlowError::api_format("No embedding returned"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // Constructor and builder tests
    // ============================================

    #[test]
    fn test_new_without_token() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model("sentence-transformers/all-MiniLM-L6-v2")
            .with_api_token("test-token");

        assert_eq!(embedder.model, "sentence-transformers/all-MiniLM-L6-v2");
        assert_eq!(embedder.api_token, Some("test-token".to_string()));
    }

    #[test]
    fn test_new_without_token_default_model() {
        let embedder = HuggingFaceEmbeddings::new_without_token();
        assert_eq!(embedder.model, DEFAULT_MODEL);
        assert!(embedder.api_token.is_none());
        assert!(embedder.model_kwargs.is_none());
        assert!(embedder.rate_limiter.is_none());
    }

    #[test]
    fn test_with_model_kwargs() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model_kwargs(serde_json::json!({"wait_for_model": true}));

        assert!(embedder.model_kwargs.is_some());
    }

    #[test]
    fn test_with_model_kwargs_complex() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model_kwargs(serde_json::json!({
                "wait_for_model": true,
                "use_cache": false,
                "custom_param": 42
            }));

        let kwargs = embedder.model_kwargs.unwrap();
        assert_eq!(kwargs["wait_for_model"], true);
        assert_eq!(kwargs["use_cache"], false);
        assert_eq!(kwargs["custom_param"], 42);
    }

    #[test]
    fn test_with_model_various_models() {
        let models = [
            "sentence-transformers/all-MiniLM-L6-v2",
            "sentence-transformers/all-mpnet-base-v2",
            "BAAI/bge-large-en-v1.5",
            "thenlper/gte-large",
            "custom/my-model",
        ];

        for model in models {
            let embedder = HuggingFaceEmbeddings::new_without_token().with_model(model);
            assert_eq!(embedder.model, model);
        }
    }

    #[test]
    fn test_with_model_string_conversion() {
        // Test with &str
        let embedder1 = HuggingFaceEmbeddings::new_without_token()
            .with_model("model-name");
        assert_eq!(embedder1.model, "model-name");

        // Test with String
        let embedder2 = HuggingFaceEmbeddings::new_without_token()
            .with_model(String::from("another-model"));
        assert_eq!(embedder2.model, "another-model");
    }

    #[test]
    fn test_with_api_token_string_conversion() {
        // Test with &str
        let embedder1 = HuggingFaceEmbeddings::new_without_token()
            .with_api_token("hf_abc123");
        assert_eq!(embedder1.api_token, Some("hf_abc123".to_string()));

        // Test with String
        let embedder2 = HuggingFaceEmbeddings::new_without_token()
            .with_api_token(String::from("hf_xyz789"));
        assert_eq!(embedder2.api_token, Some("hf_xyz789".to_string()));
    }

    #[test]
    fn test_builder_chain_all_options() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use dashflow::core::retry::RetryPolicy;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0));

        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model("test-model")
            .with_api_token("test-token")
            .with_model_kwargs(serde_json::json!({"wait_for_model": true}))
            .with_retry_policy(RetryPolicy::exponential(5))
            .with_rate_limiter(rate_limiter);

        assert_eq!(embedder.model, "test-model");
        assert_eq!(embedder.api_token, Some("test-token".to_string()));
        assert!(embedder.model_kwargs.is_some());
        assert!(embedder.rate_limiter.is_some());
    }

    #[test]
    fn test_builder_overwrites_previous_values() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model("first-model")
            .with_api_token("first-token")
            .with_model("second-model")
            .with_api_token("second-token");

        assert_eq!(embedder.model, "second-model");
        assert_eq!(embedder.api_token, Some("second-token".to_string()));
    }

    // ============================================
    // Retry policy and rate limiter tests
    // ============================================

    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN"]
    async fn test_embed_query() {
        let embedder = HuggingFaceEmbeddings::new();
        let result = embedder._embed_query("Hello, world!").await;
        assert!(result.is_ok());

        let embedding = result.unwrap();
        assert!(!embedding.is_empty());
        assert_eq!(embedding.len(), 768); // all-mpnet-base-v2 produces 768-dim embeddings
    }

    #[tokio::test]
    #[ignore = "requires HUGGINGFACEHUB_API_TOKEN"]
    async fn test_embed_documents() {
        let embedder = HuggingFaceEmbeddings::new();
        let texts = vec!["Hello, world!".to_string(), "Goodbye, world!".to_string()];
        let result = embedder._embed_documents(&texts).await;
        assert!(result.is_ok());

        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 768);
        assert_eq!(embeddings[1].len(), 768);
    }

    #[tokio::test]
    async fn test_embed_empty() {
        let embedder = HuggingFaceEmbeddings::new_without_token().with_api_token("test");
        let result = embedder._embed_documents(&[]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_with_retry_policy() {
        use dashflow::core::retry::RetryPolicy;
        let embeddings = HuggingFaceEmbeddings::new_without_token()
            .with_api_token("test-token")
            .with_retry_policy(RetryPolicy::exponential(5));
        // Test passes if no panic occurs
        assert_eq!(embeddings.model, DEFAULT_MODEL);
    }

    #[test]
    fn test_with_retry_policy_fixed() {
        use dashflow::core::retry::RetryPolicy;
        let embeddings = HuggingFaceEmbeddings::new_without_token()
            .with_api_token("test-token")
            .with_retry_policy(RetryPolicy::fixed(3, 100));
        assert_eq!(embeddings.model, DEFAULT_MODEL);
    }

    #[test]
    fn test_with_retry_policy_no_retry() {
        use dashflow::core::retry::RetryPolicy;
        let embeddings = HuggingFaceEmbeddings::new_without_token()
            .with_api_token("test-token")
            .with_retry_policy(RetryPolicy::no_retry());
        assert_eq!(embeddings.model, DEFAULT_MODEL);
    }

    #[test]
    fn test_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;
        let rate_limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0);
        let embeddings = HuggingFaceEmbeddings::new_without_token()
            .with_api_token("test-token")
            .with_rate_limiter(Arc::new(rate_limiter));
        // Test passes if no panic occurs
        assert_eq!(embeddings.model, DEFAULT_MODEL);
    }

    #[test]
    fn test_with_rate_limiter_various_configs() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        // High rate limit
        let limiter1 = InMemoryRateLimiter::new(100.0, Duration::from_millis(10), 200.0);
        let embedder1 = HuggingFaceEmbeddings::new_without_token()
            .with_api_token("test")
            .with_rate_limiter(Arc::new(limiter1));
        assert!(embedder1.rate_limiter.is_some());

        // Low rate limit
        let limiter2 = InMemoryRateLimiter::new(1.0, Duration::from_secs(1), 5.0);
        let embedder2 = HuggingFaceEmbeddings::new_without_token()
            .with_api_token("test")
            .with_rate_limiter(Arc::new(limiter2));
        assert!(embedder2.rate_limiter.is_some());
    }

    // ============================================
    // API token validation tests
    // ============================================

    #[tokio::test]
    async fn test_embed_without_token_fails() {
        let embedder = HuggingFaceEmbeddings::new_without_token();
        let result = embedder._embed_query("test").await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("token") || err.to_string().contains("not set"));
    }

    #[tokio::test]
    async fn test_embed_documents_without_token_fails() {
        let embedder = HuggingFaceEmbeddings::new_without_token();
        let texts = vec!["test".to_string()];
        let result = embedder._embed_documents(&texts).await;
        assert!(result.is_err());
    }

    // ============================================
    // Model kwargs tests
    // ============================================

    #[test]
    fn test_model_kwargs_empty_object() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model_kwargs(serde_json::json!({}));

        let kwargs = embedder.model_kwargs.unwrap();
        assert!(kwargs.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_model_kwargs_nested_object() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model_kwargs(serde_json::json!({
                "options": {
                    "nested": {
                        "value": true
                    }
                }
            }));

        let kwargs = embedder.model_kwargs.unwrap();
        assert!(kwargs["options"]["nested"]["value"].as_bool().unwrap());
    }

    #[test]
    fn test_model_kwargs_array_value() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model_kwargs(serde_json::json!({
                "tags": ["tag1", "tag2", "tag3"]
            }));

        let kwargs = embedder.model_kwargs.unwrap();
        assert_eq!(kwargs["tags"].as_array().unwrap().len(), 3);
    }

    // ============================================
    // Constants tests
    // ============================================

    #[test]
    fn test_default_model_constant() {
        assert_eq!(DEFAULT_MODEL, "sentence-transformers/all-mpnet-base-v2");
    }

    #[test]
    fn test_huggingface_api_base_constant() {
        assert_eq!(HUGGINGFACE_API_BASE, "https://api-inference.huggingface.co");
    }

    // ============================================
    // Edge case tests
    // ============================================

    #[test]
    fn test_empty_model_name() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model("");
        assert_eq!(embedder.model, "");
    }

    #[test]
    fn test_empty_api_token() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_api_token("");
        assert_eq!(embedder.api_token, Some("".to_string()));
    }

    #[test]
    fn test_unicode_model_name() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model("模型名称/test");
        assert_eq!(embedder.model, "模型名称/test");
    }

    #[test]
    fn test_special_chars_in_token() {
        let token = "hf_abc!@#$%^&*()_+-=[]{}|;':\",./<>?";
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_api_token(token);
        assert_eq!(embedder.api_token, Some(token.to_string()));
    }

    #[test]
    fn test_whitespace_handling() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model("  model-with-spaces  ")
            .with_api_token("  token-with-spaces  ");
        // Note: no trimming is done - values are stored as-is
        assert_eq!(embedder.model, "  model-with-spaces  ");
        assert_eq!(embedder.api_token, Some("  token-with-spaces  ".to_string()));
    }

    // ============================================
    // Multiple instances tests
    // ============================================

    #[test]
    fn test_multiple_independent_instances() {
        let embedder1 = HuggingFaceEmbeddings::new_without_token()
            .with_model("model-1")
            .with_api_token("token-1");

        let embedder2 = HuggingFaceEmbeddings::new_without_token()
            .with_model("model-2")
            .with_api_token("token-2");

        // Instances are independent
        assert_eq!(embedder1.model, "model-1");
        assert_eq!(embedder2.model, "model-2");
        assert_eq!(embedder1.api_token, Some("token-1".to_string()));
        assert_eq!(embedder2.api_token, Some("token-2".to_string()));
    }

    // ============================================
    // Embeddings trait tests
    // ============================================

    #[tokio::test]
    async fn test_embeddings_trait_empty_documents() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_api_token("test-token");

        // Empty slice should return empty vec without making API call
        let result = embedder._embed_documents(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_embeddings_trait_single_document_no_token() {
        let embedder = HuggingFaceEmbeddings::new_without_token();
        let texts = vec!["single document".to_string()];
        let result = embedder._embed_documents(&texts).await;
        // Should fail because no token
        assert!(result.is_err());
    }

    // ============================================
    // Long text handling tests
    // ============================================

    #[test]
    fn test_long_model_name() {
        let long_name = "a".repeat(1000);
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model(&long_name);
        assert_eq!(embedder.model.len(), 1000);
    }

    #[test]
    fn test_long_api_token() {
        let long_token = "hf_".to_string() + &"x".repeat(997);
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_api_token(&long_token);
        assert_eq!(embedder.api_token.unwrap().len(), 1000);
    }

    // ============================================
    // Model kwargs overwrite tests
    // ============================================

    #[test]
    fn test_model_kwargs_overwrite() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model_kwargs(serde_json::json!({"first": true}))
            .with_model_kwargs(serde_json::json!({"second": true}));

        let kwargs = embedder.model_kwargs.unwrap();
        // Second call should have replaced the first
        assert!(kwargs.get("first").is_none());
        assert!(kwargs["second"].as_bool().unwrap());
    }

    // ============================================
    // Retry policy variant tests
    // ============================================

    #[test]
    fn test_retry_policy_exponential_variants() {
        use dashflow::core::retry::RetryPolicy;

        // Various exponential retry counts
        for retries in [0, 1, 3, 5, 10] {
            let embeddings = HuggingFaceEmbeddings::new_without_token()
                .with_api_token("test")
                .with_retry_policy(RetryPolicy::exponential(retries));
            assert_eq!(embeddings.model, DEFAULT_MODEL);
        }
    }

    // ============================================
    // Model name format tests
    // ============================================

    #[test]
    fn test_model_name_with_slash() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model("org/model-name");
        assert_eq!(embedder.model, "org/model-name");
    }

    #[test]
    fn test_model_name_with_multiple_slashes() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model("org/sub/model-name");
        assert_eq!(embedder.model, "org/sub/model-name");
    }

    #[test]
    fn test_model_name_with_version() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model("org/model-v1.0.0");
        assert_eq!(embedder.model, "org/model-v1.0.0");
    }

    // ============================================
    // API token format tests
    // ============================================

    #[test]
    fn test_api_token_hf_prefix() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_api_token("hf_abcdefghijklmnopqrstuvwxyz");
        assert!(embedder.api_token.unwrap().starts_with("hf_"));
    }

    #[test]
    fn test_api_token_without_prefix() {
        // Some legacy tokens might not have the hf_ prefix
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_api_token("legacy_token_format");
        assert_eq!(embedder.api_token, Some("legacy_token_format".to_string()));
    }

    // ============================================
    // Rate limiter replacement tests
    // ============================================

    #[test]
    fn test_rate_limiter_can_be_replaced() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let limiter1 = Arc::new(InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0));
        let limiter2 = Arc::new(InMemoryRateLimiter::new(5.0, Duration::from_millis(200), 10.0));

        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_api_token("test")
            .with_rate_limiter(limiter1)
            .with_rate_limiter(limiter2);

        assert!(embedder.rate_limiter.is_some());
    }

    // ============================================
    // JSON value types in model_kwargs
    // ============================================

    #[test]
    fn test_model_kwargs_with_null() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model_kwargs(serde_json::json!({
                "nullable_field": null
            }));

        let kwargs = embedder.model_kwargs.unwrap();
        assert!(kwargs["nullable_field"].is_null());
    }

    #[test]
    fn test_model_kwargs_with_number_types() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model_kwargs(serde_json::json!({
                "integer": 42,
                "float": 3.14,
                "negative": -100
            }));

        let kwargs = embedder.model_kwargs.unwrap();
        assert_eq!(kwargs["integer"].as_i64().unwrap(), 42);
        assert!((kwargs["float"].as_f64().unwrap() - 3.14).abs() < 0.001);
        assert_eq!(kwargs["negative"].as_i64().unwrap(), -100);
    }

    #[test]
    fn test_model_kwargs_with_boolean() {
        let embedder = HuggingFaceEmbeddings::new_without_token()
            .with_model_kwargs(serde_json::json!({
                "enabled": true,
                "disabled": false
            }));

        let kwargs = embedder.model_kwargs.unwrap();
        assert!(kwargs["enabled"].as_bool().unwrap());
        assert!(!kwargs["disabled"].as_bool().unwrap());
    }
}
