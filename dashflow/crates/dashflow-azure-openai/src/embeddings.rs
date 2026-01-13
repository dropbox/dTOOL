//! Azure OpenAI embeddings implementation.
//!
//! This module provides embeddings using Azure OpenAI's embedding models.
//! Azure OpenAI supports the same embedding models as OpenAI but deployed
//! in your Azure subscription for enterprise compliance and security.
//!
//! # Supported Models
//!
//! - `text-embedding-3-large`: 3072 dimensions (configurable 256-3072)
//! - `text-embedding-3-small`: 1536 dimensions (configurable 256-1536)
//! - `text-embedding-ada-002`: 1536 dimensions (legacy)
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_azure_openai::AzureOpenAIEmbeddings;
//! use dashflow::{embed, embed_query};
//! use dashflow::core::config_loader::env_vars::AZURE_OPENAI_API_KEY;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = Arc::new(AzureOpenAIEmbeddings::new()
//!     .with_deployment_name("text-embedding-3-large")
//!     .with_endpoint("https://my-resource.openai.azure.com")
//!     .with_api_key(std::env::var(AZURE_OPENAI_API_KEY)?));
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

use async_openai::{
    config::AzureConfig,
    types::{CreateEmbeddingRequestArgs, EmbeddingInput},
    Client,
};
use async_trait::async_trait;
use dashflow::core::{
    config_loader::env_vars::{
        env_string, env_string_or_default, AZURE_OPENAI_API_KEY, AZURE_OPENAI_API_VERSION,
        AZURE_OPENAI_ENDPOINT, OPENAI_API_BASE, OPENAI_API_KEY,
    },
    embeddings::Embeddings,
    error::Error as DashFlowError,
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
};
use std::sync::Arc;

/// Azure OpenAI embedding model integration.
///
/// Provides access to OpenAI embedding models deployed in Azure.
///
/// # Authentication
///
/// Azure OpenAI requires:
/// - `endpoint`: Your Azure OpenAI resource endpoint
/// - `api_key`: Your Azure OpenAI API key
/// - `deployment_name`: The deployment name for your embedding model
///
/// These can be set via environment variables or builder methods:
/// - `AZURE_OPENAI_ENDPOINT` or `OPENAI_API_BASE`
/// - `AZURE_OPENAI_API_KEY` or `OPENAI_API_KEY`
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow_azure_openai::AzureOpenAIEmbeddings;
/// let embedder = AzureOpenAIEmbeddings::new()
///     .with_deployment_name("text-embedding-3-large")
///     .with_endpoint("https://my-resource.openai.azure.com")
///     .with_api_key("your-api-key")
///     .with_dimensions(512);  // Reduced dimensions for efficiency
/// ```
#[derive(Clone)]
pub struct AzureOpenAIEmbeddings {
    /// Azure OpenAI client
    client: Client<AzureConfig>,
    /// Deployment name for the embedding model
    deployment_name: String,
    /// Optional output dimensions (for text-embedding-3-* models)
    dimensions: Option<u32>,
    /// Batch size for embedding multiple documents
    batch_size: usize,
    /// Retry policy for API calls
    retry_policy: RetryPolicy,
    /// Optional rate limiter
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl AzureOpenAIEmbeddings {
    /// Create a new Azure OpenAI embeddings instance with default settings.
    ///
    /// Reads configuration from environment variables:
    /// - `AZURE_OPENAI_ENDPOINT` or `OPENAI_API_BASE`: Azure endpoint
    /// - `AZURE_OPENAI_API_KEY` or `OPENAI_API_KEY`: API key
    /// - `AZURE_OPENAI_API_VERSION`: API version (default: 2024-02-15-preview)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_azure_openai::AzureOpenAIEmbeddings;
    /// // Set environment variables first:
    /// // export AZURE_OPENAI_ENDPOINT="https://my-resource.openai.azure.com"
    /// // export AZURE_OPENAI_API_KEY="your-api-key"
    ///
    /// let embedder = AzureOpenAIEmbeddings::new()
    ///     .with_deployment_name("text-embedding-3-large");
    /// ```
    #[must_use]
    pub fn new() -> Self {
        let api_key = env_string(AZURE_OPENAI_API_KEY)
            .or_else(|| env_string(OPENAI_API_KEY))
            .unwrap_or_default();

        let endpoint = env_string(AZURE_OPENAI_ENDPOINT)
            .or_else(|| env_string(OPENAI_API_BASE))
            .unwrap_or_default();

        let api_version = env_string_or_default(AZURE_OPENAI_API_VERSION, "2024-02-15-preview");

        let config = AzureConfig::new()
            .with_api_base(&endpoint)
            .with_api_key(&api_key)
            .with_api_version(&api_version)
            .with_deployment_id("text-embedding-3-large");

        Self {
            client: Client::with_config(config),
            deployment_name: "text-embedding-3-large".to_string(),
            dimensions: None,
            batch_size: 100,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Set the Azure endpoint.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_azure_openai::AzureOpenAIEmbeddings;
    /// let embedder = AzureOpenAIEmbeddings::new()
    ///     .with_endpoint("https://my-resource.openai.azure.com");
    /// ```
    #[must_use]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        let endpoint_str = endpoint.into();
        let api_key = env_string(AZURE_OPENAI_API_KEY)
            .or_else(|| env_string(OPENAI_API_KEY))
            .unwrap_or_default();
        let api_version = env_string_or_default(AZURE_OPENAI_API_VERSION, "2024-02-15-preview");

        let config = AzureConfig::new()
            .with_api_base(&endpoint_str)
            .with_api_key(&api_key)
            .with_api_version(&api_version)
            .with_deployment_id(&self.deployment_name);

        self.client = Client::with_config(config);
        self
    }

    /// Set the API key.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_azure_openai::AzureOpenAIEmbeddings;
    /// let embedder = AzureOpenAIEmbeddings::new()
    ///     .with_api_key("your-api-key");
    /// ```
    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        let api_key_str = api_key.into();
        let endpoint = env_string(AZURE_OPENAI_ENDPOINT)
            .or_else(|| env_string(OPENAI_API_BASE))
            .unwrap_or_default();
        let api_version = env_string_or_default(AZURE_OPENAI_API_VERSION, "2024-02-15-preview");

        let config = AzureConfig::new()
            .with_api_base(&endpoint)
            .with_api_key(&api_key_str)
            .with_api_version(&api_version)
            .with_deployment_id(&self.deployment_name);

        self.client = Client::with_config(config);
        self
    }

    /// Set the API version.
    ///
    /// Default: `2024-02-15-preview`
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_azure_openai::AzureOpenAIEmbeddings;
    /// let embedder = AzureOpenAIEmbeddings::new()
    ///     .with_api_version("2024-02-01");
    /// ```
    #[must_use]
    pub fn with_api_version(mut self, api_version: impl Into<String>) -> Self {
        let api_version_str = api_version.into();
        let endpoint = env_string(AZURE_OPENAI_ENDPOINT)
            .or_else(|| env_string(OPENAI_API_BASE))
            .unwrap_or_default();
        let api_key = env_string(AZURE_OPENAI_API_KEY)
            .or_else(|| env_string(OPENAI_API_KEY))
            .unwrap_or_default();

        let config = AzureConfig::new()
            .with_api_base(&endpoint)
            .with_api_key(&api_key)
            .with_api_version(&api_version_str)
            .with_deployment_id(&self.deployment_name);

        self.client = Client::with_config(config);
        self
    }

    /// Set the deployment name for the embedding model.
    ///
    /// This should match your Azure OpenAI deployment name.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_azure_openai::AzureOpenAIEmbeddings;
    /// let embedder = AzureOpenAIEmbeddings::new()
    ///     .with_deployment_name("text-embedding-3-small");
    /// ```
    #[must_use]
    pub fn with_deployment_name(mut self, deployment_name: impl Into<String>) -> Self {
        self.deployment_name = deployment_name.into();

        // Recreate client with new deployment
        let endpoint = env_string(AZURE_OPENAI_ENDPOINT)
            .or_else(|| env_string(OPENAI_API_BASE))
            .unwrap_or_default();
        let api_key = env_string(AZURE_OPENAI_API_KEY)
            .or_else(|| env_string(OPENAI_API_KEY))
            .unwrap_or_default();
        let api_version = env_string_or_default(AZURE_OPENAI_API_VERSION, "2024-02-15-preview");

        let config = AzureConfig::new()
            .with_api_base(&endpoint)
            .with_api_key(&api_key)
            .with_api_version(&api_version)
            .with_deployment_id(&self.deployment_name);

        self.client = Client::with_config(config);
        self
    }

    /// Set the output dimensionality for embeddings.
    ///
    /// Only supported by `text-embedding-3-*` models.
    /// - `text-embedding-3-large`: 256-3072 (default 3072)
    /// - `text-embedding-3-small`: 256-1536 (default 1536)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_azure_openai::AzureOpenAIEmbeddings;
    /// let embedder = AzureOpenAIEmbeddings::new()
    ///     .with_dimensions(512);
    /// ```
    #[must_use]
    pub fn with_dimensions(mut self, dimensions: u32) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    /// Set the batch size for embedding multiple documents.
    ///
    /// Default is 100. Azure OpenAI has the same limits as OpenAI.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_azure_openai::AzureOpenAIEmbeddings;
    /// let embedder = AzureOpenAIEmbeddings::new()
    ///     .with_batch_size(50);
    /// ```
    #[must_use]
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.max(1);
        self
    }

    /// Set the retry policy for API calls.
    ///
    /// Default is exponential backoff with 3 retries.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_azure_openai::AzureOpenAIEmbeddings;
    /// # use dashflow::core::retry::RetryPolicy;
    /// let embedder = AzureOpenAIEmbeddings::new()
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
    /// # use dashflow_azure_openai::AzureOpenAIEmbeddings;
    /// # use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// # use std::time::Duration;
    /// # use std::sync::Arc;
    /// let rate_limiter = InMemoryRateLimiter::new(
    ///     10.0,  // 10 requests per second
    ///     Duration::from_millis(100),
    ///     20.0,  // Max burst
    /// );
    ///
    /// let embedder = AzureOpenAIEmbeddings::new()
    ///     .with_rate_limiter(Arc::new(rate_limiter));
    /// ```
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Embed a batch of texts.
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, DashFlowError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Acquire rate limiter token if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        let input = EmbeddingInput::StringArray(texts.to_vec());

        let client = self.client.clone();
        let deployment_name = self.deployment_name.clone();
        let dimensions = self.dimensions;

        let response = with_retry(&self.retry_policy, || {
            let client = client.clone();
            let deployment_name = deployment_name.clone();
            let input = input.clone();
            async move {
                let mut request = CreateEmbeddingRequestArgs::default()
                    .model(&deployment_name)
                    .input(input)
                    .build()
                    .map_err(|e| DashFlowError::api(format!("Failed to build request: {e}")))?;

                // Set dimensions if specified (for text-embedding-3-* models)
                if let Some(dims) = dimensions {
                    request.dimensions = Some(dims);
                }

                client
                    .embeddings()
                    .create(request)
                    .await
                    .map_err(|e| DashFlowError::api(format!("Azure OpenAI API error: {e}")))
            }
        })
        .await?;

        // Extract embeddings and sort by index
        let mut embeddings: Vec<(usize, Vec<f32>)> = response
            .data
            .into_iter()
            .map(|e| (e.index as usize, e.embedding))
            .collect();

        embeddings.sort_by_key(|(idx, _)| *idx);

        Ok(embeddings.into_iter().map(|(_, e)| e).collect())
    }
}

impl Default for AzureOpenAIEmbeddings {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Embeddings for AzureOpenAIEmbeddings {
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, DashFlowError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        // Process in batches
        for chunk in texts.chunks(self.batch_size) {
            let batch_embeddings = self.embed_batch(chunk).await?;
            all_embeddings.extend(batch_embeddings);
        }

        Ok(all_embeddings)
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, DashFlowError> {
        let embeddings = self.embed_batch(&[text.to_string()]).await?;

        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| DashFlowError::api("No embedding returned from Azure OpenAI"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // Constructor tests
    // ============================================

    #[test]
    fn test_default_constructor() {
        let embedder = AzureOpenAIEmbeddings::new();
        assert_eq!(embedder.deployment_name, "text-embedding-3-large");
        assert_eq!(embedder.batch_size, 100);
        assert!(embedder.dimensions.is_none());
    }

    #[test]
    fn test_default_trait() {
        let embedder = AzureOpenAIEmbeddings::default();
        assert_eq!(embedder.deployment_name, "text-embedding-3-large");
        assert_eq!(embedder.batch_size, 100);
    }

    #[test]
    fn test_default_matches_new() {
        let via_new = AzureOpenAIEmbeddings::new();
        let via_default = AzureOpenAIEmbeddings::default();

        assert_eq!(via_new.deployment_name, via_default.deployment_name);
        assert_eq!(via_new.batch_size, via_default.batch_size);
        assert_eq!(via_new.dimensions, via_default.dimensions);
    }

    // ============================================
    // Deployment name tests
    // ============================================

    #[test]
    fn test_with_deployment_name() {
        let embedder = AzureOpenAIEmbeddings::new().with_deployment_name("text-embedding-3-small");
        assert_eq!(embedder.deployment_name, "text-embedding-3-small");
    }

    #[test]
    fn test_with_deployment_name_ada() {
        let embedder = AzureOpenAIEmbeddings::new().with_deployment_name("text-embedding-ada-002");
        assert_eq!(embedder.deployment_name, "text-embedding-ada-002");
    }

    #[test]
    fn test_with_deployment_name_custom() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("my-custom-embedding-deployment");
        assert_eq!(embedder.deployment_name, "my-custom-embedding-deployment");
    }

    #[test]
    fn test_with_deployment_name_preserves_other_settings() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_dimensions(256)
            .with_batch_size(50)
            .with_deployment_name("text-embedding-3-small");

        assert_eq!(embedder.deployment_name, "text-embedding-3-small");
        assert_eq!(embedder.dimensions, Some(256));
        assert_eq!(embedder.batch_size, 50);
    }

    // ============================================
    // Dimensions tests
    // ============================================

    #[test]
    fn test_with_dimensions() {
        let embedder = AzureOpenAIEmbeddings::new().with_dimensions(512);
        assert_eq!(embedder.dimensions, Some(512));
    }

    #[test]
    fn test_with_dimensions_min() {
        let embedder = AzureOpenAIEmbeddings::new().with_dimensions(256);
        assert_eq!(embedder.dimensions, Some(256));
    }

    #[test]
    fn test_with_dimensions_max_large() {
        let embedder = AzureOpenAIEmbeddings::new().with_dimensions(3072);
        assert_eq!(embedder.dimensions, Some(3072));
    }

    #[test]
    fn test_with_dimensions_max_small() {
        let embedder = AzureOpenAIEmbeddings::new().with_dimensions(1536);
        assert_eq!(embedder.dimensions, Some(1536));
    }

    #[test]
    fn test_with_dimensions_various_values() {
        for dims in [256, 384, 512, 768, 1024, 1536, 2048, 3072] {
            let embedder = AzureOpenAIEmbeddings::new().with_dimensions(dims);
            assert_eq!(embedder.dimensions, Some(dims), "Failed for dimension {dims}");
        }
    }

    // ============================================
    // Batch size tests
    // ============================================

    #[test]
    fn test_with_batch_size() {
        let embedder = AzureOpenAIEmbeddings::new().with_batch_size(50);
        assert_eq!(embedder.batch_size, 50);
    }

    #[test]
    fn test_batch_size_min() {
        let embedder = AzureOpenAIEmbeddings::new().with_batch_size(0);
        assert_eq!(embedder.batch_size, 1);
    }

    #[test]
    fn test_batch_size_one() {
        let embedder = AzureOpenAIEmbeddings::new().with_batch_size(1);
        assert_eq!(embedder.batch_size, 1);
    }

    #[test]
    fn test_batch_size_large() {
        let embedder = AzureOpenAIEmbeddings::new().with_batch_size(1000);
        assert_eq!(embedder.batch_size, 1000);
    }

    #[test]
    fn test_batch_size_various() {
        for size in [1, 10, 25, 50, 100, 200, 500] {
            let embedder = AzureOpenAIEmbeddings::new().with_batch_size(size);
            assert_eq!(embedder.batch_size, size, "Failed for batch size {size}");
        }
    }

    // ============================================
    // Endpoint tests
    // ============================================

    #[test]
    fn test_with_endpoint() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_endpoint("https://my-resource.openai.azure.com");
        // Endpoint is set via client config, verify no panic
        let _ = embedder.deployment_name;
    }

    #[test]
    fn test_with_endpoint_preserves_deployment() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("text-embedding-3-small")
            .with_endpoint("https://test.azure.com");

        assert_eq!(embedder.deployment_name, "text-embedding-3-small");
    }

    #[test]
    fn test_with_endpoint_empty() {
        // Empty endpoint should not panic
        let embedder = AzureOpenAIEmbeddings::new().with_endpoint("");
        let _ = embedder.deployment_name;
    }

    #[test]
    fn test_with_endpoint_various_formats() {
        let endpoints = [
            "https://myresource.openai.azure.com",
            "https://myresource.openai.azure.com/",
            "https://eastus.api.cognitive.microsoft.com",
        ];

        for endpoint in endpoints {
            let embedder = AzureOpenAIEmbeddings::new().with_endpoint(endpoint);
            // Just verify no panic
            let _ = embedder.deployment_name;
        }
    }

    // ============================================
    // API key tests
    // ============================================

    #[test]
    fn test_with_api_key() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_api_key("test-api-key-12345");
        // API key is set via client config, verify no panic
        let _ = embedder.deployment_name;
    }

    #[test]
    fn test_with_api_key_preserves_deployment() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("text-embedding-ada-002")
            .with_api_key("my-key");

        assert_eq!(embedder.deployment_name, "text-embedding-ada-002");
    }

    #[test]
    fn test_with_api_key_empty() {
        // Empty key should not panic
        let embedder = AzureOpenAIEmbeddings::new().with_api_key("");
        let _ = embedder.deployment_name;
    }

    // ============================================
    // API version tests
    // ============================================

    #[test]
    fn test_with_api_version() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_api_version("2024-02-01");
        let _ = embedder.deployment_name;
    }

    #[test]
    fn test_with_api_version_preview() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_api_version("2024-02-15-preview");
        let _ = embedder.deployment_name;
    }

    #[test]
    fn test_with_api_version_preserves_deployment() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("text-embedding-3-small")
            .with_api_version("2025-01-01");

        assert_eq!(embedder.deployment_name, "text-embedding-3-small");
    }

    // ============================================
    // Retry policy tests
    // ============================================

    #[test]
    fn test_with_retry_policy_exponential() {
        let policy = RetryPolicy::exponential(5);
        let embedder = AzureOpenAIEmbeddings::new().with_retry_policy(policy);
        // Verify no panic
        let _ = embedder.deployment_name;
    }

    #[test]
    fn test_with_retry_policy_fixed() {
        let policy = RetryPolicy::fixed(3, 1000); // 1000ms = 1 second
        let embedder = AzureOpenAIEmbeddings::new().with_retry_policy(policy);
        let _ = embedder.deployment_name;
    }

    #[test]
    fn test_with_retry_policy_preserves_other() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_dimensions(512)
            .with_retry_policy(RetryPolicy::exponential(3));

        assert_eq!(embedder.dimensions, Some(512));
    }

    // ============================================
    // Rate limiter tests
    // ============================================

    #[test]
    fn test_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let embedder = AzureOpenAIEmbeddings::new().with_rate_limiter(rate_limiter);
        assert!(embedder.rate_limiter.is_some());
    }

    #[test]
    fn test_with_rate_limiter_preserves_other() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("text-embedding-3-small")
            .with_dimensions(512)
            .with_rate_limiter(rate_limiter);

        assert_eq!(embedder.deployment_name, "text-embedding-3-small");
        assert_eq!(embedder.dimensions, Some(512));
        assert!(embedder.rate_limiter.is_some());
    }

    // ============================================
    // Builder chaining tests
    // ============================================

    #[test]
    fn test_builder_chaining() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("text-embedding-ada-002")
            .with_dimensions(1536)
            .with_batch_size(25);

        assert_eq!(embedder.deployment_name, "text-embedding-ada-002");
        assert_eq!(embedder.dimensions, Some(1536));
        assert_eq!(embedder.batch_size, 25);
    }

    #[test]
    fn test_builder_chaining_full() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("text-embedding-3-large")
            .with_endpoint("https://myresource.openai.azure.com")
            .with_api_key("my-api-key")
            .with_api_version("2024-10-01")
            .with_dimensions(1024)
            .with_batch_size(50)
            .with_retry_policy(RetryPolicy::exponential(5))
            .with_rate_limiter(rate_limiter);

        assert_eq!(embedder.deployment_name, "text-embedding-3-large");
        assert_eq!(embedder.dimensions, Some(1024));
        assert_eq!(embedder.batch_size, 50);
        assert!(embedder.rate_limiter.is_some());
    }

    #[test]
    fn test_builder_order_independence() {
        // Test that builder methods can be called in any order
        let e1 = AzureOpenAIEmbeddings::new()
            .with_deployment_name("model-1")
            .with_dimensions(512)
            .with_batch_size(50);

        let e2 = AzureOpenAIEmbeddings::new()
            .with_batch_size(50)
            .with_dimensions(512)
            .with_deployment_name("model-1");

        assert_eq!(e1.deployment_name, e2.deployment_name);
        assert_eq!(e1.dimensions, e2.dimensions);
        assert_eq!(e1.batch_size, e2.batch_size);
    }

    // ============================================
    // Clone tests
    // ============================================

    #[test]
    fn test_clone() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("text-embedding-3-small")
            .with_dimensions(768)
            .with_batch_size(75);

        let cloned = embedder.clone();

        assert_eq!(cloned.deployment_name, "text-embedding-3-small");
        assert_eq!(cloned.dimensions, Some(768));
        assert_eq!(cloned.batch_size, 75);
    }

    #[test]
    fn test_clone_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            5.0,
            Duration::from_millis(50),
            10.0,
        ));

        let embedder = AzureOpenAIEmbeddings::new()
            .with_rate_limiter(rate_limiter);

        let cloned = embedder.clone();
        assert!(cloned.rate_limiter.is_some());
    }

    // ============================================
    // Edge case tests
    // ============================================

    #[test]
    fn test_overwrite_deployment_name() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("first-model")
            .with_deployment_name("second-model")
            .with_deployment_name("final-model");

        assert_eq!(embedder.deployment_name, "final-model");
    }

    #[test]
    fn test_overwrite_dimensions() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_dimensions(256)
            .with_dimensions(512)
            .with_dimensions(1024);

        assert_eq!(embedder.dimensions, Some(1024));
    }

    #[test]
    fn test_overwrite_batch_size() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_batch_size(10)
            .with_batch_size(50)
            .with_batch_size(100);

        assert_eq!(embedder.batch_size, 100);
    }

    #[test]
    fn test_deployment_name_with_special_chars() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("my-embedding_model-v2.1");
        assert_eq!(embedder.deployment_name, "my-embedding_model-v2.1");
    }

    #[test]
    fn test_deployment_name_unicode() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("embedding-模型-test");
        assert_eq!(embedder.deployment_name, "embedding-模型-test");
    }

    // ============================================
    // Configuration combination tests
    // ============================================

    #[test]
    fn test_text_embedding_3_large_config() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("text-embedding-3-large")
            .with_dimensions(3072);

        assert_eq!(embedder.deployment_name, "text-embedding-3-large");
        assert_eq!(embedder.dimensions, Some(3072));
    }

    #[test]
    fn test_text_embedding_3_small_config() {
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("text-embedding-3-small")
            .with_dimensions(1536);

        assert_eq!(embedder.deployment_name, "text-embedding-3-small");
        assert_eq!(embedder.dimensions, Some(1536));
    }

    #[test]
    fn test_ada_002_config() {
        // ada-002 has fixed 1536 dimensions, dimensions param is ignored
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("text-embedding-ada-002");

        assert_eq!(embedder.deployment_name, "text-embedding-ada-002");
        assert!(embedder.dimensions.is_none());
    }

    #[test]
    fn test_high_throughput_config() {
        // Configuration optimized for high throughput
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("text-embedding-3-small")
            .with_dimensions(256)  // Reduced dimensions for speed
            .with_batch_size(100); // Max batch size

        assert_eq!(embedder.dimensions, Some(256));
        assert_eq!(embedder.batch_size, 100);
    }

    #[test]
    fn test_high_quality_config() {
        // Configuration optimized for quality
        let embedder = AzureOpenAIEmbeddings::new()
            .with_deployment_name("text-embedding-3-large")
            .with_dimensions(3072) // Full dimensions
            .with_batch_size(10);  // Smaller batches for better error handling

        assert_eq!(embedder.dimensions, Some(3072));
        assert_eq!(embedder.batch_size, 10);
    }
}

/// Standard conformance tests for embeddings
///
/// These tests verify that AzureOpenAIEmbeddings behaves consistently with other
/// Embeddings implementations across the DashFlow ecosystem.
#[cfg(test)]
mod standard_tests {
    use super::*;
    use dashflow_standard_tests::embeddings_tests::*;
    use std::sync::Arc;

    /// Standard Test 1: Embed single document
    #[tokio::test]
    #[ignore = "requires AZURE_OPENAI_API_KEY and AZURE_OPENAI_ENDPOINT"]
    async fn test_embed_query_standard() {
        let embeddings = Arc::new(AzureOpenAIEmbeddings::new());
        test_embed_query(embeddings).await;
    }

    /// Standard Test 2: Embed multiple documents
    #[tokio::test]
    #[ignore = "requires AZURE_OPENAI_API_KEY and AZURE_OPENAI_ENDPOINT"]
    async fn test_embed_documents_standard() {
        let embeddings = Arc::new(AzureOpenAIEmbeddings::new());
        test_embed_documents(embeddings).await;
    }

    /// Standard Test 3: Empty input handling
    #[tokio::test]
    #[ignore = "requires AZURE_OPENAI_API_KEY and AZURE_OPENAI_ENDPOINT"]
    async fn test_empty_input_standard() {
        let embeddings = Arc::new(AzureOpenAIEmbeddings::new());
        test_empty_input(embeddings).await;
    }

    /// Standard Test 4: Dimension consistency
    #[tokio::test]
    #[ignore = "requires AZURE_OPENAI_API_KEY and AZURE_OPENAI_ENDPOINT"]
    async fn test_dimension_consistency_standard() {
        let embeddings = Arc::new(AzureOpenAIEmbeddings::new());
        test_dimension_consistency(embeddings).await;
    }

    /// Standard Test 5: Semantic similarity
    #[tokio::test]
    #[ignore = "requires AZURE_OPENAI_API_KEY and AZURE_OPENAI_ENDPOINT"]
    async fn test_semantic_similarity_standard() {
        let embeddings = Arc::new(AzureOpenAIEmbeddings::new());
        test_semantic_similarity(embeddings).await;
    }
}
