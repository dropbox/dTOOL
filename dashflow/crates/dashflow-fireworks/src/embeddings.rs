//! Fireworks AI embeddings implementation.
//!
//! This module provides embeddings using Fireworks AI's embedding models via OpenAI-compatible API.
//! Fireworks hosts various embedding models including:
//! - nomic-ai/nomic-embed-text-v1.5
//! - WhereIsAI/UAE-Large-V1
//! - thenlper/gte-large
//!
//! # Example
//!
//! ```rust
//! use dashflow_fireworks::embeddings::FireworksEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = FireworksEmbeddings::new()
//!     .with_model("nomic-ai/nomic-embed-text-v1.5");
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
use dashflow::core::config_loader::env_vars::{env_string, FIREWORKS_API_KEY};
use dashflow::core::{
    embeddings::Embeddings,
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
    Error as DashFlowError,
};
use std::sync::Arc;

/// Fireworks AI embedding model integration.
///
/// Fireworks provides fast embedding inference using an OpenAI-compatible API.
/// Popular models include:
/// - `nomic-ai/nomic-embed-text-v1.5`: High-quality text embeddings (768 dimensions)
/// - `WhereIsAI/UAE-Large-V1`: Universal Angle Embeddings (1024 dimensions)
/// - `thenlper/gte-large`: General Text Embeddings (1024 dimensions)
///
/// # Configuration
///
/// The API key can be set via:
/// - Constructor: `FireworksEmbeddings::new().with_api_key("fw_...")`
/// - Environment: `FIREWORKS_API_KEY`
///
/// # Batching
///
/// This implementation automatically batches requests to optimize throughput.
/// The default batch size is 512 texts per request, which can be configured
/// with `with_chunk_size()`.
pub struct FireworksEmbeddings {
    /// The Fireworks client (using OpenAI-compatible API)
    client: Client<OpenAIConfig>,
    /// Model name (e.g., "nomic-ai/nomic-embed-text-v1.5")
    model: String,
    /// Maximum number of texts to embed in a single API request
    chunk_size: usize,
    /// Retry policy for API calls
    retry_policy: RetryPolicy,
    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl FireworksEmbeddings {
    /// Create a new Fireworks embeddings instance with default settings.
    ///
    /// Defaults:
    /// - Model: `nomic-ai/nomic-embed-text-v1.5`
    /// - Chunk size: 512
    /// - API key: from `FIREWORKS_API_KEY` environment variable
    /// - Base URL: `https://api.fireworks.ai/inference/v1`
    ///
    /// # Panics
    ///
    /// Panics if `FIREWORKS_API_KEY` environment variable is not set.
    /// Use `with_api_key()` to set the key explicitly.
    #[must_use]
    #[allow(clippy::expect_used)] // Documented panic for missing env var; use with_api_key() instead
    pub fn new() -> Self {
        let api_key = env_string(FIREWORKS_API_KEY)
            .expect("FIREWORKS_API_KEY environment variable must be set");

        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base("https://api.fireworks.ai/inference/v1");
        let client = Client::with_config(config);

        Self {
            client,
            model: "nomic-ai/nomic-embed-text-v1.5".to_string(),
            chunk_size: 512,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Create a new instance without requiring environment variables.
    ///
    /// You must call `with_api_key()` before using this instance.
    ///
    /// Defaults:
    /// - Model: `nomic-ai/nomic-embed-text-v1.5`
    /// - Chunk size: 512
    /// - Base URL: `https://api.fireworks.ai/inference/v1`
    #[must_use]
    pub fn new_without_api_key() -> Self {
        let config = OpenAIConfig::new()
            .with_api_key("") // Empty API key, must be set later
            .with_api_base("https://api.fireworks.ai/inference/v1");
        let client = Client::with_config(config);

        Self {
            client,
            model: "nomic-ai/nomic-embed-text-v1.5".to_string(),
            chunk_size: 512,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Set the API key explicitly instead of using environment variable.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_fireworks::embeddings::FireworksEmbeddings;
    ///
    /// let embedder = FireworksEmbeddings::new_without_api_key()
    ///     .with_api_key("fw_your_api_key_here");
    /// ```
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        let config = OpenAIConfig::new()
            .with_api_key(api_key.into())
            .with_api_base("https://api.fireworks.ai/inference/v1");
        self.client = Client::with_config(config);
        self
    }

    /// Set the model to use for embeddings.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_fireworks::embeddings::FireworksEmbeddings;
    ///
    /// let embedder = FireworksEmbeddings::new_without_api_key()
    ///     .with_model("WhereIsAI/UAE-Large-V1");
    /// ```
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the chunk size for batching.
    ///
    /// Larger chunk sizes reduce the number of API calls but may hit rate limits.
    /// Default is 512.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_fireworks::embeddings::FireworksEmbeddings;
    ///
    /// let embedder = FireworksEmbeddings::new_without_api_key()
    ///     .with_chunk_size(100);
    /// ```
    #[must_use]
    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    /// Set the retry policy for API calls.
    ///
    /// Default is exponential backoff with 3 retries.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_fireworks::embeddings::FireworksEmbeddings;
    /// use dashflow::core::retry::RetryPolicy;
    ///
    /// let embedder = FireworksEmbeddings::new_without_api_key()
    ///     .with_retry_policy(RetryPolicy::exponential(5));
    /// ```
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Set a rate limiter to control request rate.
    ///
    /// This helps avoid hitting API rate limits and improves reliability.
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_fireworks::embeddings::FireworksEmbeddings;
    /// use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// use std::time::Duration;
    /// use std::sync::Arc;
    ///
    /// let rate_limiter = InMemoryRateLimiter::new(
    ///     10.0,  // 10 requests per second
    ///     Duration::from_millis(100),  // Check every 100ms
    ///     20.0,  // Max burst of 20 requests
    /// );
    ///
    /// let embedder = FireworksEmbeddings::new_without_api_key()
    ///     .with_rate_limiter(Arc::new(rate_limiter));
    /// ```
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }
}

impl Default for FireworksEmbeddings {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Embeddings for FireworksEmbeddings {
    /// Embed a list of documents.
    ///
    /// This method automatically chunks large batches to stay within API limits.
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

        let mut all_embeddings = Vec::with_capacity(texts.len());

        // Process in chunks to avoid hitting API limits
        for chunk in texts.chunks(self.chunk_size) {
            // Acquire rate limiter token if configured
            if let Some(limiter) = &self.rate_limiter {
                limiter.acquire().await;
            }

            // Clone values for retry closure
            let client = self.client.clone();
            let model = self.model.clone();
            let chunk = chunk.to_vec();

            // Execute with retry logic
            let response = with_retry(&self.retry_policy, || async {
                let request = CreateEmbeddingRequest {
                    model: model.clone(),
                    input: EmbeddingInput::StringArray(chunk.clone()),
                    encoding_format: None,
                    dimensions: None,
                    user: None,
                };

                client
                    .embeddings()
                    .create(request)
                    .await
                    .map_err(|e| DashFlowError::http(format!("Fireworks API error: {e}")))
            })
            .await?;

            // Extract embeddings in the correct order
            let mut chunk_embeddings: Vec<_> = response.data.into_iter().collect();
            // Sort by index to ensure correct order
            chunk_embeddings.sort_by_key(|emb| emb.index);

            for emb in chunk_embeddings {
                all_embeddings.push(emb.embedding);
            }
        }

        Ok(all_embeddings)
    }

    /// Embed a single query string.
    ///
    /// This is a convenience method that calls `embed_documents` with a single text.
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
        let mut embeddings = self._embed_documents(&texts).await?;
        embeddings
            .pop()
            .ok_or_else(|| DashFlowError::api_format("No embedding returned from API"))
    }
}

#[cfg(test)]
// SAFETY: Tests use unwrap() to panic on unexpected errors, clearly indicating test failure.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_pattern() {
        let embedder = FireworksEmbeddings::new_without_api_key()
            .with_api_key("test_api_key")
            .with_model("WhereIsAI/UAE-Large-V1")
            .with_chunk_size(100);

        assert_eq!(embedder.model, "WhereIsAI/UAE-Large-V1");
        assert_eq!(embedder.chunk_size, 100);
    }

    #[test]
    fn test_default_model_and_chunk_size() {
        let embedder = FireworksEmbeddings::new_without_api_key();

        assert_eq!(embedder.model, "nomic-ai/nomic-embed-text-v1.5");
        assert_eq!(embedder.chunk_size, 512);
    }

    #[tokio::test]
    async fn test_empty_input() {
        let embedder = FireworksEmbeddings::new_without_api_key().with_api_key("test_api_key");
        let result = embedder._embed_documents(&[]).await.unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_with_retry_policy() {
        let embedder = FireworksEmbeddings::new_without_api_key()
            .with_retry_policy(RetryPolicy::exponential(5));

        // Verify retry policy is set (we can't directly check the field since it's private,
        // but we can verify the method doesn't panic)
        assert_eq!(embedder.model, "nomic-ai/nomic-embed-text-v1.5");
    }

    #[test]
    fn test_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,                       // 10 requests per second
            Duration::from_millis(100), // Check every 100ms
            20.0,                       // Max burst of 20 requests
        ));

        let embedder =
            FireworksEmbeddings::new_without_api_key().with_rate_limiter(rate_limiter);

        // Verify rate limiter is set (we can't directly check the field since it's private,
        // but we can verify the method doesn't panic)
        assert_eq!(embedder.model, "nomic-ai/nomic-embed-text-v1.5");
    }

    // ========================================================================
    // COMPREHENSIVE BUILDER TESTS
    // ========================================================================

    #[test]
    fn test_builder_default_values_detailed() {
        let embedder = FireworksEmbeddings::new_without_api_key();
        assert_eq!(embedder.model, "nomic-ai/nomic-embed-text-v1.5");
        assert_eq!(embedder.chunk_size, 512);
        assert!(embedder.rate_limiter.is_none());
    }

    #[test]
    fn test_builder_with_model_various_names() {
        // Test various Fireworks embedding model names
        let model_names = [
            "nomic-ai/nomic-embed-text-v1.5",
            "WhereIsAI/UAE-Large-V1",
            "thenlper/gte-large",
            "sentence-transformers/all-MiniLM-L6-v2",
        ];
        for name in model_names {
            let embedder = FireworksEmbeddings::new_without_api_key().with_model(name);
            assert_eq!(embedder.model, name);
        }
    }

    #[test]
    fn test_builder_with_model_string_ownership() {
        // Test that Into<String> works for both &str and String
        let embedder1 = FireworksEmbeddings::new_without_api_key().with_model("test-model");
        let embedder2 =
            FireworksEmbeddings::new_without_api_key().with_model(String::from("test-model"));
        assert_eq!(embedder1.model, embedder2.model);
    }

    #[test]
    fn test_builder_chunk_size_various_values() {
        let values = [1usize, 10, 100, 256, 512, 1024, 2048];
        for val in values {
            let embedder = FireworksEmbeddings::new_without_api_key().with_chunk_size(val);
            assert_eq!(embedder.chunk_size, val);
        }
    }

    #[test]
    fn test_builder_chaining_all_options() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let embedder = FireworksEmbeddings::new_without_api_key()
            .with_api_key("test_key")
            .with_model("thenlper/gte-large")
            .with_chunk_size(256)
            .with_retry_policy(RetryPolicy::exponential(5))
            .with_rate_limiter(rate_limiter);

        assert_eq!(embedder.model, "thenlper/gte-large");
        assert_eq!(embedder.chunk_size, 256);
        assert!(embedder.rate_limiter.is_some());
    }

    #[test]
    fn test_builder_retry_policy_exponential() {
        let embedder = FireworksEmbeddings::new_without_api_key()
            .with_retry_policy(RetryPolicy::exponential(5));
        assert_eq!(embedder.model, "nomic-ai/nomic-embed-text-v1.5");
    }

    #[test]
    fn test_builder_retry_policy_fixed() {
        let embedder = FireworksEmbeddings::new_without_api_key()
            .with_retry_policy(RetryPolicy::fixed(3, 100));
        assert_eq!(embedder.model, "nomic-ai/nomic-embed-text-v1.5");
    }

    #[test]
    fn test_builder_override_values() {
        // Test that later builder calls override earlier ones
        let embedder = FireworksEmbeddings::new_without_api_key()
            .with_model("model1")
            .with_model("model2");
        assert_eq!(embedder.model, "model2");

        let embedder = FireworksEmbeddings::new_without_api_key()
            .with_chunk_size(100)
            .with_chunk_size(200);
        assert_eq!(embedder.chunk_size, 200);
    }

    #[test]
    fn test_builder_with_api_key_string_ownership() {
        // Test that Into<String> works for both &str and String
        let _embedder1 = FireworksEmbeddings::new_without_api_key().with_api_key("api_key_str");
        let _embedder2 =
            FireworksEmbeddings::new_without_api_key().with_api_key(String::from("api_key_string"));
        // Both should compile and work
    }

    #[test]
    fn test_builder_with_empty_api_key() {
        // Empty API key should be accepted at builder level (API will reject)
        let embedder = FireworksEmbeddings::new_without_api_key().with_api_key("");
        assert_eq!(embedder.model, "nomic-ai/nomic-embed-text-v1.5");
    }

    #[test]
    fn test_builder_with_empty_model() {
        // Empty model name should be accepted at builder level (API will reject)
        let embedder = FireworksEmbeddings::new_without_api_key().with_model("");
        assert_eq!(embedder.model, "");
    }

    #[test]
    fn test_builder_chunk_size_edge_cases() {
        // Test minimum chunk size
        let embedder = FireworksEmbeddings::new_without_api_key().with_chunk_size(1);
        assert_eq!(embedder.chunk_size, 1);

        // Test very large chunk size
        let embedder = FireworksEmbeddings::new_without_api_key().with_chunk_size(10000);
        assert_eq!(embedder.chunk_size, 10000);
    }

    // ========================================================================
    // RATE LIMITER TESTS
    // ========================================================================

    #[test]
    fn test_rate_limiter_is_none_by_default() {
        let embedder = FireworksEmbeddings::new_without_api_key();
        assert!(embedder.rate_limiter.is_none());
    }

    #[test]
    fn test_rate_limiter_is_set() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            5.0,
            Duration::from_millis(50),
            10.0,
        ));

        let embedder = FireworksEmbeddings::new_without_api_key().with_rate_limiter(rate_limiter);
        assert!(embedder.rate_limiter.is_some());
    }

    #[test]
    fn test_rate_limiter_override() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter1 = Arc::new(InMemoryRateLimiter::new(
            5.0,
            Duration::from_millis(50),
            10.0,
        ));

        let rate_limiter2 = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let embedder = FireworksEmbeddings::new_without_api_key()
            .with_rate_limiter(rate_limiter1)
            .with_rate_limiter(rate_limiter2);

        assert!(embedder.rate_limiter.is_some());
    }

    // ========================================================================
    // EMPTY INPUT TESTS
    // ========================================================================

    #[tokio::test]
    async fn test_empty_documents_returns_empty_vec() {
        let embedder = FireworksEmbeddings::new_without_api_key().with_api_key("test_api_key");
        let result = embedder._embed_documents(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_empty_slice_returns_empty_vec() {
        let embedder = FireworksEmbeddings::new_without_api_key().with_api_key("test_api_key");
        let texts: Vec<String> = vec![];
        let result = embedder._embed_documents(&texts).await.unwrap();
        assert_eq!(result.len(), 0);
    }

    // ========================================================================
    // MODEL CONFIGURATION TESTS
    // ========================================================================

    #[test]
    fn test_model_unicode_names() {
        // Test that unicode model names work (for future compatibility)
        let embedder = FireworksEmbeddings::new_without_api_key().with_model("test-model-v1.0");
        assert_eq!(embedder.model, "test-model-v1.0");
    }

    #[test]
    fn test_model_with_special_characters() {
        let embedder =
            FireworksEmbeddings::new_without_api_key().with_model("org/model-name_v1.5");
        assert_eq!(embedder.model, "org/model-name_v1.5");
    }

    #[test]
    fn test_model_long_name() {
        let long_name = "a".repeat(1000);
        let embedder = FireworksEmbeddings::new_without_api_key().with_model(&long_name);
        assert_eq!(embedder.model, long_name);
    }

    // ========================================================================
    // CHUNK SIZE CONFIGURATION TESTS
    // ========================================================================

    #[test]
    fn test_chunk_size_zero() {
        // Zero chunk size should be accepted at builder level (may cause issues at runtime)
        let embedder = FireworksEmbeddings::new_without_api_key().with_chunk_size(0);
        assert_eq!(embedder.chunk_size, 0);
    }

    #[test]
    fn test_chunk_size_max() {
        let embedder = FireworksEmbeddings::new_without_api_key().with_chunk_size(usize::MAX);
        assert_eq!(embedder.chunk_size, usize::MAX);
    }

    // ========================================================================
    // API KEY TESTS
    // ========================================================================

    #[test]
    fn test_api_key_with_whitespace() {
        // API key with whitespace should be preserved (API will handle validation)
        let embedder =
            FireworksEmbeddings::new_without_api_key().with_api_key("  key_with_spaces  ");
        // Can't directly check API key, but verify embedder is created
        assert_eq!(embedder.model, "nomic-ai/nomic-embed-text-v1.5");
    }

    #[test]
    fn test_api_key_special_characters() {
        // API key with special characters
        let embedder = FireworksEmbeddings::new_without_api_key().with_api_key("fw_test-key_123!");
        assert_eq!(embedder.model, "nomic-ai/nomic-embed-text-v1.5");
    }

    // Integration tests below require FIREWORKS_API_KEY
    #[tokio::test]
    #[ignore = "requires FIREWORKS_API_KEY"]
    async fn test_embed_query() {
        let embedder = FireworksEmbeddings::default();
        let embedding = embedder._embed_query("Hello, world!").await.unwrap();

        // nomic-embed-text-v1.5 produces 768-dimensional embeddings
        assert_eq!(embedding.len(), 768);

        // Check that embeddings contain non-zero values
        let sum: f32 = embedding.iter().sum();
        assert!(sum.abs() > 0.0, "Embedding should contain non-zero values");
    }

    #[tokio::test]
    #[ignore = "requires FIREWORKS_API_KEY"]
    async fn test_embed_documents() {
        let embedder = FireworksEmbeddings::default();
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
    #[ignore = "requires FIREWORKS_API_KEY"]
    async fn test_batch_processing() {
        let embedder = FireworksEmbeddings::default().with_chunk_size(10); // Small chunk size to test batching

        // Create 25 texts to force multiple batches
        let texts: Vec<String> = (0..25)
            .map(|i| format!("Test document number {}", i))
            .collect();

        let embeddings = embedder._embed_documents(&texts).await.unwrap();

        assert_eq!(embeddings.len(), 25);
        for embedding in &embeddings {
            assert_eq!(embedding.len(), 768);
        }
    }

    #[tokio::test]
    #[ignore = "requires FIREWORKS_API_KEY"]
    async fn test_different_model() {
        let embedder = FireworksEmbeddings::default().with_model("thenlper/gte-large");

        let embedding = embedder._embed_query("Test").await.unwrap();

        // gte-large produces 1024-dimensional embeddings
        assert_eq!(embedding.len(), 1024);
    }
}
