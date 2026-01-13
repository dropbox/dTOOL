//! Google Gemini embeddings implementation.
//!
//! This module provides embeddings using Google's Gemini embedding models.
//! The primary model is `text-embedding-004` which produces 768-dimensional
//! embeddings by default.
//!
//! # Example
//!
//! ```rust
//! use dashflow_gemini::embeddings::GeminiEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = GeminiEmbeddings::new()
//!     .with_api_key(std::env::var("GEMINI_API_KEY")?);
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
    config_loader::env_vars::{env_string, GEMINI_API_KEY},
    embeddings::Embeddings,
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
    Error as DashFlowError,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const DEFAULT_MODEL: &str = "text-embedding-004";
const API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Task type for embeddings, which optimizes the embedding for specific use cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskType {
    /// Unspecified task type
    TaskTypeUnspecified,
    /// Specifies the given text is a query in a search/retrieval setting
    RetrievalQuery,
    /// Specifies the given text is a document in a search/retrieval setting
    RetrievalDocument,
    /// Specifies the given text will be used for Semantic Textual Similarity (STS)
    SemanticSimilarity,
    /// Specifies the given text will be classified
    Classification,
    /// Specifies the given text will be used for clustering
    Clustering,
    /// Specifies the given text will be used for question answering
    QuestionAnswering,
    /// Specifies the given text will be used for fact verification
    FactVerification,
}

/// Google Gemini embedding model integration.
///
/// Supports the following models:
/// - `text-embedding-004`: Google's latest embedding model (768 dimensions, configurable)
/// - `embedding-001`: Previous generation model
///
/// # Configuration
///
/// The API key can be set via:
/// - Constructor: `GeminiEmbeddings::new().with_api_key("...")`
/// - Environment: `GEMINI_API_KEY`
///
/// # Task Types
///
/// Gemini embeddings support different task types that optimize the embedding
/// for specific use cases. Use `with_task_type()` to set the task type.
///
/// # Dimensions
///
/// For `text-embedding-004`, you can configure the output dimensionality
/// with `with_dimensions()`. Valid range is 1 to 768.
pub struct GeminiEmbeddings {
    /// API key for authentication
    api_key: Option<String>,
    /// Model name (e.g., "text-embedding-004")
    model: String,
    /// HTTP client
    client: Client,
    /// Task type for embedding optimization
    task_type: Option<TaskType>,
    /// Optional: The number of dimensions for the output embeddings
    output_dimensionality: Option<u32>,
    /// Title for document embeddings (only used with RETRIEVAL_DOCUMENT task type)
    title: Option<String>,
    /// Maximum number of texts to embed in a single batch request
    batch_size: usize,
    /// Retry policy for API calls
    retry_policy: RetryPolicy,
    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl GeminiEmbeddings {
    /// Create a new Gemini embeddings instance with default settings.
    ///
    /// Defaults:
    /// - Model: `text-embedding-004`
    /// - Batch size: 100
    /// - API key: from `GEMINI_API_KEY` environment variable
    #[must_use]
    pub fn new() -> Self {
        Self {
            api_key: env_string(GEMINI_API_KEY),
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            task_type: None,
            output_dimensionality: None,
            title: None,
            batch_size: 100,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        }
    }

    /// Set the API key explicitly.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_gemini::embeddings::GeminiEmbeddings;
    /// let embedder = GeminiEmbeddings::new()
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
    /// - `text-embedding-004`: Latest model, 768 dimensions (configurable)
    /// - `embedding-001`: Previous generation
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_gemini::embeddings::GeminiEmbeddings;
    /// let embedder = GeminiEmbeddings::new()
    ///     .with_model("text-embedding-004");
    /// ```
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the task type for embedding optimization.
    ///
    /// Different task types optimize the embedding for specific use cases:
    /// - `RetrievalQuery`: For search queries
    /// - `RetrievalDocument`: For documents to be searched
    /// - `SemanticSimilarity`: For comparing text similarity
    /// - `Classification`: For text classification
    /// - `Clustering`: For clustering similar texts
    /// - `QuestionAnswering`: For Q&A tasks
    /// - `FactVerification`: For fact-checking
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_gemini::embeddings::{GeminiEmbeddings, TaskType};
    /// let embedder = GeminiEmbeddings::new()
    ///     .with_task_type(TaskType::RetrievalQuery);
    /// ```
    #[must_use]
    pub fn with_task_type(mut self, task_type: TaskType) -> Self {
        self.task_type = Some(task_type);
        self
    }

    /// Set the output dimensionality for embeddings.
    ///
    /// Only supported for `text-embedding-004`. Allows you to reduce
    /// the embedding size while maintaining semantic information.
    ///
    /// # Valid Dimensions
    ///
    /// 1 to 768 (default: 768)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_gemini::embeddings::GeminiEmbeddings;
    /// let embedder = GeminiEmbeddings::new()
    ///     .with_dimensions(256);  // Reduce from 768 to 256
    /// ```
    #[must_use]
    pub fn with_dimensions(mut self, dimensions: u32) -> Self {
        self.output_dimensionality = Some(dimensions);
        self
    }

    /// Set a title for document embeddings.
    ///
    /// Only used with `RetrievalDocument` task type. The title helps
    /// improve embedding quality for document retrieval.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_gemini::embeddings::{GeminiEmbeddings, TaskType};
    /// let embedder = GeminiEmbeddings::new()
    ///     .with_task_type(TaskType::RetrievalDocument)
    ///     .with_title("Product Documentation");
    /// ```
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the batch size for batch embedding requests.
    ///
    /// Gemini's batchEmbedContents API allows up to 100 texts per request.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_gemini::embeddings::GeminiEmbeddings;
    /// let embedder = GeminiEmbeddings::new()
    ///     .with_batch_size(50);
    /// ```
    #[must_use]
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.min(100); // Gemini API limit
        self
    }

    /// Set the retry policy for API calls.
    ///
    /// Default is exponential backoff with 3 retries.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_gemini::embeddings::GeminiEmbeddings;
    /// # use dashflow::core::retry::RetryPolicy;
    /// let embedder = GeminiEmbeddings::new()
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
    /// # use dashflow_gemini::embeddings::GeminiEmbeddings;
    /// # use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// # use std::time::Duration;
    /// # use std::sync::Arc;
    /// let rate_limiter = InMemoryRateLimiter::new(
    ///     10.0,  // 10 requests per second
    ///     Duration::from_millis(100),
    ///     20.0,  // Max burst
    /// );
    ///
    /// let embedder = GeminiEmbeddings::new()
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
                "GEMINI_API_KEY not set. Set it via environment variable or with_api_key()"
                    .to_string(),
            )
        })
    }

    /// Embed a single text using the embedContent endpoint.
    async fn embed_single(&self, text: &str) -> Result<Vec<f32>, DashFlowError> {
        let api_key = self.get_api_key()?;
        let url = format!(
            "{}/models/{}:embedContent?key={}",
            API_BASE, self.model, api_key
        );

        let request = EmbedContentRequest {
            content: Content {
                parts: vec![Part {
                    text: text.to_string(),
                }],
            },
            task_type: self.task_type,
            title: self.title.clone(),
            output_dimensionality: self.output_dimensionality,
        };

        // Acquire rate limiter token if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        let response = with_retry(&self.retry_policy, || async {
            self.client
                .post(&url)
                .json(&request)
                .send()
                .await
                .map_err(|e| DashFlowError::api(format!("Gemini API request failed: {e}")))?
                .error_for_status()
                .map_err(|e| DashFlowError::api(format!("Gemini API error: {e}")))
        })
        .await?;

        let embed_response: EmbedContentResponse = response
            .json()
            .await
            .map_err(|e| DashFlowError::api(format!("Failed to parse Gemini response: {e}")))?;

        Ok(embed_response.embedding.values)
    }

    /// Embed multiple texts using the batchEmbedContents endpoint.
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, DashFlowError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let api_key = self.get_api_key()?;
        let url = format!(
            "{}/models/{}:batchEmbedContents?key={}",
            API_BASE, self.model, api_key
        );

        let requests: Vec<EmbedContentRequest> = texts
            .iter()
            .map(|text| EmbedContentRequest {
                content: Content {
                    parts: vec![Part {
                        text: text.to_string(),
                    }],
                },
                task_type: self.task_type,
                title: self.title.clone(),
                output_dimensionality: self.output_dimensionality,
            })
            .collect();

        let batch_request = BatchEmbedContentsRequest { requests };

        // Acquire rate limiter token if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        let response = with_retry(&self.retry_policy, || async {
            self.client
                .post(&url)
                .json(&batch_request)
                .send()
                .await
                .map_err(|e| DashFlowError::api(format!("Gemini API request failed: {e}")))?
                .error_for_status()
                .map_err(|e| DashFlowError::api(format!("Gemini API error: {e}")))
        })
        .await?;

        let batch_response: BatchEmbedContentsResponse = response
            .json()
            .await
            .map_err(|e| DashFlowError::api(format!("Failed to parse Gemini response: {e}")))?;

        Ok(batch_response
            .embeddings
            .into_iter()
            .map(|e| e.values)
            .collect())
    }
}

impl Default for GeminiEmbeddings {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Embeddings for GeminiEmbeddings {
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
        self.embed_single(text).await
    }
}

// Request/Response types for Gemini API

#[derive(Debug, Serialize)]
struct EmbedContentRequest {
    content: Content,
    #[serde(skip_serializing_if = "Option::is_none", rename = "taskType")]
    task_type: Option<TaskType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "outputDimensionality"
    )]
    output_dimensionality: Option<u32>,
}

#[derive(Debug, Serialize)]
struct BatchEmbedContentsRequest {
    requests: Vec<EmbedContentRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Part {
    text: String,
}

#[derive(Debug, Deserialize)]
struct EmbedContentResponse {
    embedding: ContentEmbedding,
}

#[derive(Debug, Deserialize)]
struct BatchEmbedContentsResponse {
    embeddings: Vec<ContentEmbedding>,
}

#[derive(Debug, Deserialize)]
struct ContentEmbedding {
    values: Vec<f32>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow::core::rate_limiters::InMemoryRateLimiter;
    use std::time::Duration;

    // ========================================================================
    // Constructor and Default Tests
    // ========================================================================

    #[test]
    fn test_default_constructor() {
        let embedder = GeminiEmbeddings::new();
        assert_eq!(embedder.model, "text-embedding-004");
        assert_eq!(embedder.batch_size, 100);
        assert!(embedder.task_type.is_none());
        assert!(embedder.output_dimensionality.is_none());
    }

    #[test]
    fn test_default_trait() {
        let embedder = GeminiEmbeddings::default();
        assert_eq!(embedder.model, DEFAULT_MODEL);
        assert_eq!(embedder.batch_size, 100);
    }

    #[test]
    fn test_default_no_api_key_from_env() {
        // Unless GEMINI_API_KEY is set, api_key should be None
        let embedder = GeminiEmbeddings::new();
        // This test just verifies the constructor works
        assert_eq!(embedder.model, "text-embedding-004");
    }

    #[test]
    fn test_default_no_title() {
        let embedder = GeminiEmbeddings::new();
        assert!(embedder.title.is_none());
    }

    #[test]
    fn test_default_no_rate_limiter() {
        let embedder = GeminiEmbeddings::new();
        assert!(embedder.rate_limiter.is_none());
    }

    // ========================================================================
    // Builder Pattern Tests - API Key
    // ========================================================================

    #[test]
    fn test_with_api_key() {
        let embedder = GeminiEmbeddings::new().with_api_key("test-key");
        assert_eq!(embedder.api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_with_api_key_string() {
        let key = String::from("my-api-key");
        let embedder = GeminiEmbeddings::new().with_api_key(key);
        assert_eq!(embedder.api_key, Some("my-api-key".to_string()));
    }

    #[test]
    fn test_with_api_key_empty() {
        let embedder = GeminiEmbeddings::new().with_api_key("");
        assert_eq!(embedder.api_key, Some(String::new()));
    }

    #[test]
    fn test_with_api_key_special_chars() {
        let embedder = GeminiEmbeddings::new().with_api_key("AI_key@123!#$%");
        assert_eq!(embedder.api_key, Some("AI_key@123!#$%".to_string()));
    }

    #[test]
    fn test_with_api_key_unicode() {
        let embedder = GeminiEmbeddings::new().with_api_key("キー🔑");
        assert_eq!(embedder.api_key, Some("キー🔑".to_string()));
    }

    // ========================================================================
    // Builder Pattern Tests - Model
    // ========================================================================

    #[test]
    fn test_with_model() {
        let embedder = GeminiEmbeddings::new().with_model("embedding-001");
        assert_eq!(embedder.model, "embedding-001");
    }

    #[test]
    fn test_with_model_string() {
        let model = String::from("custom-embedding-model");
        let embedder = GeminiEmbeddings::new().with_model(model);
        assert_eq!(embedder.model, "custom-embedding-model");
    }

    #[test]
    fn test_with_model_text_embedding_004() {
        let embedder = GeminiEmbeddings::new().with_model("text-embedding-004");
        assert_eq!(embedder.model, "text-embedding-004");
    }

    // ========================================================================
    // Builder Pattern Tests - Task Type
    // ========================================================================

    #[test]
    fn test_with_task_type() {
        let embedder = GeminiEmbeddings::new().with_task_type(TaskType::RetrievalQuery);
        assert_eq!(embedder.task_type, Some(TaskType::RetrievalQuery));
    }

    #[test]
    fn test_with_task_type_retrieval_document() {
        let embedder = GeminiEmbeddings::new().with_task_type(TaskType::RetrievalDocument);
        assert_eq!(embedder.task_type, Some(TaskType::RetrievalDocument));
    }

    #[test]
    fn test_with_task_type_semantic_similarity() {
        let embedder = GeminiEmbeddings::new().with_task_type(TaskType::SemanticSimilarity);
        assert_eq!(embedder.task_type, Some(TaskType::SemanticSimilarity));
    }

    #[test]
    fn test_with_task_type_classification() {
        let embedder = GeminiEmbeddings::new().with_task_type(TaskType::Classification);
        assert_eq!(embedder.task_type, Some(TaskType::Classification));
    }

    #[test]
    fn test_with_task_type_clustering() {
        let embedder = GeminiEmbeddings::new().with_task_type(TaskType::Clustering);
        assert_eq!(embedder.task_type, Some(TaskType::Clustering));
    }

    #[test]
    fn test_with_task_type_question_answering() {
        let embedder = GeminiEmbeddings::new().with_task_type(TaskType::QuestionAnswering);
        assert_eq!(embedder.task_type, Some(TaskType::QuestionAnswering));
    }

    #[test]
    fn test_with_task_type_fact_verification() {
        let embedder = GeminiEmbeddings::new().with_task_type(TaskType::FactVerification);
        assert_eq!(embedder.task_type, Some(TaskType::FactVerification));
    }

    #[test]
    fn test_with_task_type_unspecified() {
        let embedder = GeminiEmbeddings::new().with_task_type(TaskType::TaskTypeUnspecified);
        assert_eq!(embedder.task_type, Some(TaskType::TaskTypeUnspecified));
    }

    // ========================================================================
    // Builder Pattern Tests - Dimensions
    // ========================================================================

    #[test]
    fn test_with_dimensions() {
        let embedder = GeminiEmbeddings::new().with_dimensions(256);
        assert_eq!(embedder.output_dimensionality, Some(256));
    }

    #[test]
    fn test_with_dimensions_min() {
        let embedder = GeminiEmbeddings::new().with_dimensions(1);
        assert_eq!(embedder.output_dimensionality, Some(1));
    }

    #[test]
    fn test_with_dimensions_max() {
        let embedder = GeminiEmbeddings::new().with_dimensions(768);
        assert_eq!(embedder.output_dimensionality, Some(768));
    }

    #[test]
    fn test_with_dimensions_512() {
        let embedder = GeminiEmbeddings::new().with_dimensions(512);
        assert_eq!(embedder.output_dimensionality, Some(512));
    }

    #[test]
    fn test_with_dimensions_128() {
        let embedder = GeminiEmbeddings::new().with_dimensions(128);
        assert_eq!(embedder.output_dimensionality, Some(128));
    }

    // ========================================================================
    // Builder Pattern Tests - Title
    // ========================================================================

    #[test]
    fn test_with_title() {
        let embedder = GeminiEmbeddings::new().with_title("Test Title");
        assert_eq!(embedder.title, Some("Test Title".to_string()));
    }

    #[test]
    fn test_with_title_string() {
        let title = String::from("Document Title");
        let embedder = GeminiEmbeddings::new().with_title(title);
        assert_eq!(embedder.title, Some("Document Title".to_string()));
    }

    #[test]
    fn test_with_title_empty() {
        let embedder = GeminiEmbeddings::new().with_title("");
        assert_eq!(embedder.title, Some(String::new()));
    }

    #[test]
    fn test_with_title_unicode() {
        let embedder = GeminiEmbeddings::new().with_title("文档标题 📄");
        assert_eq!(embedder.title, Some("文档标题 📄".to_string()));
    }

    #[test]
    fn test_with_title_long() {
        let long_title = "A".repeat(1000);
        let embedder = GeminiEmbeddings::new().with_title(long_title.clone());
        assert_eq!(embedder.title, Some(long_title));
    }

    // ========================================================================
    // Builder Pattern Tests - Batch Size
    // ========================================================================

    #[test]
    fn test_with_batch_size() {
        let embedder = GeminiEmbeddings::new().with_batch_size(50);
        assert_eq!(embedder.batch_size, 50);
    }

    #[test]
    fn test_batch_size_clamped() {
        // Gemini API limit is 100
        let embedder = GeminiEmbeddings::new().with_batch_size(200);
        assert_eq!(embedder.batch_size, 100);
    }

    #[test]
    fn test_with_batch_size_one() {
        let embedder = GeminiEmbeddings::new().with_batch_size(1);
        assert_eq!(embedder.batch_size, 1);
    }

    #[test]
    fn test_with_batch_size_max() {
        let embedder = GeminiEmbeddings::new().with_batch_size(100);
        assert_eq!(embedder.batch_size, 100);
    }

    #[test]
    fn test_with_batch_size_very_large() {
        // Should be clamped to 100
        let embedder = GeminiEmbeddings::new().with_batch_size(10000);
        assert_eq!(embedder.batch_size, 100);
    }

    #[test]
    fn test_with_batch_size_zero() {
        let embedder = GeminiEmbeddings::new().with_batch_size(0);
        assert_eq!(embedder.batch_size, 0);
    }

    // ========================================================================
    // Builder Pattern Tests - Retry Policy
    // ========================================================================

    #[test]
    fn test_with_retry_policy() {
        let embedder = GeminiEmbeddings::new().with_retry_policy(RetryPolicy::exponential(5));
        assert_eq!(embedder.retry_policy.max_retries, 5);
    }

    #[test]
    fn test_with_retry_policy_no_retry() {
        let embedder = GeminiEmbeddings::new().with_retry_policy(RetryPolicy::no_retry());
        assert_eq!(embedder.retry_policy.max_retries, 0);
    }

    #[test]
    fn test_default_retry_policy() {
        let embedder = GeminiEmbeddings::new();
        // Default is exponential(3)
        assert_eq!(embedder.retry_policy.max_retries, 3);
    }

    // ========================================================================
    // Builder Pattern Tests - Rate Limiter
    // ========================================================================

    #[test]
    fn test_with_rate_limiter() {
        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));
        let embedder = GeminiEmbeddings::new().with_rate_limiter(rate_limiter);
        assert!(embedder.rate_limiter.is_some());
    }

    #[test]
    fn test_rate_limiter_default_none() {
        let embedder = GeminiEmbeddings::new();
        assert!(embedder.rate_limiter.is_none());
    }

    // ========================================================================
    // Builder Chaining Tests
    // ========================================================================

    #[test]
    fn test_builder_chaining() {
        let embedder = GeminiEmbeddings::new()
            .with_api_key("test-key")
            .with_model("text-embedding-004")
            .with_task_type(TaskType::SemanticSimilarity)
            .with_dimensions(512)
            .with_batch_size(50);

        assert_eq!(embedder.api_key, Some("test-key".to_string()));
        assert_eq!(embedder.model, "text-embedding-004");
        assert_eq!(embedder.task_type, Some(TaskType::SemanticSimilarity));
        assert_eq!(embedder.output_dimensionality, Some(512));
        assert_eq!(embedder.batch_size, 50);
    }

    #[test]
    fn test_full_builder_chain() {
        let rate_limiter = Arc::new(InMemoryRateLimiter::new(
            10.0,
            Duration::from_millis(100),
            20.0,
        ));

        let embedder = GeminiEmbeddings::new()
            .with_api_key("sk-test-key")
            .with_model("text-embedding-004")
            .with_task_type(TaskType::RetrievalDocument)
            .with_dimensions(256)
            .with_title("My Document")
            .with_batch_size(25)
            .with_retry_policy(RetryPolicy::exponential(5))
            .with_rate_limiter(rate_limiter);

        assert_eq!(embedder.api_key, Some("sk-test-key".to_string()));
        assert_eq!(embedder.model, "text-embedding-004");
        assert_eq!(embedder.task_type, Some(TaskType::RetrievalDocument));
        assert_eq!(embedder.output_dimensionality, Some(256));
        assert_eq!(embedder.title, Some("My Document".to_string()));
        assert_eq!(embedder.batch_size, 25);
        assert_eq!(embedder.retry_policy.max_retries, 5);
        assert!(embedder.rate_limiter.is_some());
    }

    #[test]
    fn test_builder_override() {
        let embedder = GeminiEmbeddings::new()
            .with_api_key("key1")
            .with_api_key("key2");

        assert_eq!(embedder.api_key, Some("key2".to_string()));
    }

    #[test]
    fn test_builder_override_model() {
        let embedder = GeminiEmbeddings::new()
            .with_model("model1")
            .with_model("model2");

        assert_eq!(embedder.model, "model2");
    }

    // ========================================================================
    // TaskType Serialization Tests
    // ========================================================================

    #[test]
    fn test_task_type_serialization() {
        let task_type = TaskType::RetrievalQuery;
        let serialized = serde_json::to_string(&task_type).unwrap();
        assert_eq!(serialized, "\"RETRIEVAL_QUERY\"");

        let task_type = TaskType::SemanticSimilarity;
        let serialized = serde_json::to_string(&task_type).unwrap();
        assert_eq!(serialized, "\"SEMANTIC_SIMILARITY\"");
    }

    #[test]
    fn test_task_type_retrieval_document_serialization() {
        let task_type = TaskType::RetrievalDocument;
        let serialized = serde_json::to_string(&task_type).unwrap();
        assert_eq!(serialized, "\"RETRIEVAL_DOCUMENT\"");
    }

    #[test]
    fn test_task_type_classification_serialization() {
        let task_type = TaskType::Classification;
        let serialized = serde_json::to_string(&task_type).unwrap();
        assert_eq!(serialized, "\"CLASSIFICATION\"");
    }

    #[test]
    fn test_task_type_clustering_serialization() {
        let task_type = TaskType::Clustering;
        let serialized = serde_json::to_string(&task_type).unwrap();
        assert_eq!(serialized, "\"CLUSTERING\"");
    }

    #[test]
    fn test_task_type_question_answering_serialization() {
        let task_type = TaskType::QuestionAnswering;
        let serialized = serde_json::to_string(&task_type).unwrap();
        assert_eq!(serialized, "\"QUESTION_ANSWERING\"");
    }

    #[test]
    fn test_task_type_fact_verification_serialization() {
        let task_type = TaskType::FactVerification;
        let serialized = serde_json::to_string(&task_type).unwrap();
        assert_eq!(serialized, "\"FACT_VERIFICATION\"");
    }

    #[test]
    fn test_task_type_unspecified_serialization() {
        let task_type = TaskType::TaskTypeUnspecified;
        let serialized = serde_json::to_string(&task_type).unwrap();
        assert_eq!(serialized, "\"TASK_TYPE_UNSPECIFIED\"");
    }

    #[test]
    fn test_all_task_types_serialize() {
        let task_types = vec![
            TaskType::TaskTypeUnspecified,
            TaskType::RetrievalQuery,
            TaskType::RetrievalDocument,
            TaskType::SemanticSimilarity,
            TaskType::Classification,
            TaskType::Clustering,
            TaskType::QuestionAnswering,
            TaskType::FactVerification,
        ];

        for task_type in task_types {
            let serialized = serde_json::to_string(&task_type).unwrap();
            assert!(serialized.starts_with('"'));
            assert!(serialized.ends_with('"'));
        }
    }

    // ========================================================================
    // TaskType Equality Tests
    // ========================================================================

    #[test]
    fn test_task_type_equality() {
        assert_eq!(TaskType::RetrievalQuery, TaskType::RetrievalQuery);
        assert_ne!(TaskType::RetrievalQuery, TaskType::RetrievalDocument);
    }

    #[test]
    fn test_task_type_clone() {
        let task_type = TaskType::SemanticSimilarity;
        let cloned = task_type;
        assert_eq!(task_type, cloned);
    }

    #[test]
    fn test_task_type_debug() {
        let task_type = TaskType::Classification;
        let debug_str = format!("{:?}", task_type);
        assert!(debug_str.contains("Classification"));
    }

    // ========================================================================
    // API Key Validation Tests
    // ========================================================================

    #[test]
    fn test_get_api_key_missing() {
        let embedder = GeminiEmbeddings {
            api_key: None,
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            task_type: None,
            output_dimensionality: None,
            title: None,
            batch_size: 100,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        };

        let result = embedder.get_api_key();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("GEMINI_API_KEY"));
    }

    #[test]
    fn test_get_api_key_present() {
        let embedder = GeminiEmbeddings::new().with_api_key("test-key");
        let result = embedder.get_api_key();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-key");
    }

    // ========================================================================
    // Request Serialization Tests
    // ========================================================================

    #[test]
    fn test_embed_content_request_serialization() {
        let request = EmbedContentRequest {
            content: Content {
                parts: vec![Part {
                    text: "Hello world".to_string(),
                }],
            },
            task_type: Some(TaskType::RetrievalQuery),
            title: None,
            output_dimensionality: Some(256),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Hello world"));
        assert!(json.contains("RETRIEVAL_QUERY"));
        assert!(json.contains("outputDimensionality"));
    }

    #[test]
    fn test_embed_content_request_without_optional_fields() {
        let request = EmbedContentRequest {
            content: Content {
                parts: vec![Part {
                    text: "Test".to_string(),
                }],
            },
            task_type: None,
            title: None,
            output_dimensionality: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Test"));
        // Optional fields should be skipped when None
        assert!(!json.contains("taskType"));
        assert!(!json.contains("outputDimensionality"));
    }

    #[test]
    fn test_batch_embed_contents_request_serialization() {
        let request = BatchEmbedContentsRequest {
            requests: vec![
                EmbedContentRequest {
                    content: Content {
                        parts: vec![Part {
                            text: "Doc 1".to_string(),
                        }],
                    },
                    task_type: None,
                    title: None,
                    output_dimensionality: None,
                },
                EmbedContentRequest {
                    content: Content {
                        parts: vec![Part {
                            text: "Doc 2".to_string(),
                        }],
                    },
                    task_type: None,
                    title: None,
                    output_dimensionality: None,
                },
            ],
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Doc 1"));
        assert!(json.contains("Doc 2"));
        assert!(json.contains("requests"));
    }

    // ========================================================================
    // Response Deserialization Tests
    // ========================================================================

    #[test]
    fn test_embed_content_response_deserialization() {
        let json = r#"{"embedding": {"values": [0.1, 0.2, 0.3, 0.4]}}"#;
        let response: EmbedContentResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.embedding.values.len(), 4);
        assert!((response.embedding.values[0] - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn test_batch_embed_contents_response_deserialization() {
        let json = r#"{"embeddings": [{"values": [0.1, 0.2]}, {"values": [0.3, 0.4]}]}"#;
        let response: BatchEmbedContentsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.embeddings.len(), 2);
        assert_eq!(response.embeddings[0].values.len(), 2);
        assert_eq!(response.embeddings[1].values.len(), 2);
    }

    #[test]
    fn test_content_embedding_deserialization() {
        let json = r#"{"values": [1.0, 2.0, 3.0]}"#;
        let embedding: ContentEmbedding = serde_json::from_str(json).unwrap();
        assert_eq!(embedding.values.len(), 3);
    }

    // ========================================================================
    // Content and Part Tests
    // ========================================================================

    #[test]
    fn test_content_serialization() {
        let content = Content {
            parts: vec![Part {
                text: "Hello".to_string(),
            }],
        };

        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("parts"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_part_serialization() {
        let part = Part {
            text: "Sample text".to_string(),
        };

        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("Sample text"));
    }

    #[test]
    fn test_content_deserialization() {
        let json = r#"{"parts": [{"text": "Test"}]}"#;
        let content: Content = serde_json::from_str(json).unwrap();
        assert_eq!(content.parts.len(), 1);
        assert_eq!(content.parts[0].text, "Test");
    }

    #[test]
    fn test_part_deserialization() {
        let json = r#"{"text": "Hello World"}"#;
        let part: Part = serde_json::from_str(json).unwrap();
        assert_eq!(part.text, "Hello World");
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[test]
    fn test_empty_text_part() {
        let part = Part {
            text: String::new(),
        };

        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains(r#""text":"""#));
    }

    #[test]
    fn test_unicode_text() {
        let part = Part {
            text: "你好世界 🌍 مرحبا".to_string(),
        };

        let json = serde_json::to_string(&part).unwrap();
        let deserialized: Part = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.text, "你好世界 🌍 مرحبا");
    }

    #[test]
    fn test_multiline_text() {
        let part = Part {
            text: "Line 1\nLine 2\nLine 3".to_string(),
        };

        let json = serde_json::to_string(&part).unwrap();
        let deserialized: Part = serde_json::from_str(&json).unwrap();
        assert!(deserialized.text.contains('\n'));
    }

    #[test]
    fn test_special_characters_text() {
        let part = Part {
            text: r#"Special chars: "quotes" \backslash\ /forward/"#.to_string(),
        };

        let json = serde_json::to_string(&part).unwrap();
        let deserialized: Part = serde_json::from_str(&json).unwrap();
        assert!(deserialized.text.contains("quotes"));
    }

    #[test]
    fn test_large_embedding_values() {
        // ContentEmbedding only implements Deserialize (not Serialize) - test deserialization
        let values: Vec<f32> = (0..768).map(|i| i as f32 / 1000.0).collect();

        // Build a JSON string manually for deserialization test
        let json = format!(
            r#"{{"values": [{}]}}"#,
            values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        let deserialized: ContentEmbedding = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.values.len(), 768);
    }

    // ========================================================================
    // Constants Tests
    // ========================================================================

    #[test]
    fn test_default_model_constant() {
        assert_eq!(DEFAULT_MODEL, "text-embedding-004");
    }

    #[test]
    fn test_api_base_constant() {
        assert_eq!(API_BASE, "https://generativelanguage.googleapis.com/v1beta");
    }

    // ========================================================================
    // Multiple Parts Tests
    // ========================================================================

    #[test]
    fn test_content_multiple_parts() {
        let content = Content {
            parts: vec![
                Part {
                    text: "Part 1".to_string(),
                },
                Part {
                    text: "Part 2".to_string(),
                },
                Part {
                    text: "Part 3".to_string(),
                },
            ],
        };

        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("Part 1"));
        assert!(json.contains("Part 2"));
        assert!(json.contains("Part 3"));
    }

    #[test]
    fn test_batch_request_many_items() {
        let requests: Vec<EmbedContentRequest> = (0..50)
            .map(|i| EmbedContentRequest {
                content: Content {
                    parts: vec![Part {
                        text: format!("Document {}", i),
                    }],
                },
                task_type: None,
                title: None,
                output_dimensionality: None,
            })
            .collect();

        let batch = BatchEmbedContentsRequest { requests };
        let json = serde_json::to_string(&batch).unwrap();

        assert!(json.contains("Document 0"));
        assert!(json.contains("Document 49"));
    }

    // ========================================================================
    // Task Type with Title Tests
    // ========================================================================

    #[test]
    fn test_retrieval_document_with_title() {
        let embedder = GeminiEmbeddings::new()
            .with_task_type(TaskType::RetrievalDocument)
            .with_title("Product Manual");

        assert_eq!(embedder.task_type, Some(TaskType::RetrievalDocument));
        assert_eq!(embedder.title, Some("Product Manual".to_string()));
    }

    #[test]
    fn test_title_without_retrieval_document() {
        // Title can be set without RetrievalDocument task type
        let embedder = GeminiEmbeddings::new()
            .with_task_type(TaskType::SemanticSimilarity)
            .with_title("Some Title");

        assert_eq!(embedder.task_type, Some(TaskType::SemanticSimilarity));
        assert_eq!(embedder.title, Some("Some Title".to_string()));
    }

    // ========================================================================
    // Request with All Options Tests
    // ========================================================================

    #[test]
    fn test_embed_request_all_options() {
        let request = EmbedContentRequest {
            content: Content {
                parts: vec![Part {
                    text: "Full featured request".to_string(),
                }],
            },
            task_type: Some(TaskType::RetrievalDocument),
            title: Some("My Title".to_string()),
            output_dimensionality: Some(512),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Full featured request"));
        assert!(json.contains("RETRIEVAL_DOCUMENT"));
        assert!(json.contains("My Title"));
        assert!(json.contains("512"));
    }

    // ========================================================================
    // Debug Implementation Tests
    // ========================================================================

    #[test]
    fn test_task_type_debug_all_variants() {
        let variants = vec![
            (TaskType::TaskTypeUnspecified, "TaskTypeUnspecified"),
            (TaskType::RetrievalQuery, "RetrievalQuery"),
            (TaskType::RetrievalDocument, "RetrievalDocument"),
            (TaskType::SemanticSimilarity, "SemanticSimilarity"),
            (TaskType::Classification, "Classification"),
            (TaskType::Clustering, "Clustering"),
            (TaskType::QuestionAnswering, "QuestionAnswering"),
            (TaskType::FactVerification, "FactVerification"),
        ];

        for (task_type, expected) in variants {
            let debug_str = format!("{:?}", task_type);
            assert!(debug_str.contains(expected));
        }
    }

    #[test]
    fn test_embed_content_request_debug() {
        let request = EmbedContentRequest {
            content: Content {
                parts: vec![Part {
                    text: "Test".to_string(),
                }],
            },
            task_type: None,
            title: None,
            output_dimensionality: None,
        };

        let debug_str = format!("{:?}", request);
        assert!(debug_str.contains("EmbedContentRequest"));
    }
}

/// Standard conformance tests for embeddings
///
/// These tests verify that GeminiEmbeddings behaves consistently with other
/// Embeddings implementations across the DashFlow ecosystem.
#[cfg(test)]
mod standard_tests {
    use super::*;
    use dashflow_standard_tests::embeddings_tests::*;

    /// Helper function to create a test embeddings model
    fn create_test_embeddings() -> GeminiEmbeddings {
        GeminiEmbeddings::new().with_task_type(TaskType::SemanticSimilarity)
    }

    /// Standard Test 1: Embed single document
    #[tokio::test]
    #[ignore = "requires GEMINI_API_KEY"]
    async fn test_embed_query_standard() {
        test_embed_query(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 2: Embed multiple documents
    #[tokio::test]
    #[ignore = "requires GEMINI_API_KEY"]
    async fn test_embed_documents_standard() {
        test_embed_documents(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 3: Empty input handling
    #[tokio::test]
    #[ignore = "requires GEMINI_API_KEY"]
    async fn test_empty_input_standard() {
        test_empty_input(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 4: Dimension consistency
    #[tokio::test]
    #[ignore = "requires GEMINI_API_KEY"]
    async fn test_dimension_consistency_standard() {
        test_dimension_consistency(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 5: Semantic similarity
    #[tokio::test]
    #[ignore = "requires GEMINI_API_KEY"]
    async fn test_semantic_similarity_standard() {
        test_semantic_similarity(Arc::new(create_test_embeddings())).await;
    }
}
