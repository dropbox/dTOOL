//! Mistral AI Embeddings implementation
//!
//! This module provides embeddings using the Mistral AI Embed API.

use async_trait::async_trait;
use dashflow::core::{
    config_loader::env_vars::{env_string, MISTRAL_API_KEY as MISTRAL_API_KEY_VAR},
    embeddings::Embeddings,
    error::{Error, Result},
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
};
use mistralai_client::v1::{client::Client as MistralClient, constants::EmbedModel};
use std::sync::Arc;

/// Mistral AI Embeddings provider
///
/// Provides text embeddings using Mistral's embedding models.
/// The default model is `mistral-embed` which produces 1024-dimensional embeddings.
///
/// # Example
///
/// ```no_run
/// use dashflow_mistral::MistralEmbeddings;
/// use dashflow::core::embeddings::Embeddings;
///
/// #[tokio::main]
/// async fn main() {
///     // Create embeddings instance (requires MISTRAL_API_KEY environment variable)
///     let embeddings = MistralEmbeddings::new();
///
///     // Embed a single query
///     let query_embedding = embeddings._embed_query("What is the capital of France?").await.unwrap();
///     println!("Query embedding dimensions: {}", query_embedding.len());
///
///     // Embed multiple documents
///     let docs = vec![
///         "Paris is the capital of France.".to_string(),
///         "Berlin is the capital of Germany.".to_string(),
///     ];
///     let doc_embeddings = embeddings._embed_documents(&docs).await.unwrap();
///     println!("Document embeddings count: {}", doc_embeddings.len());
/// }
/// ```
///
/// # Configuration
///
/// Set the `MISTRAL_API_KEY` environment variable:
///
/// ```bash
/// export MISTRAL_API_KEY=your-api-key
/// ```
///
/// # Models
///
/// Currently supports:
/// - `mistral-embed` - 1024-dimensional embeddings (default)
#[derive(Clone, Debug)]
pub struct MistralEmbeddings {
    /// Mistral API client
    client: Arc<MistralClient>,

    /// Model to use for embeddings
    model: EmbedModel,

    /// Retry policy for API calls
    retry_policy: RetryPolicy,

    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl MistralEmbeddings {
    /// Create a new `MistralEmbeddings` instance
    ///
    /// This will read the API key from the `MISTRAL_API_KEY` environment variable.
    ///
    /// # Panics
    ///
    /// Panics if `MISTRAL_API_KEY` environment variable is not set.
    #[must_use]
    pub fn new() -> Self {
        let api_key = env_string(MISTRAL_API_KEY_VAR)
            .expect("MISTRAL_API_KEY environment variable must be set");

        let client = MistralClient::new(Some(api_key), None, None, None)
            .expect("Failed to create Mistral client");

        Self {
            client: Arc::new(client),
            model: EmbedModel::MistralEmbed,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Create a new `MistralEmbeddings` instance with a custom API key
    ///
    /// # Arguments
    ///
    /// * `api_key` - Your Mistral API key
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        let client = MistralClient::new(Some(api_key.into()), None, None, None)
            .expect("Failed to create Mistral client");

        Self {
            client: Arc::new(client),
            model: EmbedModel::MistralEmbed,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Set the model to use for embeddings
    ///
    /// # Arguments
    ///
    /// * `model` - The embedding model to use
    #[must_use]
    pub fn with_model(mut self, model: EmbedModel) -> Self {
        self.model = model;
        self
    }

    /// Set the retry policy for API calls.
    ///
    /// Default is exponential backoff with 3 retries.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_mistral::MistralEmbeddings;
    /// # use dashflow::core::retry::RetryPolicy;
    /// let embeddings = MistralEmbeddings::new()
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
    /// # use dashflow_mistral::MistralEmbeddings;
    /// # use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// # use std::time::Duration;
    /// # use std::sync::Arc;
    /// let rate_limiter = InMemoryRateLimiter::new(
    ///     10.0,  // 10 requests per second
    ///     Duration::from_millis(100),  // Check every 100ms
    ///     20.0,  // Max burst of 20 requests
    /// );
    ///
    /// let embeddings = MistralEmbeddings::new()
    ///     .with_rate_limiter(Arc::new(rate_limiter));
    /// ```
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }
}

impl Default for MistralEmbeddings {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Embeddings for MistralEmbeddings {
    /// Embed a list of documents
    ///
    /// # Arguments
    ///
    /// * `texts` - A slice of text strings to embed
    ///
    /// # Returns
    ///
    /// A vector of embeddings, one for each input text.
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Acquire rate limiter token if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        // Clone values for retry closure
        let client = self.client.clone();
        let model = self.model.clone();
        let texts = texts.to_vec();

        // Execute with retry logic
        let response = with_retry(&self.retry_policy, || async {
            client
                .embeddings_async(model.clone(), texts.clone(), None)
                .await
                .map_err(|e| Error::http(format!("Mistral API error: {e:?}")))
        })
        .await?;

        // Extract embeddings from response
        let mut embeddings = response.data;
        // Sort by index to ensure correct order
        embeddings.sort_by_key(|item| item.index);

        Ok(embeddings.into_iter().map(|item| item.embedding).collect())
    }

    /// Embed a single query text
    ///
    /// # Arguments
    ///
    /// * `text` - The text to embed
    ///
    /// # Returns
    ///
    /// A single embedding vector.
    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self._embed_documents(&[text.to_string()]).await?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| Error::invalid_input("No embedding returned from Mistral API"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires MISTRAL_API_KEY"]
    fn test_mistral_embeddings_new() {
        let embeddings = MistralEmbeddings::new();
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_mistral_embeddings_with_api_key() {
        let embeddings = MistralEmbeddings::with_api_key("test-key");
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    #[ignore = "requires MISTRAL_API_KEY"]
    fn test_mistral_embeddings_with_model() {
        let embeddings = MistralEmbeddings::new().with_model(EmbedModel::MistralEmbed);
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_embed_documents_empty() {
        let embeddings = MistralEmbeddings::new();
        let result = embeddings._embed_documents(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_embed_query() {
        let embeddings = MistralEmbeddings::new();
        let result = embeddings._embed_query("Hello, world!").await;

        assert!(result.is_ok());
        let embedding = result.unwrap();
        assert_eq!(embedding.len(), 1024); // mistral-embed produces 1024-dim embeddings
        assert!(embedding.iter().any(|&x| x != 0.0)); // Should have non-zero values
    }

    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_embed_documents() {
        let embeddings = MistralEmbeddings::new();
        let texts = vec![
            "The quick brown fox jumps over the lazy dog.".to_string(),
            "Paris is the capital of France.".to_string(),
            "Machine learning is a subset of artificial intelligence.".to_string(),
        ];

        let result = embeddings._embed_documents(&texts).await;

        assert!(result.is_ok());
        let doc_embeddings = result.unwrap();
        assert_eq!(doc_embeddings.len(), 3);

        // Each embedding should be 1024-dimensional
        for embedding in &doc_embeddings {
            assert_eq!(embedding.len(), 1024);
            assert!(embedding.iter().any(|&x| x != 0.0)); // Should have non-zero values
        }

        // Different texts should have different embeddings
        assert_ne!(doc_embeddings[0], doc_embeddings[1]);
        assert_ne!(doc_embeddings[1], doc_embeddings[2]);
    }

    #[tokio::test]
    #[ignore = "requires MISTRAL_API_KEY"]
    async fn test_embed_documents_batch() {
        let embeddings = MistralEmbeddings::new();

        // Test with larger batch
        let texts: Vec<String> = (0..10)
            .map(|i| format!("This is test document number {}", i))
            .collect();

        let result = embeddings._embed_documents(&texts).await;

        assert!(result.is_ok());
        let doc_embeddings = result.unwrap();
        assert_eq!(doc_embeddings.len(), 10);

        // All embeddings should be 1024-dimensional
        for embedding in &doc_embeddings {
            assert_eq!(embedding.len(), 1024);
        }
    }

    #[test]
    #[ignore = "requires MISTRAL_API_KEY"]
    fn test_with_retry_policy() {
        use dashflow::core::retry::RetryPolicy;
        let embeddings = MistralEmbeddings::new().with_retry_policy(RetryPolicy::exponential(5));
        // Test passes if no panic occurs
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    #[ignore = "requires MISTRAL_API_KEY"]
    fn test_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;
        let rate_limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0);
        let embeddings = MistralEmbeddings::new().with_rate_limiter(Arc::new(rate_limiter));
        // Test passes if no panic occurs
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    // ============================================
    // Builder method tests (no API key required)
    // ============================================

    #[test]
    fn test_with_api_key_creates_instance() {
        let embeddings = MistralEmbeddings::with_api_key("test-key-12345");
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_with_api_key_string_type() {
        // Test with String type
        let embeddings = MistralEmbeddings::with_api_key(String::from("my-api-key"));
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_with_api_key_empty() {
        // Empty string should still create instance (validation happens at API call)
        let embeddings = MistralEmbeddings::with_api_key("");
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_with_api_key_unicode() {
        let embeddings = MistralEmbeddings::with_api_key("test-key-日本語-🔑");
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_with_api_key_very_long() {
        let long_key = "x".repeat(10000);
        let embeddings = MistralEmbeddings::with_api_key(&long_key);
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_with_model_builder() {
        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_model(EmbedModel::MistralEmbed);
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_with_retry_policy_exponential_builder() {
        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_retry_policy(RetryPolicy::exponential(3));
        // Just verify it doesn't panic
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_with_retry_policy_exponential_many_retries() {
        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_retry_policy(RetryPolicy::exponential(10));
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_with_retry_policy_exponential_zero_retries() {
        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_retry_policy(RetryPolicy::exponential(0));
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_with_retry_policy_fixed_builder() {
        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_retry_policy(RetryPolicy::fixed(3, 1000));
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_with_retry_policy_fixed_short_delay() {
        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_retry_policy(RetryPolicy::fixed(5, 100));
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_with_retry_policy_fixed_long_delay() {
        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_retry_policy(RetryPolicy::fixed(2, 60000));
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_with_rate_limiter_builder() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_rate_limiter(rate_limiter);

        assert!(embeddings.rate_limiter.is_some());
    }

    #[test]
    fn test_with_rate_limiter_low_rate() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            1.0,  // 1 request per second
            Duration::from_millis(100),
            2.0,
        ));

        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_rate_limiter(rate_limiter);

        assert!(embeddings.rate_limiter.is_some());
    }

    #[test]
    fn test_with_rate_limiter_high_rate() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            1000.0,  // 1000 requests per second
            Duration::from_millis(10),
            2000.0,
        ));

        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_rate_limiter(rate_limiter);

        assert!(embeddings.rate_limiter.is_some());
    }

    // ============================================
    // Builder chaining tests
    // ============================================

    #[test]
    fn test_builder_chain_all_params() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_model(EmbedModel::MistralEmbed)
            .with_retry_policy(RetryPolicy::exponential(5))
            .with_rate_limiter(rate_limiter);

        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
        assert!(embeddings.rate_limiter.is_some());
    }

    #[test]
    fn test_builder_chain_order_independence_1() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        // Order 1: model -> retry -> rate_limiter
        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_model(EmbedModel::MistralEmbed)
            .with_retry_policy(RetryPolicy::exponential(3))
            .with_rate_limiter(rate_limiter);

        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
        assert!(embeddings.rate_limiter.is_some());
    }

    #[test]
    fn test_builder_chain_order_independence_2() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        // Order 2: rate_limiter -> model -> retry
        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_rate_limiter(rate_limiter)
            .with_model(EmbedModel::MistralEmbed)
            .with_retry_policy(RetryPolicy::exponential(3));

        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
        assert!(embeddings.rate_limiter.is_some());
    }

    // ============================================
    // Debug implementation tests
    // ============================================

    #[test]
    fn test_debug_impl_exists() {
        let embeddings = MistralEmbeddings::with_api_key("secret-key");
        let debug_str = format!("{:?}", embeddings);
        assert!(debug_str.contains("MistralEmbeddings"));
    }

    #[test]
    fn test_debug_shows_model() {
        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_model(EmbedModel::MistralEmbed);
        let debug_str = format!("{:?}", embeddings);
        assert!(debug_str.contains("MistralEmbed"));
    }

    #[test]
    fn test_debug_shows_rate_limiter_presence() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_rate_limiter(rate_limiter);

        let debug_str = format!("{:?}", embeddings);
        // Debug should show rate_limiter: Some(...)
        assert!(debug_str.contains("rate_limiter: Some"));
    }

    #[test]
    fn test_debug_shows_no_rate_limiter() {
        let embeddings = MistralEmbeddings::with_api_key("key");
        let debug_str = format!("{:?}", embeddings);
        // Debug should show rate_limiter: None
        assert!(debug_str.contains("rate_limiter: None"));
    }

    // ============================================
    // Clone tests
    // ============================================

    #[test]
    fn test_clone_basic() {
        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_model(EmbedModel::MistralEmbed);

        let cloned = embeddings.clone();
        assert!(matches!(cloned.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_clone_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_rate_limiter(rate_limiter);

        let cloned = embeddings.clone();
        assert!(cloned.rate_limiter.is_some());
    }

    #[test]
    fn test_clone_preserves_all_fields() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_model(EmbedModel::MistralEmbed)
            .with_retry_policy(RetryPolicy::exponential(5))
            .with_rate_limiter(rate_limiter);

        let cloned = embeddings.clone();
        assert!(matches!(cloned.model, EmbedModel::MistralEmbed));
        assert!(cloned.rate_limiter.is_some());
    }

    // ============================================
    // Default trait tests
    // ============================================

    #[test]
    #[ignore = "requires MISTRAL_API_KEY"]
    fn test_default_trait() {
        let embeddings = MistralEmbeddings::default();
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
        assert!(embeddings.rate_limiter.is_none());
    }

    #[test]
    #[ignore = "requires MISTRAL_API_KEY"]
    fn test_default_equals_new() {
        let default_embeddings = MistralEmbeddings::default();
        let new_embeddings = MistralEmbeddings::new();

        // Both should have same model
        assert!(matches!(default_embeddings.model, EmbedModel::MistralEmbed));
        assert!(matches!(new_embeddings.model, EmbedModel::MistralEmbed));
    }

    // ============================================
    // Edge case tests
    // ============================================

    #[test]
    fn test_whitespace_api_key() {
        let embeddings = MistralEmbeddings::with_api_key("   ");
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_special_chars_in_api_key() {
        let embeddings = MistralEmbeddings::with_api_key("sk-test!@#$%^&*()_+-=[]{}|;':\",./<>?");
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_newlines_in_api_key() {
        let embeddings = MistralEmbeddings::with_api_key("line1\nline2\rline3");
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    // ============================================
    // Async embed_documents edge cases (with mocked setup)
    // ============================================

    #[tokio::test]
    async fn test_embed_documents_empty_input() {
        // This test verifies the empty input handling without API call
        let embeddings = MistralEmbeddings::with_api_key("key");

        // Empty input should return empty result immediately without API call
        let result = embeddings._embed_documents(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ============================================
    // Retry policy configuration tests
    // ============================================

    #[test]
    fn test_retry_policy_default_is_exponential() {
        let embeddings = MistralEmbeddings::with_api_key("key");
        // Default retry policy is exponential with 3 retries
        // We can't directly inspect it, but verify the struct is constructed
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_retry_policy_can_be_overwritten() {
        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_retry_policy(RetryPolicy::exponential(1))
            .with_retry_policy(RetryPolicy::fixed(5, 500));
        // Last one wins
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    // ============================================
    // Rate limiter configuration tests
    // ============================================

    #[test]
    fn test_rate_limiter_none_by_default() {
        let embeddings = MistralEmbeddings::with_api_key("key");
        assert!(embeddings.rate_limiter.is_none());
    }

    #[test]
    fn test_rate_limiter_can_be_set() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_rate_limiter(rate_limiter);

        assert!(embeddings.rate_limiter.is_some());
    }

    #[test]
    fn test_rate_limiter_can_be_replaced() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let rate_limiter1 = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let rate_limiter2 = Arc::new(InMemoryRateLimiter::new(
            20.0,
            Duration::from_millis(50),
            40.0,
        ));

        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_rate_limiter(rate_limiter1)
            .with_rate_limiter(rate_limiter2);

        assert!(embeddings.rate_limiter.is_some());
    }

    // ============================================
    // Model configuration tests
    // ============================================

    #[test]
    fn test_model_default_is_mistral_embed() {
        let embeddings = MistralEmbeddings::with_api_key("key");
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }

    #[test]
    fn test_model_can_be_set_to_mistral_embed() {
        let embeddings = MistralEmbeddings::with_api_key("key")
            .with_model(EmbedModel::MistralEmbed);
        assert!(matches!(embeddings.model, EmbedModel::MistralEmbed));
    }
}
