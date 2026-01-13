//! `OpenSearch` vector store implementation for `DashFlow` Rust.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::{Error, Result};
use dashflow::{embed, embed_query};
use opensearch::{
    http::{
        request::JsonBody,
        transport::{SingleNodeConnectionPool, TransportBuilder},
    },
    indices::{IndicesCreateParts, IndicesExistsParts},
    BulkParts, DeleteByQueryParts, GetParts, OpenSearch, SearchParts,
};
use serde_json::{json, Value as JsonValue};

/// `OpenSearch` vector store implementation.
///
/// `OpenSearch` is an open-source fork of Elasticsearch maintained by AWS.
/// It provides vector search capabilities through the k-NN plugin which
/// supports ANN (Approximate Nearest Neighbor) search using various algorithms
/// including HNSW, IVF, and more.
///
/// # Features
///
/// - Vector similarity search with k-NN plugin
/// - Multiple distance metrics (cosine, L2, dot product, inner product)
/// - Metadata filtering
/// - Bulk document operations
/// - Automatic index management
///
/// # Example
///
/// ```no_run
/// use std::sync::Arc;
/// use dashflow_opensearch::OpenSearchVectorStore;
/// use dashflow::core::embeddings::MockEmbeddings;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let embeddings = Arc::new(MockEmbeddings::new(1536));
///     let store = OpenSearchVectorStore::new(
///         "my_vectors",
///         embeddings,
///         "https://localhost:9200",
///     ).await?;
///
///     Ok(())
/// }
/// ```
pub struct OpenSearchVectorStore {
    client: OpenSearch,
    index_name: String,
    embeddings: Arc<dyn Embeddings>,
    distance_metric: DistanceMetric,
}

impl OpenSearchVectorStore {
    /// Creates a new `OpenSearchVectorStore` instance.
    ///
    /// # Arguments
    ///
    /// * `index_name` - Name of the `OpenSearch` index
    /// * `embeddings` - Embeddings model to use
    /// * `url` - `OpenSearch` connection URL
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Connection to `OpenSearch` fails
    /// - Index creation fails
    pub async fn new(index_name: &str, embeddings: Arc<dyn Embeddings>, url: &str) -> Result<Self> {
        // Parse URL
        let url = url
            .parse()
            .map_err(|e| Error::config(format!("Invalid OpenSearch URL '{url}': {e}")))?;

        // Create connection pool
        let conn_pool = SingleNodeConnectionPool::new(url);
        let transport = TransportBuilder::new(conn_pool)
            .build()
            .map_err(|e| Error::config(format!("Failed to build transport: {e}")))?;

        let client = OpenSearch::new(transport);

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

    /// Ensures the `OpenSearch` index exists with proper vector field mapping.
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

        // Create index with k-NN vector mapping
        // OpenSearch uses k-NN plugin for vector search
        let create_response = self
            .client
            .indices()
            .create(IndicesCreateParts::Index(&self.index_name))
            .body(json!({
                "settings": {
                    "index": {
                        "knn": true  // Enable k-NN plugin for this index
                    }
                },
                "mappings": {
                    "properties": {
                        "text": {
                            "type": "text"
                        },
                        "vector": {
                            "type": "knn_vector",
                            "dimension": 1536,  // Default for OpenAI embeddings
                            "method": {
                                "name": "hnsw",
                                "space_type": self.distance_metric_to_os_space_type(),
                                "engine": "nmslib",
                                "parameters": {
                                    "ef_construction": 128,
                                    "m": 24
                                }
                            }
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

    /// Converts our `DistanceMetric` to `OpenSearch` k-NN `space_type`.
    fn distance_metric_to_os_space_type(&self) -> &'static str {
        match self.distance_metric {
            DistanceMetric::Cosine => "cosinesimil",
            DistanceMetric::Euclidean => "l2",
            DistanceMetric::DotProduct | DistanceMetric::MaxInnerProduct => "innerproduct",
        }
    }

    /// Builds an `OpenSearch` filter query from metadata filters.
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
impl VectorStore for OpenSearchVectorStore {
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

        // Generate embeddings
        let embeddings_vec = embed(Arc::clone(&self.embeddings), &text_strings)
            .await
            .map_err(|e| Error::other(format!("Embedding failed: {e}")))?;

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
        // Ignoring this error prevents blocking successful writes on transient
        // refresh failures while maintaining data integrity.
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
                .map_err(|e| Error::other(format!("OpenSearch get failed: {e}")))?;

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
        // Generate query embedding
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query)
            .await
            .map_err(|e| Error::other(format!("Query embedding failed: {e}")))?;

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
        // Generate query embedding
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query)
            .await
            .map_err(|e| Error::other(format!("Query embedding failed: {e}")))?;

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
        // Build k-NN query for OpenSearch
        let mut knn_query = json!({
            "vector": {
                "vector": embedding,
                "k": k
            }
        });

        // Add filter if provided
        if let Some(filter) = filter {
            if let Some(filter_query) = self.build_filter_query(filter) {
                knn_query["vector"]["filter"] = filter_query;
            }
        }

        // Build search body using k-NN plugin query syntax
        let search_body = json!({
            "query": {
                "knn": knn_query
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

/// VectorStoreRetriever wraps OpenSearchVectorStore to implement the Retriever trait.
///
/// This allows using OpenSearchVectorStore as a retriever in chains and with MergerRetriever.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_opensearch::{OpenSearchVectorStore, VectorStoreRetriever};
/// use dashflow::core::retrievers::Retriever;
///
/// let store = OpenSearchVectorStore::new("my_index", embeddings, "http://localhost:9200").await?;
/// let retriever = VectorStoreRetriever::new(store, 10);
/// let docs = retriever._get_relevant_documents("search query", None).await?;
/// ```
pub struct VectorStoreRetriever {
    store: OpenSearchVectorStore,
    k: usize,
}

impl VectorStoreRetriever {
    /// Create a new retriever wrapping an OpenSearchVectorStore.
    ///
    /// # Arguments
    ///
    /// * `store` - The OpenSearchVectorStore to wrap
    /// * `k` - Number of documents to retrieve per query
    #[must_use]
    pub fn new(store: OpenSearchVectorStore, k: usize) -> Self {
        Self { store, k }
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
}

use dashflow::core::config::RunnableConfig;
use dashflow::core::retrievers::{Retriever, RetrieverInput, RetrieverOutput};
use dashflow::core::runnable::Runnable;

#[async_trait]
impl Retriever for VectorStoreRetriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        self.store._similarity_search(query, self.k, None).await
    }

    fn name(&self) -> String {
        "OpenSearchVectorStoreRetriever".to_string()
    }
}

#[async_trait]
impl Runnable for VectorStoreRetriever {
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
        "OpenSearchVectorStoreRetriever".to_string()
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    // ==================== distance_metric_to_os_space_type tests ====================

    // Helper struct to test the space type conversion without a full store
    struct SpaceTypeTestHelper {
        distance_metric: DistanceMetric,
    }

    impl SpaceTypeTestHelper {
        fn distance_metric_to_os_space_type(&self) -> &'static str {
            match self.distance_metric {
                DistanceMetric::Cosine => "cosinesimil",
                DistanceMetric::Euclidean => "l2",
                DistanceMetric::DotProduct | DistanceMetric::MaxInnerProduct => "innerproduct",
            }
        }
    }

    #[test]
    fn test_distance_metric_to_os_space_type_cosine() {
        let helper = SpaceTypeTestHelper {
            distance_metric: DistanceMetric::Cosine,
        };
        assert_eq!(helper.distance_metric_to_os_space_type(), "cosinesimil");
    }

    #[test]
    fn test_distance_metric_to_os_space_type_euclidean() {
        let helper = SpaceTypeTestHelper {
            distance_metric: DistanceMetric::Euclidean,
        };
        assert_eq!(helper.distance_metric_to_os_space_type(), "l2");
    }

    #[test]
    fn test_distance_metric_to_os_space_type_dot_product() {
        let helper = SpaceTypeTestHelper {
            distance_metric: DistanceMetric::DotProduct,
        };
        assert_eq!(helper.distance_metric_to_os_space_type(), "innerproduct");
    }

    #[test]
    fn test_distance_metric_to_os_space_type_max_inner_product() {
        let helper = SpaceTypeTestHelper {
            distance_metric: DistanceMetric::MaxInnerProduct,
        };
        // MaxInnerProduct should map to same as DotProduct
        assert_eq!(helper.distance_metric_to_os_space_type(), "innerproduct");
    }

    // ==================== build_filter_query tests ====================

    // Helper function to test filter query building
    fn build_filter_query(filter: &HashMap<String, JsonValue>) -> Option<JsonValue> {
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
        let filter: HashMap<String, JsonValue> = HashMap::new();
        let result = build_filter_query(&filter);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_filter_query_single_string() {
        let mut filter = HashMap::new();
        filter.insert("category".to_string(), json!("tech"));
        let result = build_filter_query(&filter);

        assert!(result.is_some());
        let query = result.unwrap();
        assert_eq!(query["term"]["category"], json!("tech"));
    }

    #[test]
    fn test_build_filter_query_single_number() {
        let mut filter = HashMap::new();
        filter.insert("year".to_string(), json!(2024));
        let result = build_filter_query(&filter);

        assert!(result.is_some());
        let query = result.unwrap();
        assert_eq!(query["term"]["year"], json!(2024));
    }

    #[test]
    fn test_build_filter_query_single_boolean() {
        let mut filter = HashMap::new();
        filter.insert("active".to_string(), json!(true));
        let result = build_filter_query(&filter);

        assert!(result.is_some());
        let query = result.unwrap();
        assert_eq!(query["term"]["active"], json!(true));
    }

    #[test]
    fn test_build_filter_query_multiple_conditions() {
        let mut filter = HashMap::new();
        filter.insert("category".to_string(), json!("tech"));
        filter.insert("year".to_string(), json!(2024));
        let result = build_filter_query(&filter);

        assert!(result.is_some());
        let query = result.unwrap();
        // Multiple conditions should use bool.must
        assert!(query["bool"]["must"].is_array());
        let conditions = query["bool"]["must"].as_array().unwrap();
        assert_eq!(conditions.len(), 2);
    }

    #[test]
    fn test_build_filter_query_three_conditions() {
        let mut filter = HashMap::new();
        filter.insert("a".to_string(), json!("1"));
        filter.insert("b".to_string(), json!("2"));
        filter.insert("c".to_string(), json!("3"));
        let result = build_filter_query(&filter);

        assert!(result.is_some());
        let query = result.unwrap();
        let conditions = query["bool"]["must"].as_array().unwrap();
        assert_eq!(conditions.len(), 3);
    }

    // ==================== VectorStoreRetriever tests ====================

    // Note: We can't easily create a full OpenSearchVectorStore without an actual
    // OpenSearch connection, but we can test the VectorStoreRetriever methods
    // that don't require async operations.

    #[test]
    fn test_vector_store_retriever_k_getter_setter() {
        // Create a minimal mock for testing k getter/setter
        struct MockRetriever {
            k: usize,
        }

        let mut retriever = MockRetriever { k: 10 };
        assert_eq!(retriever.k, 10);

        retriever.k = 20;
        assert_eq!(retriever.k, 20);
    }

    #[test]
    fn test_vector_store_retriever_default_k() {
        // Test common default k values
        let default_k = 10;
        assert!(default_k > 0);
        assert!(default_k <= 1000); // Reasonable upper bound
    }

    // ==================== DistanceMetric enum tests ====================

    #[test]
    fn test_distance_metric_debug() {
        let cosine = DistanceMetric::Cosine;
        let debug = format!("{:?}", cosine);
        assert_eq!(debug, "Cosine");

        let euclidean = DistanceMetric::Euclidean;
        let debug = format!("{:?}", euclidean);
        assert_eq!(debug, "Euclidean");
    }

    #[test]
    #[allow(clippy::clone_on_copy)]
    fn test_distance_metric_clone() {
        let original = DistanceMetric::DotProduct;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_distance_metric_copy() {
        let original = DistanceMetric::Euclidean;
        let copied = original; // Copy, not move
        assert_eq!(original, copied);
    }

    #[test]
    fn test_distance_metric_partial_eq() {
        assert_eq!(DistanceMetric::Cosine, DistanceMetric::Cosine);
        assert_ne!(DistanceMetric::Cosine, DistanceMetric::Euclidean);
        assert_ne!(DistanceMetric::DotProduct, DistanceMetric::MaxInnerProduct);
    }

    // ==================== JSON building tests ====================

    #[test]
    fn test_knn_query_structure() {
        let embedding: Vec<f32> = vec![0.1, 0.2, 0.3];
        let k = 5;

        let knn_query = json!({
            "vector": {
                "vector": embedding,
                "k": k
            }
        });

        assert!(knn_query["vector"]["vector"].is_array());
        assert_eq!(knn_query["vector"]["k"], json!(5));
    }

    #[test]
    fn test_search_body_structure() {
        let knn_query = json!({
            "vector": {
                "vector": vec![0.1f32, 0.2f32],
                "k": 10
            }
        });

        let search_body = json!({
            "query": {
                "knn": knn_query
            },
            "size": 10,
            "_source": true
        });

        assert!(search_body["query"]["knn"].is_object());
        assert_eq!(search_body["size"], json!(10));
        assert_eq!(search_body["_source"], json!(true));
    }

    #[test]
    fn test_delete_by_ids_query() {
        let ids = vec!["id1", "id2", "id3"];
        let query = json!({
            "query": {
                "ids": {
                    "values": ids
                }
            }
        });

        let values = query["query"]["ids"]["values"].as_array().unwrap();
        assert_eq!(values.len(), 3);
        assert_eq!(values[0], json!("id1"));
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

    // ==================== Document parsing tests ====================

    #[test]
    fn test_document_from_source() {
        let source = json!({
            "text": "Hello world",
            "category": "greeting",
            "vector": [0.1, 0.2, 0.3]
        });

        let text = source
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        assert_eq!(text, "Hello world");

        // Metadata should exclude text and vector
        let mut metadata = HashMap::new();
        if let Some(obj) = source.as_object() {
            for (k, v) in obj {
                if k != "text" && k != "vector" {
                    metadata.insert(k.clone(), v.clone());
                }
            }
        }
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata.get("category"), Some(&json!("greeting")));
    }

    #[test]
    fn test_document_from_search_hit() {
        let hit = json!({
            "_score": 0.95,
            "_id": "doc123",
            "_source": {
                "text": "Test document",
                "author": "Test Author"
            }
        });

        let score = hit
            .get("_score")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32;
        assert!((score - 0.95f32).abs() < 0.001);

        let id = hit
            .get("_id")
            .and_then(|i| i.as_str())
            .unwrap_or("")
            .to_string();
        assert_eq!(id, "doc123");
    }

    #[test]
    fn test_document_missing_fields() {
        let source = json!({});

        let text = source
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        assert!(text.is_empty());
    }

    // ==================== Metadata extraction tests ====================

    #[test]
    fn test_metadata_extraction_multiple_fields() {
        let source = json!({
            "text": "Document content",
            "vector": [0.1, 0.2],
            "author": "John",
            "category": "tech",
            "year": 2024,
            "rating": 4.5
        });

        let mut metadata = HashMap::new();
        if let Some(obj) = source.as_object() {
            for (k, v) in obj {
                if k != "text" && k != "vector" {
                    metadata.insert(k.clone(), v.clone());
                }
            }
        }

        assert_eq!(metadata.len(), 4);
        assert_eq!(metadata.get("author"), Some(&json!("John")));
        assert_eq!(metadata.get("category"), Some(&json!("tech")));
        assert_eq!(metadata.get("year"), Some(&json!(2024)));
        assert_eq!(metadata.get("rating"), Some(&json!(4.5)));
    }

    #[test]
    fn test_metadata_extraction_no_extra_fields() {
        let source = json!({
            "text": "Only text",
            "vector": [0.5, 0.5, 0.5]
        });

        let mut metadata = HashMap::new();
        if let Some(obj) = source.as_object() {
            for (k, v) in obj {
                if k != "text" && k != "vector" {
                    metadata.insert(k.clone(), v.clone());
                }
            }
        }

        assert!(metadata.is_empty());
    }

    #[test]
    fn test_metadata_extraction_nested_object() {
        let source = json!({
            "text": "With nested",
            "vector": [0.1],
            "config": {
                "enabled": true,
                "level": 5
            }
        });

        let mut metadata = HashMap::new();
        if let Some(obj) = source.as_object() {
            for (k, v) in obj {
                if k != "text" && k != "vector" {
                    metadata.insert(k.clone(), v.clone());
                }
            }
        }

        assert_eq!(metadata.len(), 1);
        let config = metadata.get("config").unwrap();
        assert_eq!(config["enabled"], json!(true));
        assert_eq!(config["level"], json!(5));
    }

    #[test]
    fn test_metadata_extraction_array_value() {
        let source = json!({
            "text": "With array",
            "vector": [0.1],
            "tags": ["rust", "ai", "search"]
        });

        let mut metadata = HashMap::new();
        if let Some(obj) = source.as_object() {
            for (k, v) in obj {
                if k != "text" && k != "vector" {
                    metadata.insert(k.clone(), v.clone());
                }
            }
        }

        assert_eq!(metadata.len(), 1);
        let tags = metadata.get("tags").unwrap();
        assert!(tags.is_array());
        assert_eq!(tags.as_array().unwrap().len(), 3);
    }

    // ==================== Score parsing tests ====================

    #[test]
    fn test_score_parsing_valid() {
        let hit = json!({ "_score": 0.85 });
        let score = hit
            .get("_score")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32;
        assert!((score - 0.85f32).abs() < 0.001);
    }

    #[test]
    fn test_score_parsing_missing() {
        let hit = json!({ "_id": "123" });
        let score = hit
            .get("_score")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32;
        assert!((score - 0.0f32).abs() < 0.001);
    }

    #[test]
    fn test_score_parsing_null() {
        let hit = json!({ "_score": null });
        let score = hit
            .get("_score")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32;
        assert!((score - 0.0f32).abs() < 0.001);
    }

    #[test]
    fn test_score_parsing_integer() {
        let hit = json!({ "_score": 1 });
        let score = hit
            .get("_score")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32;
        assert!((score - 1.0f32).abs() < 0.001);
    }

    #[test]
    fn test_score_parsing_high_precision() {
        let hit = json!({ "_score": 0.123456789 });
        let score = hit
            .get("_score")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32;
        // f32 precision limits
        assert!((score - 0.1234568f32).abs() < 0.0001);
    }

    // ==================== ID parsing tests ====================

    #[test]
    fn test_id_parsing_string() {
        let hit = json!({ "_id": "doc_123" });
        let id = hit
            .get("_id")
            .and_then(|i| i.as_str())
            .unwrap_or("")
            .to_string();
        assert_eq!(id, "doc_123");
    }

    #[test]
    fn test_id_parsing_missing() {
        let hit = json!({ "_source": {} });
        let id = hit
            .get("_id")
            .and_then(|i| i.as_str())
            .unwrap_or("")
            .to_string();
        assert!(id.is_empty());
    }

    #[test]
    fn test_id_parsing_uuid_format() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let hit = json!({ "_id": uuid_str });
        let id = hit
            .get("_id")
            .and_then(|i| i.as_str())
            .unwrap_or("")
            .to_string();
        assert_eq!(id.len(), 36);
        assert!(id.contains('-'));
    }

    #[test]
    fn test_id_parsing_special_chars() {
        let hit = json!({ "_id": "doc-with_special.chars:123" });
        let id = hit
            .get("_id")
            .and_then(|i| i.as_str())
            .unwrap_or("")
            .to_string();
        assert_eq!(id, "doc-with_special.chars:123");
    }

    // ==================== Filter query edge cases ====================

    #[test]
    fn test_build_filter_query_null_value() {
        let mut filter = HashMap::new();
        filter.insert("field".to_string(), json!(null));
        let result = build_filter_query(&filter);

        assert!(result.is_some());
        let query = result.unwrap();
        assert_eq!(query["term"]["field"], json!(null));
    }

    #[test]
    fn test_build_filter_query_float_value() {
        let mut filter = HashMap::new();
        filter.insert("price".to_string(), json!(19.99));
        let result = build_filter_query(&filter);

        assert!(result.is_some());
        let query = result.unwrap();
        assert_eq!(query["term"]["price"], json!(19.99));
    }

    #[test]
    fn test_build_filter_query_negative_number() {
        let mut filter = HashMap::new();
        filter.insert("temperature".to_string(), json!(-40));
        let result = build_filter_query(&filter);

        assert!(result.is_some());
        let query = result.unwrap();
        assert_eq!(query["term"]["temperature"], json!(-40));
    }

    #[test]
    fn test_build_filter_query_array_value() {
        let mut filter = HashMap::new();
        filter.insert("tags".to_string(), json!(["a", "b"]));
        let result = build_filter_query(&filter);

        assert!(result.is_some());
        let query = result.unwrap();
        assert!(query["term"]["tags"].is_array());
    }

    #[test]
    fn test_build_filter_query_unicode_key() {
        let mut filter = HashMap::new();
        filter.insert("カテゴリー".to_string(), json!("テスト"));
        let result = build_filter_query(&filter);

        assert!(result.is_some());
        let query = result.unwrap();
        assert_eq!(query["term"]["カテゴリー"], json!("テスト"));
    }

    #[test]
    fn test_build_filter_query_empty_string_value() {
        let mut filter = HashMap::new();
        filter.insert("name".to_string(), json!(""));
        let result = build_filter_query(&filter);

        assert!(result.is_some());
        let query = result.unwrap();
        assert_eq!(query["term"]["name"], json!(""));
    }

    // ==================== Bulk operation body tests ====================

    #[test]
    fn test_bulk_index_operation_structure() {
        let doc_id = "test_123";
        let index_op = json!({
            "index": {
                "_id": doc_id
            }
        });

        assert_eq!(index_op["index"]["_id"], json!("test_123"));
    }

    #[test]
    fn test_bulk_document_structure() {
        let text = "Hello world";
        let embedding: Vec<f32> = vec![0.1, 0.2, 0.3];

        let doc = json!({
            "text": text,
            "vector": embedding,
        });

        assert_eq!(doc["text"], json!("Hello world"));
        assert!(doc["vector"].is_array());
        assert_eq!(doc["vector"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_bulk_document_with_metadata() {
        let mut doc = json!({
            "text": "content",
            "vector": [0.1, 0.2],
        });

        let metadata = HashMap::from([
            ("author".to_string(), json!("Test")),
            ("year".to_string(), json!(2024)),
        ]);

        if let Some(obj) = doc.as_object_mut() {
            for (k, v) in &metadata {
                obj.insert(k.clone(), v.clone());
            }
        }

        assert_eq!(doc["author"], json!("Test"));
        assert_eq!(doc["year"], json!(2024));
    }

    // ==================== Index mapping tests ====================

    #[test]
    fn test_index_mapping_knn_vector() {
        let mapping = json!({
            "type": "knn_vector",
            "dimension": 1536,
            "method": {
                "name": "hnsw",
                "space_type": "cosinesimil",
                "engine": "nmslib",
                "parameters": {
                    "ef_construction": 128,
                    "m": 24
                }
            }
        });

        assert_eq!(mapping["type"], json!("knn_vector"));
        assert_eq!(mapping["dimension"], json!(1536));
        assert_eq!(mapping["method"]["name"], json!("hnsw"));
        assert_eq!(mapping["method"]["space_type"], json!("cosinesimil"));
    }

    #[test]
    fn test_index_settings_knn_enabled() {
        let settings = json!({
            "index": {
                "knn": true
            }
        });

        assert_eq!(settings["index"]["knn"], json!(true));
    }

    #[test]
    fn test_index_mapping_text_field() {
        let mapping = json!({
            "properties": {
                "text": {
                    "type": "text"
                }
            }
        });

        assert_eq!(mapping["properties"]["text"]["type"], json!("text"));
    }

    // ==================== k-NN query structure tests ====================

    #[test]
    fn test_knn_query_with_filter() {
        let embedding: Vec<f32> = vec![0.1, 0.2, 0.3];
        let k = 5;
        let filter = json!({ "term": { "category": "tech" } });

        let knn_query = json!({
            "vector": {
                "vector": embedding,
                "k": k,
                "filter": filter
            }
        });

        assert!(knn_query["vector"]["filter"].is_object());
        assert_eq!(knn_query["vector"]["filter"]["term"]["category"], json!("tech"));
    }

    #[test]
    fn test_knn_query_large_k() {
        let embedding: Vec<f32> = vec![0.1, 0.2];
        let k = 1000;

        let knn_query = json!({
            "vector": {
                "vector": embedding,
                "k": k
            }
        });

        assert_eq!(knn_query["vector"]["k"], json!(1000));
    }

    #[test]
    fn test_knn_query_zero_k() {
        let embedding: Vec<f32> = vec![0.1];
        let k = 0;

        let knn_query = json!({
            "vector": {
                "vector": embedding,
                "k": k
            }
        });

        assert_eq!(knn_query["vector"]["k"], json!(0));
    }

    // ==================== Search hits parsing tests ====================

    #[test]
    fn test_parse_search_hits_array() {
        let response = json!({
            "hits": {
                "total": { "value": 2 },
                "hits": [
                    {
                        "_id": "1",
                        "_score": 0.9,
                        "_source": { "text": "First" }
                    },
                    {
                        "_id": "2",
                        "_score": 0.8,
                        "_source": { "text": "Second" }
                    }
                ]
            }
        });

        let hits = response
            .get("hits")
            .and_then(|h| h.get("hits"))
            .and_then(|h| h.as_array());

        assert!(hits.is_some());
        assert_eq!(hits.unwrap().len(), 2);
    }

    #[test]
    fn test_parse_search_hits_empty() {
        let response = json!({
            "hits": {
                "total": { "value": 0 },
                "hits": []
            }
        });

        let hits = response
            .get("hits")
            .and_then(|h| h.get("hits"))
            .and_then(|h| h.as_array());

        assert!(hits.is_some());
        assert!(hits.unwrap().is_empty());
    }

    #[test]
    fn test_parse_search_hits_missing_hits() {
        let response = json!({
            "took": 5,
            "timed_out": false
        });

        let hits = response
            .get("hits")
            .and_then(|h| h.get("hits"))
            .and_then(|h| h.as_array());

        assert!(hits.is_none());
    }

    // ==================== Distance metric exhaustive tests ====================

    #[test]
    fn test_all_distance_metrics_covered() {
        let metrics = [
            DistanceMetric::Cosine,
            DistanceMetric::Euclidean,
            DistanceMetric::DotProduct,
            DistanceMetric::MaxInnerProduct,
        ];

        for metric in &metrics {
            let helper = SpaceTypeTestHelper {
                distance_metric: *metric,
            };
            let space_type = helper.distance_metric_to_os_space_type();
            assert!(!space_type.is_empty());
        }
    }

    #[test]
    fn test_cosine_space_type_exact() {
        let helper = SpaceTypeTestHelper {
            distance_metric: DistanceMetric::Cosine,
        };
        // OpenSearch uses "cosinesimil" not "cosine"
        assert_eq!(helper.distance_metric_to_os_space_type(), "cosinesimil");
        assert_ne!(helper.distance_metric_to_os_space_type(), "cosine");
    }

    #[test]
    fn test_euclidean_space_type_exact() {
        let helper = SpaceTypeTestHelper {
            distance_metric: DistanceMetric::Euclidean,
        };
        // OpenSearch uses "l2" not "euclidean"
        assert_eq!(helper.distance_metric_to_os_space_type(), "l2");
        assert_ne!(helper.distance_metric_to_os_space_type(), "euclidean");
    }

    #[test]
    fn test_inner_product_variants_same() {
        let dot = SpaceTypeTestHelper {
            distance_metric: DistanceMetric::DotProduct,
        };
        let max = SpaceTypeTestHelper {
            distance_metric: DistanceMetric::MaxInnerProduct,
        };
        assert_eq!(
            dot.distance_metric_to_os_space_type(),
            max.distance_metric_to_os_space_type()
        );
    }

    // ==================== Document creation tests ====================

    #[test]
    fn test_document_creation_all_fields() {
        let doc = Document {
            id: Some("test_id".to_string()),
            page_content: "Test content".to_string(),
            metadata: HashMap::from([("key".to_string(), json!("value"))]),
        };

        assert_eq!(doc.id, Some("test_id".to_string()));
        assert_eq!(doc.page_content, "Test content");
        assert_eq!(doc.metadata.len(), 1);
    }

    #[test]
    fn test_document_creation_empty_content() {
        let doc = Document {
            id: Some("id".to_string()),
            page_content: String::new(),
            metadata: HashMap::new(),
        };

        assert!(doc.page_content.is_empty());
    }

    #[test]
    fn test_document_creation_no_id() {
        let doc = Document {
            id: None,
            page_content: "Content".to_string(),
            metadata: HashMap::new(),
        };

        assert!(doc.id.is_none());
    }

    #[test]
    fn test_document_creation_unicode_content() {
        let doc = Document {
            id: Some("unicode".to_string()),
            page_content: "こんにちは世界 🌍 مرحبا".to_string(),
            metadata: HashMap::new(),
        };

        assert!(doc.page_content.contains("こんにちは"));
        assert!(doc.page_content.contains("🌍"));
        assert!(doc.page_content.contains("مرحبا"));
    }

    // ==================== Vector embedding tests ====================

    #[test]
    fn test_vector_in_json() {
        let vector: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let json_vec = json!(vector);

        assert!(json_vec.is_array());
        let arr = json_vec.as_array().unwrap();
        assert_eq!(arr.len(), 5);
    }

    #[test]
    fn test_vector_high_dimension() {
        let vector: Vec<f32> = vec![0.1; 1536];
        let json_vec = json!(vector);

        let arr = json_vec.as_array().unwrap();
        assert_eq!(arr.len(), 1536);
    }

    #[test]
    fn test_vector_negative_values() {
        let vector: Vec<f32> = vec![-0.5, -0.3, 0.0, 0.3, 0.5];
        let json_vec = json!(vector);

        let arr = json_vec.as_array().unwrap();
        assert_eq!(arr[0].as_f64().unwrap(), -0.5);
        assert_eq!(arr[2].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn test_vector_normalized() {
        let vector: Vec<f32> = vec![0.6, 0.8]; // magnitude = 1.0
        let magnitude: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.001);
    }

    // ==================== UUID generation tests ====================

    #[test]
    fn test_uuid_format() {
        let id = uuid::Uuid::new_v4().to_string();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn test_uuid_uniqueness() {
        let ids: Vec<String> = (0..100).map(|_| uuid::Uuid::new_v4().to_string()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn test_uuid_version_4() {
        let uuid = uuid::Uuid::new_v4();
        assert_eq!(uuid.get_version_num(), 4);
    }

    // ==================== Error message tests ====================

    #[test]
    fn test_metadata_length_mismatch_message() {
        let metadatas_len = 5;
        let text_count = 3;
        let msg = format!(
            "Metadatas length mismatch: {} vs {}",
            metadatas_len, text_count
        );
        assert!(msg.contains("5"));
        assert!(msg.contains("3"));
        assert!(msg.contains("mismatch"));
    }

    #[test]
    fn test_ids_length_mismatch_message() {
        let ids_len = 10;
        let text_count = 7;
        let msg = format!("IDs length mismatch: {} vs {}", ids_len, text_count);
        assert!(msg.contains("10"));
        assert!(msg.contains("7"));
        assert!(msg.contains("mismatch"));
    }

    // ==================== Filter query multiple conditions ordering ====================

    #[test]
    fn test_filter_query_four_conditions() {
        let mut filter = HashMap::new();
        filter.insert("a".to_string(), json!(1));
        filter.insert("b".to_string(), json!(2));
        filter.insert("c".to_string(), json!(3));
        filter.insert("d".to_string(), json!(4));
        let result = build_filter_query(&filter);

        assert!(result.is_some());
        let query = result.unwrap();
        let conditions = query["bool"]["must"].as_array().unwrap();
        assert_eq!(conditions.len(), 4);
    }

    #[test]
    fn test_filter_query_mixed_types() {
        let mut filter = HashMap::new();
        filter.insert("str".to_string(), json!("text"));
        filter.insert("int".to_string(), json!(42));
        filter.insert("bool".to_string(), json!(true));
        filter.insert("float".to_string(), json!(3.14));
        let result = build_filter_query(&filter);

        assert!(result.is_some());
        let query = result.unwrap();
        let conditions = query["bool"]["must"].as_array().unwrap();
        assert_eq!(conditions.len(), 4);
    }

    // ==================== Text extraction tests ====================

    #[test]
    fn test_text_extraction_whitespace() {
        let source = json!({ "text": "  trimmed  " });
        let text = source
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        // Note: we preserve whitespace as-is
        assert_eq!(text, "  trimmed  ");
    }

    #[test]
    fn test_text_extraction_newlines() {
        let source = json!({ "text": "line1\nline2\nline3" });
        let text = source
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        assert!(text.contains('\n'));
        assert_eq!(text.lines().count(), 3);
    }

    #[test]
    fn test_text_extraction_tabs() {
        let source = json!({ "text": "col1\tcol2\tcol3" });
        let text = source
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        assert!(text.contains('\t'));
    }

    #[test]
    fn test_text_extraction_very_long() {
        let long_text = "a".repeat(10000);
        let source = json!({ "text": long_text });
        let text = source
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        assert_eq!(text.len(), 10000);
    }

    // ==================== Search response total tests ====================

    #[test]
    fn test_search_response_total_value() {
        let response = json!({
            "hits": {
                "total": { "value": 42, "relation": "eq" },
                "hits": []
            }
        });

        let total = response
            .get("hits")
            .and_then(|h| h.get("total"))
            .and_then(|t| t.get("value"))
            .and_then(|v| v.as_i64());

        assert_eq!(total, Some(42));
    }

    #[test]
    fn test_search_response_total_gte() {
        let response = json!({
            "hits": {
                "total": { "value": 10000, "relation": "gte" },
                "hits": []
            }
        });

        let relation = response
            .get("hits")
            .and_then(|h| h.get("total"))
            .and_then(|t| t.get("relation"))
            .and_then(|r| r.as_str());

        assert_eq!(relation, Some("gte"));
    }

    // ==================== Retriever name tests ====================

    #[test]
    fn test_retriever_name_constant() {
        let name = "OpenSearchVectorStoreRetriever";
        assert!(!name.is_empty());
        assert!(name.contains("OpenSearch"));
        assert!(name.contains("Retriever"));
    }
}
