//! Cohere embeddings implementation.
//!
//! This module provides embeddings using Cohere's embedding models, including:
//! - embed-v4.0: Latest multilingual model (recommended)
//! - embed-english-v3.0: English-optimized model
//! - embed-multilingual-v3.0: Multilingual model
//! - embed-english-light-v3.0: Lightweight English model
//! - embed-multilingual-light-v3.0: Lightweight multilingual model
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_cohere::CohereEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = CohereEmbeddings::new()
//!     .with_api_key(std::env::var("COHERE_API_KEY")?);
//!
//! // Embed a single query
//! let query_vector = embedder._embed_query("What is machine learning?").await?;
//! assert!(!query_vector.is_empty());
//!
//! // Embed multiple documents
//! let docs = vec![
//!     "Machine learning is a subset of AI.".to_string(),
//!     "Deep learning uses neural networks.".to_string(),
//! ];
//! let doc_vectors = embedder._embed_documents(&docs).await?;
//! assert_eq!(doc_vectors.len(), 2);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use dashflow::core::{
    config_loader::env_vars::{
        cohere_api_v2_url, env_string, COHERE_API_KEY, DEFAULT_COHERE_EMBED_ENDPOINT,
    },
    embeddings::Embeddings,
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
    Error as DashFlowError,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const DEFAULT_MODEL: &str = "embed-v4.0";

/// Input type for embeddings, which optimizes the embedding for specific use cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputType {
    /// Used for embeddings of search queries run against a vector DB to find relevant documents
    SearchQuery,
    /// Used for embeddings stored in a vector database for search
    SearchDocument,
    /// Used for embeddings passed to a text classifier
    Classification,
    /// Used for embeddings run through a clustering algorithm
    Clustering,
}

/// Truncation strategy for inputs exceeding context length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Truncate {
    /// No truncation; returns an error if input is too long
    None,
    /// Truncate from the start of the text
    Start,
    /// Truncate from the end of the text
    End,
}

/// Embedding output type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingType {
    /// Standard float32 embeddings
    Float,
    /// Int8 quantized embeddings
    Int8,
    /// Uint8 quantized embeddings
    Uint8,
    /// Binary embeddings
    Binary,
    /// Unsigned binary embeddings
    Ubinary,
}

/// Cohere embedding model integration.
///
/// Supports the following models:
/// - `embed-v4.0`: Latest multilingual model (1536 dimensions default)
/// - `embed-english-v3.0`: English-optimized model (1024 dimensions)
/// - `embed-multilingual-v3.0`: Multilingual model (1024 dimensions)
/// - `embed-english-light-v3.0`: Lightweight English model (384 dimensions)
/// - `embed-multilingual-light-v3.0`: Lightweight multilingual model (384 dimensions)
///
/// # Configuration
///
/// The API key can be set via:
/// - Constructor: `CohereEmbeddings::new().with_api_key("...")`
/// - Environment: `COHERE_API_KEY`
///
/// # Input Types
///
/// Cohere embeddings support different input types that optimize the embedding
/// for specific use cases:
/// - `SearchQuery`: For search queries
/// - `SearchDocument`: For documents to be searched
/// - `Classification`: For text classification
/// - `Clustering`: For clustering tasks
///
/// # Truncation
///
/// Control how inputs exceeding context length are handled:
/// - `None`: Return error if too long
/// - `Start`: Truncate from beginning
/// - `End`: Truncate from end (default)
pub struct CohereEmbeddings {
    /// API key for authentication
    api_key: Option<String>,
    /// Model name (e.g., "embed-v4.0")
    model: String,
    /// HTTP client
    client: Client,
    /// Input type for embedding optimization
    input_type: Option<InputType>,
    /// Truncation strategy
    truncate: Truncate,
    /// Optional: The number of dimensions for the output embeddings
    output_dimension: Option<u32>,
    /// Embedding output type
    embedding_types: Vec<EmbeddingType>,
    /// Maximum number of texts to embed in a single batch request
    batch_size: usize,
    /// Retry policy for API calls
    retry_policy: RetryPolicy,
    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl CohereEmbeddings {
    /// Create a new Cohere embeddings instance with default settings.
    ///
    /// Defaults:
    /// - Model: `embed-v4.0`
    /// - Batch size: 96 (Cohere recommends up to 96 texts per request)
    /// - Truncate: `End`
    /// - Embedding types: `Float`
    /// - API key: from `COHERE_API_KEY` environment variable
    #[must_use]
    pub fn new() -> Self {
        Self {
            api_key: env_string(COHERE_API_KEY),
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            input_type: None,
            truncate: Truncate::End,
            output_dimension: None,
            embedding_types: vec![EmbeddingType::Float],
            batch_size: 96,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Set the API key explicitly.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_cohere::CohereEmbeddings;
    /// let embedder = CohereEmbeddings::new()
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
    /// - `embed-v4.0`: Latest multilingual model (recommended)
    /// - `embed-english-v3.0`: English-optimized model
    /// - `embed-multilingual-v3.0`: Multilingual model
    /// - `embed-english-light-v3.0`: Lightweight English model
    /// - `embed-multilingual-light-v3.0`: Lightweight multilingual model
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_cohere::CohereEmbeddings;
    /// let embedder = CohereEmbeddings::new()
    ///     .with_model("embed-english-v3.0");
    /// ```
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the input type for embedding optimization.
    ///
    /// - `SearchQuery`: For search queries
    /// - `SearchDocument`: For documents to be searched
    /// - `Classification`: For text classification
    /// - `Clustering`: For clustering tasks
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_cohere::{CohereEmbeddings, InputType};
    /// let embedder = CohereEmbeddings::new()
    ///     .with_input_type(InputType::SearchQuery);
    /// ```
    #[must_use]
    pub fn with_input_type(mut self, input_type: InputType) -> Self {
        self.input_type = Some(input_type);
        self
    }

    /// Set the truncation strategy for inputs exceeding context length.
    ///
    /// - `None`: Return error if too long
    /// - `Start`: Truncate from beginning
    /// - `End`: Truncate from end (default)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_cohere::{CohereEmbeddings, Truncate};
    /// let embedder = CohereEmbeddings::new()
    ///     .with_truncate(Truncate::Start);
    /// ```
    #[must_use]
    pub fn with_truncate(mut self, truncate: Truncate) -> Self {
        self.truncate = truncate;
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
    /// # use dashflow_cohere::CohereEmbeddings;
    /// let embedder = CohereEmbeddings::new()
    ///     .with_dimensions(512);
    /// ```
    #[must_use]
    pub fn with_dimensions(mut self, dimensions: u32) -> Self {
        self.output_dimension = Some(dimensions);
        self
    }

    /// Set the embedding output types.
    ///
    /// Cohere supports multiple embedding formats:
    /// - `Float`: Standard float32 embeddings (default)
    /// - `Int8`: Int8 quantized embeddings
    /// - `Uint8`: Uint8 quantized embeddings
    /// - `Binary`: Binary embeddings
    /// - `Ubinary`: Unsigned binary embeddings
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_cohere::{CohereEmbeddings, EmbeddingType};
    /// let embedder = CohereEmbeddings::new()
    ///     .with_embedding_types(vec![EmbeddingType::Float, EmbeddingType::Int8]);
    /// ```
    #[must_use]
    pub fn with_embedding_types(mut self, types: Vec<EmbeddingType>) -> Self {
        self.embedding_types = types;
        self
    }

    /// Set the batch size for batch embedding requests.
    ///
    /// Cohere recommends up to 96 texts per request.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_cohere::CohereEmbeddings;
    /// let embedder = CohereEmbeddings::new()
    ///     .with_batch_size(50);
    /// ```
    #[must_use]
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.min(96); // Cohere recommendation
        self
    }

    /// Set the retry policy for API calls.
    ///
    /// Default is exponential backoff with 3 retries.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_cohere::CohereEmbeddings;
    /// # use dashflow::core::retry::RetryPolicy;
    /// let embedder = CohereEmbeddings::new()
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
    /// # use dashflow_cohere::CohereEmbeddings;
    /// # use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// # use std::time::Duration;
    /// # use std::sync::Arc;
    /// let rate_limiter = InMemoryRateLimiter::new(
    ///     10.0,  // 10 requests per second
    ///     Duration::from_millis(100),
    ///     20.0,  // Max burst
    /// );
    ///
    /// let embedder = CohereEmbeddings::new()
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
                "COHERE_API_KEY not set. Set it via environment variable or with_api_key()"
                    .to_string(),
            )
        })
    }

    /// Embed texts using the Cohere API.
    async fn embed_texts(
        &self,
        texts: &[String],
        input_type: Option<InputType>,
    ) -> Result<Vec<Vec<f32>>, DashFlowError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let api_key = self.get_api_key()?;
        let url = cohere_api_v2_url(DEFAULT_COHERE_EMBED_ENDPOINT);

        let request = EmbedRequest {
            model: self.model.clone(),
            texts: texts.to_vec(),
            input_type: input_type
                .or(self.input_type)
                .unwrap_or(InputType::SearchDocument),
            truncate: self.truncate,
            embedding_types: self.embedding_types.clone(),
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
                .map_err(|e| DashFlowError::api(format!("Cohere API request failed: {e}")))?
                .error_for_status()
                .map_err(|e| DashFlowError::api(format!("Cohere API error: {e}")))
        })
        .await?;

        let embed_response: EmbedResponse = response
            .json()
            .await
            .map_err(|e| DashFlowError::api(format!("Failed to parse Cohere response: {e}")))?;

        // Extract float embeddings (primary format)
        let embeddings = embed_response
            .embeddings
            .float
            .ok_or_else(|| DashFlowError::api("No float embeddings in response"))?;

        Ok(embeddings)
    }
}

impl Default for CohereEmbeddings {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Embeddings for CohereEmbeddings {
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, DashFlowError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        // Process in batches
        for chunk in texts.chunks(self.batch_size) {
            let batch_embeddings = self
                .embed_texts(chunk, Some(InputType::SearchDocument))
                .await?;
            all_embeddings.extend(batch_embeddings);
        }

        Ok(all_embeddings)
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, DashFlowError> {
        let texts = vec![text.to_string()];
        let mut embeddings = self
            .embed_texts(&texts, Some(InputType::SearchQuery))
            .await?;

        embeddings
            .pop()
            .ok_or_else(|| DashFlowError::api("No embedding returned from Cohere"))
    }
}

// Request/Response types for Cohere API

#[derive(Debug, Serialize)]
struct EmbedRequest {
    model: String,
    texts: Vec<String>,
    input_type: InputType,
    truncate: Truncate,
    embedding_types: Vec<EmbeddingType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_dimension: Option<u32>,
}

/// Cohere API response struct. Fields marked dead_code are present in API response
/// and required for serde deserialization, but not currently used.
#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embeddings: EmbeddingsData,
    #[allow(dead_code)] // Deserialize: Cohere API request ID - reserved for request tracing
    id: String,
    #[allow(dead_code)] // Deserialize: Cohere API metadata - reserved for usage/billing metrics
    meta: Option<ResponseMeta>,
}

/// Cohere embeddings data supporting multiple output types (float, int8, etc.).
/// Only float embeddings are used; others reserved for future quantized embedding support.
#[derive(Debug, Deserialize)]
struct EmbeddingsData {
    float: Option<Vec<Vec<f32>>>,
    #[allow(dead_code)] // Deserialize: Quantized int8 embeddings - reserved for future compression
    int8: Option<Vec<Vec<i8>>>,
    #[allow(dead_code)] // Deserialize: Quantized uint8 embeddings - reserved for future compression
    uint8: Option<Vec<Vec<u8>>>,
    #[allow(dead_code)] // Deserialize: Binary embeddings - reserved for future compression
    binary: Option<Vec<Vec<i8>>>,
    #[allow(dead_code)] // Deserialize: Unsigned binary embeddings - reserved for future compression
    ubinary: Option<Vec<Vec<u8>>>,
}

#[derive(Debug, Deserialize)]
struct ResponseMeta {
    #[allow(dead_code)] // Deserialize: Cohere API version info - reserved for compatibility checks
    api_version: Option<ApiVersion>,
    #[allow(dead_code)] // Deserialize: Billing units consumed - reserved for cost tracking
    billed_units: Option<BilledUnits>,
}

#[derive(Debug, Deserialize)]
struct ApiVersion {
    #[allow(dead_code)] // Deserialize: API version string - reserved for compatibility logging
    version: String,
}

#[derive(Debug, Deserialize)]
struct BilledUnits {
    #[allow(dead_code)] // Deserialize: Token usage for billing - reserved for cost tracking
    input_tokens: Option<u32>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_default_constructor() {
        let embedder = CohereEmbeddings::new();
        assert_eq!(embedder.model, "embed-v4.0");
        assert_eq!(embedder.batch_size, 96);
        assert_eq!(embedder.truncate, Truncate::End);
        assert!(embedder.input_type.is_none());
        assert!(embedder.output_dimension.is_none());
        assert_eq!(embedder.embedding_types, vec![EmbeddingType::Float]);
    }

    #[test]
    fn test_with_model() {
        let embedder = CohereEmbeddings::new().with_model("embed-english-v3.0");
        assert_eq!(embedder.model, "embed-english-v3.0");
    }

    #[test]
    fn test_with_api_key() {
        let embedder = CohereEmbeddings::new().with_api_key("test-key");
        assert_eq!(embedder.api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_with_input_type() {
        let embedder = CohereEmbeddings::new().with_input_type(InputType::SearchQuery);
        assert_eq!(embedder.input_type, Some(InputType::SearchQuery));
    }

    #[test]
    fn test_with_truncate() {
        let embedder = CohereEmbeddings::new().with_truncate(Truncate::Start);
        assert_eq!(embedder.truncate, Truncate::Start);
    }

    #[test]
    fn test_with_dimensions() {
        let embedder = CohereEmbeddings::new().with_dimensions(512);
        assert_eq!(embedder.output_dimension, Some(512));
    }

    #[test]
    fn test_with_embedding_types() {
        let embedder = CohereEmbeddings::new()
            .with_embedding_types(vec![EmbeddingType::Float, EmbeddingType::Int8]);
        assert_eq!(embedder.embedding_types.len(), 2);
    }

    #[test]
    fn test_with_batch_size() {
        let embedder = CohereEmbeddings::new().with_batch_size(50);
        assert_eq!(embedder.batch_size, 50);
    }

    #[test]
    fn test_batch_size_clamped() {
        // Cohere recommendation is 96
        let embedder = CohereEmbeddings::new().with_batch_size(200);
        assert_eq!(embedder.batch_size, 96);
    }

    #[test]
    fn test_builder_chaining() {
        let embedder = CohereEmbeddings::new()
            .with_api_key("test-key")
            .with_model("embed-multilingual-v3.0")
            .with_input_type(InputType::Classification)
            .with_dimensions(768)
            .with_batch_size(64)
            .with_truncate(Truncate::None);

        assert_eq!(embedder.api_key, Some("test-key".to_string()));
        assert_eq!(embedder.model, "embed-multilingual-v3.0");
        assert_eq!(embedder.input_type, Some(InputType::Classification));
        assert_eq!(embedder.output_dimension, Some(768));
        assert_eq!(embedder.batch_size, 64);
        assert_eq!(embedder.truncate, Truncate::None);
    }

    #[test]
    fn test_input_type_serialization() {
        let input_type = InputType::SearchQuery;
        let serialized = serde_json::to_string(&input_type).unwrap();
        assert_eq!(serialized, "\"search_query\"");

        let input_type = InputType::SearchDocument;
        let serialized = serde_json::to_string(&input_type).unwrap();
        assert_eq!(serialized, "\"search_document\"");

        let input_type = InputType::Classification;
        let serialized = serde_json::to_string(&input_type).unwrap();
        assert_eq!(serialized, "\"classification\"");

        let input_type = InputType::Clustering;
        let serialized = serde_json::to_string(&input_type).unwrap();
        assert_eq!(serialized, "\"clustering\"");
    }

    #[test]
    fn test_truncate_serialization() {
        let truncate = Truncate::None;
        let serialized = serde_json::to_string(&truncate).unwrap();
        assert_eq!(serialized, "\"NONE\"");

        let truncate = Truncate::Start;
        let serialized = serde_json::to_string(&truncate).unwrap();
        assert_eq!(serialized, "\"START\"");

        let truncate = Truncate::End;
        let serialized = serde_json::to_string(&truncate).unwrap();
        assert_eq!(serialized, "\"END\"");
    }

    #[test]
    fn test_embedding_type_serialization() {
        let emb_type = EmbeddingType::Float;
        let serialized = serde_json::to_string(&emb_type).unwrap();
        assert_eq!(serialized, "\"float\"");

        let emb_type = EmbeddingType::Int8;
        let serialized = serde_json::to_string(&emb_type).unwrap();
        assert_eq!(serialized, "\"int8\"");
    }

    // ========== Additional EmbeddingType serialization tests ==========

    #[test]
    fn test_embedding_type_uint8_serialization() {
        let emb_type = EmbeddingType::Uint8;
        let serialized = serde_json::to_string(&emb_type).unwrap();
        assert_eq!(serialized, "\"uint8\"");
    }

    #[test]
    fn test_embedding_type_binary_serialization() {
        let emb_type = EmbeddingType::Binary;
        let serialized = serde_json::to_string(&emb_type).unwrap();
        assert_eq!(serialized, "\"binary\"");
    }

    #[test]
    fn test_embedding_type_ubinary_serialization() {
        let emb_type = EmbeddingType::Ubinary;
        let serialized = serde_json::to_string(&emb_type).unwrap();
        assert_eq!(serialized, "\"ubinary\"");
    }

    // ========== Default trait test ==========

    #[test]
    fn test_default_trait() {
        let embedder = CohereEmbeddings::default();
        assert_eq!(embedder.model, "embed-v4.0");
        assert_eq!(embedder.batch_size, 96);
    }

    // ========== Retry policy test ==========

    #[test]
    fn test_with_retry_policy() {
        let policy = RetryPolicy::exponential(5);
        let embedder = CohereEmbeddings::new().with_retry_policy(policy);
        // Just verify it compiles and runs
        assert_eq!(embedder.batch_size, 96);
    }

    // ========== Get API key error test ==========

    #[test]
    fn test_get_api_key_error() {
        let embedder = CohereEmbeddings {
            api_key: None,
            model: "embed-v4.0".to_string(),
            client: Client::new(),
            input_type: None,
            truncate: Truncate::End,
            output_dimension: None,
            embedding_types: vec![EmbeddingType::Float],
            batch_size: 96,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        };
        let result = embedder.get_api_key();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("COHERE_API_KEY"));
    }

    #[test]
    fn test_get_api_key_success() {
        let embedder = CohereEmbeddings::new().with_api_key("test-key");
        let result = embedder.get_api_key();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-key");
    }

    // ========== InputType equality tests ==========

    #[test]
    fn test_input_type_equality() {
        assert_eq!(InputType::SearchQuery, InputType::SearchQuery);
        assert_eq!(InputType::SearchDocument, InputType::SearchDocument);
        assert_eq!(InputType::Classification, InputType::Classification);
        assert_eq!(InputType::Clustering, InputType::Clustering);
        assert_ne!(InputType::SearchQuery, InputType::SearchDocument);
    }

    #[test]
    fn test_input_type_clone() {
        let input_type = InputType::SearchQuery;
        let cloned = input_type;
        assert_eq!(cloned, InputType::SearchQuery);
    }

    #[test]
    fn test_input_type_debug() {
        let input_type = InputType::Classification;
        let debug_str = format!("{:?}", input_type);
        assert!(debug_str.contains("Classification"));
    }

    // ========== Truncate equality tests ==========

    #[test]
    fn test_truncate_equality() {
        assert_eq!(Truncate::None, Truncate::None);
        assert_eq!(Truncate::Start, Truncate::Start);
        assert_eq!(Truncate::End, Truncate::End);
        assert_ne!(Truncate::None, Truncate::Start);
    }

    #[test]
    fn test_truncate_clone() {
        let truncate = Truncate::Start;
        let cloned = truncate;
        assert_eq!(cloned, Truncate::Start);
    }

    #[test]
    fn test_truncate_debug() {
        let truncate = Truncate::End;
        let debug_str = format!("{:?}", truncate);
        assert!(debug_str.contains("End"));
    }

    // ========== EmbeddingType equality tests ==========

    #[test]
    fn test_embedding_type_equality() {
        assert_eq!(EmbeddingType::Float, EmbeddingType::Float);
        assert_eq!(EmbeddingType::Int8, EmbeddingType::Int8);
        assert_ne!(EmbeddingType::Float, EmbeddingType::Int8);
    }

    #[test]
    fn test_embedding_type_clone() {
        let emb_type = EmbeddingType::Binary;
        let cloned = emb_type;
        assert_eq!(cloned, EmbeddingType::Binary);
    }

    #[test]
    fn test_embedding_type_debug() {
        let emb_type = EmbeddingType::Uint8;
        let debug_str = format!("{:?}", emb_type);
        assert!(debug_str.contains("Uint8"));
    }

    // ========== Builder edge case tests ==========

    #[test]
    fn test_with_dimensions_zero() {
        let embedder = CohereEmbeddings::new().with_dimensions(0);
        assert_eq!(embedder.output_dimension, Some(0));
    }

    #[test]
    fn test_with_dimensions_large() {
        let embedder = CohereEmbeddings::new().with_dimensions(4096);
        assert_eq!(embedder.output_dimension, Some(4096));
    }

    #[test]
    fn test_with_batch_size_zero() {
        let embedder = CohereEmbeddings::new().with_batch_size(0);
        assert_eq!(embedder.batch_size, 0);
    }

    #[test]
    fn test_with_batch_size_one() {
        let embedder = CohereEmbeddings::new().with_batch_size(1);
        assert_eq!(embedder.batch_size, 1);
    }

    #[test]
    fn test_with_model_english_v3() {
        let embedder = CohereEmbeddings::new().with_model("embed-english-v3.0");
        assert_eq!(embedder.model, "embed-english-v3.0");
    }

    #[test]
    fn test_with_model_multilingual_v3() {
        let embedder = CohereEmbeddings::new().with_model("embed-multilingual-v3.0");
        assert_eq!(embedder.model, "embed-multilingual-v3.0");
    }

    #[test]
    fn test_with_model_english_light_v3() {
        let embedder = CohereEmbeddings::new().with_model("embed-english-light-v3.0");
        assert_eq!(embedder.model, "embed-english-light-v3.0");
    }

    #[test]
    fn test_with_model_multilingual_light_v3() {
        let embedder = CohereEmbeddings::new().with_model("embed-multilingual-light-v3.0");
        assert_eq!(embedder.model, "embed-multilingual-light-v3.0");
    }

    #[test]
    fn test_with_api_key_string_ownership() {
        let api_key = String::from("owned-api-key");
        let embedder = CohereEmbeddings::new().with_api_key(api_key);
        assert_eq!(embedder.api_key, Some("owned-api-key".to_string()));
    }

    #[test]
    fn test_with_api_key_special_chars() {
        let embedder = CohereEmbeddings::new().with_api_key("key-with-dashes_and_underscores");
        assert_eq!(embedder.api_key, Some("key-with-dashes_and_underscores".to_string()));
    }

    #[test]
    fn test_with_model_string_ownership() {
        let model = String::from("custom-model");
        let embedder = CohereEmbeddings::new().with_model(model);
        assert_eq!(embedder.model, "custom-model");
    }

    #[test]
    fn test_with_empty_embedding_types() {
        let embedder = CohereEmbeddings::new().with_embedding_types(vec![]);
        assert!(embedder.embedding_types.is_empty());
    }

    #[test]
    fn test_with_all_embedding_types() {
        let embedder = CohereEmbeddings::new().with_embedding_types(vec![
            EmbeddingType::Float,
            EmbeddingType::Int8,
            EmbeddingType::Uint8,
            EmbeddingType::Binary,
            EmbeddingType::Ubinary,
        ]);
        assert_eq!(embedder.embedding_types.len(), 5);
    }

    // ========== EmbedRequest serialization tests ==========

    #[test]
    fn test_embed_request_serialization() {
        let request = EmbedRequest {
            model: "embed-v4.0".to_string(),
            texts: vec!["Hello".to_string(), "World".to_string()],
            input_type: InputType::SearchDocument,
            truncate: Truncate::End,
            embedding_types: vec![EmbeddingType::Float],
            output_dimension: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"embed-v4.0\""));
        assert!(json.contains("\"texts\""));
        assert!(json.contains("\"Hello\""));
        assert!(json.contains("\"World\""));
        assert!(json.contains("\"input_type\":\"search_document\""));
        assert!(json.contains("\"truncate\":\"END\""));
        // None field should be skipped
        assert!(!json.contains("\"output_dimension\""));
    }

    #[test]
    fn test_embed_request_with_output_dimension() {
        let request = EmbedRequest {
            model: "embed-v4.0".to_string(),
            texts: vec!["Test".to_string()],
            input_type: InputType::SearchQuery,
            truncate: Truncate::Start,
            embedding_types: vec![EmbeddingType::Float],
            output_dimension: Some(512),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"output_dimension\":512"));
    }

    // ========== EmbedResponse deserialization tests ==========

    #[test]
    fn test_embed_response_deserialization() {
        let json = r#"{
            "id": "resp-123",
            "embeddings": {
                "float": [[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]]
            }
        }"#;

        let response: EmbedResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "resp-123");
        let float_embeddings = response.embeddings.float.unwrap();
        assert_eq!(float_embeddings.len(), 2);
        assert_eq!(float_embeddings[0], vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_embed_response_with_meta() {
        let json = r#"{
            "id": "resp-456",
            "embeddings": {
                "float": [[1.0, 2.0]]
            },
            "meta": {
                "api_version": {"version": "2"},
                "billed_units": {"input_tokens": 100}
            }
        }"#;

        let response: EmbedResponse = serde_json::from_str(json).unwrap();
        assert!(response.meta.is_some());
    }

    #[test]
    fn test_embed_response_without_float_embeddings() {
        let json = r#"{
            "id": "resp-789",
            "embeddings": {}
        }"#;

        let response: EmbedResponse = serde_json::from_str(json).unwrap();
        assert!(response.embeddings.float.is_none());
    }

    // ========== EmbeddingsData tests ==========

    #[test]
    fn test_embeddings_data_all_types() {
        let json = r#"{
            "float": [[1.0, 2.0]],
            "int8": [[1, 2]],
            "uint8": [[1, 2]],
            "binary": [[0, 1]],
            "ubinary": [[0, 1]]
        }"#;

        let data: EmbeddingsData = serde_json::from_str(json).unwrap();
        assert!(data.float.is_some());
        assert!(data.int8.is_some());
        assert!(data.uint8.is_some());
        assert!(data.binary.is_some());
        assert!(data.ubinary.is_some());
    }

    // ========== Builder chaining comprehensive test ==========

    #[test]
    fn test_full_builder_chain() {
        let embedder = CohereEmbeddings::new()
            .with_api_key("my-api-key")
            .with_model("embed-english-v3.0")
            .with_input_type(InputType::Classification)
            .with_truncate(Truncate::None)
            .with_dimensions(384)
            .with_batch_size(48)
            .with_embedding_types(vec![EmbeddingType::Float, EmbeddingType::Int8])
            .with_retry_policy(RetryPolicy::exponential(2));

        assert_eq!(embedder.api_key, Some("my-api-key".to_string()));
        assert_eq!(embedder.model, "embed-english-v3.0");
        assert_eq!(embedder.input_type, Some(InputType::Classification));
        assert_eq!(embedder.truncate, Truncate::None);
        assert_eq!(embedder.output_dimension, Some(384));
        assert_eq!(embedder.batch_size, 48);
        assert_eq!(embedder.embedding_types.len(), 2);
    }

    // ========== InputType all variants tests ==========

    #[test]
    fn test_all_input_types() {
        let types = [
            InputType::SearchQuery,
            InputType::SearchDocument,
            InputType::Classification,
            InputType::Clustering,
        ];

        for input_type in types {
            let embedder = CohereEmbeddings::new().with_input_type(input_type);
            assert_eq!(embedder.input_type, Some(input_type));
        }
    }

    // ========== Truncate all variants tests ==========

    #[test]
    fn test_all_truncate_variants() {
        let variants = [Truncate::None, Truncate::Start, Truncate::End];

        for truncate in variants {
            let embedder = CohereEmbeddings::new().with_truncate(truncate);
            assert_eq!(embedder.truncate, truncate);
        }
    }
}

/// Standard conformance tests for embeddings
///
/// These tests verify that CohereEmbeddings behaves consistently with other
/// Embeddings implementations across the DashFlow ecosystem.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod standard_tests {
    use super::*;
    use dashflow_standard_tests::embeddings_tests::*;

    /// Helper function to create a test embeddings model
    fn create_test_embeddings() -> CohereEmbeddings {
        CohereEmbeddings::new()
    }

    /// Standard Test 1: Embed single document
    #[tokio::test]
    #[ignore = "requires COHERE_API_KEY"]
    async fn test_embed_query_standard() {
        let embeddings = std::sync::Arc::new(create_test_embeddings());
        test_embed_query(embeddings).await;
    }

    /// Standard Test 2: Embed multiple documents
    #[tokio::test]
    #[ignore = "requires COHERE_API_KEY"]
    async fn test_embed_documents_standard() {
        let embeddings = std::sync::Arc::new(create_test_embeddings());
        test_embed_documents(embeddings).await;
    }

    /// Standard Test 3: Empty input handling
    #[tokio::test]
    #[ignore = "requires COHERE_API_KEY"]
    async fn test_empty_input_standard() {
        let embeddings = std::sync::Arc::new(create_test_embeddings());
        test_empty_input(embeddings).await;
    }

    /// Standard Test 4: Dimension consistency
    #[tokio::test]
    #[ignore = "requires COHERE_API_KEY"]
    async fn test_dimension_consistency_standard() {
        let embeddings = std::sync::Arc::new(create_test_embeddings());
        test_dimension_consistency(embeddings).await;
    }

    /// Standard Test 5: Semantic similarity
    #[tokio::test]
    #[ignore = "requires COHERE_API_KEY"]
    async fn test_semantic_similarity_standard() {
        let embeddings = std::sync::Arc::new(create_test_embeddings());
        test_semantic_similarity(embeddings).await;
    }
}
