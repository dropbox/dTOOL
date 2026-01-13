//! Cohere rerank document compressor
//!
//! This module provides a document compressor that uses the Cohere Rerank API
//! to reorder documents by relevance to a query.

use async_trait::async_trait;
use dashflow::core::{
    config::RunnableConfig,
    config_loader::env_vars::{
        cohere_api_v1_url, env_string_or_default, COHERE_API_KEY, DEFAULT_COHERE_RERANK_ENDPOINT,
    },
    documents::{Document, DocumentCompressor},
    error::Error as DashFlowError,
    http_client::{self, json_with_limit, DEFAULT_RESPONSE_SIZE_LIMIT},
};
use serde::{Deserialize, Serialize};

const DEFAULT_MODEL: &str = "rerank-english-v3.0";

/// Request format for Cohere Rerank API
#[derive(Debug, Clone, Serialize)]
struct RerankRequest {
    query: String,
    documents: Vec<String>,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_n: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_chunks_per_doc: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    return_documents: Option<bool>,
}

/// Individual result from Cohere Rerank API
#[derive(Debug, Clone, Deserialize)]
pub struct RerankResult {
    pub index: usize,
    pub relevance_score: f64,
}

/// Response from Cohere Rerank API
#[derive(Debug, Clone, Deserialize)]
struct RerankResponse {
    results: Vec<RerankResult>,
}

/// Document compressor using Cohere's Rerank API
///
/// Reorders documents by relevance to a query using Cohere's specialized
/// reranking models. This is particularly useful for improving retrieval
/// results in RAG pipelines.
///
/// # Example
///
/// ```no_run
/// use dashflow_cohere::CohereRerank;
/// use dashflow::core::documents::{Document, DocumentCompressor};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let reranker = CohereRerank::new()
///     .with_api_key("your-api-key")
///     .with_model("rerank-english-v3.0")
///     .with_top_n(Some(3));
///
/// let documents = vec![
///     Document::new("Document about cats"),
///     Document::new("Document about dogs"),
///     Document::new("Document about birds"),
/// ];
///
/// let reranked = reranker
///     .compress_documents(documents, "cats", None)
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CohereRerank {
    /// Cohere API key
    api_key: String,
    /// Model to use for reranking
    model: String,
    /// Number of documents to return (None = return all)
    top_n: Option<usize>,
    /// Maximum number of chunks per document
    max_chunks_per_doc: Option<usize>,
    /// HTTP client for API requests
    client: reqwest::Client,
}

// Custom Debug to prevent API key exposure in logs
impl std::fmt::Debug for CohereRerank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CohereRerank")
            .field("api_key", &"[REDACTED]")
            .field("model", &self.model)
            .field("top_n", &self.top_n)
            .field("max_chunks_per_doc", &self.max_chunks_per_doc)
            .field("client", &"[reqwest::Client]")
            .finish()
    }
}

impl CohereRerank {
    /// Try to create a new `CohereRerank` instance.
    ///
    /// Reads API key from `COHERE_API_KEY` environment variable if set.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn try_new() -> Result<Self, DashFlowError> {
        let api_key = env_string_or_default(COHERE_API_KEY, "");
        Ok(Self {
            api_key,
            model: DEFAULT_MODEL.to_string(),
            top_n: Some(3),
            max_chunks_per_doc: None,
            client: http_client::create_llm_client()?,
        })
    }

    /// Create a new `CohereRerank` instance
    ///
    /// Attempts to read the API key from the `COHERE_API_KEY` environment variable.
    /// If not set, you must call `.with_api_key()` before using the compressor.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created. Use `try_new()` for fallible creation.
    #[allow(clippy::expect_used)] // Documented panic with try_new() alternative
    #[must_use]
    pub fn new() -> Self {
        Self::try_new().expect("Failed to create HTTP client for CohereRerank")
    }

    /// Set the Cohere API key
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = api_key.into();
        self
    }

    /// Set the model to use for reranking
    ///
    /// Available models:
    /// - rerank-english-v3.0 (default)
    /// - rerank-multilingual-v3.0
    /// - rerank-english-v2.0
    /// - rerank-multilingual-v2.0
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the number of top documents to return
    ///
    /// If None, all documents are returned. If Some(n), only the top n
    /// documents by relevance score are returned.
    #[must_use]
    pub fn with_top_n(mut self, top_n: Option<usize>) -> Self {
        self.top_n = top_n;
        self
    }

    /// Set the maximum number of chunks per document
    #[must_use]
    pub fn with_max_chunks_per_doc(mut self, max_chunks: Option<usize>) -> Self {
        self.max_chunks_per_doc = max_chunks;
        self
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

        if self.api_key.is_empty() {
            return Err(DashFlowError::invalid_input(
                "COHERE_API_KEY not set. Set environment variable or use .with_api_key()",
            ));
        }

        // Extract document text content
        let doc_texts: Vec<String> = documents
            .iter()
            .map(|doc| doc.page_content.clone())
            .collect();

        let request = RerankRequest {
            query: query.to_string(),
            documents: doc_texts,
            model: self.model.clone(),
            top_n: self.top_n,
            max_chunks_per_doc: self.max_chunks_per_doc,
            return_documents: Some(false),
        };

        let url = cohere_api_v1_url(DEFAULT_COHERE_RERANK_ENDPOINT);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                DashFlowError::network(format!("Failed to call Cohere Rerank API: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(DashFlowError::api(format!(
                "Cohere Rerank API returned error {status}: {error_text}"
            )));
        }

        // M-216: Use size-limited JSON parsing to prevent memory exhaustion
        let rerank_response: RerankResponse =
            json_with_limit(response, DEFAULT_RESPONSE_SIZE_LIMIT).await.map_err(|e| {
                DashFlowError::api_format(format!("Failed to parse Cohere Rerank response: {e}"))
            })?;

        Ok(rerank_response.results)
    }
}

impl Default for CohereRerank {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DocumentCompressor for CohereRerank {
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_cohere_rerank_new() {
        let reranker = CohereRerank::new();
        assert_eq!(reranker.model, DEFAULT_MODEL);
        assert_eq!(reranker.top_n, Some(3));
    }

    #[test]
    fn test_cohere_rerank_builder() {
        let reranker = CohereRerank::new()
            .with_api_key("test-key")
            .with_model("rerank-multilingual-v3.0")
            .with_top_n(Some(5))
            .with_max_chunks_per_doc(Some(10));

        assert_eq!(reranker.api_key, "test-key");
        assert_eq!(reranker.model, "rerank-multilingual-v3.0");
        assert_eq!(reranker.top_n, Some(5));
        assert_eq!(reranker.max_chunks_per_doc, Some(10));
    }

    #[tokio::test]
    async fn test_compress_empty_documents() {
        let reranker = CohereRerank::new().with_api_key("test-key");
        let result = reranker.compress_documents(vec![], "query", None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_rerank_without_api_key() {
        // Clear environment variable for this test
        let reranker = CohereRerank::new().with_api_key("");
        let documents = vec![Document::new("test")];
        let result = reranker.rerank(&documents, "query").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("COHERE_API_KEY not set"));
    }

    #[tokio::test]
    #[ignore = "requires API key"]
    async fn test_cohere_rerank_integration() {
        // This test requires a real Cohere API key
        let api_key = env::var("COHERE_API_KEY").unwrap_or_default();
        if api_key.is_empty() {
            return;
        }

        let reranker = CohereRerank::new()
            .with_api_key(api_key)
            .with_top_n(Some(2));

        let documents = vec![
            Document::new("Carson City is the capital city of the American state of Nevada."),
            Document::new("The Commonwealth of the Northern Mariana Islands is a group of islands in the Pacific Ocean. Its capital is Saipan."),
            Document::new("Capitalization or capitalisation in English grammar is the use of a capital letter at the start of a word. English usage varies from capitalization in other languages."),
            Document::new("Washington, D.C. (also known as simply Washington or D.C., and officially as the District of Columbia) is the capital of the United States. It is a federal district."),
        ];

        let result = reranker
            .compress_documents(documents, "What is the capital of the United States?", None)
            .await;

        assert!(result.is_ok());
        let reranked = result.unwrap();
        assert_eq!(reranked.len(), 2); // top_n = 2

        // The first document should be about Washington, D.C.
        assert!(reranked[0].page_content.contains("Washington"));

        // Check that relevance scores are present
        assert!(reranked[0].metadata.contains_key("relevance_score"));
    }

    // ========== try_new tests ==========

    #[test]
    fn test_try_new() {
        let result = CohereRerank::try_new();
        assert!(result.is_ok());
    }

    // ========== Default trait tests ==========

    #[test]
    fn test_default_trait() {
        let reranker = CohereRerank::default();
        assert_eq!(reranker.model, DEFAULT_MODEL);
        assert_eq!(reranker.top_n, Some(3));
    }

    // ========== Debug trait tests ==========

    #[test]
    fn test_debug_redacts_api_key() {
        let reranker = CohereRerank::new().with_api_key("super-secret-key");
        let debug_str = format!("{:?}", reranker);

        // API key should be redacted
        assert!(debug_str.contains("[REDACTED]"));
        // Should NOT contain the actual key
        assert!(!debug_str.contains("super-secret-key"));
        // Should contain model name
        assert!(debug_str.contains("rerank-english-v3.0"));
    }

    #[test]
    fn test_debug_shows_model() {
        let reranker = CohereRerank::new().with_model("rerank-multilingual-v3.0");
        let debug_str = format!("{:?}", reranker);
        assert!(debug_str.contains("rerank-multilingual-v3.0"));
    }

    #[test]
    fn test_debug_shows_top_n() {
        let reranker = CohereRerank::new().with_top_n(Some(10));
        let debug_str = format!("{:?}", reranker);
        assert!(debug_str.contains("10"));
    }

    #[test]
    fn test_debug_shows_max_chunks() {
        let reranker = CohereRerank::new().with_max_chunks_per_doc(Some(5));
        let debug_str = format!("{:?}", reranker);
        assert!(debug_str.contains("5"));
    }

    // ========== Builder additional tests ==========

    #[test]
    fn test_builder_with_all_models() {
        let models = [
            "rerank-english-v3.0",
            "rerank-multilingual-v3.0",
            "rerank-english-v2.0",
            "rerank-multilingual-v2.0",
        ];

        for model in models {
            let reranker = CohereRerank::new().with_model(model);
            assert_eq!(reranker.model, model);
        }
    }

    #[test]
    fn test_builder_with_top_n_none() {
        let reranker = CohereRerank::new().with_top_n(None);
        assert!(reranker.top_n.is_none());
    }

    #[test]
    fn test_builder_with_top_n_zero() {
        let reranker = CohereRerank::new().with_top_n(Some(0));
        assert_eq!(reranker.top_n, Some(0));
    }

    #[test]
    fn test_builder_with_top_n_large() {
        let reranker = CohereRerank::new().with_top_n(Some(1000));
        assert_eq!(reranker.top_n, Some(1000));
    }

    #[test]
    fn test_builder_with_max_chunks_none() {
        let reranker = CohereRerank::new().with_max_chunks_per_doc(None);
        assert!(reranker.max_chunks_per_doc.is_none());
    }

    #[test]
    fn test_builder_with_max_chunks_zero() {
        let reranker = CohereRerank::new().with_max_chunks_per_doc(Some(0));
        assert_eq!(reranker.max_chunks_per_doc, Some(0));
    }

    #[test]
    fn test_builder_api_key_string_ownership() {
        let api_key = String::from("owned-key");
        let reranker = CohereRerank::new().with_api_key(api_key);
        assert_eq!(reranker.api_key, "owned-key");
    }

    #[test]
    fn test_builder_model_string_ownership() {
        let model = String::from("custom-model");
        let reranker = CohereRerank::new().with_model(model);
        assert_eq!(reranker.model, "custom-model");
    }

    #[test]
    fn test_builder_api_key_special_chars() {
        let reranker = CohereRerank::new().with_api_key("key-with-dashes_and_underscores.dots");
        assert_eq!(reranker.api_key, "key-with-dashes_and_underscores.dots");
    }

    #[test]
    fn test_builder_chained() {
        let reranker = CohereRerank::new()
            .with_api_key("my-key")
            .with_model("rerank-multilingual-v3.0")
            .with_top_n(Some(10))
            .with_max_chunks_per_doc(Some(20));

        assert_eq!(reranker.api_key, "my-key");
        assert_eq!(reranker.model, "rerank-multilingual-v3.0");
        assert_eq!(reranker.top_n, Some(10));
        assert_eq!(reranker.max_chunks_per_doc, Some(20));
    }

    // ========== Clone trait tests ==========

    #[test]
    fn test_clone() {
        let reranker = CohereRerank::new()
            .with_api_key("clone-key")
            .with_model("rerank-multilingual-v3.0")
            .with_top_n(Some(7));

        let cloned = reranker.clone();

        assert_eq!(cloned.api_key, "clone-key");
        assert_eq!(cloned.model, "rerank-multilingual-v3.0");
        assert_eq!(cloned.top_n, Some(7));
    }

    // ========== RerankRequest serialization tests ==========

    #[test]
    fn test_rerank_request_serialization() {
        let request = RerankRequest {
            query: "What is AI?".to_string(),
            documents: vec!["Doc 1".to_string(), "Doc 2".to_string()],
            model: "rerank-english-v3.0".to_string(),
            top_n: Some(2),
            max_chunks_per_doc: None,
            return_documents: Some(false),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"query\":\"What is AI?\""));
        assert!(json.contains("\"documents\""));
        assert!(json.contains("\"Doc 1\""));
        assert!(json.contains("\"top_n\":2"));
        // None field should be skipped
        assert!(!json.contains("\"max_chunks_per_doc\""));
    }

    #[test]
    fn test_rerank_request_with_max_chunks() {
        let request = RerankRequest {
            query: "Test".to_string(),
            documents: vec!["Doc".to_string()],
            model: "rerank-english-v3.0".to_string(),
            top_n: None,
            max_chunks_per_doc: Some(10),
            return_documents: Some(true),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"max_chunks_per_doc\":10"));
        assert!(json.contains("\"return_documents\":true"));
    }

    #[test]
    fn test_rerank_request_minimal() {
        let request = RerankRequest {
            query: "Q".to_string(),
            documents: vec!["D".to_string()],
            model: "m".to_string(),
            top_n: None,
            max_chunks_per_doc: None,
            return_documents: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        // Only required fields should be present
        assert!(json.contains("\"query\":\"Q\""));
        assert!(json.contains("\"documents\""));
        assert!(json.contains("\"model\":\"m\""));
        assert!(!json.contains("\"top_n\""));
    }

    // ========== RerankResult tests ==========

    #[test]
    fn test_rerank_result_deserialization() {
        let json = r#"{"index": 2, "relevance_score": 0.95}"#;
        let result: RerankResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.index, 2);
        assert!((result.relevance_score - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_rerank_result_zero_score() {
        let json = r#"{"index": 0, "relevance_score": 0.0}"#;
        let result: RerankResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.index, 0);
        assert_eq!(result.relevance_score, 0.0);
    }

    #[test]
    fn test_rerank_result_high_index() {
        let json = r#"{"index": 999, "relevance_score": 0.5}"#;
        let result: RerankResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.index, 999);
    }

    #[test]
    fn test_rerank_result_clone() {
        let result = RerankResult {
            index: 5,
            relevance_score: 0.75,
        };
        let cloned = result.clone();
        assert_eq!(cloned.index, 5);
        assert_eq!(cloned.relevance_score, 0.75);
    }

    #[test]
    fn test_rerank_result_debug() {
        let result = RerankResult {
            index: 3,
            relevance_score: 0.85,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("3"));
        assert!(debug_str.contains("0.85"));
    }

    // ========== RerankResponse deserialization tests ==========

    #[test]
    fn test_rerank_response_deserialization() {
        let json = r#"{
            "results": [
                {"index": 0, "relevance_score": 0.9},
                {"index": 1, "relevance_score": 0.7}
            ]
        }"#;

        let response: RerankResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].index, 0);
        assert!((response.results[0].relevance_score - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_rerank_response_empty_results() {
        let json = r#"{"results": []}"#;
        let response: RerankResponse = serde_json::from_str(json).unwrap();
        assert!(response.results.is_empty());
    }

    #[test]
    fn test_rerank_response_single_result() {
        let json = r#"{"results": [{"index": 0, "relevance_score": 1.0}]}"#;
        let response: RerankResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.results.len(), 1);
    }

    // ========== Empty documents edge cases ==========

    #[tokio::test]
    async fn test_rerank_empty_documents() {
        let reranker = CohereRerank::new().with_api_key("test-key");
        let documents: Vec<Document> = vec![];
        let result = reranker.rerank(&documents, "query").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ========== Default values verification ==========

    #[test]
    fn test_default_values() {
        let reranker = CohereRerank::new();
        assert_eq!(reranker.model, "rerank-english-v3.0");
        assert_eq!(reranker.top_n, Some(3));
        assert!(reranker.max_chunks_per_doc.is_none());
    }

    // ========== Document content extraction tests ==========

    #[tokio::test]
    async fn test_empty_api_key_error_message() {
        let reranker = CohereRerank::new().with_api_key("");
        let documents = vec![Document::new("test content")];
        let result = reranker.rerank(&documents, "query").await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("COHERE_API_KEY"));
        assert!(err.contains("not set"));
    }

    #[test]
    fn test_document_with_metadata() {
        // Verify documents with metadata are handled correctly
        let mut doc = Document::new("Content with metadata");
        doc.metadata.insert("key".to_string(), "value".into());

        // Just verify the document is created correctly
        assert_eq!(doc.page_content, "Content with metadata");
        assert!(doc.metadata.contains_key("key"));
    }

    // ========== Builder default none values ==========

    #[test]
    fn test_builder_max_chunks_default_none() {
        let reranker = CohereRerank::new();
        assert!(reranker.max_chunks_per_doc.is_none());
    }
}
