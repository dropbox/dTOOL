//! Voyage AI rerank document compressor
//!
//! This module provides a document compressor that uses the Voyage AI Rerank API
//! to reorder documents by relevance to a query.
//!
//! # Models
//!
//! - `rerank-2.5`: Latest model with 32K context (default)
//! - `rerank-2.5-lite`: Lightweight 32K context model
//! - `rerank-2`: Previous generation
//! - `rerank-2-lite`: Lightweight previous generation
//!
//! # Example
//!
//! ```no_run
//! use dashflow_voyage::VoyageRerank;
//! use dashflow::core::documents::{Document, DocumentCompressor};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let reranker = VoyageRerank::new()
//!     .with_api_key("your-api-key")
//!     .with_model("rerank-2.5")
//!     .with_top_k(Some(3));
//!
//! let documents = vec![
//!     Document::new("Document about cats"),
//!     Document::new("Document about dogs"),
//!     Document::new("Document about birds"),
//! ];
//!
//! let reranked = reranker
//!     .compress_documents(documents, "cats", None)
//!     .await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use dashflow::core::{
    config::RunnableConfig,
    config_loader::env_vars::{env_string, VOYAGE_API_KEY},
    documents::{Document, DocumentCompressor},
    error::Error as DashFlowError,
    http_client::{self, json_with_limit, DEFAULT_RESPONSE_SIZE_LIMIT},
    rate_limiters::RateLimiter,
    retry::{with_retry, RetryPolicy},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::VOYAGE_API_BASE;

const DEFAULT_MODEL: &str = "rerank-2.5";

/// Request format for Voyage Rerank API
#[derive(Debug, Clone, Serialize)]
struct RerankRequest {
    query: String,
    documents: Vec<String>,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncation: Option<bool>,
}

/// Individual result from Voyage Rerank API
#[derive(Debug, Clone, Deserialize)]
pub struct RerankResult {
    /// Index of the document in the original list
    pub index: usize,
    /// Relevance score (higher is more relevant)
    pub relevance_score: f64,
}

/// Response from Voyage Rerank API. Usage field required for serde deserialization
/// but not currently accessed (reserved for future cost tracking).
#[derive(Debug, Clone, Deserialize)]
struct RerankResponse {
    data: Vec<RerankResult>,
    #[allow(dead_code)] // Deserialize: Token usage - reserved for cost tracking
    usage: RerankUsage,
}

/// Usage information from Voyage Rerank API
#[derive(Debug, Clone, Deserialize)]
struct RerankUsage {
    #[allow(dead_code)] // Deserialize: Tokens billed for rerank - reserved for cost tracking
    total_tokens: u32,
}

/// Document compressor using Voyage AI's Rerank API
///
/// Reorders documents by relevance to a query using Voyage AI's specialized
/// reranking models. This is particularly useful for improving retrieval
/// results in RAG pipelines.
///
/// # Models
///
/// - `rerank-2.5` (default): Latest model with 32K token context
/// - `rerank-2.5-lite`: Lightweight version with 32K context
/// - `rerank-2`: Previous generation model
/// - `rerank-2-lite`: Lightweight previous generation
///
/// # Example
///
/// ```no_run
/// use dashflow_voyage::VoyageRerank;
/// use dashflow::core::documents::{Document, DocumentCompressor};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let reranker = VoyageRerank::new()
///     .with_api_key("your-api-key")
///     .with_model("rerank-2.5")
///     .with_top_k(Some(3));
///
/// let documents = vec![
///     Document::new("Carson City is the capital city of the American state of Nevada."),
///     Document::new("The Commonwealth of the Northern Mariana Islands is a group of islands in the Pacific Ocean."),
///     Document::new("Washington, D.C. is the capital of the United States."),
/// ];
///
/// let reranked = reranker
///     .compress_documents(documents, "What is the capital of the United States?", None)
///     .await?;
///
/// // First document should be about Washington, D.C.
/// assert!(reranked[0].page_content.contains("Washington"));
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct VoyageRerank {
    /// Voyage API key
    api_key: Option<String>,
    /// Model to use for reranking
    model: String,
    /// Number of documents to return (None = return all)
    top_k: Option<usize>,
    /// Whether to truncate inputs exceeding context length
    truncation: bool,
    /// HTTP client for API requests
    client: reqwest::Client,
    /// Retry policy for API calls
    retry_policy: RetryPolicy,
    /// Optional rate limiter to control request rate
    rate_limiter: Option<Arc<dyn RateLimiter>>,
}

// Custom Debug implementation to prevent API key exposure in logs
impl std::fmt::Debug for VoyageRerank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoyageRerank")
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("model", &self.model)
            .field("top_k", &self.top_k)
            .field("truncation", &self.truncation)
            .field("client", &"reqwest::Client")
            .field("retry_policy", &self.retry_policy)
            .field("rate_limiter", &self.rate_limiter.as_ref().map(|_| "RateLimiter"))
            .finish()
    }
}

impl VoyageRerank {
    /// Try to create a new `VoyageRerank` instance.
    ///
    /// Reads API key from `VOYAGE_API_KEY` environment variable if set.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn try_new() -> Result<Self, DashFlowError> {
        Ok(Self {
            api_key: env_string(VOYAGE_API_KEY),
            model: DEFAULT_MODEL.to_string(),
            top_k: Some(3),
            truncation: true,
            client: http_client::create_llm_client()?,
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        })
    }

    /// Create a new `VoyageRerank` instance
    ///
    /// Attempts to read the API key from the `VOYAGE_API_KEY` environment variable.
    /// If not set, you must call `.with_api_key()` before using the compressor.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created. Use `try_new()` for fallible creation.
    #[must_use]
    #[allow(clippy::expect_used)] // Documented panic with try_new() fallible alternative
    pub fn new() -> Self {
        Self::try_new().expect("Failed to create HTTP client for VoyageRerank")
    }

    /// Set the Voyage API key
    #[must_use]
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the model to use for reranking
    ///
    /// Available models:
    /// - rerank-2.5 (default) - Latest model with 32K context
    /// - rerank-2.5-lite - Lightweight 32K context model
    /// - rerank-2 - Previous generation
    /// - rerank-2-lite - Lightweight previous generation
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the number of top documents to return
    ///
    /// If None, all documents are returned. If Some(k), only the top k
    /// documents by relevance score are returned.
    #[must_use]
    pub fn with_top_k(mut self, top_k: Option<usize>) -> Self {
        self.top_k = top_k;
        self
    }

    /// Set whether to truncate inputs exceeding context length
    ///
    /// Default is true. If false, inputs exceeding the model's context
    /// length will return an error.
    #[must_use]
    pub fn with_truncation(mut self, truncation: bool) -> Self {
        self.truncation = truncation;
        self
    }

    /// Set the retry policy for API calls
    ///
    /// Default is exponential backoff with 3 retries.
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Set a rate limiter to control request rate
    #[must_use]
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiter>) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Get the API key, returning an error if not configured
    fn get_api_key(&self) -> Result<&str, DashFlowError> {
        self.api_key.as_deref().ok_or_else(|| {
            DashFlowError::invalid_input(
                "VOYAGE_API_KEY not set. Set environment variable or use .with_api_key()",
            )
        })
    }

    /// Rerank documents and return results with scores
    ///
    /// This is a lower-level method that returns the raw reranking results
    /// including indices and relevance scores.
    pub async fn rerank(
        &self,
        documents: &[Document],
        query: &str,
    ) -> Result<Vec<RerankResult>, DashFlowError> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        let api_key = self.get_api_key()?;

        // Extract document text content
        let doc_texts: Vec<String> = documents
            .iter()
            .map(|doc| doc.page_content.clone())
            .collect();

        let request = RerankRequest {
            query: query.to_string(),
            documents: doc_texts,
            model: self.model.clone(),
            top_k: self.top_k,
            truncation: Some(self.truncation),
        };

        // Acquire rate limiter token if configured
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await;
        }

        let url = format!("{VOYAGE_API_BASE}/rerank");
        let response = with_retry(&self.retry_policy, || async {
            self.client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&request)
                .send()
                .await
                .map_err(|e| {
                    DashFlowError::network(format!("Failed to call Voyage Rerank API: {e}"))
                })
        })
        .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(DashFlowError::api(format!(
                "Voyage Rerank API returned error {status}: {error_text}"
            )));
        }

        // M-216: Use size-limited JSON parsing to prevent memory exhaustion
        let rerank_response: RerankResponse =
            json_with_limit(response, DEFAULT_RESPONSE_SIZE_LIMIT).await.map_err(|e| {
                DashFlowError::api_format(format!("Failed to parse Voyage Rerank response: {e}"))
            })?;

        Ok(rerank_response.data)
    }
}

impl Default for VoyageRerank {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DocumentCompressor for VoyageRerank {
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>, DashFlowError> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        let results = self.rerank(&documents, query).await?;

        // Build reranked documents with relevance scores in metadata
        let mut compressed = Vec::new();
        for result in results {
            if result.index < documents.len() {
                let doc = &documents[result.index];
                let mut new_doc = doc.clone();
                new_doc
                    .metadata
                    .insert("relevance_score".to_string(), result.relevance_score.into());
                compressed.push(new_doc);
            }
        }

        Ok(compressed)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::env;

    // ==================== DEFAULT_MODEL constant ====================

    #[test]
    fn test_default_model_constant() {
        assert_eq!(DEFAULT_MODEL, "rerank-2.5");
    }

    // ==================== RerankRequest struct ====================

    #[test]
    fn test_rerank_request_minimal() {
        let request = RerankRequest {
            query: "q".to_string(),
            documents: vec!["doc".to_string()],
            model: "rerank-2.5".to_string(),
            top_k: None,
            truncation: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"query\":\"q\""));
        assert!(json.contains("\"documents\":[\"doc\"]"));
        assert!(json.contains("\"model\":\"rerank-2.5\""));
        assert!(!json.contains("top_k"));
        assert!(!json.contains("truncation"));
    }

    #[test]
    fn test_rerank_request_full() {
        let request = RerankRequest {
            query: "test query".to_string(),
            documents: vec!["doc1".to_string(), "doc2".to_string()],
            model: "rerank-2.5".to_string(),
            top_k: Some(5),
            truncation: Some(true),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"query\":\"test query\""));
        assert!(json.contains("\"model\":\"rerank-2.5\""));
        assert!(json.contains("\"top_k\":5"));
        assert!(json.contains("\"truncation\":true"));
    }

    #[test]
    fn test_rerank_request_empty_documents() {
        let request = RerankRequest {
            query: "query".to_string(),
            documents: vec![],
            model: "rerank-2.5".to_string(),
            top_k: None,
            truncation: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"documents\":[]"));
    }

    #[test]
    fn test_rerank_request_truncation_false() {
        let request = RerankRequest {
            query: "q".to_string(),
            documents: vec!["d".to_string()],
            model: "rerank-2.5".to_string(),
            top_k: None,
            truncation: Some(false),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"truncation\":false"));
    }

    #[test]
    fn test_rerank_request_top_k_zero() {
        let request = RerankRequest {
            query: "q".to_string(),
            documents: vec!["d".to_string()],
            model: "rerank-2.5".to_string(),
            top_k: Some(0),
            truncation: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"top_k\":0"));
    }

    #[test]
    fn test_rerank_request_clone() {
        let request = RerankRequest {
            query: "q".to_string(),
            documents: vec!["d".to_string()],
            model: "m".to_string(),
            top_k: Some(5),
            truncation: Some(true),
        };
        let cloned = request.clone();
        assert_eq!(cloned.query, request.query);
        assert_eq!(cloned.documents, request.documents);
        assert_eq!(cloned.model, request.model);
        assert_eq!(cloned.top_k, request.top_k);
        assert_eq!(cloned.truncation, request.truncation);
    }

    #[test]
    fn test_rerank_request_debug() {
        let request = RerankRequest {
            query: "test".to_string(),
            documents: vec!["doc".to_string()],
            model: "rerank-2.5".to_string(),
            top_k: Some(3),
            truncation: Some(true),
        };
        let debug_str = format!("{:?}", request);
        assert!(debug_str.contains("RerankRequest"));
        assert!(debug_str.contains("test"));
    }

    // ==================== RerankResult struct ====================

    #[test]
    fn test_rerank_result_deserialization() {
        let json = r#"{"index": 0, "relevance_score": 0.95}"#;
        let result: RerankResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.index, 0);
        assert!((result.relevance_score - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rerank_result_zero_score() {
        let json = r#"{"index": 5, "relevance_score": 0.0}"#;
        let result: RerankResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.index, 5);
        assert!((result.relevance_score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rerank_result_negative_score() {
        let json = r#"{"index": 1, "relevance_score": -0.5}"#;
        let result: RerankResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.index, 1);
        assert!((result.relevance_score - (-0.5)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rerank_result_high_index() {
        let json = r#"{"index": 999, "relevance_score": 0.1}"#;
        let result: RerankResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.index, 999);
    }

    #[test]
    fn test_rerank_result_clone() {
        let json = r#"{"index": 2, "relevance_score": 0.8}"#;
        let result: RerankResult = serde_json::from_str(json).unwrap();
        let cloned = result.clone();
        assert_eq!(cloned.index, result.index);
        assert!((cloned.relevance_score - result.relevance_score).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rerank_result_debug() {
        let json = r#"{"index": 3, "relevance_score": 0.77}"#;
        let result: RerankResult = serde_json::from_str(json).unwrap();
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("RerankResult"));
        assert!(debug_str.contains("3"));
    }

    // ==================== RerankResponse struct ====================

    #[test]
    fn test_rerank_response_minimal() {
        let json = r#"{
            "data": [],
            "usage": {"total_tokens": 0}
        }"#;
        let response: RerankResponse = serde_json::from_str(json).unwrap();
        assert!(response.data.is_empty());
    }

    #[test]
    fn test_rerank_response_single_result() {
        let json = r#"{
            "data": [{"index": 0, "relevance_score": 0.9}],
            "usage": {"total_tokens": 10}
        }"#;
        let response: RerankResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].index, 0);
    }

    #[test]
    fn test_rerank_response_multiple_results() {
        let json = r#"{
            "data": [
                {"index": 2, "relevance_score": 0.95},
                {"index": 0, "relevance_score": 0.75},
                {"index": 1, "relevance_score": 0.50}
            ],
            "usage": {"total_tokens": 100}
        }"#;

        let response: RerankResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 3);
        assert_eq!(response.data[0].index, 2);
        assert!((response.data[0].relevance_score - 0.95).abs() < f64::EPSILON);
        assert_eq!(response.data[1].index, 0);
        assert_eq!(response.data[2].index, 1);
    }

    #[test]
    fn test_rerank_response_clone() {
        let json = r#"{
            "data": [{"index": 0, "relevance_score": 0.8}],
            "usage": {"total_tokens": 50}
        }"#;
        let response: RerankResponse = serde_json::from_str(json).unwrap();
        let cloned = response.clone();
        assert_eq!(cloned.data.len(), response.data.len());
        assert_eq!(cloned.data[0].index, response.data[0].index);
    }

    #[test]
    fn test_rerank_response_debug() {
        let json = r#"{
            "data": [{"index": 0, "relevance_score": 0.9}],
            "usage": {"total_tokens": 10}
        }"#;
        let response: RerankResponse = serde_json::from_str(json).unwrap();
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("RerankResponse"));
    }

    // ==================== RerankUsage struct ====================

    #[test]
    fn test_rerank_usage_deserialization() {
        let json = r#"{"total_tokens": 100}"#;
        let usage: RerankUsage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.total_tokens, 100);
    }

    #[test]
    fn test_rerank_usage_zero() {
        let json = r#"{"total_tokens": 0}"#;
        let usage: RerankUsage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_rerank_usage_large() {
        let json = r#"{"total_tokens": 4294967295}"#;
        let usage: RerankUsage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.total_tokens, u32::MAX);
    }

    #[test]
    fn test_rerank_usage_clone() {
        let json = r#"{"total_tokens": 42}"#;
        let usage: RerankUsage = serde_json::from_str(json).unwrap();
        let cloned = usage.clone();
        assert_eq!(cloned.total_tokens, usage.total_tokens);
    }

    #[test]
    fn test_rerank_usage_debug() {
        let json = r#"{"total_tokens": 77}"#;
        let usage: RerankUsage = serde_json::from_str(json).unwrap();
        let debug_str = format!("{:?}", usage);
        assert!(debug_str.contains("RerankUsage"));
        assert!(debug_str.contains("77"));
    }

    // ==================== VoyageRerank construction ====================

    #[test]
    fn test_voyage_rerank_new() {
        let reranker = VoyageRerank::new();
        assert_eq!(reranker.model, DEFAULT_MODEL);
        assert_eq!(reranker.top_k, Some(3));
        assert!(reranker.truncation);
    }

    #[test]
    fn test_voyage_rerank_try_new() {
        let result = VoyageRerank::try_new();
        assert!(result.is_ok());
        let reranker = result.unwrap();
        assert_eq!(reranker.model, DEFAULT_MODEL);
    }

    #[test]
    fn test_voyage_rerank_default() {
        let reranker = VoyageRerank::default();
        assert_eq!(reranker.model, DEFAULT_MODEL);
        assert_eq!(reranker.top_k, Some(3));
        assert!(reranker.truncation);
    }

    #[test]
    fn test_voyage_rerank_default_retry_policy() {
        let reranker = VoyageRerank::new();
        assert_eq!(reranker.retry_policy.max_retries, 3);
    }

    #[test]
    fn test_voyage_rerank_default_rate_limiter_none() {
        let reranker = VoyageRerank::new();
        assert!(reranker.rate_limiter.is_none());
    }

    // ==================== Builder methods ====================

    #[test]
    fn test_voyage_rerank_with_api_key() {
        let reranker = VoyageRerank::new().with_api_key("test-key");
        assert_eq!(reranker.api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_voyage_rerank_with_api_key_string() {
        let reranker = VoyageRerank::new().with_api_key(String::from("api-string"));
        assert_eq!(reranker.api_key, Some("api-string".to_string()));
    }

    #[test]
    fn test_voyage_rerank_with_api_key_empty() {
        let reranker = VoyageRerank::new().with_api_key("");
        assert_eq!(reranker.api_key, Some(String::new()));
    }

    #[test]
    fn test_voyage_rerank_with_model() {
        let reranker = VoyageRerank::new().with_model("rerank-2.5-lite");
        assert_eq!(reranker.model, "rerank-2.5-lite");
    }

    #[test]
    fn test_voyage_rerank_with_model_rerank_2() {
        let reranker = VoyageRerank::new().with_model("rerank-2");
        assert_eq!(reranker.model, "rerank-2");
    }

    #[test]
    fn test_voyage_rerank_with_model_rerank_2_lite() {
        let reranker = VoyageRerank::new().with_model("rerank-2-lite");
        assert_eq!(reranker.model, "rerank-2-lite");
    }

    #[test]
    fn test_voyage_rerank_with_model_string() {
        let model = String::from("custom-model");
        let reranker = VoyageRerank::new().with_model(model);
        assert_eq!(reranker.model, "custom-model");
    }

    #[test]
    fn test_voyage_rerank_with_top_k_some() {
        let reranker = VoyageRerank::new().with_top_k(Some(5));
        assert_eq!(reranker.top_k, Some(5));
    }

    #[test]
    fn test_voyage_rerank_with_top_k_none() {
        let reranker = VoyageRerank::new().with_top_k(None);
        assert!(reranker.top_k.is_none());
    }

    #[test]
    fn test_voyage_rerank_with_top_k_one() {
        let reranker = VoyageRerank::new().with_top_k(Some(1));
        assert_eq!(reranker.top_k, Some(1));
    }

    #[test]
    fn test_voyage_rerank_with_top_k_large() {
        let reranker = VoyageRerank::new().with_top_k(Some(1000));
        assert_eq!(reranker.top_k, Some(1000));
    }

    #[test]
    fn test_voyage_rerank_with_truncation_false() {
        let reranker = VoyageRerank::new().with_truncation(false);
        assert!(!reranker.truncation);
    }

    #[test]
    fn test_voyage_rerank_with_truncation_true() {
        let reranker = VoyageRerank::new().with_truncation(true);
        assert!(reranker.truncation);
    }

    #[test]
    fn test_voyage_rerank_with_retry_policy() {
        let policy = RetryPolicy::exponential(5);
        let reranker = VoyageRerank::new().with_retry_policy(policy);
        assert_eq!(reranker.retry_policy.max_retries, 5);
    }

    #[test]
    fn test_voyage_rerank_with_retry_policy_no_retry() {
        let policy = RetryPolicy::no_retry();
        let reranker = VoyageRerank::new().with_retry_policy(policy);
        assert_eq!(reranker.retry_policy.max_retries, 0);
    }

    #[test]
    fn test_voyage_rerank_with_rate_limiter() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0);
        let reranker = VoyageRerank::new().with_rate_limiter(Arc::new(limiter));
        assert!(reranker.rate_limiter.is_some());
    }

    // ==================== Builder chaining ====================

    #[test]
    fn test_voyage_rerank_builder_chaining_all() {
        use dashflow::core::rate_limiters::InMemoryRateLimiter;
        use std::time::Duration;

        let limiter = InMemoryRateLimiter::new(10.0, Duration::from_millis(100), 20.0);
        let reranker = VoyageRerank::new()
            .with_api_key("test-key")
            .with_model("rerank-2.5-lite")
            .with_top_k(Some(5))
            .with_truncation(false)
            .with_retry_policy(RetryPolicy::exponential(7))
            .with_rate_limiter(Arc::new(limiter));

        assert_eq!(reranker.api_key, Some("test-key".to_string()));
        assert_eq!(reranker.model, "rerank-2.5-lite");
        assert_eq!(reranker.top_k, Some(5));
        assert!(!reranker.truncation);
        assert_eq!(reranker.retry_policy.max_retries, 7);
        assert!(reranker.rate_limiter.is_some());
    }

    #[test]
    fn test_voyage_rerank_builder_order_independence() {
        let r1 = VoyageRerank::new()
            .with_model("m")
            .with_api_key("k")
            .with_top_k(Some(10));

        let r2 = VoyageRerank::new()
            .with_api_key("k")
            .with_top_k(Some(10))
            .with_model("m");

        assert_eq!(r1.model, r2.model);
        assert_eq!(r1.api_key, r2.api_key);
        assert_eq!(r1.top_k, r2.top_k);
    }

    #[test]
    fn test_voyage_rerank_builder_override() {
        let reranker = VoyageRerank::new()
            .with_model("model-1")
            .with_model("model-2");
        assert_eq!(reranker.model, "model-2");
    }

    // ==================== Clone trait ====================

    #[test]
    fn test_voyage_rerank_clone() {
        let reranker = VoyageRerank::new()
            .with_api_key("key")
            .with_model("rerank-2")
            .with_top_k(Some(7));
        let cloned = reranker.clone();
        assert_eq!(cloned.api_key, reranker.api_key);
        assert_eq!(cloned.model, reranker.model);
        assert_eq!(cloned.top_k, reranker.top_k);
        assert_eq!(cloned.truncation, reranker.truncation);
    }

    // ==================== Debug trait ====================

    #[test]
    fn test_voyage_rerank_debug() {
        let reranker = VoyageRerank::new().with_api_key("secret-key");
        let debug_str = format!("{:?}", reranker);
        assert!(debug_str.contains("VoyageRerank"));
        // API key should be redacted
        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains("secret-key"));
    }

    #[test]
    fn test_voyage_rerank_debug_no_api_key() {
        let reranker = VoyageRerank {
            api_key: None,
            model: DEFAULT_MODEL.to_string(),
            top_k: Some(3),
            truncation: true,
            client: reqwest::Client::new(),
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        };
        let debug_str = format!("{:?}", reranker);
        assert!(debug_str.contains("VoyageRerank"));
        assert!(debug_str.contains("None"));
    }

    #[test]
    fn test_voyage_rerank_debug_shows_model() {
        let reranker = VoyageRerank::new().with_model("custom-model");
        let debug_str = format!("{:?}", reranker);
        assert!(debug_str.contains("custom-model"));
    }

    #[test]
    fn test_voyage_rerank_debug_shows_top_k() {
        let reranker = VoyageRerank::new().with_top_k(Some(42));
        let debug_str = format!("{:?}", reranker);
        assert!(debug_str.contains("42"));
    }

    // ==================== get_api_key ====================

    #[test]
    fn test_get_api_key_success() {
        let reranker = VoyageRerank::new().with_api_key("test-api-key");
        let result = reranker.get_api_key();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test-api-key");
    }

    #[test]
    fn test_get_api_key_missing() {
        let reranker = VoyageRerank {
            api_key: None,
            model: DEFAULT_MODEL.to_string(),
            top_k: Some(3),
            truncation: true,
            client: reqwest::Client::new(),
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        };
        let result = reranker.get_api_key();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("VOYAGE_API_KEY not set"));
    }

    // ==================== Async tests ====================

    #[tokio::test]
    async fn test_compress_empty_documents() {
        let reranker = VoyageRerank::new().with_api_key("test-key");
        let result = reranker.compress_documents(vec![], "query", None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_rerank_empty_documents() {
        let reranker = VoyageRerank::new().with_api_key("test-key");
        let result = reranker.rerank(&[], "query").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_rerank_without_api_key() {
        let reranker = VoyageRerank {
            api_key: None,
            model: DEFAULT_MODEL.to_string(),
            top_k: Some(3),
            truncation: true,
            client: reqwest::Client::new(),
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        };
        let documents = vec![Document::new("test")];
        let result = reranker.rerank(&documents, "query").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("VOYAGE_API_KEY not set"));
    }

    #[tokio::test]
    async fn test_compress_documents_without_api_key() {
        let reranker = VoyageRerank {
            api_key: None,
            model: DEFAULT_MODEL.to_string(),
            top_k: Some(3),
            truncation: true,
            client: reqwest::Client::new(),
            retry_policy: RetryPolicy::exponential(3),
            rate_limiter: None,
        };
        let documents = vec![Document::new("test")];
        let result = reranker
            .compress_documents(documents, "query", None)
            .await;
        assert!(result.is_err());
    }

    // ==================== Integration tests (ignored without API key) ====================

    #[tokio::test]
    #[ignore = "requires VOYAGE_API_KEY"]
    async fn test_voyage_rerank_integration() {
        let api_key = env::var("VOYAGE_API_KEY").unwrap_or_default();
        if api_key.is_empty() {
            return;
        }

        let reranker = VoyageRerank::new()
            .with_api_key(api_key)
            .with_top_k(Some(2));

        let documents = vec![
            Document::new("Carson City is the capital city of the American state of Nevada."),
            Document::new("The Commonwealth of the Northern Mariana Islands is a group of islands in the Pacific Ocean. Its capital is Saipan."),
            Document::new("Capitalization or capitalisation in English grammar is the use of a capital letter at the start of a word."),
            Document::new("Washington, D.C. (also known as simply Washington or D.C.) is the capital of the United States. It is a federal district."),
        ];

        let result = reranker
            .compress_documents(documents, "What is the capital of the United States?", None)
            .await;

        assert!(result.is_ok());
        let reranked = result.unwrap();
        assert_eq!(reranked.len(), 2);
        assert!(reranked[0].page_content.contains("Washington"));
        assert!(reranked[0].metadata.contains_key("relevance_score"));
        assert!(reranked[1].metadata.contains_key("relevance_score"));

        let score_0: f64 = reranked[0].metadata["relevance_score"].as_f64().unwrap();
        let score_1: f64 = reranked[1].metadata["relevance_score"].as_f64().unwrap();
        assert!(score_0 >= score_1);
    }

    #[tokio::test]
    #[ignore = "requires VOYAGE_API_KEY"]
    async fn test_rerank_integration_raw() {
        let api_key = env::var("VOYAGE_API_KEY").unwrap_or_default();
        if api_key.is_empty() {
            return;
        }

        let reranker = VoyageRerank::new()
            .with_api_key(api_key)
            .with_top_k(Some(3));

        let documents = vec![
            Document::new("Python is a programming language."),
            Document::new("Rust is a systems programming language."),
            Document::new("The sky is blue."),
        ];

        let result = reranker
            .rerank(&documents, "What programming languages exist?")
            .await;

        assert!(result.is_ok());
        let results = result.unwrap();
        assert!(!results.is_empty());
        // Top results should be about programming languages
        assert!(results[0].index == 0 || results[0].index == 1);
    }

    #[tokio::test]
    #[ignore = "requires VOYAGE_API_KEY"]
    async fn test_rerank_no_top_k_returns_all() {
        let api_key = env::var("VOYAGE_API_KEY").unwrap_or_default();
        if api_key.is_empty() {
            return;
        }

        let reranker = VoyageRerank::new()
            .with_api_key(api_key)
            .with_top_k(None);

        let documents = vec![
            Document::new("Doc 1"),
            Document::new("Doc 2"),
            Document::new("Doc 3"),
            Document::new("Doc 4"),
        ];

        let result = reranker
            .compress_documents(documents, "query", None)
            .await;

        assert!(result.is_ok());
        let reranked = result.unwrap();
        assert_eq!(reranked.len(), 4);
    }
}
