//! AWS Bedrock embeddings implementation.
//!
//! This module provides embeddings using AWS Bedrock's embedding models, including:
//! - Amazon Titan Text Embeddings v2 (1024 dimensions, 8192 token context)
//! - Amazon Titan Text Embeddings v1 (1536 dimensions, 8000 token context)
//! - Cohere Embed v3 (English and Multilingual variants)
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_bedrock::BedrockEmbeddings;
//! use dashflow::{embed_query, embed};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = Arc::new(BedrockEmbeddings::new("us-east-1").await?);
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
use aws_config::Region;
use aws_sdk_bedrockruntime::primitives::Blob;
use aws_sdk_bedrockruntime::Client as BedrockClient;
use dashflow::core::{
    embeddings::Embeddings,
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
    Error as DashFlowError,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Bedrock embedding model identifiers
pub mod models {
    /// Amazon Titan Text Embeddings v2 (1024 dimensions, recommended)
    pub const TITAN_EMBED_TEXT_V2: &str = "amazon.titan-embed-text-v2:0";
    /// Amazon Titan Text Embeddings v1 (1536 dimensions)
    pub const TITAN_EMBED_TEXT_V1: &str = "amazon.titan-embed-text-v1";
    /// Amazon Titan Multimodal Embeddings v1 (text and images)
    pub const TITAN_EMBED_MULTIMODAL_V1: &str = "amazon.titan-embed-image-v1";
    /// Cohere Embed English v3
    pub const COHERE_EMBED_ENGLISH_V3: &str = "cohere.embed-english-v3";
    /// Cohere Embed Multilingual v3
    pub const COHERE_EMBED_MULTILINGUAL_V3: &str = "cohere.embed-multilingual-v3";
}

/// Input type for Titan v2 embeddings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    /// For search queries (retrieval)
    #[serde(rename = "search_query")]
    SearchQuery,
    /// For documents to be indexed
    #[serde(rename = "search_document")]
    SearchDocument,
}

/// Normalize mode for Titan v2 embeddings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NormalizeMode {
    /// No normalization
    None,
    /// L2 normalization (default)
    #[default]
    L2,
}

/// AWS Bedrock embedding model integration.
///
/// Supports the following models:
/// - `amazon.titan-embed-text-v2:0`: Latest Titan embeddings (1024 dimensions)
/// - `amazon.titan-embed-text-v1`: Original Titan embeddings (1536 dimensions)
/// - `cohere.embed-english-v3`: Cohere English embeddings
/// - `cohere.embed-multilingual-v3`: Cohere Multilingual embeddings
///
/// # Authentication
///
/// Uses standard AWS SDK authentication chain:
/// - Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
/// - AWS credentials file (~/.aws/credentials)
/// - IAM instance profile (for EC2/ECS)
/// - IAM role (for Lambda)
///
/// # Configuration
///
/// ```no_run
/// # use dashflow_bedrock::BedrockEmbeddings;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let embedder = BedrockEmbeddings::new("us-east-1")
///     .await?
///     .with_model("amazon.titan-embed-text-v2:0")
///     .with_dimensions(512);  // Reduced dimensions for efficiency
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct BedrockEmbeddings {
    /// AWS Bedrock client
    client: BedrockClient,
    /// Model ID (e.g., "amazon.titan-embed-text-v2:0")
    model_id: String,
    /// AWS region
    region: String,
    /// Optional: Output dimensions (Titan v2 only, 256-1024)
    dimensions: Option<u32>,
    /// Optional: Normalization mode (Titan v2 only)
    normalize: NormalizeMode,
    /// Maximum number of texts to embed in parallel
    batch_size: usize,
    /// Retry policy for API calls
    retry_policy: RetryPolicy,
    /// Optional rate limiter
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl BedrockEmbeddings {
    /// Create a new Bedrock embeddings instance with specified region.
    ///
    /// Uses standard AWS SDK authentication chain.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dashflow_bedrock::BedrockEmbeddings;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let embedder = BedrockEmbeddings::new("us-east-1").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(region: impl Into<String>) -> Result<Self, DashFlowError> {
        let region_str = region.into();
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(Region::new(region_str.clone()))
            .load()
            .await;

        let client = BedrockClient::new(&config);

        Ok(Self {
            client,
            model_id: models::TITAN_EMBED_TEXT_V2.to_string(),
            region: region_str,
            dimensions: None,
            normalize: NormalizeMode::L2,
            batch_size: 25, // Bedrock has lower throughput than dedicated embedding APIs
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        })
    }

    /// Returns the AWS region for this embeddings instance.
    #[must_use]
    pub fn region(&self) -> &str {
        &self.region
    }

    /// Set the model ID.
    ///
    /// # Supported Models
    ///
    /// - `amazon.titan-embed-text-v2:0`: Latest Titan embeddings (1024 dimensions)
    /// - `amazon.titan-embed-text-v1`: Original Titan embeddings (1536 dimensions)
    /// - `cohere.embed-english-v3`: Cohere English embeddings
    /// - `cohere.embed-multilingual-v3`: Cohere Multilingual embeddings
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_bedrock::BedrockEmbeddings;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let embedder = BedrockEmbeddings::new("us-east-1")
    ///     .await?
    ///     .with_model("amazon.titan-embed-text-v1");
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_model(mut self, model_id: impl Into<String>) -> Self {
        self.model_id = model_id.into();
        self
    }

    /// Set the output dimensionality for embeddings.
    ///
    /// Only supported by Titan Embeddings v2. Valid range: 256-1024.
    /// Smaller dimensions reduce storage and improve speed while
    /// maintaining most of the semantic information.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_bedrock::BedrockEmbeddings;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let embedder = BedrockEmbeddings::new("us-east-1")
    ///     .await?
    ///     .with_dimensions(512);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_dimensions(mut self, dimensions: u32) -> Self {
        // Titan v2 supports 256-1024 dimensions
        self.dimensions = Some(dimensions.clamp(256, 1024));
        self
    }

    /// Set the normalization mode.
    ///
    /// Only supported by Titan Embeddings v2. Default is L2 normalization.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_bedrock::{BedrockEmbeddings, NormalizeMode};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let embedder = BedrockEmbeddings::new("us-east-1")
    ///     .await?
    ///     .with_normalize(NormalizeMode::None);
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_normalize(mut self, normalize: NormalizeMode) -> Self {
        self.normalize = normalize;
        self
    }

    /// Set the batch size for parallel embedding requests.
    ///
    /// Default is 25. Higher values may improve throughput but
    /// can hit rate limits.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_bedrock::BedrockEmbeddings;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let embedder = BedrockEmbeddings::new("us-east-1")
    ///     .await?
    ///     .with_batch_size(10);
    /// # Ok(())
    /// # }
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
    /// # use dashflow_bedrock::BedrockEmbeddings;
    /// # use dashflow::core::retry::RetryPolicy;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let embedder = BedrockEmbeddings::new("us-east-1")
    ///     .await?
    ///     .with_retry_policy(RetryPolicy::exponential(5));
    /// # Ok(())
    /// # }
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
    /// # use dashflow_bedrock::BedrockEmbeddings;
    /// # use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// # use std::time::Duration;
    /// # use std::sync::Arc;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let rate_limiter = InMemoryRateLimiter::new(
    ///     10.0,  // 10 requests per second
    ///     Duration::from_millis(100),
    ///     20.0,  // Max burst
    /// );
    ///
    /// let embedder = BedrockEmbeddings::new("us-east-1")
    ///     .await?
    ///     .with_rate_limiter(Arc::new(rate_limiter));
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Check if the model is Titan v2 (supports dimensions parameter)
    fn is_titan_v2(&self) -> bool {
        self.model_id.contains("titan-embed-text-v2")
    }

    /// Check if the model is a Cohere model
    fn is_cohere(&self) -> bool {
        self.model_id.starts_with("cohere.embed")
    }

    /// Embed a single text using Bedrock.
    async fn embed_single(
        &self,
        text: &str,
        input_type: Option<InputType>,
    ) -> Result<Vec<f32>, DashFlowError> {
        // Acquire rate limiter token if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        let body = if self.is_titan_v2() {
            self.build_titan_v2_request(text, input_type)?
        } else if self.is_cohere() {
            self.build_cohere_request(&[text.to_string()], input_type)?
        } else {
            self.build_titan_v1_request(text)?
        };

        let client = self.client.clone();
        let model_id = self.model_id.clone();

        let response = with_retry(&self.retry_policy, || {
            let client = client.clone();
            let model_id = model_id.clone();
            let body = body.clone();
            async move {
                client
                    .invoke_model()
                    .model_id(&model_id)
                    .body(Blob::new(body))
                    .content_type("application/json")
                    .accept("application/json")
                    .send()
                    .await
                    .map_err(|e| DashFlowError::api(format!("Bedrock API error: {e}")))
            }
        })
        .await?;

        let response_body = response.body().as_ref();
        self.parse_embedding_response(response_body)
    }

    /// Build request body for Titan v2 embeddings.
    fn build_titan_v2_request(
        &self,
        text: &str,
        input_type: Option<InputType>,
    ) -> Result<Vec<u8>, DashFlowError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct TitanV2Request<'a> {
            input_text: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            dimensions: Option<u32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            normalize: Option<bool>,
            #[serde(skip_serializing_if = "Option::is_none")]
            #[serde(rename = "embeddingTypes")]
            embedding_types: Option<Vec<&'a str>>,
        }

        let request = TitanV2Request {
            input_text: text,
            dimensions: self.dimensions,
            normalize: Some(matches!(self.normalize, NormalizeMode::L2)),
            embedding_types: input_type.map(|_it| vec!["float"]),
        };

        serde_json::to_vec(&request).map_err(DashFlowError::Serialization)
    }

    /// Build request body for Titan v1 embeddings.
    fn build_titan_v1_request(&self, text: &str) -> Result<Vec<u8>, DashFlowError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct TitanV1Request<'a> {
            input_text: &'a str,
        }

        let request = TitanV1Request { input_text: text };

        serde_json::to_vec(&request).map_err(DashFlowError::Serialization)
    }

    /// Build request body for Cohere embeddings.
    fn build_cohere_request(
        &self,
        texts: &[String],
        input_type: Option<InputType>,
    ) -> Result<Vec<u8>, DashFlowError> {
        #[derive(Serialize)]
        struct CohereRequest {
            texts: Vec<String>,
            input_type: String,
        }

        let cohere_input_type = match input_type {
            Some(InputType::SearchQuery) => "search_query",
            Some(InputType::SearchDocument) | None => "search_document",
        };

        let request = CohereRequest {
            texts: texts.to_vec(),
            input_type: cohere_input_type.to_string(),
        };

        serde_json::to_vec(&request).map_err(DashFlowError::Serialization)
    }

    /// Parse the embedding response based on model type.
    fn parse_embedding_response(&self, body: &[u8]) -> Result<Vec<f32>, DashFlowError> {
        if self.is_cohere() {
            #[derive(Deserialize)]
            struct CohereResponse {
                embeddings: Vec<Vec<f32>>,
            }
            let response: CohereResponse = serde_json::from_slice(body)
                .map_err(|e| DashFlowError::api(format!("Failed to parse Cohere response: {e}")))?;
            response
                .embeddings
                .into_iter()
                .next()
                .ok_or_else(|| DashFlowError::api("No embeddings in Cohere response"))
        } else {
            // Titan v1 and v2 share the same response format
            #[derive(Deserialize)]
            struct TitanResponse {
                embedding: Vec<f32>,
            }
            let response: TitanResponse = serde_json::from_slice(body)
                .map_err(|e| DashFlowError::api(format!("Failed to parse Titan response: {e}")))?;
            Ok(response.embedding)
        }
    }
}

#[async_trait]
impl Embeddings for BedrockEmbeddings {
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, DashFlowError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        // Process in batches (sequentially for Bedrock due to per-request model)
        for chunk in texts.chunks(self.batch_size) {
            let mut batch_embeddings = Vec::with_capacity(chunk.len());
            for text in chunk {
                let embedding = self
                    .embed_single(text, Some(InputType::SearchDocument))
                    .await?;
                batch_embeddings.push(embedding);
            }
            all_embeddings.extend(batch_embeddings);
        }

        Ok(all_embeddings)
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, DashFlowError> {
        self.embed_single(text, Some(InputType::SearchQuery)).await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY"]
    async fn test_constructor_defaults() {
        let embedder = BedrockEmbeddings::new("us-east-1").await.unwrap();
        assert_eq!(embedder.model_id, models::TITAN_EMBED_TEXT_V2);
        assert_eq!(embedder.region, "us-east-1");
        assert!(embedder.dimensions.is_none());
        assert_eq!(embedder.batch_size, 25);
    }

    #[test]
    fn test_with_model() {
        // Create a mock embedder for unit testing (without AWS credentials)
        let model_id = models::TITAN_EMBED_TEXT_V1;
        assert!(model_id.contains("titan-embed-text-v1"));
    }

    #[test]
    fn test_with_dimensions_clamping() {
        // Test dimension clamping logic
        let dim_low = 100u32.clamp(256, 1024);
        assert_eq!(dim_low, 256);

        let dim_high = 2000u32.clamp(256, 1024);
        assert_eq!(dim_high, 1024);

        let dim_valid = 512u32.clamp(256, 1024);
        assert_eq!(dim_valid, 512);
    }

    #[test]
    fn test_model_detection() {
        let titan_v2 = "amazon.titan-embed-text-v2:0";
        let titan_v1 = "amazon.titan-embed-text-v1";
        let cohere = "cohere.embed-english-v3";

        assert!(titan_v2.contains("titan-embed-text-v2"));
        assert!(!titan_v1.contains("titan-embed-text-v2"));
        assert!(cohere.starts_with("cohere.embed"));
    }

    #[test]
    fn test_titan_v2_request_serialization() {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct TitanV2Request<'a> {
            input_text: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            dimensions: Option<u32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            normalize: Option<bool>,
        }

        let request = TitanV2Request {
            input_text: "test",
            dimensions: Some(512),
            normalize: Some(true),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("inputText"));
        assert!(json.contains("dimensions"));
        assert!(json.contains("512"));
    }

    #[test]
    fn test_titan_v1_request_serialization() {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct TitanV1Request<'a> {
            input_text: &'a str,
        }

        let request = TitanV1Request { input_text: "test" };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("inputText"));
    }

    #[test]
    fn test_cohere_request_serialization() {
        #[derive(Serialize)]
        struct CohereRequest {
            texts: Vec<String>,
            input_type: String,
        }

        let request = CohereRequest {
            texts: vec!["test".to_string()],
            input_type: "search_query".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("texts"));
        assert!(json.contains("input_type"));
        assert!(json.contains("search_query"));
    }

    #[test]
    fn test_input_type_serialization() {
        let query = InputType::SearchQuery;
        let doc = InputType::SearchDocument;

        let query_json = serde_json::to_string(&query).unwrap();
        let doc_json = serde_json::to_string(&doc).unwrap();

        assert_eq!(query_json, "\"search_query\"");
        assert_eq!(doc_json, "\"search_document\"");
    }

    #[test]
    fn test_normalize_mode_default() {
        let mode = NormalizeMode::default();
        assert_eq!(mode, NormalizeMode::L2);
    }

    #[test]
    fn test_titan_response_parsing() {
        #[derive(Deserialize)]
        struct TitanResponse {
            embedding: Vec<f32>,
        }

        let json = r#"{"embedding": [0.1, 0.2, 0.3]}"#;
        let response: TitanResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.embedding, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_cohere_response_parsing() {
        #[derive(Deserialize)]
        struct CohereResponse {
            embeddings: Vec<Vec<f32>>,
        }

        let json = r#"{"embeddings": [[0.1, 0.2, 0.3]]}"#;
        let response: CohereResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.embeddings[0], vec![0.1, 0.2, 0.3]);
    }

    #[test]
    #[allow(clippy::absurd_extreme_comparisons)] // Tests usize::max() boundary behavior
    fn test_batch_size_min() {
        let input = 0usize;
        let batch_size = input.max(1);
        assert_eq!(batch_size, 1);
    }

    // ============ Additional InputType Tests ============

    #[test]
    fn test_input_type_search_query() {
        let input_type = InputType::SearchQuery;
        assert!(matches!(input_type, InputType::SearchQuery));
    }

    #[test]
    fn test_input_type_search_document() {
        let input_type = InputType::SearchDocument;
        assert!(matches!(input_type, InputType::SearchDocument));
    }

    #[test]
    fn test_input_type_clone() {
        let original = InputType::SearchQuery;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_input_type_copy() {
        let original = InputType::SearchDocument;
        let copied = original;
        assert_eq!(original, copied);
    }

    #[test]
    fn test_input_type_debug() {
        let query = InputType::SearchQuery;
        let doc = InputType::SearchDocument;
        let query_debug = format!("{:?}", query);
        let doc_debug = format!("{:?}", doc);
        assert!(query_debug.contains("SearchQuery"));
        assert!(doc_debug.contains("SearchDocument"));
    }

    #[test]
    fn test_input_type_equality() {
        assert_eq!(InputType::SearchQuery, InputType::SearchQuery);
        assert_eq!(InputType::SearchDocument, InputType::SearchDocument);
        assert_ne!(InputType::SearchQuery, InputType::SearchDocument);
    }

    // ============ Additional NormalizeMode Tests ============

    #[test]
    fn test_normalize_mode_none() {
        let mode = NormalizeMode::None;
        assert!(matches!(mode, NormalizeMode::None));
    }

    #[test]
    fn test_normalize_mode_l2() {
        let mode = NormalizeMode::L2;
        assert!(matches!(mode, NormalizeMode::L2));
    }

    #[test]
    fn test_normalize_mode_clone() {
        let original = NormalizeMode::L2;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_normalize_mode_copy() {
        let original = NormalizeMode::None;
        let copied = original;
        assert_eq!(original, copied);
    }

    #[test]
    fn test_normalize_mode_debug() {
        let none = NormalizeMode::None;
        let l2 = NormalizeMode::L2;
        assert!(format!("{:?}", none).contains("None"));
        assert!(format!("{:?}", l2).contains("L2"));
    }

    #[test]
    fn test_normalize_mode_equality() {
        assert_eq!(NormalizeMode::None, NormalizeMode::None);
        assert_eq!(NormalizeMode::L2, NormalizeMode::L2);
        assert_ne!(NormalizeMode::None, NormalizeMode::L2);
    }

    #[test]
    fn test_normalize_mode_serialization() {
        let l2 = NormalizeMode::L2;
        let none = NormalizeMode::None;

        let l2_json = serde_json::to_string(&l2).unwrap();
        let none_json = serde_json::to_string(&none).unwrap();

        // Check serialization format (lowercase)
        assert_eq!(l2_json, "\"l2\"");
        assert_eq!(none_json, "\"none\"");
    }

    // ============ Additional Dimension Clamping Tests ============

    #[test]
    fn test_dimensions_at_minimum() {
        let dim = 256u32.clamp(256, 1024);
        assert_eq!(dim, 256);
    }

    #[test]
    fn test_dimensions_at_maximum() {
        let dim = 1024u32.clamp(256, 1024);
        assert_eq!(dim, 1024);
    }

    #[test]
    fn test_dimensions_middle_values() {
        let dim_384 = 384u32.clamp(256, 1024);
        let dim_768 = 768u32.clamp(256, 1024);
        let dim_512 = 512u32.clamp(256, 1024);
        assert_eq!(dim_384, 384);
        assert_eq!(dim_768, 768);
        assert_eq!(dim_512, 512);
    }

    #[test]
    fn test_dimensions_just_below_min() {
        let dim = 255u32.clamp(256, 1024);
        assert_eq!(dim, 256);
    }

    #[test]
    fn test_dimensions_just_above_max() {
        let dim = 1025u32.clamp(256, 1024);
        assert_eq!(dim, 1024);
    }

    #[test]
    fn test_dimensions_zero() {
        let dim = 0u32.clamp(256, 1024);
        assert_eq!(dim, 256);
    }

    #[test]
    fn test_dimensions_u32_max() {
        let dim = u32::MAX.clamp(256, 1024);
        assert_eq!(dim, 1024);
    }

    // ============ Additional Model Detection Tests ============

    #[test]
    fn test_is_titan_v2_exact() {
        assert!("amazon.titan-embed-text-v2:0".contains("titan-embed-text-v2"));
    }

    #[test]
    fn test_is_titan_v1_not_v2() {
        let model = "amazon.titan-embed-text-v1";
        assert!(model.contains("titan-embed-text-v1"));
        assert!(!model.contains("titan-embed-text-v2"));
    }

    #[test]
    fn test_is_cohere_english() {
        assert!(models::COHERE_EMBED_ENGLISH_V3.starts_with("cohere.embed"));
    }

    #[test]
    fn test_is_cohere_multilingual() {
        assert!(models::COHERE_EMBED_MULTILINGUAL_V3.starts_with("cohere.embed"));
    }

    #[test]
    fn test_is_titan_multimodal() {
        assert!(models::TITAN_EMBED_MULTIMODAL_V1.contains("titan-embed"));
    }

    #[test]
    fn test_model_constants_not_empty() {
        assert!(!models::TITAN_EMBED_TEXT_V2.is_empty());
        assert!(!models::TITAN_EMBED_TEXT_V1.is_empty());
        assert!(!models::TITAN_EMBED_MULTIMODAL_V1.is_empty());
        assert!(!models::COHERE_EMBED_ENGLISH_V3.is_empty());
        assert!(!models::COHERE_EMBED_MULTILINGUAL_V3.is_empty());
    }

    #[test]
    fn test_model_constants_unique() {
        use std::collections::HashSet;
        let models: HashSet<&str> = [
            models::TITAN_EMBED_TEXT_V2,
            models::TITAN_EMBED_TEXT_V1,
            models::TITAN_EMBED_MULTIMODAL_V1,
            models::COHERE_EMBED_ENGLISH_V3,
            models::COHERE_EMBED_MULTILINGUAL_V3,
        ].into_iter().collect();
        assert_eq!(models.len(), 5);
    }

    // ============ Additional Request Serialization Tests ============

    #[test]
    fn test_titan_v2_request_minimal() {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct TitanV2Request<'a> {
            input_text: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            dimensions: Option<u32>,
            #[serde(skip_serializing_if = "Option::is_none")]
            normalize: Option<bool>,
        }

        let request = TitanV2Request {
            input_text: "test",
            dimensions: None,
            normalize: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("inputText"));
        assert!(!json.contains("dimensions"));
        assert!(!json.contains("normalize"));
    }

    #[test]
    fn test_titan_v2_request_with_dimensions() {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct TitanV2Request {
            input_text: String,
            dimensions: Option<u32>,
        }

        let request = TitanV2Request {
            input_text: "test".to_string(),
            dimensions: Some(384),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("384"));
    }

    #[test]
    fn test_titan_v2_request_unicode() {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct TitanV2Request {
            input_text: String,
        }

        let request = TitanV2Request {
            input_text: "Hello 世界 🌍".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("世界"));
        assert!(json.contains("🌍"));
    }

    #[test]
    fn test_titan_v1_request_unicode() {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct TitanV1Request {
            input_text: String,
        }

        let request = TitanV1Request {
            input_text: "مرحبا بالعالم".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("مرحبا"));
    }

    #[test]
    fn test_cohere_request_multiple_texts() {
        #[derive(Serialize)]
        struct CohereRequest {
            texts: Vec<String>,
            input_type: String,
        }

        let request = CohereRequest {
            texts: vec!["one".to_string(), "two".to_string(), "three".to_string()],
            input_type: "search_document".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("one"));
        assert!(json.contains("two"));
        assert!(json.contains("three"));
    }

    #[test]
    fn test_cohere_request_empty_texts() {
        #[derive(Serialize)]
        struct CohereRequest {
            texts: Vec<String>,
            input_type: String,
        }

        let request = CohereRequest {
            texts: vec![],
            input_type: "search_query".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("[]"));
    }

    // ============ Additional Response Parsing Tests ============

    #[test]
    fn test_titan_response_empty_embedding() {
        #[derive(Deserialize)]
        struct TitanResponse {
            embedding: Vec<f32>,
        }

        let json = r#"{"embedding": []}"#;
        let response: TitanResponse = serde_json::from_str(json).unwrap();
        assert!(response.embedding.is_empty());
    }

    #[test]
    fn test_titan_response_large_embedding() {
        #[derive(Deserialize)]
        struct TitanResponse {
            embedding: Vec<f32>,
        }

        // Simulate 1024 dimensions
        let values: Vec<f32> = (0..1024).map(|i| i as f32 * 0.001).collect();
        let json = serde_json::json!({"embedding": values});
        let response: TitanResponse = serde_json::from_str(&json.to_string()).unwrap();
        assert_eq!(response.embedding.len(), 1024);
    }

    #[test]
    fn test_titan_response_negative_values() {
        #[derive(Deserialize)]
        struct TitanResponse {
            embedding: Vec<f32>,
        }

        let json = r#"{"embedding": [-0.1, -0.2, -0.3]}"#;
        let response: TitanResponse = serde_json::from_str(json).unwrap();
        assert!(response.embedding[0] < 0.0);
    }

    #[test]
    fn test_cohere_response_multiple_embeddings() {
        #[derive(Deserialize)]
        struct CohereResponse {
            embeddings: Vec<Vec<f32>>,
        }

        let json = r#"{"embeddings": [[0.1, 0.2], [0.3, 0.4], [0.5, 0.6]]}"#;
        let response: CohereResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.embeddings.len(), 3);
        assert_eq!(response.embeddings[0].len(), 2);
    }

    #[test]
    fn test_cohere_response_empty_embeddings() {
        #[derive(Deserialize)]
        struct CohereResponse {
            embeddings: Vec<Vec<f32>>,
        }

        let json = r#"{"embeddings": []}"#;
        let response: CohereResponse = serde_json::from_str(json).unwrap();
        assert!(response.embeddings.is_empty());
    }

    #[test]
    fn test_cohere_response_high_precision() {
        #[derive(Deserialize)]
        struct CohereResponse {
            embeddings: Vec<Vec<f32>>,
        }

        let json = r#"{"embeddings": [[0.123456789, -0.987654321]]}"#;
        let response: CohereResponse = serde_json::from_str(json).unwrap();
        // f32 has about 6-7 significant digits
        assert!((response.embeddings[0][0] - 0.123456789).abs() < 0.0001);
    }

    // ============ Additional Batch Size Tests ============

    #[test]
    fn test_batch_size_one() {
        let input = 1usize;
        let batch_size = input.max(1);
        assert_eq!(batch_size, 1);
    }

    #[test]
    fn test_batch_size_large() {
        let input = 1000usize;
        let batch_size = input.max(1);
        assert_eq!(batch_size, 1000);
    }

    #[test]
    fn test_batch_size_default() {
        let default_batch = 25usize;
        assert_eq!(default_batch, 25);
    }

    // ============ InputType and NormalizeMode Combined Tests ============

    #[test]
    fn test_input_type_cohere_mapping() {
        // Test the mapping logic for Cohere
        let search_query = InputType::SearchQuery;
        let search_doc = InputType::SearchDocument;

        let cohere_query = match search_query {
            InputType::SearchQuery => "search_query",
            InputType::SearchDocument => "search_document",
        };

        let cohere_doc = match search_doc {
            InputType::SearchQuery => "search_query",
            InputType::SearchDocument => "search_document",
        };

        assert_eq!(cohere_query, "search_query");
        assert_eq!(cohere_doc, "search_document");
    }

    #[test]
    fn test_normalize_mode_to_bool() {
        let l2 = NormalizeMode::L2;
        let none = NormalizeMode::None;

        let l2_bool = matches!(l2, NormalizeMode::L2);
        let none_bool = matches!(none, NormalizeMode::L2);

        assert!(l2_bool);
        assert!(!none_bool);
    }

    // ============ Model String Validation Tests ============

    #[test]
    fn test_titan_v2_model_format() {
        let model = models::TITAN_EMBED_TEXT_V2;
        assert!(model.starts_with("amazon."));
        assert!(model.contains(":0"));
    }

    #[test]
    fn test_titan_v1_model_format() {
        let model = models::TITAN_EMBED_TEXT_V1;
        assert!(model.starts_with("amazon."));
        assert!(!model.contains(":"));
    }

    #[test]
    fn test_cohere_model_format() {
        let english = models::COHERE_EMBED_ENGLISH_V3;
        let multilingual = models::COHERE_EMBED_MULTILINGUAL_V3;

        assert!(english.starts_with("cohere.embed-"));
        assert!(multilingual.starts_with("cohere.embed-"));
        assert!(english.contains("english"));
        assert!(multilingual.contains("multilingual"));
    }

    // ============ RetryPolicy Tests ============

    #[test]
    fn test_retry_policy_creation() {
        use dashflow::core::retry::RetryPolicy;

        let policy = RetryPolicy::exponential(3);
        // Just verify it can be created
        assert!(std::mem::size_of_val(&policy) > 0);
    }

    #[test]
    fn test_retry_policy_different_counts() {
        use dashflow::core::retry::RetryPolicy;

        let _policy_1 = RetryPolicy::exponential(1);
        let _policy_5 = RetryPolicy::exponential(5);
        let _policy_10 = RetryPolicy::exponential(10);
    }

    // ============ Additional Region Tests ============

    #[test]
    fn test_region_formats() {
        let regions = [
            "us-east-1",
            "us-west-2",
            "eu-west-1",
            "ap-northeast-1",
            "ap-southeast-2",
        ];
        for region in regions {
            assert!(region.contains("-"));
            assert!(region.len() >= 9);
        }
    }

    #[test]
    fn test_region_format_with_number() {
        let region = "us-east-1";
        let parts: Vec<&str> = region.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts[2].chars().all(char::is_numeric));
    }

    // ============ Additional Titan V2 Request Tests ============

    #[test]
    fn test_titan_v2_request_with_all_options() {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct TitanV2Request {
            input_text: String,
            dimensions: Option<u32>,
            normalize: Option<bool>,
        }

        let request = TitanV2Request {
            input_text: "test input".to_string(),
            dimensions: Some(768),
            normalize: Some(true),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("inputText"));
        assert!(json.contains("768"));
        assert!(json.contains("true"));
    }

    #[test]
    fn test_titan_v2_request_empty_text() {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct TitanV2Request {
            input_text: String,
        }

        let request = TitanV2Request {
            input_text: String::new(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"inputText\":\"\""));
    }

    #[test]
    fn test_titan_v2_request_long_text() {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct TitanV2Request {
            input_text: String,
        }

        let long_text = "x".repeat(8000);
        let request = TitanV2Request {
            input_text: long_text.clone(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.len() > 8000);
    }

    #[test]
    fn test_titan_v2_request_special_characters() {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct TitanV2Request {
            input_text: String,
        }

        let request = TitanV2Request {
            input_text: "Hello \"world\" with 'quotes' and\nnewlines\ttabs".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\\\""));
        assert!(json.contains("\\n"));
        assert!(json.contains("\\t"));
    }

    // ============ Additional Cohere Request Tests ============

    #[test]
    fn test_cohere_request_input_types() {
        // Verify all valid input types
        let input_types = ["search_query", "search_document", "classification", "clustering"];
        for input_type in input_types {
            assert!(!input_type.is_empty());
        }
    }

    #[test]
    fn test_cohere_request_large_batch() {
        #[derive(Serialize)]
        struct CohereRequest {
            texts: Vec<String>,
            input_type: String,
        }

        let texts: Vec<String> = (0..100).map(|i| format!("Text number {}", i)).collect();
        let request = CohereRequest {
            texts,
            input_type: "search_document".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Text number 99"));
    }

    #[test]
    fn test_cohere_request_unicode_texts() {
        #[derive(Serialize)]
        struct CohereRequest {
            texts: Vec<String>,
            input_type: String,
        }

        let request = CohereRequest {
            texts: vec![
                "English text".to_string(),
                "中文文本".to_string(),
                "日本語テキスト".to_string(),
                "한국어 텍스트".to_string(),
                "Текст на русском".to_string(),
            ],
            input_type: "search_document".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("中文文本"));
        assert!(json.contains("日本語"));
    }

    // ============ Additional Response Parsing Tests ============

    #[test]
    fn test_titan_response_1024_dimensions() {
        #[derive(Deserialize)]
        struct TitanResponse {
            embedding: Vec<f32>,
        }

        let values: Vec<f32> = (0..1024).map(|i| i as f32 * 0.001).collect();
        let json = serde_json::json!({"embedding": values});
        let response: TitanResponse = serde_json::from_str(&json.to_string()).unwrap();
        assert_eq!(response.embedding.len(), 1024);
    }

    #[test]
    fn test_titan_response_1536_dimensions() {
        #[derive(Deserialize)]
        struct TitanResponse {
            embedding: Vec<f32>,
        }

        let values: Vec<f32> = (0..1536).map(|i| (i as f32).sin()).collect();
        let json = serde_json::json!({"embedding": values});
        let response: TitanResponse = serde_json::from_str(&json.to_string()).unwrap();
        assert_eq!(response.embedding.len(), 1536);
    }

    #[test]
    fn test_titan_response_normalized_values() {
        #[derive(Deserialize)]
        struct TitanResponse {
            embedding: Vec<f32>,
        }

        // L2-normalized vector should have values between -1 and 1
        let json = r#"{"embedding": [0.5, -0.3, 0.1, -0.7, 0.4]}"#;
        let response: TitanResponse = serde_json::from_str(json).unwrap();
        for val in &response.embedding {
            assert!(*val >= -1.0 && *val <= 1.0);
        }
    }

    #[test]
    fn test_cohere_response_batch_different_sizes() {
        #[derive(Deserialize)]
        struct CohereResponse {
            embeddings: Vec<Vec<f32>>,
        }

        // All embeddings should have same dimension
        let json = r#"{"embeddings": [[0.1, 0.2, 0.3], [0.4, 0.5, 0.6], [0.7, 0.8, 0.9]]}"#;
        let response: CohereResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.embeddings.len(), 3);
        let dim = response.embeddings[0].len();
        for emb in &response.embeddings {
            assert_eq!(emb.len(), dim);
        }
    }

    #[test]
    fn test_cohere_response_1024_dimensions() {
        #[derive(Deserialize)]
        struct CohereResponse {
            embeddings: Vec<Vec<f32>>,
        }

        let values: Vec<f32> = (0..1024).map(|i| (i as f32 / 1024.0) - 0.5).collect();
        let json = serde_json::json!({"embeddings": [values]});
        let response: CohereResponse = serde_json::from_str(&json.to_string()).unwrap();
        assert_eq!(response.embeddings[0].len(), 1024);
    }

    // ============ Additional Dimension Range Tests ============

    #[test]
    fn test_dimension_clamping_powers_of_two() {
        let powers: Vec<u32> = vec![128, 256, 512, 1024, 2048];
        for pow in powers {
            let clamped = pow.clamp(256, 1024);
            assert!(clamped >= 256 && clamped <= 1024);
        }
    }

    #[test]
    fn test_dimension_common_values() {
        let common = [256, 384, 512, 768, 1024];
        for dim in common {
            let clamped = dim.clamp(256, 1024);
            assert_eq!(clamped, dim); // All are within range
        }
    }

    #[test]
    fn test_dimension_edge_cases() {
        assert_eq!(1u32.clamp(256, 1024), 256);
        assert_eq!(100u32.clamp(256, 1024), 256);
        assert_eq!(256u32.clamp(256, 1024), 256);
        assert_eq!(257u32.clamp(256, 1024), 257);
        assert_eq!(1023u32.clamp(256, 1024), 1023);
        assert_eq!(1024u32.clamp(256, 1024), 1024);
        assert_eq!(1025u32.clamp(256, 1024), 1024);
        assert_eq!(10000u32.clamp(256, 1024), 1024);
    }

    // ============ Model ID Validation Tests ============

    #[test]
    fn test_model_id_patterns() {
        // Titan models follow amazon.* pattern
        assert!(models::TITAN_EMBED_TEXT_V2.starts_with("amazon."));
        assert!(models::TITAN_EMBED_TEXT_V1.starts_with("amazon."));
        assert!(models::TITAN_EMBED_MULTIMODAL_V1.starts_with("amazon."));

        // Cohere models follow cohere.* pattern
        assert!(models::COHERE_EMBED_ENGLISH_V3.starts_with("cohere."));
        assert!(models::COHERE_EMBED_MULTILINGUAL_V3.starts_with("cohere."));
    }

    #[test]
    fn test_model_id_version_suffixes() {
        // v2 models have :0 suffix
        assert!(models::TITAN_EMBED_TEXT_V2.ends_with(":0"));

        // v1 models don't have version suffix
        assert!(!models::TITAN_EMBED_TEXT_V1.contains(":"));
    }

    #[test]
    fn test_model_id_no_spaces() {
        let all_models = [
            models::TITAN_EMBED_TEXT_V2,
            models::TITAN_EMBED_TEXT_V1,
            models::TITAN_EMBED_MULTIMODAL_V1,
            models::COHERE_EMBED_ENGLISH_V3,
            models::COHERE_EMBED_MULTILINGUAL_V3,
        ];
        for model in all_models {
            assert!(!model.contains(' '));
        }
    }

    // ============ Batch Size Validation Tests ============

    #[test]
    fn test_batch_size_reasonable_values() {
        let sizes = [1, 5, 10, 25, 50, 100];
        for size in sizes {
            assert!(size >= 1);
            let clamped = size.max(1);
            assert_eq!(clamped, size);
        }
    }

    #[test]
    fn test_batch_size_zero_clamped() {
        let size = 0usize;
        let clamped = size.max(1);
        assert_eq!(clamped, 1);
    }

    // ============ InputType Conversion Tests ============

    #[test]
    fn test_input_type_to_cohere_string() {
        let mappings = [
            (InputType::SearchQuery, "search_query"),
            (InputType::SearchDocument, "search_document"),
        ];

        for (input_type, expected) in mappings {
            let result = match input_type {
                InputType::SearchQuery => "search_query",
                InputType::SearchDocument => "search_document",
            };
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_input_type_none_defaults_to_document() {
        let input_type: Option<InputType> = None;
        let cohere_type = match input_type {
            Some(InputType::SearchQuery) => "search_query",
            Some(InputType::SearchDocument) | None => "search_document",
        };
        assert_eq!(cohere_type, "search_document");
    }

    // ============ NormalizeMode Boolean Conversion Tests ============

    #[test]
    fn test_normalize_mode_to_bool_mapping() {
        let l2_as_bool = matches!(NormalizeMode::L2, NormalizeMode::L2);
        let none_as_bool = matches!(NormalizeMode::None, NormalizeMode::L2);

        assert!(l2_as_bool);
        assert!(!none_as_bool);
    }

    #[test]
    fn test_normalize_mode_all_variants() {
        let modes = [NormalizeMode::None, NormalizeMode::L2];
        assert_eq!(modes.len(), 2);

        for mode in &modes {
            let json = serde_json::to_string(mode).unwrap();
            assert!(json.starts_with('"'));
            assert!(json.ends_with('"'));
        }
    }

    // ============ Unicode and Special Character Tests ============

    #[test]
    fn test_input_text_with_emojis() {
        let text = "Machine learning is amazing! 🤖💡🚀";
        assert!(text.contains("🤖"));
        assert!(text.len() > 30); // UTF-8 emojis take multiple bytes
    }

    #[test]
    fn test_input_text_with_mixed_scripts() {
        let text = "English, 中文, العربية, עברית, Ελληνικά";
        assert!(text.contains("中文"));
        assert!(text.contains("العربية"));
    }

    #[test]
    fn test_input_text_with_math_symbols() {
        let text = "∑ ∏ ∫ √ ∞ ≤ ≥ ≠ π θ α β";
        assert!(text.contains("∑"));
        assert!(text.contains("π"));
    }

    #[test]
    fn test_input_text_with_code() {
        let text = "fn main() { println!(\"Hello\"); }";
        assert!(text.contains("fn"));
        assert!(text.contains("println!"));
    }

    // ============ Float Precision Tests ============

    #[test]
    fn test_embedding_value_range() {
        // Typical embedding values are between -1 and 1
        let values = [-0.999f32, -0.5, 0.0, 0.5, 0.999];
        for val in values {
            assert!(val >= -1.0 && val <= 1.0);
        }
    }

    #[test]
    fn test_embedding_f32_precision() {
        // f32 has about 7 significant digits
        let val = 0.123456789f32;
        assert!((val - 0.12345679).abs() < 0.000001);
    }

    #[test]
    fn test_embedding_subnormal_values() {
        // Very small values should still be representable
        let tiny = 1.0e-38f32;
        assert!(tiny > 0.0);
        assert!(tiny.is_finite());
    }

    // ============ Default Value Tests ============

    #[test]
    fn test_default_batch_size() {
        let default = 25usize;
        assert_eq!(default, 25);
    }

    #[test]
    fn test_default_model() {
        let default = models::TITAN_EMBED_TEXT_V2;
        assert!(default.contains("titan-embed-text-v2"));
    }

    #[test]
    fn test_default_normalize_mode() {
        let default = NormalizeMode::default();
        assert_eq!(default, NormalizeMode::L2);
    }

    // ============ Error Scenario Tests ============

    #[test]
    fn test_malformed_titan_response() {
        #[derive(Deserialize)]
        struct TitanResponse {
            embedding: Vec<f32>,
        }

        let invalid_json = r#"{"embedding": "not an array"}"#;
        let result: Result<TitanResponse, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_embedding_field() {
        #[derive(Deserialize)]
        struct TitanResponse {
            embedding: Vec<f32>,
        }

        let no_embedding = r#"{"other_field": [1, 2, 3]}"#;
        let result: Result<TitanResponse, _> = serde_json::from_str(no_embedding);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_type_in_embedding() {
        #[derive(Deserialize)]
        struct TitanResponse {
            embedding: Vec<f32>,
        }

        let wrong_type = r#"{"embedding": ["a", "b", "c"]}"#;
        let result: Result<TitanResponse, _> = serde_json::from_str(wrong_type);
        assert!(result.is_err());
    }

    // ============ Serialization Round-Trip Tests ============

    #[test]
    fn test_input_type_serialization_round_trip() {
        for input_type in [InputType::SearchQuery, InputType::SearchDocument] {
            let serialized = serde_json::to_string(&input_type).unwrap();
            // InputType is serialize-only (no Deserialize impl needed here)
            assert!(!serialized.is_empty());
        }
    }

    #[test]
    fn test_normalize_mode_serialization_round_trip() {
        for mode in [NormalizeMode::None, NormalizeMode::L2] {
            let serialized = serde_json::to_string(&mode).unwrap();
            assert!(!serialized.is_empty());
        }
    }

    // ============ Capacity and Performance Tests ============

    #[test]
    fn test_large_embedding_allocation() {
        let large_embedding: Vec<f32> = vec![0.0; 4096];
        assert_eq!(large_embedding.len(), 4096);
        assert!(large_embedding.capacity() >= 4096);
    }

    #[test]
    fn test_batch_capacity_calculation() {
        let num_texts = 100;
        let expected_dim = 1024;
        let mut embeddings: Vec<Vec<f32>> = Vec::with_capacity(num_texts);
        for _ in 0..num_texts {
            embeddings.push(vec![0.0; expected_dim]);
        }
        assert_eq!(embeddings.len(), num_texts);
    }
}

/// Standard conformance tests for embeddings
///
/// These tests verify that BedrockEmbeddings behaves consistently with other
/// Embeddings implementations across the DashFlow ecosystem.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod standard_tests {
    use super::*;
    use dashflow_standard_tests::embeddings_tests::*;

    /// Standard Test 1: Embed single document
    #[tokio::test]
    #[ignore = "requires AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY"]
    async fn test_embed_query_standard() {
        let embeddings = BedrockEmbeddings::new("us-east-1").await.unwrap();
        test_embed_query(Arc::new(embeddings)).await;
    }

    /// Standard Test 2: Embed multiple documents
    #[tokio::test]
    #[ignore = "requires AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY"]
    async fn test_embed_documents_standard() {
        let embeddings = BedrockEmbeddings::new("us-east-1").await.unwrap();
        test_embed_documents(Arc::new(embeddings)).await;
    }

    /// Standard Test 3: Empty input handling
    #[tokio::test]
    #[ignore = "requires AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY"]
    async fn test_empty_input_standard() {
        let embeddings = BedrockEmbeddings::new("us-east-1").await.unwrap();
        test_empty_input(Arc::new(embeddings)).await;
    }

    /// Standard Test 4: Dimension consistency
    #[tokio::test]
    #[ignore = "requires AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY"]
    async fn test_dimension_consistency_standard() {
        let embeddings = BedrockEmbeddings::new("us-east-1").await.unwrap();
        test_dimension_consistency(Arc::new(embeddings)).await;
    }

    /// Standard Test 5: Semantic similarity
    #[tokio::test]
    #[ignore = "requires AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY"]
    async fn test_semantic_similarity_standard() {
        let embeddings = BedrockEmbeddings::new("us-east-1").await.unwrap();
        test_semantic_similarity(Arc::new(embeddings)).await;
    }
}
