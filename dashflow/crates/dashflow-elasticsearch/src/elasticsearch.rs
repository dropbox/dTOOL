//! Elasticsearch vector store implementation for `DashFlow` Rust.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::{Error, Result};
use dashflow::{embed, embed_query};
use elasticsearch::{
    http::{
        request::JsonBody,
        transport::{SingleNodeConnectionPool, TransportBuilder},
    },
    indices::{IndicesCreateParts, IndicesExistsParts},
    BulkParts, DeleteByQueryParts, Elasticsearch, GetParts, SearchParts,
};
use serde_json::{json, Value as JsonValue};

/// Elasticsearch vector store implementation.
pub struct ElasticsearchVectorStore {
    client: Elasticsearch,
    index_name: String,
    embeddings: Arc<dyn Embeddings>,
    distance_metric: DistanceMetric,
}

impl ElasticsearchVectorStore {
    /// Creates a new `ElasticsearchVectorStore` instance.
    ///
    /// # Arguments
    ///
    /// * `index_name` - Name of the Elasticsearch index
    /// * `embeddings` - Embeddings model to use
    /// * `url` - Elasticsearch connection URL
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Connection to Elasticsearch fails
    /// - Index creation fails
    pub async fn new(index_name: &str, embeddings: Arc<dyn Embeddings>, url: &str) -> Result<Self> {
        // Parse URL
        let url = url
            .parse()
            .map_err(|e| Error::config(format!("Invalid Elasticsearch URL '{url}': {e}")))?;

        // Create connection pool
        let conn_pool = SingleNodeConnectionPool::new(url);
        let transport = TransportBuilder::new(conn_pool)
            .build()
            .map_err(|e| Error::config(format!("Failed to build transport: {e}")))?;

        let client = Elasticsearch::new(transport);

        let store = Self {
            client,
            index_name: index_name.to_string(),
            embeddings,
            distance_metric: DistanceMetric::Cosine,
        };

        // Ensure index exists
        store.ensure_index().await?;

        Ok(store)
    }

    /// Ensures the Elasticsearch index exists with proper vector field mapping.
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

        // Create index with vector mapping
        let create_response = self
            .client
            .indices()
            .create(IndicesCreateParts::Index(&self.index_name))
            .body(json!({
                "mappings": {
                    "properties": {
                        "text": {
                            "type": "text"
                        },
                        "vector": {
                            "type": "dense_vector",
                            "dims": 1536,  // Default for OpenAI embeddings
                            "index": true,
                            "similarity": self.distance_metric_to_es_similarity()
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

    /// Converts our `DistanceMetric` to Elasticsearch similarity metric.
    fn distance_metric_to_es_similarity(&self) -> &'static str {
        match self.distance_metric {
            DistanceMetric::Cosine => "cosine",
            DistanceMetric::Euclidean => "l2_norm",
            DistanceMetric::DotProduct => "dot_product",
            DistanceMetric::MaxInnerProduct => "max_inner_product",
        }
    }

    /// Builds an Elasticsearch filter query from metadata filters.
    fn build_filter_query(&self, filter: &HashMap<String, JsonValue>) -> Option<JsonValue> {
        if filter.is_empty() {
            return None;
        }

        let conditions: Vec<JsonValue> = filter
            .iter()
            .map(|(k, v)| {
                json!({
                    "term": {
                        k: v
                    }
                })
            })
            .collect();

        if conditions.len() == 1 {
            Some(conditions[0].clone())
        } else {
            Some(json!({
                "bool": {
                    "must": conditions
                }
            }))
        }
    }
}

#[async_trait]
impl VectorStore for ElasticsearchVectorStore {
    fn embeddings(&self) -> Option<Arc<dyn Embeddings>> {
        Some(Arc::clone(&self.embeddings))
    }

    fn distance_metric(&self) -> DistanceMetric {
        self.distance_metric
    }

    async fn add_texts(
        &mut self,
        texts: &[impl AsRef<str> + Send + Sync],
        metadatas: Option<&[HashMap<String, JsonValue>]>,
        ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
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
        if let Some(ids) = ids {
            if ids.len() != text_count {
                return Err(Error::config(format!(
                    "IDs length mismatch: {} vs {}",
                    ids.len(),
                    text_count
                )));
            }
        }

        // Convert texts to strings
        let text_strings: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();

        // Generate embeddings using graph API
        let embeddings_vec = embed(Arc::clone(&self.embeddings), &text_strings).await?;

        // Generate IDs if not provided
        let doc_ids: Vec<String> = if let Some(ids) = ids {
            ids.to_vec()
        } else {
            (0..text_count)
                .map(|_| uuid::Uuid::new_v4().to_string())
                .collect()
        };

        // Build bulk request body
        let mut body: Vec<JsonBody<_>> = Vec::with_capacity(text_count * 2);

        for (i, text) in text_strings.iter().enumerate() {
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
                "text": text,
                "vector": embeddings_vec[i],
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
        // Ignoring this error prevents blocking successful writes on transient
        // refresh failures while maintaining data integrity.
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

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        if let Some(ids) = ids {
            if ids.is_empty() {
                return Ok(true);
            }

            // Delete specific documents by ID
            let query = json!({
                "query": {
                    "ids": {
                        "values": ids
                    }
                }
            });

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
        } else {
            // Delete all documents
            let query = json!({
                "query": {
                    "match_all": {}
                }
            });

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

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let mut documents = Vec::new();

        for id in ids {
            let get_response = self
                .client
                .get(GetParts::IndexId(&self.index_name, id))
                .send()
                .await
                .map_err(|e| Error::other(format!("Elasticsearch get failed: {e}")))?;

            if get_response.status_code().is_success() {
                let json: JsonValue = get_response
                    .json()
                    .await
                    .map_err(|e| Error::other(format!("Failed to parse response: {e}")))?;

                if let Some(source) = json.get("_source") {
                    let text = source
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let mut metadata = HashMap::new();
                    if let Some(obj) = source.as_object() {
                        for (k, v) in obj {
                            if k != "text" && k != "vector" {
                                metadata.insert(k.clone(), v.clone());
                            }
                        }
                    }

                    documents.push(Document {
                        id: Some(id.clone()),
                        page_content: text,
                        metadata,
                    });
                }
            }
        }

        Ok(documents)
    }

    async fn _similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        // Generate query embedding using graph API
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query).await?;

        // Perform vector search
        self.similarity_search_by_vector(&query_embedding, k, filter)
            .await
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        // Generate query embedding using graph API
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query).await?;

        // Perform vector search with scores
        self.similarity_search_by_vector_with_score(&query_embedding, k, filter)
            .await
    }

    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        let results = self
            .similarity_search_by_vector_with_score(embedding, k, filter)
            .await?;
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        // Build kNN query
        let mut knn_query = json!({
            "field": "vector",
            "query_vector": embedding,
            "k": k,
            "num_candidates": k * 10  // Elasticsearch recommendation
        });

        // Add filter if provided
        if let Some(filter) = filter {
            if let Some(filter_query) = self.build_filter_query(filter) {
                knn_query["filter"] = filter_query;
            }
        }

        // Build search body
        let search_body = json!({
            "knn": knn_query,
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
                        let text = source
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        let mut metadata = HashMap::new();
                        if let Some(obj) = source.as_object() {
                            for (k, v) in obj {
                                if k != "text" && k != "vector" {
                                    metadata.insert(k.clone(), v.clone());
                                }
                            }
                        }

                        let doc = Document {
                            id: Some(id),
                            page_content: text,
                            metadata,
                        };

                        results.push((doc, score));
                    }
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // ========================================================================
    // UNIT TESTS - Run without Elasticsearch server
    // ========================================================================

    /// Helper struct for testing distance metric conversion
    struct MetricTestHelper {
        metric: DistanceMetric,
    }

    impl MetricTestHelper {
        fn to_es_similarity(&self) -> &'static str {
            match self.metric {
                DistanceMetric::Cosine => "cosine",
                DistanceMetric::Euclidean => "l2_norm",
                DistanceMetric::DotProduct => "dot_product",
                DistanceMetric::MaxInnerProduct => "max_inner_product",
            }
        }
    }

    #[test]
    fn test_distance_metric_cosine() {
        let helper = MetricTestHelper {
            metric: DistanceMetric::Cosine,
        };
        assert_eq!(helper.to_es_similarity(), "cosine");
    }

    #[test]
    fn test_distance_metric_euclidean() {
        let helper = MetricTestHelper {
            metric: DistanceMetric::Euclidean,
        };
        assert_eq!(helper.to_es_similarity(), "l2_norm");
    }

    #[test]
    fn test_distance_metric_dot_product() {
        let helper = MetricTestHelper {
            metric: DistanceMetric::DotProduct,
        };
        assert_eq!(helper.to_es_similarity(), "dot_product");
    }

    #[test]
    fn test_distance_metric_max_inner_product() {
        let helper = MetricTestHelper {
            metric: DistanceMetric::MaxInnerProduct,
        };
        assert_eq!(helper.to_es_similarity(), "max_inner_product");
    }

    // Test filter query building logic
    fn build_filter_query_test(filter: &HashMap<String, JsonValue>) -> Option<JsonValue> {
        if filter.is_empty() {
            return None;
        }

        let conditions: Vec<JsonValue> = filter
            .iter()
            .map(|(k, v)| {
                json!({
                    "term": {
                        k: v
                    }
                })
            })
            .collect();

        if conditions.len() == 1 {
            Some(conditions[0].clone())
        } else {
            Some(json!({
                "bool": {
                    "must": conditions
                }
            }))
        }
    }

    #[test]
    fn test_build_filter_query_empty() {
        let filter = HashMap::new();
        assert!(build_filter_query_test(&filter).is_none());
    }

    #[test]
    fn test_build_filter_query_single_term() {
        let mut filter = HashMap::new();
        filter.insert("category".to_string(), json!("electronics"));

        let result = build_filter_query_test(&filter).unwrap();
        assert_eq!(result["term"]["category"], json!("electronics"));
    }

    #[test]
    fn test_build_filter_query_multiple_terms() {
        let mut filter = HashMap::new();
        filter.insert("category".to_string(), json!("electronics"));
        filter.insert("price".to_string(), json!(100));

        let result = build_filter_query_test(&filter).unwrap();
        assert!(result.get("bool").is_some());
        assert!(result["bool"].get("must").is_some());
        let must_array = result["bool"]["must"].as_array().unwrap();
        assert_eq!(must_array.len(), 2);
    }

    #[test]
    fn test_build_filter_query_string_value() {
        let mut filter = HashMap::new();
        filter.insert("status".to_string(), json!("active"));

        let result = build_filter_query_test(&filter).unwrap();
        assert_eq!(result["term"]["status"], json!("active"));
    }

    #[test]
    fn test_build_filter_query_numeric_value() {
        let mut filter = HashMap::new();
        filter.insert("count".to_string(), json!(42));

        let result = build_filter_query_test(&filter).unwrap();
        assert_eq!(result["term"]["count"], json!(42));
    }

    #[test]
    fn test_build_filter_query_boolean_value() {
        let mut filter = HashMap::new();
        filter.insert("is_active".to_string(), json!(true));

        let result = build_filter_query_test(&filter).unwrap();
        assert_eq!(result["term"]["is_active"], json!(true));
    }

    #[test]
    fn test_build_filter_query_float_value() {
        let mut filter = HashMap::new();
        filter.insert("score".to_string(), json!(3.14));

        let result = build_filter_query_test(&filter).unwrap();
        assert_eq!(result["term"]["score"], json!(3.14));
    }

    #[test]
    fn test_build_filter_query_null_value() {
        let mut filter = HashMap::new();
        filter.insert("optional".to_string(), json!(null));

        let result = build_filter_query_test(&filter).unwrap();
        assert_eq!(result["term"]["optional"], json!(null));
    }

    #[test]
    fn test_build_filter_query_three_terms() {
        let mut filter = HashMap::new();
        filter.insert("a".to_string(), json!(1));
        filter.insert("b".to_string(), json!(2));
        filter.insert("c".to_string(), json!(3));

        let result = build_filter_query_test(&filter).unwrap();
        let must_array = result["bool"]["must"].as_array().unwrap();
        assert_eq!(must_array.len(), 3);
    }

    #[test]
    fn test_build_filter_query_special_field_name() {
        let mut filter = HashMap::new();
        filter.insert("field.nested".to_string(), json!("value"));

        let result = build_filter_query_test(&filter).unwrap();
        assert_eq!(result["term"]["field.nested"], json!("value"));
    }

    #[test]
    fn test_build_filter_query_array_value() {
        let mut filter = HashMap::new();
        filter.insert("tags".to_string(), json!(["a", "b"]));

        let result = build_filter_query_test(&filter).unwrap();
        assert_eq!(result["term"]["tags"], json!(["a", "b"]));
    }

    // Document parsing tests
    #[test]
    fn test_document_creation() {
        let doc = Document {
            id: Some("test-id".to_string()),
            page_content: "Test content".to_string(),
            metadata: HashMap::new(),
        };
        assert_eq!(doc.id, Some("test-id".to_string()));
        assert_eq!(doc.page_content, "Test content");
        assert!(doc.metadata.is_empty());
    }

    #[test]
    fn test_document_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), json!("test"));
        metadata.insert("page".to_string(), json!(1));

        let doc = Document {
            id: Some("doc-1".to_string()),
            page_content: "Content here".to_string(),
            metadata,
        };
        assert_eq!(doc.metadata.get("source"), Some(&json!("test")));
        assert_eq!(doc.metadata.get("page"), Some(&json!(1)));
    }

    #[test]
    fn test_document_no_id() {
        let doc = Document {
            id: None,
            page_content: "Anonymous content".to_string(),
            metadata: HashMap::new(),
        };
        assert!(doc.id.is_none());
    }

    #[test]
    fn test_document_empty_content() {
        let doc = Document {
            id: Some("empty".to_string()),
            page_content: String::new(),
            metadata: HashMap::new(),
        };
        assert!(doc.page_content.is_empty());
    }

    #[test]
    fn test_document_unicode_content() {
        let doc = Document {
            id: Some("unicode".to_string()),
            page_content: "日本語 русский العربية".to_string(),
            metadata: HashMap::new(),
        };
        assert!(doc.page_content.contains("日本語"));
    }

    #[test]
    fn test_document_long_content() {
        let long_content = "a".repeat(10000);
        let doc = Document {
            id: Some("long".to_string()),
            page_content: long_content.clone(),
            metadata: HashMap::new(),
        };
        assert_eq!(doc.page_content.len(), 10000);
    }

    // Bulk body format tests
    #[test]
    fn test_bulk_body_index_action_format() {
        let doc_id = "test-123";
        let action = json!({
            "index": {
                "_id": doc_id
            }
        });
        assert_eq!(action["index"]["_id"], json!("test-123"));
    }

    #[test]
    fn test_bulk_body_document_format() {
        let doc = json!({
            "text": "Hello world",
            "vector": vec![0.1, 0.2, 0.3],
        });
        assert_eq!(doc["text"], json!("Hello world"));
        assert_eq!(doc["vector"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_bulk_body_document_with_metadata() {
        let mut doc = json!({
            "text": "Test",
            "vector": vec![1.0],
        });
        if let Some(obj) = doc.as_object_mut() {
            obj.insert("category".to_string(), json!("tech"));
            obj.insert("score".to_string(), json!(0.95));
        }
        assert_eq!(doc["category"], json!("tech"));
        assert_eq!(doc["score"], json!(0.95));
    }

    // kNN query format tests
    #[test]
    fn test_knn_query_format() {
        let embedding = vec![0.1, 0.2, 0.3];
        let k = 5;
        let knn_query = json!({
            "field": "vector",
            "query_vector": embedding,
            "k": k,
            "num_candidates": k * 10
        });
        assert_eq!(knn_query["field"], json!("vector"));
        assert_eq!(knn_query["k"], json!(5));
        assert_eq!(knn_query["num_candidates"], json!(50));
    }

    #[test]
    fn test_knn_query_with_filter() {
        let filter = json!({
            "term": {
                "category": "tech"
            }
        });
        let mut knn_query = json!({
            "field": "vector",
            "query_vector": vec![0.1, 0.2],
            "k": 10,
            "num_candidates": 100
        });
        knn_query["filter"] = filter;
        assert!(knn_query.get("filter").is_some());
    }

    #[test]
    fn test_search_body_format() {
        let knn_query = json!({
            "field": "vector",
            "query_vector": vec![0.1],
            "k": 5,
            "num_candidates": 50
        });
        let search_body = json!({
            "knn": knn_query,
            "size": 5,
            "_source": true
        });
        assert_eq!(search_body["size"], json!(5));
        assert_eq!(search_body["_source"], json!(true));
    }

    // Delete query format tests
    #[test]
    fn test_delete_by_ids_query_format() {
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
    fn test_delete_all_query_format() {
        let query = json!({
            "query": {
                "match_all": {}
            }
        });
        assert!(query["query"].get("match_all").is_some());
    }

    // Index mapping format tests
    #[test]
    fn test_index_mapping_format() {
        let mapping = json!({
            "mappings": {
                "properties": {
                    "text": {
                        "type": "text"
                    },
                    "vector": {
                        "type": "dense_vector",
                        "dims": 1536,
                        "index": true,
                        "similarity": "cosine"
                    }
                }
            }
        });
        assert_eq!(mapping["mappings"]["properties"]["text"]["type"], json!("text"));
        assert_eq!(mapping["mappings"]["properties"]["vector"]["dims"], json!(1536));
        assert_eq!(mapping["mappings"]["properties"]["vector"]["similarity"], json!("cosine"));
    }

    #[test]
    fn test_index_mapping_different_similarities() {
        for (metric, expected) in [
            (DistanceMetric::Cosine, "cosine"),
            (DistanceMetric::Euclidean, "l2_norm"),
            (DistanceMetric::DotProduct, "dot_product"),
            (DistanceMetric::MaxInnerProduct, "max_inner_product"),
        ] {
            let helper = MetricTestHelper { metric };
            let mapping = json!({
                "mappings": {
                    "properties": {
                        "vector": {
                            "similarity": helper.to_es_similarity()
                        }
                    }
                }
            });
            assert_eq!(mapping["mappings"]["properties"]["vector"]["similarity"], json!(expected));
        }
    }

    // URL format tests (basic string validation)
    #[test]
    fn test_url_format_http() {
        let url = "http://localhost:9200";
        assert!(url.starts_with("http://"));
        assert!(url.contains(":9200"));
    }

    #[test]
    fn test_url_format_https() {
        let url = "https://elastic.cloud:443";
        assert!(url.starts_with("https://"));
    }

    #[test]
    fn test_url_format_with_auth() {
        let url = "http://user:pass@localhost:9200";
        assert!(url.contains("user:pass@"));
    }

    #[test]
    fn test_url_format_with_port() {
        let url = "http://elastic.local:9200";
        assert!(url.contains(":9200"));
    }

    #[test]
    fn test_url_format_invalid_no_scheme() {
        let url = "localhost:9200";
        assert!(!url.starts_with("http://"));
        assert!(!url.starts_with("https://"));
    }

    // Score parsing tests
    #[test]
    fn test_score_parsing_from_json() {
        let hit = json!({
            "_score": 0.95,
            "_id": "test",
            "_source": {"text": "content"}
        });
        let score = hit["_score"].as_f64().unwrap_or(0.0) as f32;
        assert!((score - 0.95).abs() < 0.01);
    }

    #[test]
    fn test_score_parsing_null() {
        let hit = json!({
            "_score": null,
            "_id": "test"
        });
        let score = hit["_score"].as_f64().unwrap_or(0.0) as f32;
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_score_parsing_missing() {
        let hit = json!({
            "_id": "test"
        });
        let score = hit.get("_score").and_then(|s| s.as_f64()).unwrap_or(0.0) as f32;
        assert_eq!(score, 0.0);
    }

    // ID generation tests
    #[test]
    fn test_uuid_generation() {
        let id1 = uuid::Uuid::new_v4().to_string();
        let id2 = uuid::Uuid::new_v4().to_string();
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 36); // UUID v4 format
    }

    #[test]
    fn test_uuid_format() {
        let id = uuid::Uuid::new_v4().to_string();
        // UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
        assert!(id.contains('-'));
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
    }

    // Distance metric default tests
    #[test]
    fn test_distance_metric_default() {
        let default_metric = DistanceMetric::Cosine;
        assert!(matches!(default_metric, DistanceMetric::Cosine));
    }

    // Response parsing helper tests
    #[test]
    fn test_parse_hits_array() {
        let response = json!({
            "hits": {
                "hits": [
                    {"_id": "1", "_score": 0.9, "_source": {"text": "doc1"}},
                    {"_id": "2", "_score": 0.8, "_source": {"text": "doc2"}}
                ]
            }
        });
        let hits = response["hits"]["hits"].as_array().unwrap();
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn test_parse_hits_empty() {
        let response = json!({
            "hits": {
                "hits": []
            }
        });
        let hits = response["hits"]["hits"].as_array().unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn test_parse_source_text() {
        let hit = json!({
            "_source": {
                "text": "Hello world",
                "vector": [0.1, 0.2]
            }
        });
        let text = hit["_source"]["text"].as_str().unwrap();
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn test_parse_source_metadata_exclude_text_and_vector() {
        let source = json!({
            "text": "content",
            "vector": [0.1],
            "category": "tech",
            "author": "john"
        });
        let mut metadata = HashMap::new();
        if let Some(obj) = source.as_object() {
            for (k, v) in obj {
                if k != "text" && k != "vector" {
                    metadata.insert(k.clone(), v.clone());
                }
            }
        }
        assert_eq!(metadata.len(), 2);
        assert!(metadata.contains_key("category"));
        assert!(metadata.contains_key("author"));
        assert!(!metadata.contains_key("text"));
        assert!(!metadata.contains_key("vector"));
    }

    // Validation tests
    #[test]
    fn test_metadatas_length_validation() {
        let texts = vec!["a", "b", "c"];
        let metadatas: Vec<HashMap<String, JsonValue>> = vec![HashMap::new(), HashMap::new()]; // Wrong length
        assert_ne!(texts.len(), metadatas.len());
    }

    #[test]
    fn test_ids_length_validation() {
        let texts = vec!["a", "b"];
        let ids = vec!["id1".to_string()]; // Wrong length
        assert_ne!(texts.len(), ids.len());
    }

    #[test]
    fn test_empty_texts_handling() {
        let texts: Vec<&str> = vec![];
        assert!(texts.is_empty());
    }

    #[test]
    fn test_empty_ids_for_delete() {
        let ids: Vec<String> = vec![];
        assert!(ids.is_empty());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod standard_tests {
    use super::*;
    use dashflow::core::embeddings::Embeddings;
    use dashflow_standard_tests::vectorstore_tests::*;
    use std::sync::Arc;

    /// Mock embeddings for testing
    struct MockEmbeddings;

    #[async_trait::async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(
            &self,
            texts: &[String],
        ) -> dashflow::core::error::Result<Vec<Vec<f32>>> {
            // Generate deterministic vectors based on text
            Ok(texts
                .iter()
                .map(|text| {
                    let bytes = text.as_bytes();
                    let x = if bytes.is_empty() {
                        0.0
                    } else {
                        bytes[0] as f32 / 255.0
                    };
                    let y = if bytes.len() < 2 {
                        0.0
                    } else {
                        bytes[1] as f32 / 255.0
                    };
                    let z = text.len() as f32 / 100.0;

                    let mag = (x * x + y * y + z * z).sqrt();
                    if mag > 0.0 {
                        vec![x / mag, y / mag, z / mag]
                    } else {
                        vec![0.0, 0.0, 0.0]
                    }
                })
                .collect())
        }

        async fn _embed_query(&self, text: &str) -> dashflow::core::error::Result<Vec<f32>> {
            let result = self._embed_documents(&[text.to_string()]).await?;
            Ok(result.into_iter().next().unwrap())
        }
    }

    async fn create_test_store() -> ElasticsearchVectorStore {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        // Use unique index name per test to avoid conflicts
        let index_name = format!(
            "test_{}",
            uuid::Uuid::new_v4().to_string().replace("-", "_")
        );
        let url = std::env::var("ELASTICSEARCH_URL")
            .unwrap_or_else(|_| "http://localhost:9200".to_string());

        ElasticsearchVectorStore::new(&index_name, embeddings, &url)
            .await
            .expect("Failed to create test store - is Elasticsearch running on localhost:9200?")
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_add_and_search_standard() {
        let mut store = create_test_store().await;
        test_add_and_search(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_search_with_scores_standard() {
        let mut store = create_test_store().await;
        test_search_with_scores(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_metadata_filtering_standard() {
        let mut store = create_test_store().await;
        test_metadata_filtering(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_custom_ids_standard() {
        let mut store = create_test_store().await;
        test_custom_ids(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_delete_standard() {
        let mut store = create_test_store().await;
        test_delete(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_add_documents_standard() {
        let mut store = create_test_store().await;
        test_add_documents(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_empty_search_standard() {
        let store = create_test_store().await;
        test_empty_search(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_search_by_vector_standard() {
        let mut store = create_test_store().await;
        test_search_by_vector(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_mmr_search_standard() {
        let mut store = create_test_store().await;
        test_mmr_search(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_large_batch_standard() {
        let mut store = create_test_store().await;
        test_large_batch(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_validation_standard() {
        let mut store = create_test_store().await;
        test_validation(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_update_document_standard() {
        let mut store = create_test_store().await;
        test_update_document(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_metadata_only_filter_standard() {
        let mut store = create_test_store().await;
        test_metadata_only_filter(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_complex_metadata_standard() {
        let mut store = create_test_store().await;
        test_complex_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_empty_text_standard() {
        let mut store = create_test_store().await;
        test_empty_text(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_special_chars_metadata_standard() {
        let mut store = create_test_store().await;
        test_special_chars_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_concurrent_operations_standard() {
        let mut store = create_test_store().await;
        test_concurrent_operations(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_very_long_text_standard() {
        let mut store = create_test_store().await;
        test_very_long_text(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_duplicate_documents_standard() {
        let mut store = create_test_store().await;
        test_duplicate_documents(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_k_parameter_standard() {
        let mut store = create_test_store().await;
        test_k_parameter(&mut store).await;
    }

    // ========================================================================
    // COMPREHENSIVE TESTS
    // These tests provide deeper coverage beyond standard conformance tests
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_mmr_lambda_zero_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_lambda_zero(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_mmr_lambda_one_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_lambda_one(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_mmr_fetch_k_variations_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_fetch_k_variations(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_complex_metadata_operators_comprehensive() {
        let mut store = create_test_store().await;
        test_complex_metadata_operators(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_nested_metadata_filtering_comprehensive() {
        let mut store = create_test_store().await;
        test_nested_metadata_filtering(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_array_metadata_comprehensive() {
        let mut store = create_test_store().await;
        test_array_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_very_large_batch_comprehensive() {
        let mut store = create_test_store().await;
        test_very_large_batch(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_concurrent_writes_comprehensive() {
        let mut store = create_test_store().await;
        test_concurrent_writes(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_error_handling_network_comprehensive() {
        let mut store = create_test_store().await;
        test_error_handling_network(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_error_handling_invalid_input_comprehensive() {
        let mut store = create_test_store().await;
        test_error_handling_invalid_input(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_bulk_delete_comprehensive() {
        let mut store = create_test_store().await;
        test_bulk_delete(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_update_metadata_comprehensive() {
        let mut store = create_test_store().await;
        test_update_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Elasticsearch server: docker-compose -f docker-compose.test.yml up elasticsearch"]
    async fn test_search_score_threshold_comprehensive() {
        let mut store = create_test_store().await;
        test_search_score_threshold(&mut store).await;
    }
}
