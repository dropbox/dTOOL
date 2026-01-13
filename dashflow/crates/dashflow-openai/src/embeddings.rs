//! `OpenAI` embeddings implementation.
//!
//! This module provides embeddings using `OpenAI`'s embedding models, including:
//! - text-embedding-3-small
//! - text-embedding-3-large
//! - text-embedding-ada-002
//!
//! # Example
//!
//! ```rust
//! use dashflow_openai::embeddings::OpenAIEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = OpenAIEmbeddings::new()
//!     .with_model("text-embedding-3-small");
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

use async_openai::{
    config::OpenAIConfig,
    types::{CreateEmbeddingRequest, EmbeddingInput},
    Client,
};
use async_trait::async_trait;
use dashflow::core::config_loader::env_vars::{env_string, OPENAI_API_KEY};
use dashflow::core::{
    embeddings::Embeddings,
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
    Error as DashFlowError,
};
use std::sync::Arc;

/// `OpenAI` embedding model integration.
///
/// Supports the following models:
/// - `text-embedding-3-small`: `OpenAI`'s newest and most efficient small embedding model (1536 dimensions)
/// - `text-embedding-3-large`: `OpenAI`'s most capable embedding model (3072 dimensions, configurable)
/// - `text-embedding-ada-002`: Legacy model (1536 dimensions)
///
/// # Configuration
///
/// The API key can be set via:
/// - Constructor: `OpenAIEmbeddings::new().with_api_key("sk-...")`
/// - Environment: `OPENAI_API_KEY`
///
/// # Batching
///
/// This implementation automatically batches requests to optimize throughput.
/// The default batch size is 512 texts per request, which can be configured
/// with `with_chunk_size()`.
///
/// # Dimensions
///
/// For `text-embedding-3-small` and `text-embedding-3-large`, you can configure
/// the output dimensionality with `with_dimensions()`. This allows you to reduce
/// the embedding size for storage efficiency while maintaining most of the
/// semantic information.
///
/// # See Also
///
/// - [`Embeddings`] - The trait implemented by this type
/// - [`ChatOpenAI`](crate::ChatOpenAI) - Chat completions API
/// - [`RetryPolicy`] - Configure retry behavior
/// - [`dashflow_voyage`](https://docs.rs/dashflow-voyage) - Voyage AI embeddings (alternative)
pub struct OpenAIEmbeddings {
    /// The `OpenAI` client
    client: Client<OpenAIConfig>,
    /// Model name (e.g., "text-embedding-3-small")
    model: String,
    /// Maximum number of texts to embed in a single API request
    chunk_size: usize,
    /// Optional: The number of dimensions for the output embeddings
    /// Only supported in text-embedding-3 and later models
    dimensions: Option<u32>,
    /// Retry policy for API calls
    retry_policy: RetryPolicy,
    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl OpenAIEmbeddings {
    /// Try to create a new `OpenAI` embeddings instance with default settings.
    ///
    /// Defaults:
    /// - Model: `text-embedding-ada-002`
    /// - Chunk size: 512
    /// - API key: from `OPENAI_API_KEY` environment variable
    ///
    /// # Errors
    ///
    /// Returns an error if `OPENAI_API_KEY` environment variable is not set.
    /// Use `with_api_key()` to set the key explicitly after creation.
    pub fn try_new() -> Result<Self, DashFlowError> {
        let api_key = env_string(OPENAI_API_KEY).ok_or_else(|| {
            DashFlowError::config(format!("{OPENAI_API_KEY} environment variable must be set"))
        })?;

        let config = OpenAIConfig::new().with_api_key(api_key);
        let client = Client::with_config(config);

        Ok(Self {
            client,
            model: "text-embedding-ada-002".to_string(),
            chunk_size: 512,
            dimensions: None,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        })
    }

    /// Create a new `OpenAI` embeddings instance with default settings.
    ///
    /// Defaults:
    /// - Model: `text-embedding-ada-002`
    /// - Chunk size: 512
    /// - API key: from `OPENAI_API_KEY` environment variable
    ///
    /// # Panics
    ///
    /// Panics if `OPENAI_API_KEY` environment variable is not set.
    /// Use `try_new()` for a fallible constructor, or `with_api_key()` to set the key explicitly.
    #[must_use]
    #[allow(clippy::expect_used)] // Intentional panic when API key missing - documented behavior
    pub fn new() -> Self {
        Self::try_new().expect("OPENAI_API_KEY environment variable must be set")
    }

    /// Set the model name.
    ///
    /// # Supported Models
    ///
    /// - `text-embedding-3-small`: 1536 dimensions, most efficient
    /// - `text-embedding-3-large`: 3072 dimensions (default), highest quality
    /// - `text-embedding-ada-002`: 1536 dimensions, legacy model
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_openai::embeddings::OpenAIEmbeddings;
    /// let embedder = OpenAIEmbeddings::new()
    ///     .with_model("text-embedding-3-small");
    /// ```
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the API key explicitly.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_openai::embeddings::OpenAIEmbeddings;
    /// let embedder = OpenAIEmbeddings::new()
    ///     .with_api_key("sk-...");
    /// ```
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        let config = OpenAIConfig::new().with_api_key(api_key.into());
        self.client = Client::with_config(config);
        self
    }

    /// Set the batch size for embedding requests.
    ///
    /// `OpenAI`'s API accepts up to 2048 texts per request, but smaller batches
    /// may be more efficient depending on your use case. The default is 512.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_openai::embeddings::OpenAIEmbeddings;
    /// let embedder = OpenAIEmbeddings::new()
    ///     .with_chunk_size(100);
    /// ```
    #[must_use]
    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    /// Set the output dimensionality for embeddings.
    ///
    /// Only supported for `text-embedding-3-small` and `text-embedding-3-large`.
    /// Allows you to reduce the embedding size while maintaining most semantic
    /// information, useful for storage optimization.
    ///
    /// # Valid Dimensions
    ///
    /// - `text-embedding-3-small`: 512 to 1536 (default: 1536)
    /// - `text-embedding-3-large`: 256 to 3072 (default: 3072)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_openai::embeddings::OpenAIEmbeddings;
    /// let embedder = OpenAIEmbeddings::new()
    ///     .with_model("text-embedding-3-small")
    ///     .with_dimensions(512);  // Reduce from 1536 to 512
    /// ```
    #[must_use]
    pub fn with_dimensions(mut self, dimensions: u32) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    /// Set the retry policy for API calls.
    ///
    /// Default is exponential backoff with 3 retries.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_openai::embeddings::OpenAIEmbeddings;
    /// # use dashflow::core::retry::RetryPolicy;
    /// let embedder = OpenAIEmbeddings::new()
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
    /// # use dashflow_openai::embeddings::OpenAIEmbeddings;
    /// # use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// # use std::time::Duration;
    /// # use std::sync::Arc;
    /// let rate_limiter = InMemoryRateLimiter::new(
    ///     10.0,  // 10 requests per second
    ///     Duration::from_millis(100),  // Check every 100ms
    ///     20.0,  // Max burst of 20 requests
    /// );
    ///
    /// let embedder = OpenAIEmbeddings::new()
    ///     .with_rate_limiter(Arc::new(rate_limiter));
    /// ```
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }
}

#[allow(clippy::disallowed_methods)] // Default reads OPENAI_API_KEY from environment
impl Default for OpenAIEmbeddings {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAIEmbeddings {
    /// Create `OpenAI` embeddings from configuration
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use dashflow::core::config_loader::{DashFlowConfig, EmbeddingConfig};
    /// use dashflow_openai::embeddings::OpenAIEmbeddings;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let yaml = r#"
    /// embeddings:
    ///   default:
    ///     type: openai
    ///     model: text-embedding-3-small
    ///     api_key:
    ///       env: OPENAI_API_KEY
    ///     batch_size: 64
    /// "#;
    ///
    /// let config = DashFlowConfig::from_yaml(yaml)?;
    /// let embedding_config = config.get_embedding("default").unwrap();
    /// let embedder = OpenAIEmbeddings::from_config(embedding_config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_config(
        config: &dashflow::core::config_loader::EmbeddingConfig,
    ) -> Result<Self, DashFlowError> {
        match config {
            dashflow::core::config_loader::EmbeddingConfig::OpenAI {
                model,
                api_key,
                batch_size,
            } => {
                let api_key_str = api_key.resolve().map_err(|e| {
                    DashFlowError::Configuration(format!("Failed to resolve API key: {e}"))
                })?;

                #[allow(clippy::disallowed_methods)] // new() may read defaults from env
                let embedder = OpenAIEmbeddings::new()
                    .with_api_key(api_key_str)
                    .with_model(model.clone())
                    .with_chunk_size(*batch_size);

                Ok(embedder)
            }
            _ => Err(DashFlowError::Configuration(
                "Expected OpenAI embedding config".to_string(),
            )),
        }
    }
}

#[async_trait]
impl Embeddings for OpenAIEmbeddings {
    async fn _embed_documents(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, DashFlowError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        // Process in batches
        for chunk in texts.chunks(self.chunk_size) {
            let input = EmbeddingInput::StringArray(chunk.to_vec());

            let request = CreateEmbeddingRequest {
                model: self.model.clone(),
                input,
                encoding_format: None,
                dimensions: self.dimensions,
                user: None,
            };

            // Acquire rate limiter token if configured
            if let Some(limiter) = &self.rate_limiter {
                limiter.acquire().await;
            }

            // Execute with retry logic
            let response = with_retry(&self.retry_policy, || async {
                self.client
                    .embeddings()
                    .create(request.clone())
                    .await
                    .map_err(|e| DashFlowError::api(format!("OpenAI API error: {e}")))
            })
            .await?;

            // Extract embeddings from response
            for data in response.data {
                all_embeddings.push(data.embedding);
            }
        }

        Ok(all_embeddings)
    }

    async fn _embed_query(
        &self,
        text: &str,
    ) -> Result<Vec<f32>, DashFlowError> {
        let texts = vec![text.to_string()];
        let mut embeddings = self._embed_documents(&texts).await?;

        embeddings
            .pop()
            .ok_or_else(|| DashFlowError::api("No embedding returned from OpenAI"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::disallowed_methods)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    // Mutex to serialize env var access across tests.
    // Tests that manipulate OPENAI_API_KEY must acquire this lock.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    #[ignore = "requires OPENAI_API_KEY"]
    fn test_default_constructor() {
        let embedder = OpenAIEmbeddings::new();
        assert_eq!(embedder.model, "text-embedding-ada-002");
        assert_eq!(embedder.chunk_size, 512);
        assert_eq!(embedder.dimensions, None);
    }

    #[test]
    fn test_with_model() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Set a dummy API key for testing builder methods
        env::set_var("OPENAI_API_KEY", "sk-test");

        let embedder = OpenAIEmbeddings::new().with_model("text-embedding-3-small");

        assert_eq!(embedder.model, "text-embedding-3-small");
    }

    #[test]
    fn test_with_chunk_size() {
        let _guard = ENV_MUTEX.lock().unwrap();
        env::set_var("OPENAI_API_KEY", "sk-test");

        let embedder = OpenAIEmbeddings::new().with_chunk_size(100);

        assert_eq!(embedder.chunk_size, 100);
    }

    #[test]
    fn test_with_dimensions() {
        let _guard = ENV_MUTEX.lock().unwrap();
        env::set_var("OPENAI_API_KEY", "sk-test");

        let embedder = OpenAIEmbeddings::new()
            .with_model("text-embedding-3-small")
            .with_dimensions(512);

        assert_eq!(embedder.dimensions, Some(512));
    }

    #[test]
    fn test_builder_chaining() {
        let _guard = ENV_MUTEX.lock().unwrap();
        env::set_var("OPENAI_API_KEY", "sk-test");

        let embedder = OpenAIEmbeddings::new()
            .with_model("text-embedding-3-large")
            .with_chunk_size(256)
            .with_dimensions(1024);

        assert_eq!(embedder.model, "text-embedding-3-large");
        assert_eq!(embedder.chunk_size, 256);
        assert_eq!(embedder.dimensions, Some(1024));
    }

    #[test]
    fn test_with_api_key() {
        let _guard = ENV_MUTEX.lock().unwrap();
        env::set_var("OPENAI_API_KEY", "sk-original");

        let embedder = OpenAIEmbeddings::new().with_api_key("sk-custom");

        // We can't easily test the internal client configuration, but we can verify
        // the method doesn't panic
        assert_eq!(embedder.model, "text-embedding-ada-002");
    }

    #[test]
    fn test_with_retry_policy() {
        let _guard = ENV_MUTEX.lock().unwrap();
        env::set_var("OPENAI_API_KEY", "sk-test");

        let embedder = OpenAIEmbeddings::new().with_retry_policy(RetryPolicy::exponential(5));

        // Verify default retry policy is replaced (can't directly test private field,
        // but we can verify method doesn't panic)
        assert_eq!(embedder.model, "text-embedding-ada-002");
    }

    #[test]
    fn test_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let _guard = ENV_MUTEX.lock().unwrap();
        env::set_var("OPENAI_API_KEY", "sk-test");

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let embedder = OpenAIEmbeddings::new().with_rate_limiter(rate_limiter);

        // Verify rate limiter is set (can't directly test private field,
        // but we can verify method doesn't panic)
        assert_eq!(embedder.model, "text-embedding-ada-002");
    }
}

/// Standard conformance tests for embeddings
///
/// These tests verify that OpenAIEmbeddings behaves consistently with other
/// Embeddings implementations across the DashFlow ecosystem.
#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod standard_tests {
    use super::*;
    use dashflow_standard_tests::embeddings_tests::*;

    /// Helper function to create a test embeddings model
    fn create_test_embeddings() -> OpenAIEmbeddings {
        OpenAIEmbeddings::new().with_model("text-embedding-3-small") // Cost-effective model
    }

    /// Standard Test 1: Embed single document
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_embed_query_standard() {
        test_embed_query(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 2: Embed multiple documents
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_embed_documents_standard() {
        test_embed_documents(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 3: Empty input handling
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_empty_input_standard() {
        test_empty_input(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 4: Dimension consistency
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_dimension_consistency_standard() {
        test_dimension_consistency(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 5: Semantic similarity
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_semantic_similarity_standard() {
        test_semantic_similarity(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 6: Large text handling
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_large_text_standard() {
        test_large_text(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 7: Special characters
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_special_characters_embeddings_standard() {
        test_special_characters_embeddings(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 8: Batch consistency
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_batch_consistency_standard() {
        test_batch_consistency(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 9: Whitespace handling
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_whitespace_standard() {
        test_whitespace(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 10: Repeated embeddings
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_repeated_embeddings_standard() {
        test_repeated_embeddings(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 11: Concurrent embeddings
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_concurrent_embeddings_standard() {
        test_concurrent_embeddings(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 12: Numeric text
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_numeric_text_standard() {
        test_numeric_text(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 13: Single character
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_single_character_standard() {
        test_single_character(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 14: Large batch
    #[tokio::test]
    #[ignore = "requires OPENAI_API_KEY"]
    async fn test_large_batch_embeddings_standard() {
        test_large_batch_embeddings(Arc::new(create_test_embeddings())).await;
    }
}
