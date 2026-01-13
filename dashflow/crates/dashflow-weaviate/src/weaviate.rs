//! Weaviate vector store implementation.
//!
//! This module provides the main `WeaviateVectorStore` struct for interacting with
//! a Weaviate vector database.

use dashflow::core::config::RunnableConfig;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::error::{Error, Result};
use dashflow::core::indexing::document_index::{DeleteResponse, DocumentIndex, UpsertResponse};
use dashflow::core::retrievers::Retriever;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore as VectorStoreTrait};
use dashflow::{embed, embed_query};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use weaviate_community::WeaviateClient;

/// A vector store backed by Weaviate.
///
/// Weaviate is an open-source vector database that supports:
/// - Dense vector search (embedding-based similarity)
/// - Hybrid search (combining vectors with BM25 keyword matching)
/// - GraphQL query interface
/// - Multi-tenancy support
/// - Rich metadata filtering
///
/// # Architecture
///
/// - **Client**: Uses `weaviate-community` for REST API communication with Weaviate server
/// - **Classes**: Data is stored in named classes (similar to tables/collections)
/// - **Objects**: Each document becomes an "object" with a UUID, vector, and properties
/// - **Properties**: Store document content and metadata as key-value pairs
///
/// # Current Status
///
/// - ✅ Struct definition and builder pattern
/// - ✅ Dense vector search (similarity_search, similarity_search_with_score)
/// - ✅ Vector search by embedding (similarity_search_by_vector)
/// - ⏳ Hybrid search (planned - requires GraphQL query builder)
/// - ⏳ MMR search (planned - requires maximal marginal relevance algorithm)
///
/// # Examples
///
/// ```ignore
/// use dashflow_weaviate::WeaviateVectorStore;
/// use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
/// // Connect to Weaviate and create a vector store
/// let store = WeaviateVectorStore::new(
///     "http://localhost:8080",
///     "MyDocuments",
///     embeddings,
/// ).await?;
///
/// // Add documents
/// let texts = vec!["Hello world", "Goodbye world"];
/// let ids = store.add_texts(&texts, None, None).await?;
///
/// // Search
/// let results = store._similarity_search("Hello", 2, None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct WeaviateVectorStore {
    /// The Weaviate client for REST API communication
    client: Arc<WeaviateClient>,

    /// Name of the class in Weaviate (equivalent to collection/table)
    class_name: String,

    /// Embeddings provider for dense vectors
    embeddings: Arc<dyn Embeddings>,

    /// Distance metric for similarity calculations
    distance_metric: DistanceMetric,

    /// Key for document content in object properties (default: "text")
    text_key: String,

    /// Optional tenant name for multi-tenancy support
    tenant: Option<String>,
}

impl std::fmt::Debug for WeaviateVectorStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeaviateVectorStore")
            .field("class_name", &self.class_name)
            .field("distance_metric", &self.distance_metric)
            .field("text_key", &self.text_key)
            .field("tenant", &self.tenant)
            .finish_non_exhaustive()
    }
}

impl WeaviateVectorStore {
    /// Creates a new `WeaviateVectorStore` by connecting to a Weaviate server.
    ///
    /// This constructor creates a client connection and sets up the vector store with
    /// default configuration. The class will be created automatically if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `url` - URL of the Weaviate server (e.g., "<http://localhost:8080>")
    /// * `class_name` - Name of the class to use (will be created if doesn't exist)
    /// * `embeddings` - Embeddings provider for vector generation
    ///
    /// # Returns
    ///
    /// Returns a `Result` with the created `WeaviateVectorStore` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to connect to Weaviate server
    /// - Invalid URL format
    /// - Class creation fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use dashflow_weaviate::WeaviateVectorStore;
    /// use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
    /// let store = WeaviateVectorStore::new(
    ///     "http://localhost:8080",
    ///     "MyDocuments",
    ///     embeddings,
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(
        url: &str,
        class_name: impl Into<String>,
        embeddings: Arc<dyn Embeddings>,
    ) -> Result<Self> {
        let client = WeaviateClient::builder(url)
            .build()
            .map_err(|e| Error::config(format!("Failed to create Weaviate client: {e}")))?;

        Ok(Self {
            client: Arc::new(client),
            class_name: class_name.into(),
            embeddings,
            distance_metric: DistanceMetric::Cosine, // Default to Cosine (most common)
            text_key: "text".to_string(),            // Matches common convention
            tenant: None,
        })
    }

    /// Returns the class name.
    #[must_use]
    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    /// Returns the distance metric.
    #[must_use]
    pub fn distance_metric(&self) -> DistanceMetric {
        self.distance_metric
    }

    /// Returns a reference to the embeddings provider.
    #[must_use]
    pub fn embeddings(&self) -> &Arc<dyn Embeddings> {
        &self.embeddings
    }

    /// Returns the text key used in object properties.
    #[must_use]
    pub fn text_key(&self) -> &str {
        &self.text_key
    }

    /// Returns the tenant name if multi-tenancy is enabled.
    #[must_use]
    pub fn tenant(&self) -> Option<&str> {
        self.tenant.as_deref()
    }

    /// Sets the distance metric.
    ///
    /// **Note**: This should match the distance metric configured for the
    /// class in Weaviate. The default is Cosine.
    #[must_use]
    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Sets the text key used in object properties.
    ///
    /// Default is "text".
    pub fn with_text_key(mut self, key: impl Into<String>) -> Self {
        self.text_key = key.into();
        self
    }

    /// Sets the tenant name for multi-tenancy support.
    ///
    /// When set, all operations will be scoped to this tenant.
    pub fn with_tenant(mut self, tenant: impl Into<String>) -> Self {
        self.tenant = Some(tenant.into());
        self
    }
}

/// Implementation of Retriever trait for `WeaviateVectorStore`
///
/// Enables the Weaviate vector store to be used as a retriever in chains and workflows.
#[async_trait::async_trait]
impl Retriever for WeaviateVectorStore {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // Default to k=4 (standard retriever behavior)
        self._similarity_search(query, 4, None).await
    }

    fn name(&self) -> String {
        "WeaviateVectorStore".to_string()
    }
}

/// Implementation of `DocumentIndex` trait for `WeaviateVectorStore`
///
/// This enables the Weaviate vector store to be used with the document indexing API,
/// providing intelligent change detection and cleanup of outdated documents.
#[async_trait::async_trait]
impl DocumentIndex for WeaviateVectorStore {
    async fn upsert(
        &self,
        items: &[Document],
    ) -> std::result::Result<UpsertResponse, Box<dyn std::error::Error + Send + Sync>> {
        if items.is_empty() {
            return Ok(UpsertResponse::all_succeeded(vec![]));
        }

        // Extract IDs from documents
        let ids: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(i, doc)| doc.id.clone().unwrap_or_else(|| format!("doc_{i}")))
            .collect();

        // Convert documents to texts and metadata
        let texts: Vec<String> = items.iter().map(|doc| doc.page_content.clone()).collect();
        let metadatas: Vec<HashMap<String, JsonValue>> =
            items.iter().map(|doc| doc.metadata.clone()).collect();

        // Call add_texts with the extracted data
        match self
            .add_texts_internal(&texts, Some(&metadatas), Some(&ids))
            .await
        {
            Ok(_) => Ok(UpsertResponse::all_succeeded(ids)),
            Err(_) => Ok(UpsertResponse::all_failed(ids)),
        }
    }

    async fn delete(
        &self,
        ids: Option<&[String]>,
    ) -> std::result::Result<DeleteResponse, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ids) = ids {
            if ids.is_empty() {
                return Ok(DeleteResponse::with_count(0));
            }

            match self.delete_internal(ids).await {
                Ok(()) => Ok(DeleteResponse::with_count(ids.len())),
                Err(e) => Err(Box::new(e)),
            }
        } else {
            // Weaviate requires explicit IDs for deletion
            Err(Box::new(Error::InvalidInput(
                "Weaviate requires explicit IDs for deletion".to_string(),
            )))
        }
    }

    async fn get(
        &self,
        ids: &[String],
    ) -> std::result::Result<Vec<Document>, Box<dyn std::error::Error + Send + Sync>> {
        self.get_by_ids(ids)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

/// Implementation of `VectorStore` trait for `WeaviateVectorStore`
#[async_trait::async_trait]
impl VectorStoreTrait for WeaviateVectorStore {
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
        // Convert texts to owned Strings
        let text_strings: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();

        // Call internal implementation
        self.add_texts_internal(&text_strings, metadatas, ids).await
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        match ids {
            Some(ids) if !ids.is_empty() => {
                self.delete_internal(ids).await?;
                Ok(true)
            }
            Some(_) => Ok(true), // Empty list, nothing to delete
            None => Err(Error::NotImplemented(
                "delete all documents not supported for Weaviate".to_string(),
            )),
        }
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        WeaviateVectorStore::get_by_ids(self, ids).await
    }

    async fn _similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        // Call the inherent implementation method
        WeaviateVectorStore::_similarity_search_impl(self, query, k, filter.cloned()).await
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        WeaviateVectorStore::similarity_search_with_score_impl(self, query, k, filter.cloned())
            .await
    }

    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        WeaviateVectorStore::similarity_search_by_vector_impl(self, embedding, k, filter.cloned())
            .await
    }
}

// Internal implementation methods
impl WeaviateVectorStore {
    fn escape_graphql_string_literal(value: &str) -> String {
        let mut escaped = String::with_capacity(value.len());
        for ch in value.chars() {
            match ch {
                '\\' => escaped.push_str("\\\\"),
                '"' => escaped.push_str("\\\""),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\t' => escaped.push_str("\\t"),
                _ => escaped.push(ch),
            }
        }
        escaped
    }

    fn build_near_text(query: &str) -> String {
        let escaped = Self::escape_graphql_string_literal(query);
        format!(r#"{{concepts: ["{escaped}"]}}"#)
    }

    /// Internal method to add texts (compatible with `DocumentIndex`)
    async fn add_texts_internal(
        &self,
        texts: &[String],
        metadatas: Option<&[HashMap<String, JsonValue>]>,
        ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
        use uuid::Uuid;
        use weaviate_community::collections::objects::{MultiObjects, Object};

        if texts.is_empty() {
            return Ok(vec![]);
        }

        // Generate embeddings for all texts using graph API
        let embeddings = embed(Arc::clone(&self.embeddings), texts).await?;

        // Build Weaviate objects
        let mut objects = Vec::with_capacity(texts.len());
        let mut result_ids = Vec::with_capacity(texts.len());

        for (i, text) in texts.iter().enumerate() {
            // Create properties JSON with text and metadata
            let mut properties = serde_json::Map::new();
            properties.insert(self.text_key.clone(), JsonValue::String(text.clone()));

            // Add metadata if provided
            if let Some(metadatas) = metadatas {
                if i < metadatas.len() {
                    for (key, value) in &metadatas[i] {
                        properties.insert(key.clone(), value.clone());
                    }
                }
            }

            // Generate or use provided ID
            let id = if let Some(ids) = ids {
                if i < ids.len() {
                    Uuid::parse_str(&ids[i]).unwrap_or_else(|_| Uuid::new_v4())
                } else {
                    Uuid::new_v4()
                }
            } else {
                Uuid::new_v4()
            };

            // Convert f32 embeddings to f64 (Weaviate client uses f64)
            let vector: Vec<f64> = embeddings[i].iter().map(|&v| f64::from(v)).collect();

            // Build the object
            let mut obj_builder = Object::builder(&self.class_name, JsonValue::Object(properties))
                .with_id(id)
                .with_vector(vector);

            // Add tenant if configured
            if let Some(tenant) = &self.tenant {
                obj_builder = obj_builder.with_tenant(tenant);
            }

            let obj = obj_builder.build();
            objects.push(obj);
            result_ids.push(id.to_string());
        }

        // Batch add objects to Weaviate
        let multi_objects = MultiObjects::new(objects);
        let tenant = self.tenant.as_deref();

        self.client
            .batch
            .objects_batch_add(multi_objects, None, tenant)
            .await
            .map_err(|e| Error::api(format!("Failed to add objects to Weaviate: {e}")))?;

        Ok(result_ids)
    }

    /// Internal method to delete objects by IDs
    async fn delete_internal(&self, ids: &[String]) -> Result<()> {
        use uuid::Uuid;

        if ids.is_empty() {
            return Ok(());
        }

        // Delete each object individually
        // Note: Weaviate batch delete requires complex filter syntax,
        // so we use individual deletes for simplicity and reliability
        let tenant = self.tenant.as_deref();

        for id_str in ids {
            let id = Uuid::parse_str(id_str)
                .map_err(|e| Error::InvalidInput(format!("Invalid UUID: {e}")))?;

            self.client
                .objects
                .delete(&self.class_name, &id, None, tenant)
                .await
                .map_err(|e| Error::api(format!("Failed to delete object {id_str}: {e}")))?;
        }

        Ok(())
    }

    /// Retrieves documents from Weaviate by their IDs.
    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        use uuid::Uuid;

        if ids.is_empty() {
            return Ok(vec![]);
        }

        let tenant = self.tenant.as_deref();
        let mut documents = Vec::new();

        for id_str in ids {
            let id = Uuid::parse_str(id_str)
                .map_err(|e| Error::InvalidInput(format!("Invalid UUID: {e}")))?;

            match self
                .client
                .objects
                .get(&self.class_name, &id, None, None, tenant)
                .await
            {
                Ok(obj) => {
                    // Extract text from properties
                    let text = obj
                        .properties
                        .get(&self.text_key)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    // Extract metadata from properties (excluding text_key)
                    let mut metadata = HashMap::new();
                    if let Some(props) = obj.properties.as_object() {
                        for (key, value) in props {
                            if key != &self.text_key {
                                metadata.insert(key.clone(), value.clone());
                            }
                        }
                    }

                    documents.push(Document {
                        page_content: text,
                        metadata,
                        id: Some(id_str.clone()),
                    });
                }
                Err(_) => {
                    // Skip documents that don't exist (rather than failing the entire operation)
                    continue;
                }
            }
        }

        Ok(documents)
    }

    /// Searches for documents similar to the given query text.
    async fn _similarity_search_impl(
        &self,
        query: &str,
        k: usize,
        _filter: Option<HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        use weaviate_community::collections::query::GetBuilder;

        // Build GraphQL query using nearText
        let near_text = Self::build_near_text(query);

        let mut query_builder = GetBuilder::new(&self.class_name, vec![&self.text_key])
            .with_near_text(&near_text)
            .with_limit(k as u32);

        // Add tenant if configured
        if let Some(tenant) = &self.tenant {
            query_builder = query_builder.with_tenant(tenant);
        }

        let graphql_query = query_builder.build();

        // Execute the query
        let result = self
            .client
            .query
            .get(graphql_query)
            .await
            .map_err(|e| Error::api(format!("Failed to execute Weaviate query: {e}")))?;

        // Parse the results
        self.parse_query_results(&result)
    }

    /// Searches for documents similar to the given query text, returning scores.
    async fn similarity_search_with_score_impl(
        &self,
        query: &str,
        k: usize,
        _filter: Option<HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        use weaviate_community::collections::query::GetBuilder;

        // Build GraphQL query using nearText with vector in additional
        let near_text = Self::build_near_text(query);

        let mut query_builder = GetBuilder::new(&self.class_name, vec![&self.text_key])
            .with_near_text(&near_text)
            .with_limit(k as u32)
            .with_additional(vec!["vector", "distance"]);

        // Add tenant if configured
        if let Some(tenant) = &self.tenant {
            query_builder = query_builder.with_tenant(tenant);
        }

        let graphql_query = query_builder.build();

        // Execute the query
        let result = self
            .client
            .query
            .get(graphql_query)
            .await
            .map_err(|e| Error::api(format!("Failed to execute Weaviate query: {e}")))?;

        // Generate query embedding for score calculation using graph API
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query).await?;

        // Parse the results with scores
        self.parse_query_results_with_scores(&result, &query_embedding)
    }

    /// Searches for documents similar to the given embedding vector.
    async fn similarity_search_by_vector_impl(
        &self,
        embedding: &[f32],
        k: usize,
        _filter: Option<HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        use weaviate_community::collections::query::GetBuilder;

        // Convert f32 vector to f64 and format for GraphQL
        let vector_f64: Vec<f64> = embedding.iter().map(|&v| f64::from(v)).collect();
        let vector_str = serde_json::to_string(&vector_f64)
            .map_err(|e| Error::other(format!("Failed to serialize vector: {e}")))?;

        let near_vector = format!(r"{{vector: {vector_str}}}");

        let mut query_builder = GetBuilder::new(&self.class_name, vec![&self.text_key])
            .with_near_vector(&near_vector)
            .with_limit(k as u32);

        // Add tenant if configured
        if let Some(tenant) = &self.tenant {
            query_builder = query_builder.with_tenant(tenant);
        }

        let graphql_query = query_builder.build();

        // Execute the query
        let result = self
            .client
            .query
            .get(graphql_query)
            .await
            .map_err(|e| Error::api(format!("Failed to execute Weaviate query: {e}")))?;

        // Parse the results
        self.parse_query_results(&result)
    }

    /// Helper method to parse Weaviate GraphQL query results into Documents
    fn parse_query_results(&self, result: &JsonValue) -> Result<Vec<Document>> {
        // Weaviate returns results in format: {"data": {"Get": {"ClassName": [...]}}}
        let data = result
            .get("data")
            .and_then(|d| d.get("Get"))
            .and_then(|g| g.get(&self.class_name))
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                Error::api_format(format!("Invalid Weaviate response format: {result}"))
            })?;

        let mut documents = Vec::new();
        for obj in data {
            let text = obj
                .get(&self.text_key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Extract all properties as metadata (except text_key)
            let mut metadata = HashMap::new();
            if let Some(obj_map) = obj.as_object() {
                for (key, value) in obj_map {
                    if key != &self.text_key && key != "_additional" {
                        metadata.insert(key.clone(), value.clone());
                    }
                }
            }

            documents.push(Document {
                page_content: text,
                metadata,
                id: None,
            });
        }

        Ok(documents)
    }

    /// Helper method to parse Weaviate GraphQL query results with scores
    fn parse_query_results_with_scores(
        &self,
        result: &JsonValue,
        query_embedding: &[f32],
    ) -> Result<Vec<(Document, f32)>> {
        // Weaviate returns results in format: {"data": {"Get": {"ClassName": [...]}}}
        let data = result
            .get("data")
            .and_then(|d| d.get("Get"))
            .and_then(|g| g.get(&self.class_name))
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                Error::api_format(format!("Invalid Weaviate response format: {result}"))
            })?;

        let mut documents = Vec::new();
        for obj in data {
            let text = obj
                .get(&self.text_key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Extract all properties as metadata (except text_key and _additional)
            let mut metadata = HashMap::new();
            if let Some(obj_map) = obj.as_object() {
                for (key, value) in obj_map {
                    if key != &self.text_key && key != "_additional" {
                        metadata.insert(key.clone(), value.clone());
                    }
                }
            }

            // Calculate score from vector dot product
            let score = if let Some(additional) = obj.get("_additional") {
                if let Some(vector) = additional.get("vector").and_then(|v| v.as_array()) {
                    let doc_embedding: Vec<f32> = vector
                        .iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect();

                    // Dot product for similarity score
                    if doc_embedding.len() == query_embedding.len() {
                        query_embedding
                            .iter()
                            .zip(doc_embedding.iter())
                            .map(|(a, b)| a * b)
                            .sum()
                    } else {
                        0.0
                    }
                } else if let Some(distance) = additional
                    .get("distance")
                    .and_then(serde_json::Value::as_f64)
                {
                    // Use distance if provided (convert to similarity)
                    1.0 - distance as f32
                } else {
                    0.0
                }
            } else {
                0.0
            };

            documents.push((
                Document {
                    page_content: text,
                    metadata,
                    id: None,
                },
                score,
            ));
        }

        Ok(documents)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow_test_utils::MockEmbeddings;
    use serde_json::json;
    use std::sync::Arc;

    /// Helper function to create mock embeddings for testing
    fn get_embeddings_for_test() -> Arc<dyn Embeddings> {
        Arc::new(MockEmbeddings::with_dimensions(3))
    }

    async fn new_store_for_test(class_name: &str) -> WeaviateVectorStore {
        WeaviateVectorStore::new(
            "http://localhost:8080",
            class_name.to_string(),
            get_embeddings_for_test(),
        )
        .await
        .unwrap()
    }

    // ==================== Construction Tests ====================

    #[tokio::test]
    async fn test_new_creates_store() {
        let store = new_store_for_test("TestClass").await;
        assert_eq!(store.class_name(), "TestClass");
        assert_eq!(store.distance_metric(), DistanceMetric::Cosine);
        assert_eq!(store.text_key(), "text");
        assert_eq!(store.tenant(), None);
    }

    #[tokio::test]
    async fn test_new_rejects_invalid_url() {
        let err = WeaviateVectorStore::new("not a url", "TestClass", get_embeddings_for_test())
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Configuration(_)));
        assert!(err.to_string().contains("Failed to create Weaviate client"));
    }

    #[tokio::test]
    async fn test_embeddings_getter_is_stable_arc() {
        let store = new_store_for_test("TestClass").await;
        let embeddings_ref = Arc::clone(store.embeddings());
        let from_trait = VectorStoreTrait::embeddings(&store).unwrap();
        assert!(Arc::ptr_eq(&embeddings_ref, &from_trait));
    }

    #[tokio::test]
    async fn test_distance_metric_matches_trait() {
        let store = new_store_for_test("TestClass")
            .await
            .with_distance_metric(DistanceMetric::DotProduct);
        assert_eq!(store.distance_metric(), DistanceMetric::DotProduct);
        assert_eq!(
            VectorStoreTrait::distance_metric(&store),
            DistanceMetric::DotProduct
        );
    }

    // ==================== Clone / Debug Tests ====================

    #[tokio::test]
    async fn test_clone_preserves_configuration() {
        let store = new_store_for_test("TestClass")
            .await
            .with_distance_metric(DistanceMetric::Euclidean)
            .with_text_key("content")
            .with_tenant("tenant1");

        let cloned = store.clone();
        assert_eq!(cloned.class_name(), "TestClass");
        assert_eq!(cloned.distance_metric(), DistanceMetric::Euclidean);
        assert_eq!(cloned.text_key(), "content");
        assert_eq!(cloned.tenant(), Some("tenant1"));
    }

    #[tokio::test]
    async fn test_debug_format_contains_key_fields() {
        let store = new_store_for_test("TestClass").await;
        let debug_str = format!("{store:?}");
        assert!(debug_str.contains("WeaviateVectorStore"));
        assert!(debug_str.contains("TestClass"));
        assert!(debug_str.contains("Cosine"));
        assert!(debug_str.contains("text_key"));
        assert!(debug_str.contains("tenant"));
    }

    // ==================== Builder Pattern Tests ====================

    #[tokio::test]
    async fn test_with_distance_metric() {
        let store = new_store_for_test("TestClass")
            .await
            .with_distance_metric(DistanceMetric::DotProduct);
        assert_eq!(store.distance_metric(), DistanceMetric::DotProduct);
    }

    #[tokio::test]
    async fn test_with_text_key() {
        let store = new_store_for_test("TestClass").await.with_text_key("content");
        assert_eq!(store.text_key(), "content");
    }

    #[tokio::test]
    async fn test_with_tenant() {
        let store = new_store_for_test("TestClass").await.with_tenant("tenantA");
        assert_eq!(store.tenant(), Some("tenantA"));
    }

    #[tokio::test]
    async fn test_builder_chaining() {
        let store = new_store_for_test("TestClass")
            .await
            .with_distance_metric(DistanceMetric::Euclidean)
            .with_text_key("content")
            .with_tenant("tenantA");

        assert_eq!(store.distance_metric(), DistanceMetric::Euclidean);
        assert_eq!(store.text_key(), "content");
        assert_eq!(store.tenant(), Some("tenantA"));
    }

    // ==================== Non-Network Behavior Tests ====================

    #[tokio::test]
    async fn test_add_texts_empty_is_noop() {
        let mut store = new_store_for_test("TestClass").await;
        let ids = store
            .add_texts(&[] as &[&str], None, None)
            .await
            .unwrap();
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn test_vectorstore_delete_empty_is_true() {
        let mut store = new_store_for_test("TestClass").await;
        let ok = VectorStoreTrait::delete(&mut store, Some(&[])).await.unwrap();
        assert!(ok);
    }

    #[tokio::test]
    async fn test_vectorstore_delete_none_is_not_implemented() {
        let mut store = new_store_for_test("TestClass").await;
        let err = VectorStoreTrait::delete(&mut store, None).await.unwrap_err();
        assert!(matches!(err, Error::NotImplemented(_)));
    }

    #[tokio::test]
    async fn test_vectorstore_get_by_ids_empty_returns_empty() {
        let store = new_store_for_test("TestClass").await;
        let docs = VectorStoreTrait::get_by_ids(&store, &[]).await.unwrap();
        assert!(docs.is_empty());
    }

    #[tokio::test]
    async fn test_vectorstore_get_by_ids_invalid_uuid_errors() {
        let store = new_store_for_test("TestClass").await;
        let ids = vec![String::from("not-a-uuid")];
        let err = VectorStoreTrait::get_by_ids(&store, &ids).await.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
        assert!(err.to_string().contains("Invalid UUID"));
    }

    #[tokio::test]
    async fn test_vectorstore_delete_invalid_uuid_errors() {
        let mut store = new_store_for_test("TestClass").await;
        let ids = vec![String::from("not-a-uuid")];
        let err = VectorStoreTrait::delete(&mut store, Some(&ids))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
        assert!(err.to_string().contains("Invalid UUID"));
    }

    #[tokio::test]
    async fn test_document_index_upsert_empty_succeeds() {
        let store = new_store_for_test("TestClass").await;
        let response = DocumentIndex::upsert(&store, &[]).await.unwrap();
        assert!(response.is_success());
        assert_eq!(response.total(), 0);
        assert!(response.succeeded.is_empty());
        assert!(response.failed.is_empty());
    }

    #[tokio::test]
    async fn test_document_index_delete_empty_returns_zero_count() {
        let store = new_store_for_test("TestClass").await;
        let response = DocumentIndex::delete(&store, Some(&[])).await.unwrap();
        assert_eq!(response.num_deleted, Some(0));
    }

    #[tokio::test]
    async fn test_document_index_delete_requires_explicit_ids() {
        let store = new_store_for_test("TestClass").await;
        let err = DocumentIndex::delete(&store, None).await.unwrap_err();
        assert!(err.to_string().contains("Weaviate requires explicit IDs for deletion"));
    }

    #[tokio::test]
    async fn test_document_index_get_empty_returns_empty() {
        let store = new_store_for_test("TestClass").await;
        let docs = DocumentIndex::get(&store, &[]).await.unwrap();
        assert!(docs.is_empty());
    }

    #[tokio::test]
    async fn test_document_index_get_invalid_uuid_errors() {
        let store = new_store_for_test("TestClass").await;
        let ids = vec![String::from("not-a-uuid")];
        let err = DocumentIndex::get(&store, &ids).await.unwrap_err();
        assert!(err.to_string().contains("Invalid UUID"));
    }

    // ==================== GraphQL Formatting Tests ====================

    #[test]
    fn test_escape_graphql_string_literal_no_changes() {
        let input = "abc123_-";
        let escaped = WeaviateVectorStore::escape_graphql_string_literal(input);
        assert_eq!(escaped, input);
    }

    #[test]
    fn test_escape_graphql_string_literal_quotes_and_backslashes() {
        let input = r#"a"b\c"#;
        let escaped = WeaviateVectorStore::escape_graphql_string_literal(input);
        assert_eq!(escaped, r#"a\"b\\c"#);
    }

    #[test]
    fn test_escape_graphql_string_literal_control_chars() {
        let input = "a\nb\rc\t";
        let escaped = WeaviateVectorStore::escape_graphql_string_literal(input);
        assert_eq!(escaped, "a\\nb\\rc\\t");
    }

    #[test]
    fn test_escape_graphql_string_literal_unicode_unchanged() {
        let input = "こんにちは世界";
        let escaped = WeaviateVectorStore::escape_graphql_string_literal(input);
        assert_eq!(escaped, input);
    }

    #[test]
    fn test_build_near_text_basic() {
        let near_text = WeaviateVectorStore::build_near_text("hello");
        assert_eq!(near_text, r#"{concepts: ["hello"]}"#);
    }

    #[test]
    fn test_build_near_text_empty_string() {
        let near_text = WeaviateVectorStore::build_near_text("");
        assert_eq!(near_text, r#"{concepts: [""]}"#);
    }

    #[test]
    fn test_build_near_text_escapes_quotes() {
        let near_text = WeaviateVectorStore::build_near_text(r#"he said "hi""#);
        assert_eq!(near_text, r#"{concepts: ["he said \"hi\""]}"#);
    }

    #[test]
    fn test_build_near_text_escapes_newlines() {
        let near_text = WeaviateVectorStore::build_near_text("a\nb");
        assert_eq!(near_text, r#"{concepts: ["a\nb"]}"#);
    }

    // ==================== Parse Results Tests ====================

    #[tokio::test]
    async fn test_parse_query_results_single_document() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        { "text": "hello", "source": "unit-test" }
                    ]
                }
            }
        });

        let docs = store.parse_query_results(&response).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "hello");
        assert_eq!(docs[0].metadata.get("source").unwrap(), &json!("unit-test"));
        assert!(docs[0].id.is_none());
    }

    #[tokio::test]
    async fn test_parse_query_results_multiple_documents() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        { "text": "a" },
                        { "text": "b" },
                        { "text": "c", "n": 3 }
                    ]
                }
            }
        });

        let docs = store.parse_query_results(&response).unwrap();
        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].page_content, "a");
        assert_eq!(docs[1].page_content, "b");
        assert_eq!(docs[2].page_content, "c");
        assert_eq!(docs[2].metadata.get("n").unwrap(), &json!(3));
    }

    #[tokio::test]
    async fn test_parse_query_results_excludes_text_key_from_metadata() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        { "text": "hello", "text_key_shadow": "keep" }
                    ]
                }
            }
        });

        let docs = store.parse_query_results(&response).unwrap();
        assert_eq!(docs.len(), 1);
        assert!(docs[0].metadata.get("text").is_none());
        assert_eq!(
            docs[0].metadata.get("text_key_shadow").unwrap(),
            &json!("keep")
        );
    }

    #[tokio::test]
    async fn test_parse_query_results_excludes_additional_from_metadata() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        {
                            "text": "hello",
                            "_additional": { "distance": 0.1 },
                            "tag": "x"
                        }
                    ]
                }
            }
        });

        let docs = store.parse_query_results(&response).unwrap();
        assert_eq!(docs.len(), 1);
        assert!(docs[0].metadata.get("_additional").is_none());
        assert_eq!(docs[0].metadata.get("tag").unwrap(), &json!("x"));
    }

    #[tokio::test]
    async fn test_parse_query_results_missing_text_is_empty_string() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        { "source": "unit-test" }
                    ]
                }
            }
        });

        let docs = store.parse_query_results(&response).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "");
        assert_eq!(docs[0].metadata.get("source").unwrap(), &json!("unit-test"));
    }

    #[tokio::test]
    async fn test_parse_query_results_non_object_entries_are_handled() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        "not-an-object"
                    ]
                }
            }
        });

        let docs = store.parse_query_results(&response).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "");
        assert!(docs[0].metadata.is_empty());
    }

    #[tokio::test]
    async fn test_parse_query_results_invalid_format_errors() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({ "unexpected": true });
        let err = store.parse_query_results(&response).unwrap_err();
        assert!(matches!(err, Error::ApiFormat(_)));
        assert!(err.to_string().contains("Invalid Weaviate response format"));
    }

    #[tokio::test]
    async fn test_parse_query_results_class_name_mismatch_errors() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "OtherClass": [ { "text": "hello" } ]
                }
            }
        });

        let err = store.parse_query_results(&response).unwrap_err();
        assert!(matches!(err, Error::ApiFormat(_)));
    }

    #[tokio::test]
    async fn test_parse_query_results_custom_text_key() {
        let store = new_store_for_test("MyClass").await.with_text_key("content");
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        { "content": "hello", "tag": "x" }
                    ]
                }
            }
        });

        let docs = store.parse_query_results(&response).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "hello");
        assert_eq!(docs[0].metadata.get("tag").unwrap(), &json!("x"));
        assert!(docs[0].metadata.get("content").is_none());
    }

    // ==================== Parse Results With Scores Tests ====================

    #[tokio::test]
    async fn test_parse_query_results_with_scores_vector_dot_product() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        {
                            "text": "hello",
                            "_additional": { "vector": [1.0, 0.0, 0.0] }
                        }
                    ]
                }
            }
        });

        let results = store
            .parse_query_results_with_scores(&response, &[1.0, 2.0, 3.0])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.page_content, "hello");
        assert!((results[0].1 - 1.0).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn test_parse_query_results_with_scores_distance_fallback() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        {
                            "text": "hello",
                            "_additional": { "distance": 0.25 }
                        }
                    ]
                }
            }
        });

        let results = store
            .parse_query_results_with_scores(&response, &[0.0, 0.0, 0.0])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!((results[0].1 - 0.75).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_parse_query_results_with_scores_missing_additional_is_zero() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        { "text": "hello" }
                    ]
                }
            }
        });

        let results = store
            .parse_query_results_with_scores(&response, &[1.0, 1.0, 1.0])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 0.0);
    }

    #[tokio::test]
    async fn test_parse_query_results_with_scores_vector_length_mismatch_is_zero() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        {
                            "text": "hello",
                            "_additional": { "vector": [1.0, 2.0], "distance": 0.1 }
                        }
                    ]
                }
            }
        });

        let results = store
            .parse_query_results_with_scores(&response, &[1.0, 2.0, 3.0])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 0.0);
    }

    #[tokio::test]
    async fn test_parse_query_results_with_scores_vector_takes_precedence_over_distance() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        {
                            "text": "hello",
                            "_additional": { "vector": [1.0, 0.0, 0.0], "distance": 0.9 }
                        }
                    ]
                }
            }
        });

        let results = store
            .parse_query_results_with_scores(&response, &[2.0, 0.0, 0.0])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!((results[0].1 - 2.0).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn test_parse_query_results_with_scores_vector_with_non_numbers_is_zero() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        {
                            "text": "hello",
                            "_additional": { "vector": [1.0, "x", 3.0] }
                        }
                    ]
                }
            }
        });

        let results = store
            .parse_query_results_with_scores(&response, &[1.0, 2.0, 3.0])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 0.0);
    }

    #[tokio::test]
    async fn test_parse_query_results_with_scores_distance_can_go_negative() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        {
                            "text": "hello",
                            "_additional": { "distance": 2.0 }
                        }
                    ]
                }
            }
        });

        let results = store
            .parse_query_results_with_scores(&response, &[0.0, 0.0, 0.0])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, -1.0);
    }

    #[tokio::test]
    async fn test_parse_query_results_with_scores_additional_without_vector_or_distance_is_zero() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        {
                            "text": "hello",
                            "_additional": { "foo": "bar" }
                        }
                    ]
                }
            }
        });

        let results = store
            .parse_query_results_with_scores(&response, &[1.0, 1.0, 1.0])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 0.0);
    }

    #[tokio::test]
    async fn test_parse_query_results_with_scores_excludes_text_key_from_metadata() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        {
                            "text": "hello",
                            "_additional": { "distance": 0.1 }
                        }
                    ]
                }
            }
        });

        let results = store
            .parse_query_results_with_scores(&response, &[0.0, 0.0, 0.0])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].0.metadata.get("text").is_none());
    }

    #[tokio::test]
    async fn test_parse_query_results_with_scores_metadata_excludes_additional() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        {
                            "text": "hello",
                            "tag": "x",
                            "_additional": { "distance": 0.5 }
                        }
                    ]
                }
            }
        });

        let results = store
            .parse_query_results_with_scores(&response, &[0.0, 0.0, 0.0])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.metadata.get("tag").unwrap(), &json!("x"));
        assert!(results[0].0.metadata.get("_additional").is_none());
    }

    #[tokio::test]
    async fn test_parse_query_results_with_scores_custom_text_key() {
        let store = new_store_for_test("MyClass").await.with_text_key("content");
        let response = json!({
            "data": {
                "Get": {
                    "MyClass": [
                        {
                            "content": "hello",
                            "_additional": { "distance": 0.5 }
                        }
                    ]
                }
            }
        });

        let results = store
            .parse_query_results_with_scores(&response, &[0.0, 0.0, 0.0])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.page_content, "hello");
        assert!(results[0].0.metadata.get("content").is_none());
    }

    #[tokio::test]
    async fn test_parse_query_results_with_scores_invalid_format_errors() {
        let store = new_store_for_test("MyClass").await;
        let response = json!({ "unexpected": true });
        let err = store
            .parse_query_results_with_scores(&response, &[1.0, 2.0, 3.0])
            .unwrap_err();
        assert!(matches!(err, Error::ApiFormat(_)));
        assert!(err.to_string().contains("Invalid Weaviate response format"));
    }
}
