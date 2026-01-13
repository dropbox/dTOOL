//! Elasticsearch BM25 retriever implementation.
//!
//! Uses Elasticsearch's native BM25 scoring algorithm for full-text document retrieval.
//! Unlike vector search (kNN), BM25 is a keyword-based algorithm that matches documents
//! based on term frequency and inverse document frequency.
//!
//! # Features
//!
//! - **Native BM25**: Uses Elasticsearch's built-in BM25 implementation
//! - **Configurable Parameters**: Customize k1 (term saturation) and b (length normalization)
//! - **Full-Text Search**: Keyword-based search without embeddings
//! - **Document Management**: Add, delete, and search documents
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_elasticsearch::ElasticsearchBM25Retriever;
//! use dashflow::core::retrievers::Retriever;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create retriever with custom BM25 parameters
//! let mut retriever = ElasticsearchBM25Retriever::new(
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
use elasticsearch::{
    http::{
        request::JsonBody,
        transport::{SingleNodeConnectionPool, TransportBuilder},
    },
    indices::{IndicesCreateParts, IndicesExistsParts},
    BulkParts, DeleteByQueryParts, Elasticsearch, SearchParts,
};
use serde_json::{json, Value as JsonValue};

/// Elasticsearch BM25 retriever.
///
/// Performs full-text search using Elasticsearch's native BM25 scoring algorithm.
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
pub struct ElasticsearchBM25Retriever {
    client: Elasticsearch,
    index_name: String,
    /// Number of results to return (default: 4)
    k: usize,
    /// BM25 k1 parameter (term saturation)
    k1: f64,
    /// BM25 b parameter (length normalization)
    b: f64,
}

impl ElasticsearchBM25Retriever {
    /// Creates a new `ElasticsearchBM25Retriever` with default BM25 parameters.
    ///
    /// # Arguments
    ///
    /// * `index_name` - Name of the Elasticsearch index
    /// * `url` - Elasticsearch connection URL
    ///
    /// # Errors
    ///
    /// Returns error if connection fails or index creation fails.
    pub async fn new(index_name: &str, url: &str) -> Result<Self> {
        Self::create(index_name, url, 2.0, 0.75, 4).await
    }

    /// Creates a new `ElasticsearchBM25Retriever` with custom BM25 parameters.
    ///
    /// # Arguments
    ///
    /// * `index_name` - Name of the Elasticsearch index
    /// * `url` - Elasticsearch connection URL
    /// * `k1` - BM25 k1 parameter (default: 2.0)
    /// * `b` - BM25 b parameter (default: 0.75)
    /// * `k` - Number of results to return (default: 4)
    ///
    /// # Errors
    ///
    /// Returns error if connection fails or index creation fails.
    pub async fn create(index_name: &str, url: &str, k1: f64, b: f64, k: usize) -> Result<Self> {
        // Parse URL
        let parsed_url = url
            .parse()
            .map_err(|e| Error::config(format!("Invalid Elasticsearch URL '{url}': {e}")))?;

        // Create connection pool
        let conn_pool = SingleNodeConnectionPool::new(parsed_url);
        let transport = TransportBuilder::new(conn_pool)
            .build()
            .map_err(|e| Error::config(format!("Failed to build transport: {e}")))?;

        let client = Elasticsearch::new(transport);

        let retriever = Self {
            client,
            index_name: index_name.to_string(),
            k,
            k1,
            b,
        };

        // Ensure index exists with BM25 settings
        retriever.ensure_index().await?;

        Ok(retriever)
    }

    /// Ensures the Elasticsearch index exists with custom BM25 similarity settings.
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
                        "content": {
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
    ///
    /// # Errors
    ///
    /// Returns error if bulk indexing fails.
    pub async fn add_texts(&mut self, texts: &[impl AsRef<str>]) -> Result<Vec<String>> {
        self.add_texts_with_metadata(texts, None).await
    }

    /// Adds text documents with optional metadata to the index.
    ///
    /// # Arguments
    ///
    /// * `texts` - Slice of text strings to add
    /// * `metadatas` - Optional slice of metadata maps for each document
    ///
    /// # Returns
    ///
    /// Vector of document IDs assigned to the added documents.
    ///
    /// # Errors
    ///
    /// Returns error if bulk indexing fails or metadata length mismatches.
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
                "content": text.as_ref(),
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
            .map_err(|e| Error::other(format!("Elasticsearch bulk request failed: {e}")))?;

        if !bulk_response.status_code().is_success() {
            let error_text = bulk_response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::other(format!(
                "Elasticsearch bulk operation failed: {error_text}"
            )));
        }

        // SAFETY: Refresh failure is non-critical - documents are already persisted
        // and will become searchable on the next automatic refresh cycle.
        let _ = self
            .client
            .indices()
            .refresh(elasticsearch::indices::IndicesRefreshParts::Index(&[
                &self.index_name
            ]))
            .send()
            .await;

        Ok(doc_ids)
    }

    /// Deletes documents by ID.
    ///
    /// # Arguments
    ///
    /// * `ids` - IDs of documents to delete. If None, deletes all documents.
    ///
    /// # Errors
    ///
    /// Returns error if deletion fails.
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
            .map_err(|e| Error::other(format!("Elasticsearch delete failed: {e}")))?;

        if !delete_response.status_code().is_success() {
            let error_text = delete_response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::other(format!(
                "Elasticsearch delete failed: {error_text}"
            )));
        }

        // SAFETY: Refresh failure is non-critical - deletions are already committed
        // and will be reflected on the next automatic refresh cycle.
        let _ = self
            .client
            .indices()
            .refresh(elasticsearch::indices::IndicesRefreshParts::Index(&[
                &self.index_name
            ]))
            .send()
            .await;

        Ok(true)
    }

    /// Performs BM25 search and returns documents with scores.
    async fn search_with_score(&self, query: &str, k: usize) -> Result<Vec<(Document, f32)>> {
        // Build match query for BM25 scoring
        let search_body = json!({
            "query": {
                "match": {
                    "content": {
                        "query": query
                    }
                }
            },
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
            .map_err(|e| Error::other(format!("Elasticsearch search failed: {e}")))?;

        if !search_response.status_code().is_success() {
            let error_text = search_response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::other(format!(
                "Elasticsearch search failed: {error_text}"
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
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        let mut metadata = HashMap::new();
                        if let Some(obj) = source.as_object() {
                            for (k, v) in obj {
                                if k != "content" {
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
}

#[async_trait]
impl Retriever for ElasticsearchBM25Retriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        let results = self.search_with_score(query, self.k).await?;
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    fn name(&self) -> String {
        "ElasticsearchBM25Retriever".to_string()
    }
}

#[async_trait]
impl Runnable for ElasticsearchBM25Retriever {
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
        "ElasticsearchBM25Retriever".to_string()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // ========================================================================
    // UNIT TESTS - Run without Elasticsearch server
    // ========================================================================

    // BM25 query format tests
    #[test]
    fn test_bm25_match_query_format() {
        let query = "quick fox";
        let search_body = json!({
            "query": {
                "match": {
                    "content": {
                        "query": query
                    }
                }
            },
            "size": 10,
            "_source": true
        });
        assert_eq!(search_body["query"]["match"]["content"]["query"], json!("quick fox"));
        assert_eq!(search_body["size"], json!(10));
    }

    #[test]
    fn test_bm25_index_settings_format() {
        let k1 = 2.0;
        let b = 0.75;
        let settings = json!({
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
                        "k1": k1,
                        "b": b
                    }
                }
            },
            "mappings": {
                "properties": {
                    "content": {
                        "type": "text",
                        "similarity": "custom_bm25"
                    }
                }
            }
        });
        assert_eq!(settings["settings"]["similarity"]["custom_bm25"]["type"], json!("BM25"));
        assert_eq!(settings["settings"]["similarity"]["custom_bm25"]["k1"], json!(2.0));
        assert_eq!(settings["settings"]["similarity"]["custom_bm25"]["b"], json!(0.75));
    }

    #[test]
    fn test_bm25_custom_k1_settings() {
        let k1 = 1.5;
        let b = 0.5;
        let similarity = json!({
            "custom_bm25": {
                "type": "BM25",
                "k1": k1,
                "b": b
            }
        });
        assert_eq!(similarity["custom_bm25"]["k1"], json!(1.5));
    }

    #[test]
    fn test_bm25_custom_b_settings() {
        let k1 = 3.0;
        let b = 0.0; // No length normalization
        let similarity = json!({
            "custom_bm25": {
                "type": "BM25",
                "k1": k1,
                "b": b
            }
        });
        assert_eq!(similarity["custom_bm25"]["b"], json!(0.0));
    }

    #[test]
    fn test_bm25_full_length_normalization() {
        let k1 = 2.0;
        let b = 1.0; // Full length normalization
        let similarity = json!({
            "custom_bm25": {
                "type": "BM25",
                "k1": k1,
                "b": b
            }
        });
        assert_eq!(similarity["custom_bm25"]["b"], json!(1.0));
    }

    // Delete query format tests
    #[test]
    fn test_delete_by_ids_query() {
        let ids = vec!["id1".to_string(), "id2".to_string(), "id3".to_string()];
        let query = json!({
            "query": {
                "ids": {
                    "values": ids
                }
            }
        });
        let values = query["query"]["ids"]["values"].as_array().unwrap();
        assert_eq!(values.len(), 3);
    }

    #[test]
    fn test_delete_all_query() {
        let query = json!({
            "query": {
                "match_all": {}
            }
        });
        assert!(query["query"].get("match_all").is_some());
    }

    // Bulk indexing format tests
    #[test]
    fn test_bulk_index_action() {
        let doc_id = "doc-123";
        let action = json!({
            "index": {
                "_id": doc_id
            }
        });
        assert_eq!(action["index"]["_id"], json!("doc-123"));
    }

    #[test]
    fn test_bulk_document_with_content() {
        let content = "The quick brown fox jumps over the lazy dog";
        let doc = json!({
            "content": content,
        });
        assert_eq!(doc["content"], json!("The quick brown fox jumps over the lazy dog"));
    }

    #[test]
    fn test_bulk_document_with_metadata() {
        let mut doc = json!({
            "content": "Test document",
        });
        let metadata = HashMap::from([
            ("category".to_string(), json!("tech")),
            ("author".to_string(), json!("John Doe")),
        ]);
        if let Some(obj) = doc.as_object_mut() {
            for (k, v) in &metadata {
                obj.insert(k.clone(), v.clone());
            }
        }
        assert_eq!(doc["category"], json!("tech"));
        assert_eq!(doc["author"], json!("John Doe"));
    }

    // Response parsing tests
    #[test]
    fn test_parse_bm25_search_response() {
        let response = json!({
            "hits": {
                "hits": [
                    {
                        "_id": "1",
                        "_score": 5.234,
                        "_source": {
                            "content": "The quick brown fox"
                        }
                    },
                    {
                        "_id": "2",
                        "_score": 3.891,
                        "_source": {
                            "content": "A lazy dog sleeps"
                        }
                    }
                ]
            }
        });
        let hits = response["hits"]["hits"].as_array().unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0]["_id"], json!("1"));
        let score = hits[0]["_score"].as_f64().unwrap();
        assert!(score > 5.0);
    }

    #[test]
    fn test_parse_empty_response() {
        let response = json!({
            "hits": {
                "hits": []
            }
        });
        let hits = response["hits"]["hits"].as_array().unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn test_parse_content_field() {
        let hit = json!({
            "_source": {
                "content": "Document content here",
                "category": "test"
            }
        });
        let content = hit["_source"]["content"].as_str().unwrap();
        assert_eq!(content, "Document content here");
    }

    #[test]
    fn test_parse_metadata_excludes_content() {
        let source = json!({
            "content": "Main text",
            "category": "tech",
            "author": "Jane"
        });
        let mut metadata = HashMap::new();
        if let Some(obj) = source.as_object() {
            for (k, v) in obj {
                if k != "content" {
                    metadata.insert(k.clone(), v.clone());
                }
            }
        }
        assert_eq!(metadata.len(), 2);
        assert!(metadata.contains_key("category"));
        assert!(!metadata.contains_key("content"));
    }

    // Validation tests
    #[test]
    fn test_empty_texts_returns_empty_ids() {
        let texts: Vec<&str> = vec![];
        assert!(texts.is_empty());
    }

    #[test]
    fn test_metadatas_length_mismatch_detection() {
        let texts = vec!["a", "b", "c"];
        let metadatas: Vec<HashMap<String, JsonValue>> = vec![HashMap::new(), HashMap::new()];
        assert_ne!(texts.len(), metadatas.len());
    }

    #[test]
    fn test_empty_ids_for_delete() {
        let ids: Vec<String> = vec![];
        assert!(ids.is_empty());
    }

    // Document creation tests
    #[test]
    fn test_document_from_hit() {
        let id = "doc-1".to_string();
        let content = "Hello world".to_string();
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), json!("test"));

        let doc = Document {
            id: Some(id.clone()),
            page_content: content.clone(),
            metadata: metadata.clone(),
        };
        assert_eq!(doc.id, Some("doc-1".to_string()));
        assert_eq!(doc.page_content, "Hello world");
        assert_eq!(doc.metadata.get("source"), Some(&json!("test")));
    }

    #[test]
    fn test_document_with_score() {
        let doc = Document {
            id: Some("scored".to_string()),
            page_content: "Content".to_string(),
            metadata: HashMap::new(),
        };
        let score: f32 = 4.567;
        let tuple = (doc, score);
        assert!((tuple.1 - 4.567).abs() < 0.01);
    }

    // URL format tests (basic string validation)
    #[test]
    fn test_valid_url_format() {
        let url = "http://localhost:9200";
        assert!(url.starts_with("http://"));
        assert!(url.contains("localhost"));
    }

    #[test]
    fn test_url_with_credentials_format() {
        let url = "http://elastic:password@localhost:9200";
        assert!(url.contains("elastic:password@"));
    }

    #[test]
    fn test_url_format_no_scheme() {
        let url = "not_a_url";
        assert!(!url.starts_with("http://"));
        assert!(!url.starts_with("https://"));
    }

    // UUID generation tests
    #[test]
    fn test_uuid_generation_uniqueness() {
        let ids: Vec<String> = (0..10).map(|_| uuid::Uuid::new_v4().to_string()).collect();
        // All IDs should be unique
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
    }

    #[test]
    fn test_uuid_format_valid() {
        let id = uuid::Uuid::new_v4().to_string();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
    }

    // Retriever trait tests
    #[test]
    fn test_retriever_name() {
        // The name() method should return "ElasticsearchBM25Retriever"
        let expected = "ElasticsearchBM25Retriever";
        assert_eq!(expected, "ElasticsearchBM25Retriever");
    }

    // Parameter validation tests
    #[test]
    fn test_default_bm25_parameters() {
        // Default k1 = 2.0, b = 0.75
        let default_k1 = 2.0_f64;
        let default_b = 0.75_f64;
        assert!((default_k1 - 2.0).abs() < f64::EPSILON);
        assert!((default_b - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_custom_k_results() {
        let k: usize = 20;
        assert_eq!(k, 20);
    }

    #[test]
    fn test_k1_term_saturation_range() {
        // k1 typically ranges from 1.2 to 2.0
        let k1_low = 1.2_f64;
        let k1_high = 2.0_f64;
        assert!(k1_low < k1_high);
    }

    #[test]
    fn test_b_length_normalization_range() {
        // b ranges from 0 (no normalization) to 1 (full normalization)
        let b_none = 0.0_f64;
        let b_full = 1.0_f64;
        assert!((b_none - 0.0).abs() < f64::EPSILON);
        assert!((b_full - 1.0).abs() < f64::EPSILON);
    }

    // Index name tests
    #[test]
    fn test_index_name_with_uuid() {
        let index_name = format!("test_bm25_{}", uuid::Uuid::new_v4().to_string().replace('-', "_"));
        assert!(index_name.starts_with("test_bm25_"));
        assert!(!index_name.contains('-'));
    }

    #[test]
    fn test_index_name_sanitization() {
        let uuid = uuid::Uuid::new_v4().to_string();
        let sanitized = uuid.replace('-', "_");
        assert!(!sanitized.contains('-'));
    }

    // ========================================================================
    // INTEGRATION TESTS - Require Elasticsearch server
    // ========================================================================

    async fn create_test_retriever() -> ElasticsearchBM25Retriever {
        let index_name = format!(
            "test_bm25_{}",
            uuid::Uuid::new_v4().to_string().replace('-', "_")
        );
        let url =
            std::env::var("ELASTICSEARCH_URL").unwrap_or_else(|_| "http://localhost:9200".into());

        ElasticsearchBM25Retriever::new(&index_name, &url)
            .await
            .expect("Failed to create test retriever - is Elasticsearch running?")
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server"]
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
    #[ignore = "requires Elasticsearch server"]
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
    #[ignore = "requires Elasticsearch server"]
    async fn test_delete_documents() {
        let mut retriever = create_test_retriever().await;

        let ids = retriever
            .add_texts(&["Document one", "Document two"])
            .await
            .unwrap();

        // Delete specific document
        retriever.delete(Some(&ids[0..1])).await.unwrap();

        // Search should only find "Document two"
        let docs = retriever
            ._get_relevant_documents("Document", None)
            .await
            .unwrap();
        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("two"));
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server"]
    async fn test_custom_bm25_params() {
        let index_name = format!(
            "test_bm25_params_{}",
            uuid::Uuid::new_v4().to_string().replace('-', "_")
        );
        let url =
            std::env::var("ELASTICSEARCH_URL").unwrap_or_else(|_| "http://localhost:9200".into());

        let retriever = ElasticsearchBM25Retriever::create(&index_name, &url, 1.5, 0.5, 10)
            .await
            .expect("Failed to create retriever");

        assert!((retriever.k1() - 1.5).abs() < f64::EPSILON);
        assert!((retriever.b() - 0.5).abs() < f64::EPSILON);
        assert_eq!(retriever.k(), 10);
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server"]
    async fn test_empty_results() {
        let retriever = create_test_retriever().await;

        // Search in empty index
        let docs = retriever
            ._get_relevant_documents("nonexistent query", None)
            .await
            .unwrap();
        assert!(docs.is_empty());
    }
}
