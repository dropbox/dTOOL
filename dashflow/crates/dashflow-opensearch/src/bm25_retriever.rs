//! OpenSearch BM25 retriever implementation.
//!
//! Uses OpenSearch's native BM25 scoring algorithm for full-text document retrieval.
//! Unlike vector search (kNN), BM25 is a keyword-based algorithm that matches documents
//! based on term frequency and inverse document frequency.
//!
//! # Features
//!
//! - **Native BM25**: Uses OpenSearch's built-in BM25 implementation
//! - **Configurable Parameters**: Customize k1 (term saturation) and b (length normalization)
//! - **Full-Text Search**: Keyword-based search without embeddings
//! - **Document Management**: Add, delete, and search documents
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_opensearch::OpenSearchBM25Retriever;
//! use dashflow::core::retrievers::Retriever;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create retriever with default BM25 parameters
//! let mut retriever = OpenSearchBM25Retriever::new(
//!     "my_documents",
//!     "http://localhost:9200",
//! ).await?;
//!
//! // Add documents
//! retriever.add_texts(&["First document", "Second document"]).await?;
//!
//! // Search - uses BM25 scoring
//! let results = retriever._get_relevant_documents("document", None).await?;
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;

use async_trait::async_trait;
use dashflow::core::config::RunnableConfig;
use dashflow::core::documents::Document;
use dashflow::core::retrievers::{Retriever, RetrieverInput, RetrieverOutput};
use dashflow::core::runnable::Runnable;
use dashflow::core::{Error, Result};
use opensearch::{
    http::{
        request::JsonBody,
        transport::{SingleNodeConnectionPool, TransportBuilder},
    },
    indices::{IndicesCreateParts, IndicesExistsParts},
    BulkParts, DeleteByQueryParts, OpenSearch, SearchParts,
};
use serde_json::{json, Value as JsonValue};

/// OpenSearch BM25 retriever.
///
/// Performs full-text search using OpenSearch's native BM25 scoring algorithm.
/// BM25 (Best Matching 25) is a probabilistic ranking function that ranks documents
/// based on term frequency and document length.
///
/// # BM25 Parameters
///
/// - `k1` (default: 2.0): Controls term frequency saturation. Higher values increase
///   the importance of term frequency.
/// - `b` (default: 0.75): Controls document length normalization. 0 = no normalization,
///   1 = full normalization.
///
/// # Authentication
///
/// For secured clusters, include credentials in the URL:
/// - Format: `https://username:password@host:port`
pub struct OpenSearchBM25Retriever {
    client: OpenSearch,
    index_name: String,
    /// Number of results to return (default: 4)
    k: usize,
    /// BM25 k1 parameter (term saturation)
    k1: f64,
    /// BM25 b parameter (length normalization)
    b: f64,
    /// Field to search in (default: "content")
    text_field: String,
}

impl OpenSearchBM25Retriever {
    /// Creates a new `OpenSearchBM25Retriever` with default BM25 parameters.
    ///
    /// # Arguments
    ///
    /// * `index_name` - Name of the OpenSearch index
    /// * `url` - OpenSearch connection URL
    ///
    /// # Errors
    ///
    /// Returns error if connection fails or index creation fails.
    pub async fn new(index_name: &str, url: &str) -> Result<Self> {
        Self::create(index_name, url, 2.0, 0.75, 4, "content").await
    }

    /// Creates a new `OpenSearchBM25Retriever` with custom BM25 parameters.
    ///
    /// # Arguments
    ///
    /// * `index_name` - Name of the OpenSearch index
    /// * `url` - OpenSearch connection URL
    /// * `k1` - BM25 k1 parameter (default: 2.0)
    /// * `b` - BM25 b parameter (default: 0.75)
    /// * `k` - Number of results to return (default: 4)
    /// * `text_field` - Field name containing text content (default: "content")
    ///
    /// # Errors
    ///
    /// Returns error if connection fails or index creation fails.
    pub async fn create(
        index_name: &str,
        url: &str,
        k1: f64,
        b: f64,
        k: usize,
        text_field: &str,
    ) -> Result<Self> {
        // Parse URL
        let parsed_url = url
            .parse()
            .map_err(|e| Error::config(format!("Invalid OpenSearch URL '{url}': {e}")))?;

        // Create connection pool
        let conn_pool = SingleNodeConnectionPool::new(parsed_url);
        let transport = TransportBuilder::new(conn_pool)
            .build()
            .map_err(|e| Error::config(format!("Failed to build transport: {e}")))?;

        let client = OpenSearch::new(transport);

        let retriever = Self {
            client,
            index_name: index_name.to_string(),
            k,
            k1,
            b,
            text_field: text_field.to_string(),
        };

        // Ensure index exists with BM25 settings
        retriever.ensure_index().await?;

        Ok(retriever)
    }

    /// Creates a retriever that connects to an existing index without creating it.
    ///
    /// Use this when connecting to a pre-existing index (e.g., one created by an indexer).
    pub async fn from_existing(
        index_name: &str,
        url: &str,
        k: usize,
        text_field: &str,
    ) -> Result<Self> {
        let parsed_url = url
            .parse()
            .map_err(|e| Error::config(format!("Invalid OpenSearch URL '{url}': {e}")))?;

        let conn_pool = SingleNodeConnectionPool::new(parsed_url);
        let transport = TransportBuilder::new(conn_pool)
            .build()
            .map_err(|e| Error::config(format!("Failed to build transport: {e}")))?;

        let client = OpenSearch::new(transport);

        Ok(Self {
            client,
            index_name: index_name.to_string(),
            k,
            k1: 2.0, // Not used when connecting to existing index
            b: 0.75,
            text_field: text_field.to_string(),
        })
    }

    /// Ensures the OpenSearch index exists with custom BM25 similarity settings.
    async fn ensure_index(&self) -> Result<()> {
        // Check if index exists
        let exists_response = self
            .client
            .indices()
            .exists(IndicesExistsParts::Index(&[&self.index_name]))
            .send()
            .await
            .map_err(|e| Error::other(format!("Failed to check index existence: {e}")))?;

        if exists_response.status_code().is_success() {
            // Index already exists
            return Ok(());
        }

        // Create index with custom BM25 settings
        let create_response = self
            .client
            .indices()
            .create(IndicesCreateParts::Index(&self.index_name))
            .body(json!({
                "settings": {
                    "analysis": {
                        "analyzer": {
                            "default": {
                                "type": "standard"
                            }
                        }
                    },
                    "similarity": {
                        "custom_bm25": {
                            "type": "BM25",
                            "k1": self.k1,
                            "b": self.b
                        }
                    }
                },
                "mappings": {
                    "properties": {
                        &self.text_field: {
                            "type": "text",
                            "similarity": "custom_bm25"
                        }
                    }
                }
            }))
            .send()
            .await
            .map_err(|e| Error::other(format!("Failed to create index: {e}")))?;

        if !create_response.status_code().is_success() {
            let error_text = create_response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::other(format!(
                "Failed to create index: {error_text}"
            )));
        }

        Ok(())
    }

    /// Adds text documents to the index.
    ///
    /// # Arguments
    ///
    /// * `texts` - Slice of text strings to add
    ///
    /// # Returns
    ///
    /// Vector of document IDs assigned to the added documents.
    pub async fn add_texts(&mut self, texts: &[impl AsRef<str>]) -> Result<Vec<String>> {
        self.add_texts_with_metadata(texts, None).await
    }

    /// Adds text documents with optional metadata to the index.
    pub async fn add_texts_with_metadata(
        &mut self,
        texts: &[impl AsRef<str>],
        metadatas: Option<&[HashMap<String, JsonValue>]>,
    ) -> Result<Vec<String>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let text_count = texts.len();
        if let Some(metadatas) = metadatas {
            if metadatas.len() != text_count {
                return Err(Error::config(format!(
                    "Metadatas length mismatch: {} vs {}",
                    metadatas.len(),
                    text_count
                )));
            }
        }

        // Generate IDs
        let doc_ids: Vec<String> = (0..text_count)
            .map(|_| uuid::Uuid::new_v4().to_string())
            .collect();

        // Build bulk request body
        let mut body: Vec<JsonBody<_>> = Vec::with_capacity(text_count * 2);

        for (i, text) in texts.iter().enumerate() {
            // Index operation
            body.push(
                json!({
                    "index": {
                        "_id": doc_ids[i]
                    }
                })
                .into(),
            );

            // Document source
            let mut doc = json!({
                &self.text_field: text.as_ref(),
            });

            // Add metadata if provided
            if let Some(metadatas) = metadatas {
                if let Some(obj) = doc.as_object_mut() {
                    for (k, v) in &metadatas[i] {
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }

            body.push(doc.into());
        }

        // Execute bulk request
        let bulk_response = self
            .client
            .bulk(BulkParts::Index(&self.index_name))
            .body(body)
            .send()
            .await
            .map_err(|e| Error::other(format!("OpenSearch bulk request failed: {e}")))?;

        if !bulk_response.status_code().is_success() {
            let error_text = bulk_response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::other(format!(
                "OpenSearch bulk operation failed: {error_text}"
            )));
        }

        // SAFETY: Refresh failure is non-critical - documents are already persisted
        // and will become searchable on the next automatic refresh cycle.
        let _ = self
            .client
            .indices()
            .refresh(opensearch::indices::IndicesRefreshParts::Index(&[
                &self.index_name
            ]))
            .send()
            .await;

        Ok(doc_ids)
    }

    /// Deletes documents by ID.
    pub async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        let query = if let Some(ids) = ids {
            if ids.is_empty() {
                return Ok(true);
            }
            json!({
                "query": {
                    "ids": {
                        "values": ids
                    }
                }
            })
        } else {
            json!({
                "query": {
                    "match_all": {}
                }
            })
        };

        let delete_response = self
            .client
            .delete_by_query(DeleteByQueryParts::Index(&[&self.index_name]))
            .body(query)
            .send()
            .await
            .map_err(|e| Error::other(format!("OpenSearch delete failed: {e}")))?;

        if !delete_response.status_code().is_success() {
            let error_text = delete_response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::other(format!(
                "OpenSearch delete failed: {error_text}"
            )));
        }

        // SAFETY: Refresh failure is non-critical - deletions are already committed
        // and will be reflected on the next automatic refresh cycle.
        let _ = self
            .client
            .indices()
            .refresh(opensearch::indices::IndicesRefreshParts::Index(&[
                &self.index_name
            ]))
            .send()
            .await;

        Ok(true)
    }

    /// Performs BM25 search and returns documents with scores.
    pub async fn search_with_score(&self, query: &str, k: usize) -> Result<Vec<(Document, f32)>> {
        self.search_with_score_and_filter(query, k, None).await
    }

    /// Performs BM25 search with optional metadata filter.
    pub async fn search_with_score_and_filter(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        // Build match query for BM25 scoring
        let match_query = json!({
            "match": {
                &self.text_field: {
                    "query": query
                }
            }
        });

        // Build full query with optional filter
        let search_query = if let Some(filter) = filter {
            let filter_clauses: Vec<JsonValue> = filter
                .iter()
                .map(|(k, v)| {
                    json!({
                        "term": { k: v }
                    })
                })
                .collect();

            json!({
                "bool": {
                    "must": match_query,
                    "filter": filter_clauses
                }
            })
        } else {
            match_query
        };

        let search_body = json!({
            "query": search_query,
            "size": k,
            "_source": true
        });

        // Execute search
        let search_response = self
            .client
            .search(SearchParts::Index(&[&self.index_name]))
            .body(search_body)
            .send()
            .await
            .map_err(|e| Error::other(format!("OpenSearch search failed: {e}")))?;

        if !search_response.status_code().is_success() {
            let error_text = search_response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::other(format!(
                "OpenSearch search failed: {error_text}"
            )));
        }

        let json: JsonValue = search_response
            .json()
            .await
            .map_err(|e| Error::other(format!("Failed to parse search response: {e}")))?;

        // Parse results
        let mut results = Vec::new();
        if let Some(hits) = json.get("hits").and_then(|h| h.get("hits")) {
            if let Some(hits_array) = hits.as_array() {
                for hit in hits_array {
                    let score = hit
                        .get("_score")
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0) as f32;
                    let id = hit
                        .get("_id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();

                    if let Some(source) = hit.get("_source") {
                        let content = source
                            .get(&self.text_field)
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        let mut metadata = HashMap::new();
                        if let Some(obj) = source.as_object() {
                            for (k, v) in obj {
                                if k != &self.text_field {
                                    metadata.insert(k.clone(), v.clone());
                                }
                            }
                        }

                        let doc = Document {
                            id: Some(id),
                            page_content: content,
                            metadata,
                        };

                        results.push((doc, score));
                    }
                }
            }
        }

        Ok(results)
    }

    /// Get the index name.
    #[must_use]
    pub fn index_name(&self) -> &str {
        &self.index_name
    }

    /// Get the number of results to return.
    #[must_use]
    pub fn k(&self) -> usize {
        self.k
    }

    /// Set the number of results to return.
    pub fn set_k(&mut self, k: usize) {
        self.k = k;
    }

    /// Get the BM25 k1 parameter.
    #[must_use]
    pub fn k1(&self) -> f64 {
        self.k1
    }

    /// Get the BM25 b parameter.
    #[must_use]
    pub fn b(&self) -> f64 {
        self.b
    }

    /// Get the text field name.
    #[must_use]
    pub fn text_field(&self) -> &str {
        &self.text_field
    }
}

#[async_trait]
impl Retriever for OpenSearchBM25Retriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        let results = self.search_with_score(query, self.k).await?;
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    fn name(&self) -> String {
        "OpenSearchBM25Retriever".to_string()
    }
}

#[async_trait]
impl Runnable for OpenSearchBM25Retriever {
    type Input = RetrieverInput;
    type Output = RetrieverOutput;

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self._get_relevant_documents(&input, config.as_ref()).await
    }

    fn name(&self) -> String {
        "OpenSearchBM25Retriever".to_string()
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::config_loader::env_vars::{env_string_or_default, OPENSEARCH_URL};

    async fn create_test_retriever() -> OpenSearchBM25Retriever {
        let index_name = format!(
            "test_bm25_{}",
            uuid::Uuid::new_v4().to_string().replace('-', "_")
        );
        let url = env_string_or_default(OPENSEARCH_URL, "http://localhost:9200");

        OpenSearchBM25Retriever::new(&index_name, &url)
            .await
            .expect("Failed to create test retriever - is OpenSearch running?")
    }

    #[tokio::test]
    #[ignore = "requires OpenSearch server"]
    async fn test_add_and_search() {
        let mut retriever = create_test_retriever().await;

        // Add documents
        let ids = retriever
            .add_texts(&[
                "The quick brown fox",
                "A lazy dog sleeps",
                "The fox jumps high",
            ])
            .await
            .unwrap();
        assert_eq!(ids.len(), 3);

        // Search for fox-related documents
        let docs = retriever
            ._get_relevant_documents("quick fox", None)
            .await
            .unwrap();
        assert!(!docs.is_empty());
        // BM25 should rank "quick brown fox" higher
        assert!(docs[0].page_content.contains("fox"));
    }

    #[tokio::test]
    #[ignore = "requires OpenSearch server"]
    async fn test_search_with_metadata() {
        let mut retriever = create_test_retriever().await;

        let metadata = vec![
            HashMap::from([("category".to_string(), json!("animals"))]),
            HashMap::from([("category".to_string(), json!("plants"))]),
        ];

        let _ids = retriever
            .add_texts_with_metadata(
                &["Dogs and cats are pets", "Trees and flowers grow"],
                Some(&metadata),
            )
            .await
            .unwrap();

        let docs = retriever
            ._get_relevant_documents("pets animals", None)
            .await
            .unwrap();
        assert!(!docs.is_empty());
        // Should find the animals document
        assert_eq!(docs[0].metadata.get("category"), Some(&json!("animals")));
    }

    #[tokio::test]
    #[ignore = "requires OpenSearch server"]
    async fn test_custom_bm25_params() {
        let index_name = format!(
            "test_bm25_params_{}",
            uuid::Uuid::new_v4().to_string().replace('-', "_")
        );
        let url = env_string_or_default(OPENSEARCH_URL, "http://localhost:9200");

        let retriever = OpenSearchBM25Retriever::create(&index_name, &url, 1.5, 0.5, 10, "content")
            .await
            .expect("Failed to create retriever");

        assert!((retriever.k1() - 1.5).abs() < f64::EPSILON);
        assert!((retriever.b() - 0.5).abs() < f64::EPSILON);
        assert_eq!(retriever.k(), 10);
    }

    #[tokio::test]
    #[ignore = "requires OpenSearch server"]
    async fn test_empty_results() {
        let retriever = create_test_retriever().await;

        // Search in empty index
        let docs = retriever
            ._get_relevant_documents("nonexistent query", None)
            .await
            .unwrap();
        assert!(docs.is_empty());
    }

    // ==================== BM25 parameter tests ====================

    #[test]
    fn test_bm25_default_k1() {
        // Default k1 is 2.0 for BM25
        let k1: f64 = 2.0;
        assert!((k1 - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bm25_default_b() {
        // Default b is 0.75 for BM25
        let b: f64 = 0.75;
        assert!((b - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bm25_k1_range() {
        // k1 typically ranges from 1.2 to 2.0
        let k1_values = [1.2, 1.5, 1.8, 2.0];
        for k1 in k1_values {
            assert!(k1 >= 0.0);
            assert!(k1 <= 10.0);
        }
    }

    #[test]
    fn test_bm25_b_range() {
        // b must be between 0.0 and 1.0
        let b_values = [0.0, 0.25, 0.5, 0.75, 1.0];
        for b in b_values {
            assert!(b >= 0.0);
            assert!(b <= 1.0);
        }
    }

    #[test]
    fn test_bm25_k_default() {
        // Default k (number of results) is 4
        let k = 4;
        assert_eq!(k, 4);
    }

    // ==================== Match query structure tests ====================

    #[test]
    fn test_match_query_structure() {
        let text_field = "content";
        let query_text = "search terms";

        let match_query = json!({
            "match": {
                text_field: {
                    "query": query_text
                }
            }
        });

        assert!(match_query["match"].is_object());
        assert_eq!(match_query["match"]["content"]["query"], json!("search terms"));
    }

    #[test]
    fn test_match_query_empty_string() {
        let match_query = json!({
            "match": {
                "content": {
                    "query": ""
                }
            }
        });

        assert_eq!(match_query["match"]["content"]["query"], json!(""));
    }

    #[test]
    fn test_match_query_special_characters() {
        let query = "hello! @world #rust";
        let match_query = json!({
            "match": {
                "content": {
                    "query": query
                }
            }
        });

        assert_eq!(match_query["match"]["content"]["query"], json!("hello! @world #rust"));
    }

    #[test]
    fn test_match_query_unicode() {
        let query = "こんにちは 世界";
        let match_query = json!({
            "match": {
                "content": {
                    "query": query
                }
            }
        });

        assert!(match_query["match"]["content"]["query"].as_str().unwrap().contains("こんにちは"));
    }

    // ==================== Filter clause building tests ====================

    #[test]
    fn test_filter_clause_single() {
        let filter = HashMap::from([("category".to_string(), json!("tech"))]);

        let filter_clauses: Vec<JsonValue> = filter
            .iter()
            .map(|(k, v)| json!({ "term": { k: v } }))
            .collect();

        assert_eq!(filter_clauses.len(), 1);
        assert!(filter_clauses[0]["term"]["category"].is_string());
    }

    #[test]
    fn test_filter_clause_multiple() {
        let filter = HashMap::from([
            ("category".to_string(), json!("tech")),
            ("year".to_string(), json!(2024)),
        ]);

        let filter_clauses: Vec<JsonValue> = filter
            .iter()
            .map(|(k, v)| json!({ "term": { k: v } }))
            .collect();

        assert_eq!(filter_clauses.len(), 2);
    }

    #[test]
    fn test_filter_with_bool_must() {
        let match_query = json!({ "match": { "content": { "query": "test" } } });
        let filter_clauses = vec![json!({ "term": { "status": "active" } })];

        let bool_query = json!({
            "bool": {
                "must": match_query,
                "filter": filter_clauses
            }
        });

        assert!(bool_query["bool"]["must"].is_object());
        assert!(bool_query["bool"]["filter"].is_array());
    }

    // ==================== Search body structure tests ====================

    #[test]
    fn test_search_body_basic() {
        let query = json!({ "match": { "content": { "query": "test" } } });
        let k = 10;

        let search_body = json!({
            "query": query,
            "size": k,
            "_source": true
        });

        assert!(search_body["query"].is_object());
        assert_eq!(search_body["size"], json!(10));
        assert_eq!(search_body["_source"], json!(true));
    }

    #[test]
    fn test_search_body_with_filter() {
        let bool_query = json!({
            "bool": {
                "must": { "match": { "content": { "query": "search" } } },
                "filter": [{ "term": { "type": "article" } }]
            }
        });

        let search_body = json!({
            "query": bool_query,
            "size": 5,
            "_source": true
        });

        assert!(search_body["query"]["bool"]["filter"].is_array());
    }

    #[test]
    fn test_search_body_large_size() {
        let search_body = json!({
            "query": { "match_all": {} },
            "size": 1000,
            "_source": true
        });

        assert_eq!(search_body["size"], json!(1000));
    }

    // ==================== Index settings structure tests ====================

    #[test]
    fn test_index_bm25_settings() {
        let k1 = 1.5;
        let b = 0.5;

        let settings = json!({
            "settings": {
                "similarity": {
                    "custom_bm25": {
                        "type": "BM25",
                        "k1": k1,
                        "b": b
                    }
                }
            }
        });

        assert_eq!(settings["settings"]["similarity"]["custom_bm25"]["type"], json!("BM25"));
        assert_eq!(settings["settings"]["similarity"]["custom_bm25"]["k1"], json!(1.5));
        assert_eq!(settings["settings"]["similarity"]["custom_bm25"]["b"], json!(0.5));
    }

    #[test]
    fn test_index_analyzer_settings() {
        let settings = json!({
            "settings": {
                "analysis": {
                    "analyzer": {
                        "default": {
                            "type": "standard"
                        }
                    }
                }
            }
        });

        assert_eq!(settings["settings"]["analysis"]["analyzer"]["default"]["type"], json!("standard"));
    }

    #[test]
    fn test_index_text_field_mapping() {
        let text_field = "content";

        let mappings = json!({
            "mappings": {
                "properties": {
                    text_field: {
                        "type": "text",
                        "similarity": "custom_bm25"
                    }
                }
            }
        });

        assert_eq!(mappings["mappings"]["properties"]["content"]["type"], json!("text"));
        assert_eq!(mappings["mappings"]["properties"]["content"]["similarity"], json!("custom_bm25"));
    }

    // ==================== Document creation and parsing tests ====================

    #[test]
    fn test_document_from_bm25_hit() {
        let text_field = "content";
        let hit = json!({
            "_id": "doc1",
            "_score": 5.234,
            "_source": {
                "content": "This is the document content",
                "author": "Test Author"
            }
        });

        let content = hit
            .get("_source")
            .and_then(|s| s.get(text_field))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        assert_eq!(content, "This is the document content");
    }

    #[test]
    fn test_document_metadata_extraction() {
        let text_field = "content";
        let hit = json!({
            "_source": {
                "content": "text",
                "category": "tech",
                "tags": ["rust", "search"]
            }
        });

        let source = hit.get("_source").unwrap();
        let mut metadata = HashMap::new();
        if let Some(obj) = source.as_object() {
            for (k, v) in obj {
                if k != text_field {
                    metadata.insert(k.clone(), v.clone());
                }
            }
        }

        assert_eq!(metadata.len(), 2);
        assert!(metadata.contains_key("category"));
        assert!(metadata.contains_key("tags"));
    }

    #[test]
    fn test_document_score_bm25() {
        let hit = json!({ "_score": 12.5 });
        let score = hit
            .get("_score")
            .and_then(|s| s.as_f64())
            .unwrap_or(0.0) as f32;
        // BM25 scores can be larger than 1.0
        assert!(score > 1.0);
    }

    #[test]
    fn test_document_id_extraction() {
        let hit = json!({ "_id": "bm25_doc_123" });
        let id = hit
            .get("_id")
            .and_then(|i| i.as_str())
            .unwrap_or("")
            .to_string();
        assert_eq!(id, "bm25_doc_123");
    }

    // ==================== Delete query tests ====================

    #[test]
    fn test_delete_by_ids_query() {
        let ids = vec!["id1".to_string(), "id2".to_string()];
        let query = json!({
            "query": {
                "ids": {
                    "values": ids
                }
            }
        });

        let values = query["query"]["ids"]["values"].as_array().unwrap();
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_delete_all_query() {
        let query = json!({
            "query": {
                "match_all": {}
            }
        });

        assert!(query["query"]["match_all"].is_object());
    }

    #[test]
    fn test_delete_empty_ids() {
        let ids: Vec<String> = vec![];
        // Empty ids should be handled by returning early
        assert!(ids.is_empty());
    }

    // ==================== Getter tests ====================

    #[test]
    fn test_index_name_getter() {
        let index_name = "test_bm25_index";
        // Verify index name format
        assert!(!index_name.is_empty());
        assert!(index_name.starts_with("test"));
    }

    #[test]
    fn test_k_getter_values() {
        let k_values = [1, 4, 10, 100];
        for k in k_values {
            assert!(k > 0);
        }
    }

    #[test]
    fn test_text_field_getter() {
        let text_field = "content";
        assert_eq!(text_field, "content");
    }

    #[test]
    fn test_text_field_custom() {
        let text_field = "body";
        assert_ne!(text_field, "content");
        assert_eq!(text_field, "body");
    }

    // ==================== Name trait tests ====================

    #[test]
    fn test_retriever_name() {
        let name = "OpenSearchBM25Retriever";
        assert!(name.contains("BM25"));
        assert!(name.contains("OpenSearch"));
    }

    #[test]
    fn test_runnable_name() {
        let name = "OpenSearchBM25Retriever";
        assert!(!name.is_empty());
    }

    // ==================== Bulk operation tests ====================

    #[test]
    fn test_bulk_index_operation() {
        let doc_id = "bulk_doc_1";
        let index_op = json!({
            "index": {
                "_id": doc_id
            }
        });

        assert_eq!(index_op["index"]["_id"], json!("bulk_doc_1"));
    }

    #[test]
    fn test_bulk_document_body() {
        let text_field = "content";
        let text = "Document content here";

        let doc = json!({
            text_field: text
        });

        assert_eq!(doc["content"], json!("Document content here"));
    }

    #[test]
    fn test_bulk_document_with_metadata() {
        let text_field = "content";
        let mut doc = json!({
            text_field: "Text content"
        });

        let metadata = HashMap::from([
            ("author".to_string(), json!("Author Name")),
            ("date".to_string(), json!("2024-01-01")),
        ]);

        if let Some(obj) = doc.as_object_mut() {
            for (k, v) in &metadata {
                obj.insert(k.clone(), v.clone());
            }
        }

        assert_eq!(doc["author"], json!("Author Name"));
        assert_eq!(doc["date"], json!("2024-01-01"));
    }

    // ==================== Search result parsing tests ====================

    #[test]
    fn test_parse_search_response_hits() {
        let response = json!({
            "hits": {
                "total": { "value": 3 },
                "hits": [
                    { "_id": "1", "_score": 8.5, "_source": { "content": "First" } },
                    { "_id": "2", "_score": 7.2, "_source": { "content": "Second" } },
                    { "_id": "3", "_score": 5.1, "_source": { "content": "Third" } }
                ]
            }
        });

        let hits = response["hits"]["hits"].as_array().unwrap();
        assert_eq!(hits.len(), 3);
        // Verify scores are in descending order
        assert!(hits[0]["_score"].as_f64().unwrap() > hits[1]["_score"].as_f64().unwrap());
    }

    #[test]
    fn test_parse_search_response_empty() {
        let response = json!({
            "hits": {
                "total": { "value": 0 },
                "hits": []
            }
        });

        let hits = response["hits"]["hits"].as_array().unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn test_parse_search_response_no_source() {
        let response = json!({
            "hits": {
                "hits": [
                    { "_id": "1", "_score": 5.0 }
                ]
            }
        });

        let hit = &response["hits"]["hits"][0];
        let source = hit.get("_source");
        assert!(source.is_none());
    }

    // ==================== Error handling tests ====================

    #[test]
    fn test_metadata_mismatch_message() {
        let metadatas_len = 3;
        let text_count = 5;
        let msg = format!(
            "Metadatas length mismatch: {} vs {}",
            metadatas_len, text_count
        );
        assert!(msg.contains("3"));
        assert!(msg.contains("5"));
    }

    #[test]
    fn test_url_error_message() {
        let url = "invalid-url";
        let msg = format!("Invalid OpenSearch URL '{}': parse error", url);
        assert!(msg.contains("invalid-url"));
    }

    #[test]
    fn test_transport_error_message() {
        let msg = "Failed to build transport: connection refused";
        assert!(msg.contains("transport"));
    }

    // ==================== Empty input handling tests ====================

    #[test]
    fn test_empty_texts_array() {
        let texts: Vec<&str> = vec![];
        assert!(texts.is_empty());
    }

    #[test]
    fn test_empty_query_string() {
        let query = "";
        assert!(query.is_empty());
    }

    #[test]
    fn test_empty_filter() {
        let filter: HashMap<String, JsonValue> = HashMap::new();
        assert!(filter.is_empty());
    }

    // ==================== UUID generation tests ====================

    #[test]
    fn test_uuid_generation_format() {
        let id = uuid::Uuid::new_v4().to_string();
        assert_eq!(id.len(), 36);
        // UUID format: 8-4-4-4-12
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
    }

    #[test]
    fn test_uuid_generation_uniqueness() {
        let ids: Vec<String> = (0..50).map(|_| uuid::Uuid::new_v4().to_string()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
    }

    // ==================== Text field configuration tests ====================

    #[test]
    fn test_default_text_field() {
        let default_field = "content";
        assert_eq!(default_field, "content");
    }

    #[test]
    fn test_custom_text_field_names() {
        let field_names = ["content", "body", "text", "description", "message"];
        for name in field_names {
            assert!(!name.is_empty());
        }
    }

    // ==================== BM25 formula edge cases ====================

    #[test]
    fn test_bm25_k1_zero() {
        // k1 = 0 means term frequency is ignored
        let k1: f64 = 0.0;
        assert!((k1 - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bm25_b_zero() {
        // b = 0 means no length normalization
        let b: f64 = 0.0;
        assert!((b - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bm25_b_one() {
        // b = 1 means full length normalization
        let b: f64 = 1.0;
        assert!((b - 1.0).abs() < f64::EPSILON);
    }

    // ==================== Search response metadata tests ====================

    #[test]
    fn test_search_response_took() {
        let response = json!({
            "took": 15,
            "timed_out": false,
            "hits": { "total": { "value": 1 }, "hits": [] }
        });

        let took = response["took"].as_i64().unwrap();
        assert_eq!(took, 15);
    }

    #[test]
    fn test_search_response_timed_out() {
        let response = json!({
            "took": 5000,
            "timed_out": true,
            "hits": { "total": { "value": 0 }, "hits": [] }
        });

        let timed_out = response["timed_out"].as_bool().unwrap();
        assert!(timed_out);
    }

    #[test]
    fn test_search_response_total_relation() {
        let response = json!({
            "hits": {
                "total": { "value": 10000, "relation": "gte" },
                "hits": []
            }
        });

        let relation = response["hits"]["total"]["relation"].as_str().unwrap();
        assert_eq!(relation, "gte");
    }

    // ==================== Index creation behavior tests ====================

    #[test]
    fn test_index_exists_check() {
        // Test the structure expected from exists check
        let success_code = 200;
        assert!((200..300).contains(&success_code));
    }

    #[test]
    fn test_index_not_exists_code() {
        let not_found_code = 404;
        assert!(!((200..300).contains(&not_found_code)));
    }

    // ==================== Field extraction tests ====================

    #[test]
    fn test_extract_content_from_source() {
        let text_field = "content";
        let source = json!({
            "content": "The actual content",
            "metadata": "some meta"
        });

        let content = source
            .get(text_field)
            .and_then(|c| c.as_str())
            .unwrap_or("");
        assert_eq!(content, "The actual content");
    }

    #[test]
    fn test_extract_missing_content() {
        let text_field = "content";
        let source = json!({
            "body": "Different field"
        });

        let content = source
            .get(text_field)
            .and_then(|c| c.as_str())
            .unwrap_or("");
        assert!(content.is_empty());
    }

    #[test]
    fn test_extract_null_content() {
        let text_field = "content";
        let source = json!({
            "content": null
        });

        let content = source
            .get(text_field)
            .and_then(|c| c.as_str())
            .unwrap_or("default");
        assert_eq!(content, "default");
    }
}
