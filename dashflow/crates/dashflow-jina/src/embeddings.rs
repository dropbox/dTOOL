//! Jina AI embeddings implementation.
//!
//! This module provides embeddings using Jina AI's embedding models, including:
//! - jina-embeddings-v3: Multilingual model supporting 89 languages
//! - jina-embeddings-v2-base-en: English-optimized model (8192 tokens)
//! - jina-embeddings-v2-small-en: Lightweight English model
//! - jina-clip-v2: Multimodal text and image embeddings
//!
//! # Example
//!
//! ```rust
//! use dashflow_jina::JinaEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let embedder = JinaEmbeddings::new()
//!     .with_api_key(std::env::var("JINA_API_KEY")?);
//!
//! // Embed a single query
//! let query_vector = embedder.embed_query("What is semantic search?").await?;
//! assert!(!query_vector.is_empty());
//!
//! // Embed multiple documents
//! let docs = vec![
//!     "Semantic search uses meaning.".to_string(),
//!     "Keyword search matches exact terms.".to_string(),
//! ];
//! let doc_vectors = embedder.embed_documents(&docs).await?;
//! assert_eq!(doc_vectors.len(), 2);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use dashflow::core::{
    config_loader::env_vars::{env_string, JINA_API_KEY as JINA_API_KEY_VAR},
    embeddings::Embeddings,
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
    Error as DashFlowError,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const DEFAULT_MODEL: &str = "jina-embeddings-v3";
const API_BASE: &str = "https://api.jina.ai/v1";

/// Task type for embeddings, which optimizes the embedding for specific use cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// For encoding texts in retrieval tasks (queries)
    RetrievalQuery,
    /// For encoding documents in retrieval tasks
    RetrievalDocument,
    /// For text matching tasks (finding similar texts)
    TextMatching,
    /// For classification tasks
    Classification,
    /// For separation/clustering tasks
    Separation,
}

/// Embedding output type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingType {
    /// Standard float embeddings
    Float,
    /// Base64 encoded embeddings
    Base64,
    /// Binary embeddings
    Binary,
}

/// Jina AI embedding model integration.
///
/// Supports the following models:
/// - `jina-embeddings-v3`: Multilingual model supporting 89 languages (1024 dimensions)
/// - `jina-embeddings-v2-base-en`: English-optimized model (768 dimensions)
/// - `jina-embeddings-v2-small-en`: Lightweight English model (512 dimensions)
/// - `jina-clip-v2`: Multimodal text and image embeddings
///
/// # Configuration
///
/// The API key can be set via:
/// - Constructor: `JinaEmbeddings::new().with_api_key("...")`
/// - Environment: `JINA_API_KEY`
///
/// # Task Types
///
/// Jina embeddings support different task types that optimize the embedding:
/// - `RetrievalQuery`: For search queries
/// - `RetrievalDocument`: For documents to be searched
/// - `TextMatching`: For finding similar texts
/// - `Classification`: For classification tasks
/// - `Separation`: For clustering tasks
///
/// # Dimensions
///
/// Use `with_dimensions()` to reduce output dimensionality while maintaining
/// semantic information (Matryoshka representation learning).
pub struct JinaEmbeddings {
    /// API key for authentication
    api_key: Option<String>,
    /// Model name (e.g., "jina-embeddings-v3")
    model: String,
    /// HTTP client
    client: Client,
    /// Task type for embedding optimization
    task: Option<TaskType>,
    /// Optional: The number of dimensions for the output embeddings
    dimensions: Option<u32>,
    /// Whether to normalize embeddings (L2 normalization)
    normalized: bool,
    /// Embedding output type
    embedding_type: EmbeddingType,
    /// Maximum number of texts to embed in a single batch request
    batch_size: usize,
    /// Retry policy for API calls
    retry_policy: RetryPolicy,
    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

impl JinaEmbeddings {
    /// Create a new Jina AI embeddings instance with default settings.
    ///
    /// Defaults:
    /// - Model: `jina-embeddings-v3`
    /// - Batch size: 128
    /// - Normalized: true
    /// - Embedding type: Float
    /// - API key: from `JINA_API_KEY` environment variable
    #[must_use]
    pub fn new() -> Self {
        Self {
            api_key: env_string(JINA_API_KEY_VAR),
            model: DEFAULT_MODEL.to_string(),
            client: Client::new(),
            task: None,
            dimensions: None,
            normalized: true,
            embedding_type: EmbeddingType::Float,
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
    /// # use dashflow_jina::JinaEmbeddings;
    /// let embedder = JinaEmbeddings::new()
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
    /// - `jina-embeddings-v3`: Multilingual model supporting 89 languages
    /// - `jina-embeddings-v2-base-en`: English-optimized model
    /// - `jina-embeddings-v2-small-en`: Lightweight English model
    /// - `jina-clip-v2`: Multimodal text and image embeddings
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_jina::JinaEmbeddings;
    /// let embedder = JinaEmbeddings::new()
    ///     .with_model("jina-embeddings-v2-base-en");
    /// ```
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the task type for embedding optimization.
    ///
    /// - `RetrievalQuery`: For search queries
    /// - `RetrievalDocument`: For documents to be searched
    /// - `TextMatching`: For finding similar texts
    /// - `Classification`: For classification tasks
    /// - `Separation`: For clustering tasks
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_jina::{JinaEmbeddings, TaskType};
    /// let embedder = JinaEmbeddings::new()
    ///     .with_task(TaskType::RetrievalQuery);
    /// ```
    #[must_use]
    pub fn with_task(mut self, task: TaskType) -> Self {
        self.task = Some(task);
        self
    }

    /// Set the output dimensionality for embeddings.
    ///
    /// Jina embeddings use Matryoshka representation learning, which allows
    /// reducing dimensionality while preserving semantic information.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_jina::JinaEmbeddings;
    /// let embedder = JinaEmbeddings::new()
    ///     .with_dimensions(512);
    /// ```
    #[must_use]
    pub fn with_dimensions(mut self, dimensions: u32) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    /// Set whether to normalize embeddings (L2 normalization).
    ///
    /// Default is true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_jina::JinaEmbeddings;
    /// let embedder = JinaEmbeddings::new()
    ///     .with_normalized(false);
    /// ```
    #[must_use]
    pub fn with_normalized(mut self, normalized: bool) -> Self {
        self.normalized = normalized;
        self
    }

    /// Set the embedding output type.
    ///
    /// - `Float`: Standard float embeddings (default)
    /// - `Base64`: Base64 encoded embeddings
    /// - `Binary`: Binary embeddings
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_jina::{JinaEmbeddings, EmbeddingType};
    /// let embedder = JinaEmbeddings::new()
    ///     .with_embedding_type(EmbeddingType::Base64);
    /// ```
    #[must_use]
    pub fn with_embedding_type(mut self, embedding_type: EmbeddingType) -> Self {
        self.embedding_type = embedding_type;
        self
    }

    /// Set the batch size for batch embedding requests.
    ///
    /// Jina AI supports up to 2048 texts per request, but default is 128
    /// for better performance.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_jina::JinaEmbeddings;
    /// let embedder = JinaEmbeddings::new()
    ///     .with_batch_size(256);
    /// ```
    #[must_use]
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.min(2048); // Jina API limit
        self
    }

    /// Set the retry policy for API calls.
    ///
    /// Default is exponential backoff with 3 retries.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_jina::JinaEmbeddings;
    /// # use dashflow::core::retry::RetryPolicy;
    /// let embedder = JinaEmbeddings::new()
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
    /// # use dashflow_jina::JinaEmbeddings;
    /// # use dashflow::core::rate_limiters::InMemoryRateLimiter;
    /// # use std::time::Duration;
    /// # use std::sync::Arc;
    /// let rate_limiter = InMemoryRateLimiter::new(
    ///     10.0,  // 10 requests per second
    ///     Duration::from_millis(100),
    ///     20.0,  // Max burst
    /// );
    ///
    /// let embedder = JinaEmbeddings::new()
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
                "JINA_API_KEY not set. Set it via environment variable or with_api_key()"
                    .to_string(),
            )
        })
    }

    /// Embed texts using the Jina AI API.
    async fn embed_texts(
        &self,
        texts: &[String],
        task: Option<TaskType>,
    ) -> Result<Vec<Vec<f32>>, DashFlowError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let api_key = self.get_api_key()?;
        let url = format!("{}/embeddings", API_BASE);

        let request = EmbedRequest {
            model: self.model.clone(),
            input: texts.to_vec(),
            task: task.or(self.task),
            dimensions: self.dimensions,
            normalized: self.normalized,
            embedding_type: self.embedding_type,
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
                .map_err(|e| DashFlowError::api(format!("Jina API request failed: {e}")))?
                .error_for_status()
                .map_err(|e| DashFlowError::api(format!("Jina API error: {e}")))
        })
        .await?;

        let embed_response: EmbedResponse = response
            .json()
            .await
            .map_err(|e| DashFlowError::api(format!("Failed to parse Jina response: {e}")))?;

        Ok(embed_response
            .data
            .into_iter()
            .map(|e| e.embedding)
            .collect())
    }
}

impl Default for JinaEmbeddings {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Embeddings for JinaEmbeddings {
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, DashFlowError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        // Process in batches
        for chunk in texts.chunks(self.batch_size) {
            let batch_embeddings = self
                .embed_texts(chunk, Some(TaskType::RetrievalDocument))
                .await?;
            all_embeddings.extend(batch_embeddings);
        }

        Ok(all_embeddings)
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, DashFlowError> {
        let texts = vec![text.to_string()];
        let mut embeddings = self
            .embed_texts(&texts, Some(TaskType::RetrievalQuery))
            .await?;

        embeddings
            .pop()
            .ok_or_else(|| DashFlowError::api("No embedding returned from Jina AI"))
    }
}

// Request/Response types for Jina API

#[derive(Debug, Serialize)]
struct EmbedRequest {
    model: String,
    input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    task: Option<TaskType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<u32>,
    normalized: bool,
    embedding_type: EmbeddingType,
}

/// Jina API response struct. Fields marked dead_code are present in API response
/// and required for serde deserialization, but not currently used.
#[derive(Debug, Deserialize)]
struct EmbedResponse {
    data: Vec<EmbeddingData>,
    #[allow(dead_code)] // Deserialize: Jina model name - reserved for model version logging
    model: String,
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
    #[allow(dead_code)] // Deserialize: Input tokens - reserved for cost breakdown
    prompt_tokens: Option<u32>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to serialize env var access across tests.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // ==================== Constructor Tests ====================

    #[test]
    fn test_default_constructor() {
        let embedder = JinaEmbeddings::new();
        assert_eq!(embedder.model, "jina-embeddings-v3");
        assert_eq!(embedder.batch_size, 128);
        assert!(embedder.normalized);
        assert!(embedder.task.is_none());
        assert!(embedder.dimensions.is_none());
        assert_eq!(embedder.embedding_type, EmbeddingType::Float);
    }

    #[test]
    fn test_default_trait() {
        let embedder = JinaEmbeddings::default();
        assert_eq!(embedder.model, "jina-embeddings-v3");
        assert_eq!(embedder.batch_size, 128);
        assert!(embedder.normalized);
    }

    #[test]
    fn test_new_reads_env_api_key() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("JINA_API_KEY", "env-test-key-xyz");
        let embedder = JinaEmbeddings::new();
        assert_eq!(embedder.api_key, Some("env-test-key-xyz".to_string()));
        std::env::remove_var("JINA_API_KEY");
    }

    #[test]
    fn test_new_without_env_key() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("JINA_API_KEY");
        let embedder = JinaEmbeddings::new();
        assert!(embedder.api_key.is_none());
    }

    // ==================== Builder Method Tests ====================

    #[test]
    fn test_with_model() {
        let embedder = JinaEmbeddings::new().with_model("jina-embeddings-v2-base-en");
        assert_eq!(embedder.model, "jina-embeddings-v2-base-en");
    }

    #[test]
    fn test_with_model_v2_small() {
        let embedder = JinaEmbeddings::new().with_model("jina-embeddings-v2-small-en");
        assert_eq!(embedder.model, "jina-embeddings-v2-small-en");
    }

    #[test]
    fn test_with_model_clip() {
        let embedder = JinaEmbeddings::new().with_model("jina-clip-v2");
        assert_eq!(embedder.model, "jina-clip-v2");
    }

    #[test]
    fn test_with_model_custom_string() {
        let embedder = JinaEmbeddings::new().with_model(String::from("custom-model"));
        assert_eq!(embedder.model, "custom-model");
    }

    #[test]
    fn test_with_api_key() {
        let embedder = JinaEmbeddings::new().with_api_key("test-key");
        assert_eq!(embedder.api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_with_api_key_overrides_env() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("JINA_API_KEY", "env-key");
        let embedder = JinaEmbeddings::new().with_api_key("explicit-key");
        assert_eq!(embedder.api_key, Some("explicit-key".to_string()));
        std::env::remove_var("JINA_API_KEY");
    }

    #[test]
    fn test_with_api_key_string_type() {
        let embedder = JinaEmbeddings::new().with_api_key(String::from("string-key"));
        assert_eq!(embedder.api_key, Some("string-key".to_string()));
    }

    #[test]
    fn test_with_task() {
        let embedder = JinaEmbeddings::new().with_task(TaskType::RetrievalQuery);
        assert_eq!(embedder.task, Some(TaskType::RetrievalQuery));
    }

    #[test]
    fn test_with_task_retrieval_document() {
        let embedder = JinaEmbeddings::new().with_task(TaskType::RetrievalDocument);
        assert_eq!(embedder.task, Some(TaskType::RetrievalDocument));
    }

    #[test]
    fn test_with_task_text_matching() {
        let embedder = JinaEmbeddings::new().with_task(TaskType::TextMatching);
        assert_eq!(embedder.task, Some(TaskType::TextMatching));
    }

    #[test]
    fn test_with_task_classification() {
        let embedder = JinaEmbeddings::new().with_task(TaskType::Classification);
        assert_eq!(embedder.task, Some(TaskType::Classification));
    }

    #[test]
    fn test_with_task_separation() {
        let embedder = JinaEmbeddings::new().with_task(TaskType::Separation);
        assert_eq!(embedder.task, Some(TaskType::Separation));
    }

    #[test]
    fn test_with_dimensions() {
        let embedder = JinaEmbeddings::new().with_dimensions(512);
        assert_eq!(embedder.dimensions, Some(512));
    }

    #[test]
    fn test_with_dimensions_256() {
        let embedder = JinaEmbeddings::new().with_dimensions(256);
        assert_eq!(embedder.dimensions, Some(256));
    }

    #[test]
    fn test_with_dimensions_1024() {
        let embedder = JinaEmbeddings::new().with_dimensions(1024);
        assert_eq!(embedder.dimensions, Some(1024));
    }

    #[test]
    fn test_with_dimensions_small() {
        let embedder = JinaEmbeddings::new().with_dimensions(64);
        assert_eq!(embedder.dimensions, Some(64));
    }

    #[test]
    fn test_with_normalized() {
        let embedder = JinaEmbeddings::new().with_normalized(false);
        assert!(!embedder.normalized);
    }

    #[test]
    fn test_with_normalized_true() {
        let embedder = JinaEmbeddings::new().with_normalized(true);
        assert!(embedder.normalized);
    }

    #[test]
    fn test_with_embedding_type() {
        let embedder = JinaEmbeddings::new().with_embedding_type(EmbeddingType::Binary);
        assert_eq!(embedder.embedding_type, EmbeddingType::Binary);
    }

    #[test]
    fn test_with_embedding_type_float() {
        let embedder = JinaEmbeddings::new().with_embedding_type(EmbeddingType::Float);
        assert_eq!(embedder.embedding_type, EmbeddingType::Float);
    }

    #[test]
    fn test_with_embedding_type_base64() {
        let embedder = JinaEmbeddings::new().with_embedding_type(EmbeddingType::Base64);
        assert_eq!(embedder.embedding_type, EmbeddingType::Base64);
    }

    #[test]
    fn test_with_batch_size() {
        let embedder = JinaEmbeddings::new().with_batch_size(256);
        assert_eq!(embedder.batch_size, 256);
    }

    #[test]
    fn test_with_batch_size_small() {
        let embedder = JinaEmbeddings::new().with_batch_size(16);
        assert_eq!(embedder.batch_size, 16);
    }

    #[test]
    fn test_with_batch_size_at_limit() {
        let embedder = JinaEmbeddings::new().with_batch_size(2048);
        assert_eq!(embedder.batch_size, 2048);
    }

    #[test]
    fn test_batch_size_clamped() {
        // Jina API limit is 2048
        let embedder = JinaEmbeddings::new().with_batch_size(5000);
        assert_eq!(embedder.batch_size, 2048);
    }

    #[test]
    fn test_batch_size_clamped_large() {
        let embedder = JinaEmbeddings::new().with_batch_size(10000);
        assert_eq!(embedder.batch_size, 2048);
    }

    #[test]
    fn test_with_retry_policy() {
        let policy = RetryPolicy::exponential(5);
        let embedder = JinaEmbeddings::new().with_retry_policy(policy);
        // Verify policy is set (can't directly compare, but can verify no panic)
        assert_eq!(embedder.model, "jina-embeddings-v3");
    }

    #[test]
    fn test_with_retry_policy_fixed() {
        let policy = RetryPolicy::fixed(3, 100);
        let embedder = JinaEmbeddings::new().with_retry_policy(policy);
        assert_eq!(embedder.model, "jina-embeddings-v3");
    }

    // ==================== Builder Chaining Tests ====================

    #[test]
    fn test_builder_chaining() {
        let embedder = JinaEmbeddings::new()
            .with_api_key("test-key")
            .with_model("jina-embeddings-v2-small-en")
            .with_task(TaskType::Classification)
            .with_dimensions(256)
            .with_batch_size(64)
            .with_normalized(false)
            .with_embedding_type(EmbeddingType::Base64);

        assert_eq!(embedder.api_key, Some("test-key".to_string()));
        assert_eq!(embedder.model, "jina-embeddings-v2-small-en");
        assert_eq!(embedder.task, Some(TaskType::Classification));
        assert_eq!(embedder.dimensions, Some(256));
        assert_eq!(embedder.batch_size, 64);
        assert!(!embedder.normalized);
        assert_eq!(embedder.embedding_type, EmbeddingType::Base64);
    }

    #[test]
    fn test_builder_chaining_all_options() {
        let embedder = JinaEmbeddings::new()
            .with_api_key("my-key")
            .with_model("jina-clip-v2")
            .with_task(TaskType::RetrievalQuery)
            .with_dimensions(768)
            .with_batch_size(512)
            .with_normalized(true)
            .with_embedding_type(EmbeddingType::Float)
            .with_retry_policy(RetryPolicy::exponential(2));

        assert_eq!(embedder.api_key, Some("my-key".to_string()));
        assert_eq!(embedder.model, "jina-clip-v2");
        assert_eq!(embedder.task, Some(TaskType::RetrievalQuery));
        assert_eq!(embedder.dimensions, Some(768));
        assert_eq!(embedder.batch_size, 512);
        assert!(embedder.normalized);
        assert_eq!(embedder.embedding_type, EmbeddingType::Float);
    }

    #[test]
    fn test_builder_chaining_partial() {
        let embedder = JinaEmbeddings::new()
            .with_model("jina-embeddings-v2-base-en")
            .with_dimensions(512);

        assert!(embedder.api_key.is_none() || embedder.api_key.is_some()); // Depends on env
        assert_eq!(embedder.model, "jina-embeddings-v2-base-en");
        assert!(embedder.task.is_none());
        assert_eq!(embedder.dimensions, Some(512));
        assert_eq!(embedder.batch_size, 128); // Default
        assert!(embedder.normalized); // Default
        assert_eq!(embedder.embedding_type, EmbeddingType::Float); // Default
    }

    // ==================== Serialization Tests ====================

    #[test]
    fn test_task_type_serialization() {
        let task = TaskType::RetrievalQuery;
        let serialized = serde_json::to_string(&task).unwrap();
        assert_eq!(serialized, "\"retrieval_query\"");

        let task = TaskType::RetrievalDocument;
        let serialized = serde_json::to_string(&task).unwrap();
        assert_eq!(serialized, "\"retrieval_document\"");

        let task = TaskType::TextMatching;
        let serialized = serde_json::to_string(&task).unwrap();
        assert_eq!(serialized, "\"text_matching\"");

        let task = TaskType::Classification;
        let serialized = serde_json::to_string(&task).unwrap();
        assert_eq!(serialized, "\"classification\"");

        let task = TaskType::Separation;
        let serialized = serde_json::to_string(&task).unwrap();
        assert_eq!(serialized, "\"separation\"");
    }

    #[test]
    fn test_embedding_type_serialization() {
        let emb_type = EmbeddingType::Float;
        let serialized = serde_json::to_string(&emb_type).unwrap();
        assert_eq!(serialized, "\"float\"");

        let emb_type = EmbeddingType::Base64;
        let serialized = serde_json::to_string(&emb_type).unwrap();
        assert_eq!(serialized, "\"base64\"");

        let emb_type = EmbeddingType::Binary;
        let serialized = serde_json::to_string(&emb_type).unwrap();
        assert_eq!(serialized, "\"binary\"");
    }

    #[test]
    fn test_embed_request_serialization() {
        let request = EmbedRequest {
            model: "jina-embeddings-v3".to_string(),
            input: vec!["test text".to_string()],
            task: Some(TaskType::RetrievalQuery),
            dimensions: Some(512),
            normalized: true,
            embedding_type: EmbeddingType::Float,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"jina-embeddings-v3\""));
        assert!(json.contains("\"input\":[\"test text\"]"));
        assert!(json.contains("\"task\":\"retrieval_query\""));
        assert!(json.contains("\"dimensions\":512"));
        assert!(json.contains("\"normalized\":true"));
        assert!(json.contains("\"embedding_type\":\"float\""));
    }

    #[test]
    fn test_embed_request_without_optional_fields() {
        let request = EmbedRequest {
            model: "jina-embeddings-v3".to_string(),
            input: vec!["test".to_string()],
            task: None,
            dimensions: None,
            normalized: false,
            embedding_type: EmbeddingType::Binary,
        };
        let json = serde_json::to_string(&request).unwrap();
        // task and dimensions should be skipped when None
        assert!(!json.contains("\"task\""));
        assert!(!json.contains("\"dimensions\""));
        assert!(json.contains("\"normalized\":false"));
    }

    #[test]
    fn test_embed_request_multiple_inputs() {
        let request = EmbedRequest {
            model: "jina-embeddings-v3".to_string(),
            input: vec![
                "first text".to_string(),
                "second text".to_string(),
                "third text".to_string(),
            ],
            task: None,
            dimensions: None,
            normalized: true,
            embedding_type: EmbeddingType::Float,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"first text\""));
        assert!(json.contains("\"second text\""));
        assert!(json.contains("\"third text\""));
    }

    #[test]
    fn test_embed_response_deserialization() {
        let json = r#"{
            "data": [
                {"embedding": [0.1, 0.2, 0.3], "index": 0}
            ],
            "model": "jina-embeddings-v3",
            "usage": {"total_tokens": 10, "prompt_tokens": 10}
        }"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(response.data[0].index, 0);
        assert_eq!(response.model, "jina-embeddings-v3");
        assert_eq!(response.usage.total_tokens, 10);
    }

    #[test]
    fn test_embed_response_multiple_embeddings() {
        let json = r#"{
            "data": [
                {"embedding": [0.1, 0.2], "index": 0},
                {"embedding": [0.3, 0.4], "index": 1},
                {"embedding": [0.5, 0.6], "index": 2}
            ],
            "model": "jina-embeddings-v3",
            "usage": {"total_tokens": 30}
        }"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 3);
        assert_eq!(response.data[0].embedding, vec![0.1, 0.2]);
        assert_eq!(response.data[1].embedding, vec![0.3, 0.4]);
        assert_eq!(response.data[2].embedding, vec![0.5, 0.6]);
    }

    #[test]
    fn test_embed_response_without_prompt_tokens() {
        let json = r#"{
            "data": [{"embedding": [1.0], "index": 0}],
            "model": "test",
            "usage": {"total_tokens": 5}
        }"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();
        assert!(response.usage.prompt_tokens.is_none());
    }

    // ==================== API Key Error Tests ====================

    #[test]
    fn test_get_api_key_when_set() {
        let embedder = JinaEmbeddings::new().with_api_key("my-api-key");
        let result = embedder.get_api_key();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "my-api-key");
    }

    #[test]
    fn test_get_api_key_when_missing() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("JINA_API_KEY");
        let embedder = JinaEmbeddings::new();
        let result = embedder.get_api_key();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("JINA_API_KEY not set"));
    }

    // ==================== Empty Input Tests ====================

    #[tokio::test]
    async fn test_embed_texts_empty_input() {
        let embedder = JinaEmbeddings::new().with_api_key("test-key");
        let result = embedder.embed_texts(&[], None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_embed_documents_empty_input() {
        let embedder = JinaEmbeddings::new().with_api_key("test-key");
        let result = embedder._embed_documents(&[]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    // ==================== TaskType Enum Tests ====================

    #[test]
    fn test_task_type_debug() {
        let task = TaskType::RetrievalQuery;
        let debug = format!("{:?}", task);
        assert_eq!(debug, "RetrievalQuery");
    }

    #[test]
    fn test_task_type_clone() {
        let task = TaskType::Classification;
        let cloned = task.clone();
        assert_eq!(task, cloned);
    }

    #[test]
    fn test_task_type_copy() {
        let task = TaskType::Separation;
        let copied = task;
        assert_eq!(copied, TaskType::Separation);
    }

    #[test]
    fn test_task_type_eq() {
        assert_eq!(TaskType::RetrievalQuery, TaskType::RetrievalQuery);
        assert_ne!(TaskType::RetrievalQuery, TaskType::RetrievalDocument);
    }

    // ==================== EmbeddingType Enum Tests ====================

    #[test]
    fn test_embedding_type_debug() {
        let emb_type = EmbeddingType::Float;
        let debug = format!("{:?}", emb_type);
        assert_eq!(debug, "Float");
    }

    #[test]
    fn test_embedding_type_clone() {
        let emb_type = EmbeddingType::Base64;
        let cloned = emb_type.clone();
        assert_eq!(emb_type, cloned);
    }

    #[test]
    fn test_embedding_type_copy() {
        let emb_type = EmbeddingType::Binary;
        let copied = emb_type;
        assert_eq!(copied, EmbeddingType::Binary);
    }

    #[test]
    fn test_embedding_type_eq() {
        assert_eq!(EmbeddingType::Float, EmbeddingType::Float);
        assert_ne!(EmbeddingType::Float, EmbeddingType::Binary);
    }

    // ==================== Constants Tests ====================

    #[test]
    fn test_default_model_constant() {
        assert_eq!(DEFAULT_MODEL, "jina-embeddings-v3");
    }

    #[test]
    fn test_api_base_constant() {
        assert_eq!(API_BASE, "https://api.jina.ai/v1");
    }
}

/// Standard conformance tests for embeddings
///
/// These tests verify that JinaEmbeddings behaves consistently with other
/// Embeddings implementations across the DashFlow ecosystem.
#[cfg(test)]
#[allow(clippy::expect_used)]
mod standard_tests {
    use super::*;
    use dashflow_standard_tests::embeddings_tests::*;

    /// Helper function to create a test embeddings model
    fn create_test_embeddings() -> JinaEmbeddings {
        JinaEmbeddings::new()
    }

    /// Standard Test 1: Embed single document
    #[tokio::test]
    #[ignore = "requires JINA_API_KEY"]
    async fn test_embed_query_standard() {
        let _api_key = std::env::var("JINA_API_KEY").expect("JINA_API_KEY must be set");
        test_embed_query(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 2: Embed multiple documents
    #[tokio::test]
    #[ignore = "requires JINA_API_KEY"]
    async fn test_embed_documents_standard() {
        let _api_key = std::env::var("JINA_API_KEY").expect("JINA_API_KEY must be set");
        test_embed_documents(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 3: Empty input handling
    #[tokio::test]
    #[ignore = "requires JINA_API_KEY"]
    async fn test_empty_input_standard() {
        let _api_key = std::env::var("JINA_API_KEY").expect("JINA_API_KEY must be set");
        test_empty_input(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 4: Dimension consistency
    #[tokio::test]
    #[ignore = "requires JINA_API_KEY"]
    async fn test_dimension_consistency_standard() {
        let _api_key = std::env::var("JINA_API_KEY").expect("JINA_API_KEY must be set");
        test_dimension_consistency(Arc::new(create_test_embeddings())).await;
    }

    /// Standard Test 5: Semantic similarity
    #[tokio::test]
    #[ignore = "requires JINA_API_KEY"]
    async fn test_semantic_similarity_standard() {
        let _api_key = std::env::var("JINA_API_KEY").expect("JINA_API_KEY must be set");
        test_semantic_similarity(Arc::new(create_test_embeddings())).await;
    }
}
