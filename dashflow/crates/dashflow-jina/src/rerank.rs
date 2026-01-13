use async_trait::async_trait;
use dashflow::core::config_loader::env_vars::{env_string, JINA_API_KEY};
use dashflow::core::documents::{Document, DocumentCompressor};
use dashflow::core::error::Error;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const JINA_API_URL: &str = "https://api.jina.ai/v1/rerank";
const DEFAULT_MODEL: &str = "jina-reranker-v1-base-en";

/// Response from Jina Rerank API
#[derive(Debug, Deserialize)]
struct JinaRerankResponse {
    results: Vec<JinaRerankResult>,
}

/// Individual result from Jina Rerank API
#[derive(Debug, Deserialize)]
struct JinaRerankResult {
    index: usize,
    relevance_score: f64,
}

/// Request body for Jina Rerank API
#[derive(Debug, Serialize)]
struct JinaRerankRequest {
    query: String,
    documents: Vec<String>,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_n: Option<usize>,
}

/// Document compressor using Jina's Rerank API.
///
/// Jina Rerank uses specialized reranking models to reorder documents
/// by relevance to a query. This is useful for improving retrieval quality
/// in RAG pipelines.
///
/// # Models
/// - `jina-reranker-v1-base-en` (default): Base English reranker
/// - `jina-reranker-v1-turbo-en`: Faster English reranker
/// - `jina-reranker-v1-tiny-en`: Smallest/fastest English reranker
///
/// # Example
/// ```no_run
/// use dashflow_jina::rerank::JinaRerank;
/// use dashflow::core::documents::{Document, DocumentCompressor};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Set JINA_API_KEY environment variable
/// std::env::set_var("JINA_API_KEY", "your-api-key");
///
/// let reranker = JinaRerank::new()?;
/// let docs = vec![
///     Document::new("Paris is the capital of France."),
///     Document::new("Berlin is the capital of Germany."),
///     Document::new("The sky is blue."),
/// ];
///
/// let reranked = reranker.compress_documents(docs, "What is the capital of France?", None).await?;
/// // Returns: [Paris doc, Berlin doc] (sky doc filtered out by top_n=2)
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct JinaRerank {
    client: Client,
    api_key: String,
    model: String,
    top_n: Option<usize>,
    user_agent: String,
}

impl JinaRerank {
    /// Create a new `JinaRerank` compressor.
    ///
    /// Reads API key from `JINA_API_KEY` environment variable.
    ///
    /// # Errors
    /// Returns error if `JINA_API_KEY` is not set.
    pub fn new() -> Result<Self, Error> {
        Self::builder().build()
    }

    /// Create a builder for `JinaRerank`.
    #[must_use]
    pub fn builder() -> JinaRerankBuilder {
        JinaRerankBuilder::default()
    }

    /// Rerank documents and return results with relevance scores.
    ///
    /// This is the core reranking method that calls the Jina API.
    ///
    /// # Arguments
    /// * `documents` - Documents to rerank (can be strings or Documents)
    /// * `query` - Query to rerank documents against
    ///
    /// # Returns
    /// Vec of (index, `relevance_score`) tuples, ordered by relevance
    async fn rerank_internal(
        &self,
        documents: &[String],
        query: &str,
    ) -> Result<Vec<JinaRerankResult>, Error> {
        if documents.is_empty() {
            return Ok(vec![]);
        }

        let request = JinaRerankRequest {
            query: query.to_string(),
            documents: documents.to_vec(),
            model: self.model.clone(),
            top_n: self.top_n,
        };

        let response = self
            .client
            .post(JINA_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept-Encoding", "identity")
            .header("Content-Type", "application/json")
            .header("User-Agent", &self.user_agent)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Network(format!("Jina API request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::Api(format!(
                "Jina API error ({status}): {error_text}"
            )));
        }

        let rerank_response: JinaRerankResponse = response
            .json()
            .await
            .map_err(|e| Error::ApiFormat(format!("Failed to parse Jina API response: {e}")))?;

        Ok(rerank_response.results)
    }
}

#[async_trait]
impl DocumentCompressor for JinaRerank {
    /// Compress documents using Jina's reranking API.
    ///
    /// Documents are reordered by relevance to the query, with the most
    /// relevant documents first. Only the top N documents are returned
    /// (if `top_n` is set), with relevance scores added to metadata.
    ///
    /// # Metadata
    /// Adds `relevance_score` to each document's metadata.
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
        _config: Option<&dashflow::core::config::RunnableConfig>,
    ) -> Result<Vec<Document>, Error> {
        let doc_texts: Vec<String> = documents
            .iter()
            .map(|doc| doc.page_content.clone())
            .collect();

        let results = self.rerank_internal(&doc_texts, query).await?;

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

/// Builder for `JinaRerank`
pub struct JinaRerankBuilder {
    api_key: Option<String>,
    model: Option<String>,
    top_n: Option<usize>,
    user_agent: Option<String>,
}

impl Default for JinaRerankBuilder {
    fn default() -> Self {
        Self {
            api_key: None,
            model: None,
            top_n: Some(3), // Default to top 3 results
            user_agent: Some("dashflow".to_string()),
        }
    }
}

impl JinaRerankBuilder {
    /// Set the Jina API key.
    ///
    /// If not set, will read from `JINA_API_KEY` environment variable.
    #[must_use]
    pub fn api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Set the reranking model.
    ///
    /// Defaults to `jina-reranker-v1-base-en`.
    ///
    /// Available models:
    /// - `jina-reranker-v1-base-en` (default)
    /// - `jina-reranker-v1-turbo-en`
    /// - `jina-reranker-v1-tiny-en`
    #[must_use]
    pub fn model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    /// Set the number of top documents to return.
    ///
    /// Defaults to 3. Set to None to return all documents.
    #[must_use]
    pub fn top_n(mut self, top_n: Option<usize>) -> Self {
        self.top_n = top_n;
        self
    }

    /// Set the user agent for API requests.
    ///
    /// Defaults to "dashflow".
    #[must_use]
    pub fn user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = Some(user_agent);
        self
    }

    /// Build the `JinaRerank` compressor.
    ///
    /// # Errors
    /// Returns error if API key is not provided and `JINA_API_KEY` env var is not set.
    pub fn build(self) -> Result<JinaRerank, Error> {
        let api_key = match self.api_key {
            Some(key) => key,
            None => env_string(JINA_API_KEY).ok_or_else(|| {
                Error::InvalidInput(format!(
                    "{JINA_API_KEY} environment variable must be set. \
                     Get your API key from https://jina.ai/"
                ))
            })?,
        };

        let model = self.model.unwrap_or_else(|| DEFAULT_MODEL.to_string());
        let user_agent = self.user_agent.unwrap_or_else(|| "dashflow".to_string());

        Ok(JinaRerank {
            client: Client::new(),
            api_key,
            model,
            top_n: self.top_n,
            user_agent,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    // Mutex to serialize env var access across tests.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // ==================== Builder Tests ====================

    #[test]
    fn test_jina_rerank_builder() {
        let reranker = JinaRerank::builder()
            .api_key("test-key".to_string())
            .model("jina-reranker-v1-turbo-en".to_string())
            .top_n(Some(5))
            .build();

        assert!(reranker.is_ok());
        let reranker = reranker.unwrap();
        assert_eq!(reranker.api_key, "test-key");
        assert_eq!(reranker.model, "jina-reranker-v1-turbo-en");
        assert_eq!(reranker.top_n, Some(5));
    }

    #[test]
    fn test_jina_rerank_builder_defaults() {
        // Don't use env var - use builder with explicit key
        let reranker = JinaRerank::builder()
            .api_key("test-key".to_string())
            .build();

        assert!(reranker.is_ok());
        let reranker = reranker.unwrap();
        assert_eq!(reranker.api_key, "test-key");
        assert_eq!(reranker.model, DEFAULT_MODEL);
        assert_eq!(reranker.top_n, Some(3));
    }

    #[test]
    fn test_jina_rerank_builder_default_struct() {
        let builder = JinaRerankBuilder::default();
        assert!(builder.api_key.is_none());
        assert!(builder.model.is_none());
        assert_eq!(builder.top_n, Some(3));
        assert_eq!(builder.user_agent, Some("dashflow".to_string()));
    }

    #[test]
    fn test_builder_api_key() {
        let reranker = JinaRerank::builder()
            .api_key("my-api-key-123".to_string())
            .build()
            .unwrap();
        assert_eq!(reranker.api_key, "my-api-key-123");
    }

    #[test]
    fn test_builder_model_turbo() {
        let reranker = JinaRerank::builder()
            .api_key("key".to_string())
            .model("jina-reranker-v1-turbo-en".to_string())
            .build()
            .unwrap();
        assert_eq!(reranker.model, "jina-reranker-v1-turbo-en");
    }

    #[test]
    fn test_builder_model_tiny() {
        let reranker = JinaRerank::builder()
            .api_key("key".to_string())
            .model("jina-reranker-v1-tiny-en".to_string())
            .build()
            .unwrap();
        assert_eq!(reranker.model, "jina-reranker-v1-tiny-en");
    }

    #[test]
    fn test_builder_model_base() {
        let reranker = JinaRerank::builder()
            .api_key("key".to_string())
            .model("jina-reranker-v1-base-en".to_string())
            .build()
            .unwrap();
        assert_eq!(reranker.model, "jina-reranker-v1-base-en");
    }

    #[test]
    fn test_builder_top_n_some() {
        let reranker = JinaRerank::builder()
            .api_key("key".to_string())
            .top_n(Some(10))
            .build()
            .unwrap();
        assert_eq!(reranker.top_n, Some(10));
    }

    #[test]
    fn test_builder_top_n_none() {
        let reranker = JinaRerank::builder()
            .api_key("key".to_string())
            .top_n(None)
            .build()
            .unwrap();
        assert!(reranker.top_n.is_none());
    }

    #[test]
    fn test_builder_top_n_one() {
        let reranker = JinaRerank::builder()
            .api_key("key".to_string())
            .top_n(Some(1))
            .build()
            .unwrap();
        assert_eq!(reranker.top_n, Some(1));
    }

    #[test]
    fn test_builder_user_agent() {
        let reranker = JinaRerank::builder()
            .api_key("key".to_string())
            .user_agent("custom-agent/1.0".to_string())
            .build()
            .unwrap();
        assert_eq!(reranker.user_agent, "custom-agent/1.0");
    }

    #[test]
    fn test_builder_user_agent_default() {
        let reranker = JinaRerank::builder()
            .api_key("key".to_string())
            .build()
            .unwrap();
        assert_eq!(reranker.user_agent, "dashflow");
    }

    #[test]
    fn test_builder_all_options() {
        let reranker = JinaRerank::builder()
            .api_key("full-key".to_string())
            .model("jina-reranker-v1-turbo-en".to_string())
            .top_n(Some(7))
            .user_agent("my-app/2.0".to_string())
            .build()
            .unwrap();

        assert_eq!(reranker.api_key, "full-key");
        assert_eq!(reranker.model, "jina-reranker-v1-turbo-en");
        assert_eq!(reranker.top_n, Some(7));
        assert_eq!(reranker.user_agent, "my-app/2.0");
    }

    // ==================== Constructor Tests ====================

    #[test]
    fn test_jina_rerank_no_api_key() {
        let _guard = ENV_MUTEX.lock().unwrap();
        env::remove_var("JINA_API_KEY");
        let result = JinaRerank::new();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("JINA_API_KEY environment variable must be set"));
    }

    #[test]
    fn test_jina_rerank_new_with_env() {
        let _guard = ENV_MUTEX.lock().unwrap();
        env::set_var("JINA_API_KEY", "env-jina-key");
        let result = JinaRerank::new();
        assert!(result.is_ok());
        let reranker = result.unwrap();
        assert_eq!(reranker.api_key, "env-jina-key");
        env::remove_var("JINA_API_KEY");
    }

    #[test]
    fn test_jina_rerank_builder_returns_builder() {
        let builder = JinaRerank::builder();
        // Verify we can chain methods
        let reranker = builder
            .api_key("key".to_string())
            .model("test-model".to_string())
            .build();
        assert!(reranker.is_ok());
    }

    // ==================== Empty Input Tests ====================

    #[tokio::test]
    async fn test_empty_documents() {
        // Use builder to avoid environment variable race conditions in parallel tests
        let reranker = JinaRerank::builder()
            .api_key("test-key".to_string())
            .build()
            .unwrap();
        let docs = vec![];
        let result = reranker
            .compress_documents(docs, "test query", None)
            .await
            .unwrap();
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_rerank_internal_empty_documents() {
        let reranker = JinaRerank::builder()
            .api_key("test-key".to_string())
            .build()
            .unwrap();
        let result = reranker.rerank_internal(&[], "query").await.unwrap();
        assert_eq!(result.len(), 0);
    }

    // ==================== Serialization Tests ====================

    #[test]
    fn test_jina_rerank_request_serialization() {
        let request = JinaRerankRequest {
            query: "What is the capital?".to_string(),
            documents: vec!["Paris is in France.".to_string(), "Berlin is in Germany.".to_string()],
            model: "jina-reranker-v1-base-en".to_string(),
            top_n: Some(2),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"query\":\"What is the capital?\""));
        assert!(json.contains("\"model\":\"jina-reranker-v1-base-en\""));
        assert!(json.contains("\"top_n\":2"));
        assert!(json.contains("Paris is in France."));
        assert!(json.contains("Berlin is in Germany."));
    }

    #[test]
    fn test_jina_rerank_request_without_top_n() {
        let request = JinaRerankRequest {
            query: "test".to_string(),
            documents: vec!["doc".to_string()],
            model: "model".to_string(),
            top_n: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        // top_n should be skipped when None
        assert!(!json.contains("\"top_n\""));
    }

    #[test]
    fn test_jina_rerank_request_many_documents() {
        let docs: Vec<String> = (0..10).map(|i| format!("Document {}", i)).collect();
        let request = JinaRerankRequest {
            query: "query".to_string(),
            documents: docs.clone(),
            model: "model".to_string(),
            top_n: Some(5),
        };
        let json = serde_json::to_string(&request).unwrap();
        for i in 0..10 {
            assert!(json.contains(&format!("Document {}", i)));
        }
    }

    #[test]
    fn test_jina_rerank_response_deserialization() {
        let json = r#"{
            "results": [
                {"index": 0, "relevance_score": 0.95},
                {"index": 2, "relevance_score": 0.80},
                {"index": 1, "relevance_score": 0.50}
            ]
        }"#;
        let response: JinaRerankResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.results.len(), 3);
        assert_eq!(response.results[0].index, 0);
        assert!((response.results[0].relevance_score - 0.95).abs() < 0.001);
        assert_eq!(response.results[1].index, 2);
        assert!((response.results[1].relevance_score - 0.80).abs() < 0.001);
        assert_eq!(response.results[2].index, 1);
        assert!((response.results[2].relevance_score - 0.50).abs() < 0.001);
    }

    #[test]
    fn test_jina_rerank_response_empty() {
        let json = r#"{"results": []}"#;
        let response: JinaRerankResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.results.len(), 0);
    }

    #[test]
    fn test_jina_rerank_response_single_result() {
        let json = r#"{"results": [{"index": 5, "relevance_score": 0.123}]}"#;
        let response: JinaRerankResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].index, 5);
        assert!((response.results[0].relevance_score - 0.123).abs() < 0.001);
    }

    #[test]
    fn test_jina_rerank_result_debug() {
        let result = JinaRerankResult {
            index: 3,
            relevance_score: 0.75,
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("JinaRerankResult"));
        assert!(debug.contains("index: 3"));
        assert!(debug.contains("0.75"));
    }

    // ==================== Constants Tests ====================

    #[test]
    fn test_jina_api_url_constant() {
        assert_eq!(JINA_API_URL, "https://api.jina.ai/v1/rerank");
    }

    #[test]
    fn test_default_model_constant() {
        assert_eq!(DEFAULT_MODEL, "jina-reranker-v1-base-en");
    }

    // ==================== JinaRerank Debug Test ====================

    #[test]
    fn test_jina_rerank_debug() {
        let reranker = JinaRerank::builder()
            .api_key("key".to_string())
            .build()
            .unwrap();
        let debug = format!("{:?}", reranker);
        assert!(debug.contains("JinaRerank"));
    }

    // ==================== Integration Tests ====================

    #[tokio::test]
    #[ignore = "requires JINA_API_KEY"]
    async fn test_jina_rerank_integration() {
        // This test requires a real Jina API key
        // Run with: JINA_API_KEY=your-key cargo test test_jina_rerank_integration -- --ignored
        let reranker = JinaRerank::new().unwrap();

        let docs = vec![
            Document::new("Paris is the capital of France."),
            Document::new("Berlin is the capital of Germany."),
            Document::new("The sky is blue."),
            Document::new("London is the capital of the United Kingdom."),
        ];

        let result = reranker
            .compress_documents(docs, "What is the capital of France?", None)
            .await
            .unwrap();

        // Should return top 3 documents (default top_n=3)
        assert_eq!(result.len(), 3);

        // First document should be about Paris (most relevant)
        assert!(result[0].page_content.contains("Paris"));

        // All documents should have relevance_score metadata
        for doc in &result {
            assert!(doc.metadata.contains_key("relevance_score"));
        }
    }

    #[tokio::test]
    #[ignore = "requires JINA_API_KEY"]
    async fn test_jina_rerank_integration_top_n_1() {
        let reranker = JinaRerank::builder()
            .top_n(Some(1))
            .build()
            .unwrap();

        let docs = vec![
            Document::new("Paris is the capital of France."),
            Document::new("Berlin is the capital of Germany."),
        ];

        let result = reranker
            .compress_documents(docs, "What is the capital of France?", None)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert!(result[0].page_content.contains("Paris"));
    }

    #[tokio::test]
    #[ignore = "requires JINA_API_KEY"]
    async fn test_jina_rerank_integration_all_results() {
        let reranker = JinaRerank::builder()
            .top_n(None) // Return all documents
            .build()
            .unwrap();

        let docs = vec![
            Document::new("Paris is the capital of France."),
            Document::new("Berlin is the capital of Germany."),
            Document::new("The sky is blue."),
        ];

        let result = reranker
            .compress_documents(docs, "What is the capital of France?", None)
            .await
            .unwrap();

        // Should return all 3 documents when top_n is None
        assert_eq!(result.len(), 3);
    }
}
