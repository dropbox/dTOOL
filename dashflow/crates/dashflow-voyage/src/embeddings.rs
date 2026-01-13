//! Voyage AI embeddings implementation.
//!
//! This module provides embeddings using Voyage AI's embedding models, including:
//! - voyage-3.5: Latest general-purpose model
//! - voyage-3-large: High-performance large model
//! - voyage-3.5-lite: Efficient lightweight model
//! - voyage-code-3: Optimized for code
//! - voyage-finance-2: Optimized for financial documents
//! - voyage-law-2: Optimized for legal documents
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_voyage::VoyageEmbeddings;
//! use dashflow::{embed, embed_query};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = Arc::new(VoyageEmbeddings::new()
//!     .with_api_key(std::env::var("VOYAGE_API_KEY")?));
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
use dashflow::core::{
    config_loader::env_vars::{env_string, VOYAGE_API_KEY as VOYAGE_API_KEY_VAR},
    embeddings::Embeddings,
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
    Error as DashFlowError,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::VOYAGE_API_BASE;

const DEFAULT_MODEL: &str = "voyage-3.5";

/// Input type for embeddings, which optimizes the embedding for specific use cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    /// No specific optimization
    None,
    /// Optimized for search queries
    Query,
    /// Optimized for documents to be searched
    Document,
}

/// Voyage AI embedding model integration.
///
/// Supports the following models:
/// - `voyage-3.5`: Latest general-purpose model (1024 dimensions default)
/// - `voyage-3-large`: High-performance large model (1024 dimensions)
/// - `voyage-3.5-lite`: Efficient lightweight model (512 dimensions)
/// - `voyage-code-3`: Optimized for code (1024 dimensions)
/// - `voyage-finance-2`: Optimized for financial documents (1024 dimensions)
/// - `voyage-law-2`: Optimized for legal documents (1024 dimensions)
///
/// # Configuration
///
/// The API key can be set via:
/// - Constructor: `VoyageEmbeddings::new().with_api_key("...")`
/// - Environment: `VOYAGE_API_KEY`
///
/// # Input Types
///
/// Voyage AI embeddings support different input types that optimize the embedding
/// for specific use cases. Use `with_input_type()` to set the input type.
///
/// # Dimensions
///
/// You can configure the output dimensionality with `with_dimensions()` to
/// reduce embedding size while maintaining semantic information.
pub struct VoyageEmbeddings {
    /// API key for authentication
    api_key: Option<String>,
    /// Model name (e.g., "voyage-3.5")
    model: String,
    /// HTTP client
    client: Client,
    /// Input type for embedding optimization
    input_type: Option<InputType>,
    /// Optional: The number of dimensions for the output embeddings
    output_dimension: Option<u32>,
    /// Whether to truncate inputs exceeding context length
    truncation: bool,
    /// Maximum number of texts to embed in a single batch request
    batch_size: usize,
    /// Retry policy for API calls
    retry_policy: RetryPolicy,
    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl VoyageEmbeddings {
    /// Create a new Voyage AI embeddings instance with default settings.
    ///
    /// Defaults:
    /// - Model: `voyage-3.5`
    /// - Batch size: 128
    /// - Truncation: true
    /// - API key: from `VOYAGE_API_KEY` environment variable
    #[must_use]
    pub fn new() -> Self {
        Self {
            api_key: env_string(VOYAGE_API_KEY_VAR),
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            input_type: None,
            output_dimension: None,
            truncation: true,
            batch_size: 128,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Set the API key explicitly.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_voyage::VoyageEmbeddings;
    /// let embedder = VoyageEmbeddings::new()
    ///     .with_api_key("your-api-key");
    /// ```
    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the model name.
    ///
    /// # Supported Models
    ///
    /// - `voyage-3.5`: Latest general-purpose model
    /// - `voyage-3-large`: High-performance large model
    /// - `voyage-3.5-lite`: Efficient lightweight model
    /// - `voyage-code-3`: Optimized for code
    /// - `voyage-finance-2`: Optimized for financial documents
    /// - `voyage-law-2`: Optimized for legal documents
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_voyage::VoyageEmbeddings;
    /// let embedder = VoyageEmbeddings::new()
    ///     .with_model("voyage-code-3");
    /// ```
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the input type for embedding optimization.
    ///
    /// - `None`: No specific optimization
    /// - `Query`: For search queries
    /// - `Document`: For documents to be searched
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_voyage::{VoyageEmbeddings, InputType};
    /// let embedder = VoyageEmbeddings::new()
    ///     .with_input_type(InputType::Query);
    /// ```
    #[must_use]
    pub fn with_input_type(mut self, input_type: InputType) -> Self {
        self.input_type = Some(input_type);
        self
    }

    /// Set the output dimensionality for embeddings.
    ///
    /// Allows you to reduce the embedding size while maintaining semantic
    /// information, useful for storage optimization.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_voyage::VoyageEmbeddings;
    /// let embedder = VoyageEmbeddings::new()
    ///     .with_dimensions(512);
    /// ```
    #[must_use]
    pub fn with_dimensions(mut self, dimensions: u32) -> Self {
        self.output_dimension = Some(dimensions);
        self
    }

    /// Set whether to truncate inputs exceeding context length.
    ///
    /// Default is true. If false, inputs exceeding the model's context
    /// length will return an error.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_voyage::VoyageEmbeddings;
    /// let embedder = VoyageEmbeddings::new()
    ///     .with_truncation(false);
    /// ```
    #[must_use]
    pub fn with_truncation(mut self, truncation: bool) -> Self {
        self.truncation = truncation;
        self
    }

    /// Set the batch size for batch embedding requests.
    ///
    /// Voyage AI's API allows up to 1000 texts per request, but the default
    /// is 128 for better performance.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_voyage::VoyageEmbeddings;
    /// let embedder = VoyageEmbeddings::new()
    ///     .with_batch_size(256);
    /// ```
    #[must_use]
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.min(1000); // Voyage API limit
        self
    }

    /// Set the retry policy for API calls.
    ///
    /// Default is exponential backoff with 3 retries.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_voyage::VoyageEmbeddings;
    /// # use dashflow::core::retry::RetryPolicy;
    /// let embedder = VoyageEmbeddings::new()
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
    /// # use dashflow_voyage::VoyageEmbeddings;
    /// # use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// # use std::time::Duration;
    /// # use std::sync::Arc;
    /// let rate_limiter = InMemoryRateLimiter::new(
    ///     10.0,  // 10 requests per second
    ///     Duration::from_millis(100),
    ///     20.0,  // Max burst
    /// );
    ///
    /// let embedder = VoyageEmbeddings::new()
    ///     .with_rate_limiter(Arc::new(rate_limiter));
    /// ```
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Get the API key, returning an error if not configured.
    fn get_api_key(&self) -> Result<&str, DashFlowError> {
        self.api_key.as_deref().ok_or_else(|| {
            DashFlowError::Configuration(
                "VOYAGE_API_KEY not set. Set it via environment variable or with_api_key()"
                    .to_string(),
            )
        })
    }

    /// Embed texts using the Voyage AI API.
    async fn embed_texts(
        &self,
        texts: &[String],
        input_type: Option<InputType>,
    ) -> Result<Vec<Vec<f32>>, DashFlowError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let api_key = self.get_api_key()?;
        let url = format!("{VOYAGE_API_BASE}/embeddings");

        let request = EmbedRequest {
            input: texts.to_vec(),
            model: self.model.clone(),
            input_type: input_type.or(self.input_type),
            truncation: Some(self.truncation),
            output_dimension: self.output_dimension,
        };

        // Acquire rate limiter token if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        let response = with_retry(&self.retry_policy, || async {
            self.client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&request)
                .send()
                .await
                .map_err(|e| DashFlowError::api(format!("Voyage API request failed: {e}")))?
                .error_for_status()
                .map_err(|e| DashFlowError::api(format!("Voyage API error: {e}")))
        })
        .await?;

        let embed_response: EmbedResponse = response
            .json()
            .await
            .map_err(|e| DashFlowError::api(format!("Failed to parse Voyage response: {e}")))?;

        Ok(embed_response
            .data
            .into_iter()
            .map(|e| e.embedding)
            .collect())
    }
}

impl Default for VoyageEmbeddings {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Embeddings for VoyageEmbeddings {
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, DashFlowError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        // Process in batches
        for chunk in texts.chunks(self.batch_size) {
            let batch_embeddings = self.embed_texts(chunk, Some(InputType::Document)).await?;
            all_embeddings.extend(batch_embeddings);
        }

        Ok(all_embeddings)
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, DashFlowError> {
        let texts = vec![text.to_string()];
        let mut embeddings = self.embed_texts(&texts, Some(InputType::Query)).await?;

        embeddings
            .pop()
            .ok_or_else(|| DashFlowError::api("No embedding returned from Voyage AI"))
    }
}

// Request/Response types for Voyage API

#[derive(Debug, Serialize)]
struct EmbedRequest {
    input: Vec<String>,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_type: Option<InputType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_dimension: Option<u32>,
}

/// Voyage API response struct. Fields marked dead_code are present in API response
/// and required for serde deserialization, but not currently used.
#[derive(Debug, Deserialize)]
struct EmbedResponse {
    data: Vec<EmbeddingData>,
    #[allow(dead_code)] // Deserialize: Token usage - reserved for cost tracking
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    #[allow(dead_code)] // Deserialize: Original input index - reserved for batch reordering
    index: usize,
}

#[derive(Debug, Deserialize)]
struct Usage {
    #[allow(dead_code)] // Deserialize: Total tokens billed - reserved for cost tracking
    total_tokens: u32,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // ==================== DEFAULT_MODEL constant ====================

    #[test]
    fn test_default_model_constant() {
        assert_eq!(DEFAULT_MODEL, "voyage-3.5");
    }

    // ==================== InputType enum ====================

    #[test]
    fn test_input_type_none_serialization() {
        let input_type = InputType::None;
        let serialized = serde_json::to_string(&input_type).unwrap();
        assert_eq!(serialized, "\"none\"");
    }

    #[test]
    fn test_input_type_query_serialization() {
        let input_type = InputType::Query;
        let serialized = serde_json::to_string(&input_type).unwrap();
        assert_eq!(serialized, "\"query\"");
    }

    #[test]
    fn test_input_type_document_serialization() {
        let input_type = InputType::Document;
        let serialized = serde_json::to_string(&input_type).unwrap();
        assert_eq!(serialized, "\"document\"");
    }

    #[test]
    fn test_input_type_clone() {
        let input_type = InputType::Query;
        let cloned = input_type.clone();
        assert_eq!(input_type, cloned);
    }

    #[test]
    fn test_input_type_copy() {
        let input_type = InputType::Document;
        let copied = input_type; // Copy
        assert_eq!(copied, InputType::Document);
    }

    #[test]
    fn test_input_type_debug() {
        let debug_str = format!("{:?}", InputType::Query);
        assert!(debug_str.contains("Query"));
    }

    #[test]
    fn test_input_type_eq() {
        assert_eq!(InputType::None, InputType::None);
        assert_eq!(InputType::Query, InputType::Query);
        assert_eq!(InputType::Document, InputType::Document);
        assert_ne!(InputType::None, InputType::Query);
        assert_ne!(InputType::Query, InputType::Document);
    }

    #[test]
    fn test_input_type_partial_eq() {
        let a = InputType::Query;
        let b = InputType::Query;
        assert!(a == b);

        let c = InputType::Document;
        assert!(a != c);
    }

    // ==================== VoyageEmbeddings construction ====================

    #[test]
    fn test_default_constructor() {
        let embedder = VoyageEmbeddings::new();
        assert_eq!(embedder.model, "voyage-3.5");
        assert_eq!(embedder.batch_size, 128);
        assert!(embedder.truncation);
        assert!(embedder.input_type.is_none());
        assert!(embedder.output_dimension.is_none());
    }

    #[test]
    fn test_default_trait() {
        let embedder = VoyageEmbeddings::default();
        assert_eq!(embedder.model, "voyage-3.5");
        assert_eq!(embedder.batch_size, 128);
        assert!(embedder.truncation);
    }

    #[test]
    fn test_default_retry_policy() {
        let embedder = VoyageEmbeddings::new();
        // Default is exponential with 3 retries
        assert_eq!(embedder.retry_policy.max_retries, 3);
    }

    #[test]
    fn test_default_rate_limiter_none() {
        let embedder = VoyageEmbeddings::new();
        assert!(embedder.rate_limiter.is_none());
    }

    // ==================== Builder methods ====================

    #[test]
    fn test_with_api_key() {
        let embedder = VoyageEmbeddings::new().with_api_key("test-key");
        assert_eq!(embedder.api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_with_api_key_string() {
        let embedder = VoyageEmbeddings::new().with_api_key(String::from("api-key-string"));
        assert_eq!(embedder.api_key, Some("api-key-string".to_string()));
    }

    #[test]
    fn test_with_api_key_empty() {
        let embedder = VoyageEmbeddings::new().with_api_key("");
        assert_eq!(embedder.api_key, Some(String::new()));
    }

    #[test]
    fn test_with_model() {
        let embedder = VoyageEmbeddings::new().with_model("voyage-code-3");
        assert_eq!(embedder.model, "voyage-code-3");
    }

    #[test]
    fn test_with_model_voyage_3_large() {
        let embedder = VoyageEmbeddings::new().with_model("voyage-3-large");
        assert_eq!(embedder.model, "voyage-3-large");
    }

    #[test]
    fn test_with_model_voyage_3_5_lite() {
        let embedder = VoyageEmbeddings::new().with_model("voyage-3.5-lite");
        assert_eq!(embedder.model, "voyage-3.5-lite");
    }

    #[test]
    fn test_with_model_voyage_finance_2() {
        let embedder = VoyageEmbeddings::new().with_model("voyage-finance-2");
        assert_eq!(embedder.model, "voyage-finance-2");
    }

    #[test]
    fn test_with_model_voyage_law_2() {
        let embedder = VoyageEmbeddings::new().with_model("voyage-law-2");
        assert_eq!(embedder.model, "voyage-law-2");
    }

    #[test]
    fn test_with_model_string() {
        let model = String::from("custom-model");
        let embedder = VoyageEmbeddings::new().with_model(model);
        assert_eq!(embedder.model, "custom-model");
    }

    #[test]
    fn test_with_input_type_query() {
        let embedder = VoyageEmbeddings::new().with_input_type(InputType::Query);
        assert_eq!(embedder.input_type, Some(InputType::Query));
    }

    #[test]
    fn test_with_input_type_document() {
        let embedder = VoyageEmbeddings::new().with_input_type(InputType::Document);
        assert_eq!(embedder.input_type, Some(InputType::Document));
    }

    #[test]
    fn test_with_input_type_none() {
        let embedder = VoyageEmbeddings::new().with_input_type(InputType::None);
        assert_eq!(embedder.input_type, Some(InputType::None));
    }

    #[test]
    fn test_with_dimensions() {
        let embedder = VoyageEmbeddings::new().with_dimensions(512);
        assert_eq!(embedder.output_dimension, Some(512));
    }

    #[test]
    fn test_with_dimensions_1024() {
        let embedder = VoyageEmbeddings::new().with_dimensions(1024);
        assert_eq!(embedder.output_dimension, Some(1024));
    }

    #[test]
    fn test_with_dimensions_256() {
        let embedder = VoyageEmbeddings::new().with_dimensions(256);
        assert_eq!(embedder.output_dimension, Some(256));
    }

    #[test]
    fn test_with_dimensions_zero() {
        let embedder = VoyageEmbeddings::new().with_dimensions(0);
        assert_eq!(embedder.output_dimension, Some(0));
    }

    #[test]
    fn test_with_truncation_false() {
        let embedder = VoyageEmbeddings::new().with_truncation(false);
        assert!(!embedder.truncation);
    }

    #[test]
    fn test_with_truncation_true() {
        let embedder = VoyageEmbeddings::new().with_truncation(true);
        assert!(embedder.truncation);
    }

    #[test]
    fn test_with_batch_size() {
        let embedder = VoyageEmbeddings::new().with_batch_size(256);
        assert_eq!(embedder.batch_size, 256);
    }

    #[test]
    fn test_with_batch_size_1() {
        let embedder = VoyageEmbeddings::new().with_batch_size(1);
        assert_eq!(embedder.batch_size, 1);
    }

    #[test]
    fn test_with_batch_size_1000() {
        let embedder = VoyageEmbeddings::new().with_batch_size(1000);
        assert_eq!(embedder.batch_size, 1000);
    }

    #[test]
    fn test_batch_size_clamped_at_1000() {
        // Voyage API limit is 1000
        let embedder = VoyageEmbeddings::new().with_batch_size(2000);
        assert_eq!(embedder.batch_size, 1000);
    }

    #[test]
    fn test_batch_size_clamped_large_value() {
        let embedder = VoyageEmbeddings::new().with_batch_size(usize::MAX);
        assert_eq!(embedder.batch_size, 1000);
    }

    #[test]
    fn test_with_retry_policy() {
        let policy = RetryPolicy::exponential(5);
        let embedder = VoyageEmbeddings::new().with_retry_policy(policy);
        assert_eq!(embedder.retry_policy.max_retries, 5);
    }

    #[test]
    fn test_with_retry_policy_no_retries() {
        let policy = RetryPolicy::no_retry();
        let embedder = VoyageEmbeddings::new().with_retry_policy(policy);
        assert_eq!(embedder.retry_policy.max_retries, 0);
    }

    #[test]
    fn test_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0);
        let embedder = VoyageEmbeddings::new().with_rate_limiter(Arc::new(limiter));
        assert!(embedder.rate_limiter.is_some());
    }

    // ==================== Builder chaining ====================

    #[test]
    fn test_builder_chaining_all_options() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0);
        let embedder = VoyageEmbeddings::new()
            .with_api_key("test-key")
            .with_model("voyage-finance-2")
            .with_input_type(InputType::Document)
            .with_dimensions(768)
            .with_batch_size(64)
            .with_truncation(false)
            .with_retry_policy(RetryPolicy::exponential(5))
            .with_rate_limiter(Arc::new(limiter));

        assert_eq!(embedder.api_key, Some("test-key".to_string()));
        assert_eq!(embedder.model, "voyage-finance-2");
        assert_eq!(embedder.input_type, Some(InputType::Document));
        assert_eq!(embedder.output_dimension, Some(768));
        assert_eq!(embedder.batch_size, 64);
        assert!(!embedder.truncation);
        assert_eq!(embedder.retry_policy.max_retries, 5);
        assert!(embedder.rate_limiter.is_some());
    }

    #[test]
    fn test_builder_order_independence() {
        let e1 = VoyageEmbeddings::new()
            .with_model("m")
            .with_api_key("k")
            .with_dimensions(256);

        let e2 = VoyageEmbeddings::new()
            .with_api_key("k")
            .with_dimensions(256)
            .with_model("m");

        assert_eq!(e1.model, e2.model);
        assert_eq!(e1.api_key, e2.api_key);
        assert_eq!(e1.output_dimension, e2.output_dimension);
    }

    #[test]
    fn test_builder_override() {
        let embedder = VoyageEmbeddings::new()
            .with_model("model-1")
            .with_model("model-2");
        assert_eq!(embedder.model, "model-2");
    }

    // ==================== get_api_key ====================

    #[test]
    fn test_get_api_key_success() {
        let embedder = VoyageEmbeddings::new().with_api_key("test-api-key");
        let result = embedder.get_api_key();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-api-key");
    }

    #[test]
    fn test_get_api_key_missing() {
        // Create embedder without setting API key (and ensuring env var is not set)
        let embedder = VoyageEmbeddings {
            api_key: None,
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            input_type: None,
            output_dimension: None,
            truncation: true,
            batch_size: 128,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        };
        let result = embedder.get_api_key();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("VOYAGE_API_KEY not set"));
    }

    // ==================== EmbedRequest serialization ====================

    #[test]
    fn test_embed_request_minimal() {
        let request = EmbedRequest {
            input: vec!["test".to_string()],
            model: "voyage-3.5".to_string(),
            input_type: None,
            truncation: None,
            output_dimension: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"input\":[\"test\"]"));
        assert!(json.contains("\"model\":\"voyage-3.5\""));
        // Optional fields should be skipped
        assert!(!json.contains("input_type"));
        assert!(!json.contains("truncation"));
        assert!(!json.contains("output_dimension"));
    }

    #[test]
    fn test_embed_request_full() {
        let request = EmbedRequest {
            input: vec!["text1".to_string(), "text2".to_string()],
            model: "voyage-code-3".to_string(),
            input_type: Some(InputType::Query),
            truncation: Some(true),
            output_dimension: Some(512),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"input\":[\"text1\",\"text2\"]"));
        assert!(json.contains("\"model\":\"voyage-code-3\""));
        assert!(json.contains("\"input_type\":\"query\""));
        assert!(json.contains("\"truncation\":true"));
        assert!(json.contains("\"output_dimension\":512"));
    }

    #[test]
    fn test_embed_request_empty_input() {
        let request = EmbedRequest {
            input: vec![],
            model: "voyage-3.5".to_string(),
            input_type: None,
            truncation: None,
            output_dimension: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"input\":[]"));
    }

    #[test]
    fn test_embed_request_skip_serializing_if() {
        let request = EmbedRequest {
            input: vec!["test".to_string()],
            model: "voyage-3.5".to_string(),
            input_type: None,
            truncation: None,
            output_dimension: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        // None values should not appear in JSON
        assert!(!json.contains("null"));
        assert!(!json.contains("input_type"));
    }

    #[test]
    fn test_embed_request_document_input_type() {
        let request = EmbedRequest {
            input: vec!["doc".to_string()],
            model: "voyage-3.5".to_string(),
            input_type: Some(InputType::Document),
            truncation: None,
            output_dimension: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"input_type\":\"document\""));
    }

    #[test]
    fn test_embed_request_truncation_false() {
        let request = EmbedRequest {
            input: vec!["test".to_string()],
            model: "voyage-3.5".to_string(),
            input_type: None,
            truncation: Some(false),
            output_dimension: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"truncation\":false"));
    }

    #[test]
    fn test_embed_request_debug() {
        let request = EmbedRequest {
            input: vec!["test".to_string()],
            model: "voyage-3.5".to_string(),
            input_type: Some(InputType::Query),
            truncation: Some(true),
            output_dimension: Some(512),
        };
        let debug_str = format!("{:?}", request);
        assert!(debug_str.contains("EmbedRequest"));
        assert!(debug_str.contains("voyage-3.5"));
    }

    // ==================== EmbedResponse deserialization ====================

    #[test]
    fn test_embed_response_minimal() {
        let json = r#"{
            "data": [],
            "usage": {"total_tokens": 0}
        }"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();
        assert!(response.data.is_empty());
    }

    #[test]
    fn test_embed_response_single_embedding() {
        let json = r#"{
            "data": [{"embedding": [0.1, 0.2, 0.3], "index": 0}],
            "usage": {"total_tokens": 10}
        }"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(response.data[0].index, 0);
    }

    #[test]
    fn test_embed_response_multiple_embeddings() {
        let json = r#"{
            "data": [
                {"embedding": [0.1, 0.2], "index": 0},
                {"embedding": [0.3, 0.4], "index": 1},
                {"embedding": [0.5, 0.6], "index": 2}
            ],
            "usage": {"total_tokens": 30}
        }"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 3);
        assert_eq!(response.data[0].index, 0);
        assert_eq!(response.data[1].index, 1);
        assert_eq!(response.data[2].index, 2);
    }

    #[test]
    fn test_embed_response_debug() {
        let json = r#"{
            "data": [{"embedding": [0.1], "index": 0}],
            "usage": {"total_tokens": 5}
        }"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("EmbedResponse"));
    }

    // ==================== EmbeddingData ====================

    #[test]
    fn test_embedding_data_empty_vector() {
        let json = r#"{"embedding": [], "index": 0}"#;
        let data: EmbeddingData = serde_json::from_str(json).unwrap();
        assert!(data.embedding.is_empty());
        assert_eq!(data.index, 0);
    }

    #[test]
    fn test_embedding_data_large_vector() {
        let embedding: Vec<f32> = (0..1024).map(|i| i as f32 / 1024.0).collect();
        let json = format!(r#"{{"embedding": {:?}, "index": 42}}"#, embedding);
        let data: EmbeddingData = serde_json::from_str(&json).unwrap();
        assert_eq!(data.embedding.len(), 1024);
        assert_eq!(data.index, 42);
    }

    #[test]
    fn test_embedding_data_debug() {
        let json = r#"{"embedding": [0.5], "index": 1}"#;
        let data: EmbeddingData = serde_json::from_str(json).unwrap();
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("EmbeddingData"));
    }

    // ==================== Usage ====================

    #[test]
    fn test_usage_deserialization() {
        let json = r#"{"total_tokens": 100}"#;
        let usage: Usage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.total_tokens, 100);
    }

    #[test]
    fn test_usage_zero_tokens() {
        let json = r#"{"total_tokens": 0}"#;
        let usage: Usage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_usage_large_tokens() {
        let json = r#"{"total_tokens": 4294967295}"#;
        let usage: Usage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.total_tokens, u32::MAX);
    }

    #[test]
    fn test_usage_debug() {
        let json = r#"{"total_tokens": 50}"#;
        let usage: Usage = serde_json::from_str(json).unwrap();
        let debug_str = format!("{:?}", usage);
        assert!(debug_str.contains("Usage"));
        assert!(debug_str.contains("50"));
    }

    // ==================== Async tests ====================

    #[tokio::test]
    async fn test_embed_texts_empty() {
        let embedder = VoyageEmbeddings::new().with_api_key("test-key");
        let result = embedder.embed_texts(&[], None).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_embed_texts_no_api_key() {
        let embedder = VoyageEmbeddings {
            api_key: None,
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            input_type: None,
            output_dimension: None,
            truncation: true,
            batch_size: 128,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        };
        let texts = vec!["test".to_string()];
        let result = embedder.embed_texts(&texts, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("VOYAGE_API_KEY"));
    }

    #[tokio::test]
    async fn test_embed_documents_empty() {
        let embedder = VoyageEmbeddings::new().with_api_key("test-key");
        let result = embedder._embed_documents(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ==================== Integration tests (ignored without API key) ====================

    #[tokio::test]
    #[ignore = "requires VOYAGE_API_KEY"]
    async fn test_embed_query_integration() {
        let embedder = VoyageEmbeddings::new();
        let result = embedder._embed_query("What is machine learning?").await;
        assert!(result.is_ok());
        let embedding = result.unwrap();
        assert!(!embedding.is_empty());
        // voyage-3.5 default dimension is 1024
        assert!(embedding.len() >= 512);
    }

    #[tokio::test]
    #[ignore = "requires VOYAGE_API_KEY"]
    async fn test_embed_documents_integration() {
        let embedder = VoyageEmbeddings::new();
        let docs = vec![
            "First document about AI".to_string(),
            "Second document about ML".to_string(),
        ];
        let result = embedder._embed_documents(&docs).await;
        assert!(result.is_ok());
        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 2);
    }

    #[tokio::test]
    #[ignore = "requires VOYAGE_API_KEY"]
    async fn test_embed_with_custom_dimensions() {
        let embedder = VoyageEmbeddings::new().with_dimensions(256);
        let result = embedder._embed_query("Test query").await;
        assert!(result.is_ok());
        let embedding = result.unwrap();
        assert_eq!(embedding.len(), 256);
    }
}

/// Standard conformance tests for embeddings
///
/// These tests verify that VoyageEmbeddings behaves consistently with other
/// Embeddings implementations across the DashFlow ecosystem.
#[cfg(test)]
mod standard_tests {
    use super::*;
    use dashflow_standard_tests::embeddings_tests::*;

    /// Helper function to create a test embeddings model
    fn create_test_embeddings() -> VoyageEmbeddings {
        VoyageEmbeddings::new()
    }

    /// Standard Test 1: Embed single document
    #[tokio::test]
    #[ignore = "requires VOYAGE_API_KEY"]
    async fn test_embed_query_standard() {
        assert!(
            std::env::var("VOYAGE_API_KEY").is_ok(),
            "VOYAGE_API_KEY must be set"
        );
        test_embed_query(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 2: Embed multiple documents
    #[tokio::test]
    #[ignore = "requires VOYAGE_API_KEY"]
    async fn test_embed_documents_standard() {
        assert!(
            std::env::var("VOYAGE_API_KEY").is_ok(),
            "VOYAGE_API_KEY must be set"
        );
        test_embed_documents(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 3: Empty input handling
    #[tokio::test]
    #[ignore = "requires VOYAGE_API_KEY"]
    async fn test_empty_input_standard() {
        assert!(
            std::env::var("VOYAGE_API_KEY").is_ok(),
            "VOYAGE_API_KEY must be set"
        );
        test_empty_input(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 4: Dimension consistency
    #[tokio::test]
    #[ignore = "requires VOYAGE_API_KEY"]
    async fn test_dimension_consistency_standard() {
        assert!(
            std::env::var("VOYAGE_API_KEY").is_ok(),
            "VOYAGE_API_KEY must be set"
        );
        test_dimension_consistency(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 5: Semantic similarity
    #[tokio::test]
    #[ignore = "requires VOYAGE_API_KEY"]
    async fn test_semantic_similarity_standard() {
        assert!(
            std::env::var("VOYAGE_API_KEY").is_ok(),
            "VOYAGE_API_KEY must be set"
        );
        test_semantic_similarity(Arc::new(create_test_embeddings())).await;
    }
}
