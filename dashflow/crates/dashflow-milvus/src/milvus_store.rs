//! DashFlow.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::{Error, Result};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use milvus::client::Client;
use milvus::collection::{Collection, SearchOption};
use milvus::data::FieldColumn;
use milvus::index::{IndexParams, IndexType, MetricType};
use milvus::schema::{CollectionSchema, CollectionSchemaBuilder, FieldSchema};
use milvus::value::Value;

/// Field names used in Milvus collections
const ID_FIELD: &str = "id";
const TEXT_FIELD: &str = "text";
const VECTOR_FIELD: &str = "vector";
const METADATA_FIELD: &str = "metadata";

/// Milvus vector database implementation.
///
/// This implementation uses Milvus's native vector search capabilities for efficient
/// similarity search. Milvus is a cloud-native vector database that supports various
/// indexing algorithms including IVF_FLAT, IVF_SQ8, IVF_PQ, HNSW, and more.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_milvus::MilvusVectorStore;
/// use dashflow::core::embeddings::Embeddings;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # struct MockEmbeddings;
/// # #[async_trait::async_trait]
/// # impl Embeddings for MockEmbeddings {
/// #     async fn embed_documents(&self, texts: &[String]) -> dashflow::core::Result<Vec<Vec<f32>>> {
/// #         Ok(vec![vec![0.0; 384]; texts.len()])
/// #     }
/// #     async fn embed_query(&self, text: &str) -> dashflow::core::Result<Vec<f32>> {
/// #         Ok(vec![0.0; 384])
/// #     }
/// # }
/// let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
///
/// let mut store = MilvusVectorStore::new(
///     "http://localhost:19530",
///     "documents",
///     embeddings,
///     384,  // embedding dimension
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub struct MilvusVectorStore {
    client: Client,
    collection_name: String,
    collection_schema: CollectionSchema,
    embeddings: Arc<dyn Embeddings>,
    distance_metric: DistanceMetric,
    /// Embedding vector dimension (stored for reference)
    ///
    /// Embedding dimension for potential runtime schema validation
    #[allow(dead_code)] // Architectural: Reserved for runtime dimension validation before insertion
    dimension: i64,
}

impl MilvusVectorStore {
    /// Get the Milvus collection instance.
    async fn get_collection(&self) -> Result<Collection> {
        self.client
            .get_collection(&self.collection_name)
            .await
            .map_err(|e| Error::api(format!("Failed to get collection: {}", e)))
    }
}

impl MilvusVectorStore {
    /// Creates a new MilvusVectorStore instance.
    ///
    /// # Arguments
    ///
    /// * `url` - Milvus server URL (e.g., "http://localhost:19530")
    /// * `collection_name` - Name of the Milvus collection to store documents in
    /// * `embeddings` - Embeddings model to use
    /// * `dimension` - Dimension of the embedding vectors
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Connection to Milvus fails
    /// - Collection creation fails
    /// - Index creation fails
    ///
    /// # Note
    ///
    /// This will automatically create the collection if it doesn't exist.
    /// The collection schema includes:
    /// - `id` field (VARCHAR, primary key)
    /// - `text` field (VARCHAR)
    /// - `vector` field (FloatVector with specified dimension)
    /// - `metadata` field (JSON)
    pub async fn new(
        url: &str,
        collection_name: &str,
        embeddings: Arc<dyn Embeddings>,
        dimension: i64,
    ) -> Result<Self> {
        let client = Client::new(url.to_string())
            .await
            .map_err(|e| Error::api(format!("Failed to connect to Milvus: {}", e)))?;

        let collection_schema = Self::create_collection_schema(collection_name, dimension)?;

        // Check if collection exists
        let has_collection = client
            .has_collection(collection_name)
            .await
            .map_err(|e| Error::api(format!("Failed to check collection existence: {}", e)))?;

        if !has_collection {
            // Create collection
            client
                .create_collection(collection_schema.clone(), None)
                .await
                .map_err(|e| Error::api(format!("Failed to create collection: {}", e)))?;

            // Get collection to perform operations on it
            let collection = client
                .get_collection(collection_name)
                .await
                .map_err(|e| Error::api(format!("Failed to get collection: {}", e)))?;

            // Create index on vector field
            let mut params_map = HashMap::new();
            params_map.insert("nlist".to_string(), "128".to_string());

            let index_params = IndexParams::new(
                "vector_index".to_string(),
                IndexType::IvfFlat,
                MetricType::L2,
                params_map,
            );

            collection
                .create_index(VECTOR_FIELD, index_params)
                .await
                .map_err(|e| Error::api(format!("Failed to create index: {}", e)))?;

            // Load collection into memory
            collection
                .load(1) // replica number
                .await
                .map_err(|e| Error::api(format!("Failed to load collection: {}", e)))?;
        }

        Ok(Self {
            client,
            collection_name: collection_name.to_string(),
            collection_schema,
            embeddings,
            distance_metric: DistanceMetric::Euclidean, // IVF_FLAT typically uses L2
            dimension,
        })
    }

    /// Creates the collection schema for Milvus.
    fn create_collection_schema(
        collection_name: &str,
        dimension: i64,
    ) -> Result<CollectionSchema> {
        CollectionSchemaBuilder::new(collection_name, "DashFlow document collection")
            .add_field(FieldSchema::new_primary_varchar(
                ID_FIELD,
                "Document ID",
                false, // not auto_id
                256,   // max length
            ))
            .add_field(FieldSchema::new_varchar(TEXT_FIELD, "Document text", 65535))
            .add_field(FieldSchema::new_float_vector(
                VECTOR_FIELD,
                "Embedding vector",
                dimension,
            ))
            .add_field(FieldSchema::new_string(METADATA_FIELD, "Document metadata"))
            .build()
            .map_err(|e| Error::config(format!("Failed to build collection schema: {}", e)))
    }

    /// Sets the distance metric for this store.
    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Converts DistanceMetric to Milvus MetricType.
    fn metric_to_milvus(&self) -> MetricType {
        match self.distance_metric {
            DistanceMetric::Euclidean => MetricType::L2,
            DistanceMetric::Cosine => MetricType::IP, // Cosine not available, use IP
            DistanceMetric::DotProduct | DistanceMetric::MaxInnerProduct => MetricType::IP,
        }
    }
}

#[async_trait]
impl VectorStore for MilvusVectorStore {
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
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Validate input lengths
        if let Some(metas) = metadatas {
            if metas.len() != texts.len() {
                return Err(Error::config(format!(
                    "Metadatas length ({}) must match texts length ({})",
                    metas.len(),
                    texts.len()
                )));
            }
        }

        if let Some(ids_vec) = ids {
            if ids_vec.len() != texts.len() {
                return Err(Error::config(format!(
                    "IDs length ({}) must match texts length ({})",
                    ids_vec.len(),
                    texts.len()
                )));
            }
        }

        // Generate embeddings
        let text_strings: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();
        let embeddings = self
            .embeddings
            .embed_documents(&text_strings)
            .await
            .map_err(|e| Error::other(format!("Failed to embed documents: {}", e)))?;

        // Prepare IDs
        let doc_ids: Vec<String> = if let Some(ids_vec) = ids {
            ids_vec.to_vec()
        } else {
            (0..texts.len()).map(|_| Uuid::new_v4().to_string()).collect()
        };

        // Prepare field data
        let id_field = self
            .collection_schema
            .get_field(ID_FIELD)
            .ok_or_else(|| Error::other("ID field not found in schema"))?;
        let text_field = self
            .collection_schema
            .get_field(TEXT_FIELD)
            .ok_or_else(|| Error::other("Text field not found in schema"))?;
        let vector_field = self
            .collection_schema
            .get_field(VECTOR_FIELD)
            .ok_or_else(|| Error::other("Vector field not found in schema"))?;
        let metadata_field = self
            .collection_schema
            .get_field(METADATA_FIELD)
            .ok_or_else(|| Error::other("Metadata field not found in schema"))?;

        // Flatten embeddings for Milvus (expects flat Vec<f32>)
        let flat_embeddings: Vec<f32> = embeddings.into_iter().flatten().collect();

        // Prepare metadata JSON strings
        let metadata_jsons: Vec<String> = if let Some(metas) = metadatas {
            metas
                .iter()
                .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string()))
                .collect()
        } else {
            vec!["{}".to_string(); texts.len()]
        };

        // Create field columns
        let columns = vec![
            FieldColumn::new(id_field, doc_ids.clone()),
            FieldColumn::new(text_field, text_strings),
            FieldColumn::new(vector_field, flat_embeddings),
            FieldColumn::new(metadata_field, metadata_jsons),
        ];

        // Get collection and insert data
        let collection = self.get_collection().await?;

        collection
            .insert(columns, None)
            .await
            .map_err(|e| Error::api(format!("Failed to insert documents: {}", e)))?;

        // Flush to persist
        collection
            .flush()
            .await
            .map_err(|e| Error::api(format!("Failed to flush collection: {}", e)))?;

        Ok(doc_ids)
    }

    async fn _similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        let results = self
            .similarity_search_with_score(query, k, filter)
            .await?;
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        // Embed query
        let query_embedding = self
            .embeddings
            .embed_query(query)
            .await
            .map_err(|e| Error::other(format!("Failed to embed query: {}", e)))?;

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
        // Prepare search vector
        let search_vector = Value::from(embedding.to_vec());

        // Build search options
        let mut search_option = SearchOption::new();

        // Add filter expression if provided
        if let Some(filter_map) = filter {
            let expr = Self::build_filter_expression(filter_map)?;
            search_option.set_expr(expr);
        }

        // Perform search using Collection
        let collection = self.get_collection().await?;

        let results = collection
            .search(
                vec![search_vector],
                VECTOR_FIELD,
                k as i32,
                self.metric_to_milvus(),
                vec![ID_FIELD, TEXT_FIELD, METADATA_FIELD],
                &search_option,
            )
            .await
            .map_err(|e| Error::api(format!("Search failed: {}", e)))?;

        // Parse results
        let mut documents = Vec::new();

        for result in results {
            // Each SearchResult contains vectors of matches
            // Extract field column vectors
            let id_col = result
                .field
                .iter()
                .find(|f| f.name == ID_FIELD)
                .ok_or_else(|| Error::other("ID field not found in search results"))?;

            let text_col = result
                .field
                .iter()
                .find(|f| f.name == TEXT_FIELD)
                .ok_or_else(|| Error::other("Text field not found in search results"))?;

            let metadata_col = result
                .field
                .iter()
                .find(|f| f.name == METADATA_FIELD)
                .ok_or_else(|| Error::other("Metadata field not found in search results"))?;

            // Extract vectors from each column
            let id_vec: Vec<String> = id_col
                .value
                .clone()
                .try_into()
                .map_err(|e| Error::other(format!("Failed to parse ID field: {e:?}")))?;

            let text_vec: Vec<String> = text_col
                .value
                .clone()
                .try_into()
                .map_err(|e| Error::other(format!("Failed to parse text field: {e:?}")))?;

            let metadata_vec: Vec<String> = metadata_col
                .value
                .clone()
                .try_into()
                .map_err(|e| Error::other(format!("Failed to parse metadata field: {e:?}")))?;

            // Iterate through each match in this SearchResult
            for (idx, ((id, text), metadata_str)) in id_vec
                .into_iter()
                .zip(text_vec.into_iter())
                .zip(metadata_vec.into_iter())
                .enumerate()
            {
                let metadata: HashMap<String, JsonValue> =
                    serde_json::from_str(&metadata_str).unwrap_or_default();

                // Get score for this match
                let score = result.score.get(idx).copied().unwrap_or(0.0);

                // Convert distance to relevance score [0, 1]
                let relevance_score = self.distance_metric.distance_to_relevance(score);

                let document = Document {
                    id: Some(id),
                    page_content: text,
                    metadata,
                };

                documents.push((document, relevance_score));
            }
        }

        Ok(documents)
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        if let Some(ids_to_delete) = ids {
            if ids_to_delete.is_empty() {
                return Ok(true);
            }

            // Build delete expression: id in ["id1", "id2", ...]
            let ids_str = ids_to_delete
                .iter()
                .map(|id| format!("\"{}\"", id))
                .collect::<Vec<_>>()
                .join(", ");
            let expr = format!("{} in [{}]", ID_FIELD, ids_str);

            // Get collection and delete
            let collection = self
                .client
                .get_collection(&self.collection_name)
                .await
                .map_err(|e| Error::api(format!("Failed to get collection: {}", e)))?;

            collection
                .delete(&expr, None)
                .await
                .map_err(|e| Error::api(format!("Failed to delete documents: {}", e)))?;

            collection
                .flush()
                .await
                .map_err(|e| Error::api(format!("Failed to flush after delete: {}", e)))?;

            Ok(true)
        } else {
            // Delete all - drop and recreate collection
            self.client
                .drop_collection(&self.collection_name)
                .await
                .map_err(|e| Error::api(format!("Failed to drop collection: {}", e)))?;

            // Recreate collection
            self.client
                .create_collection(self.collection_schema.clone(), None)
                .await
                .map_err(|e| Error::api(format!("Failed to recreate collection: {}", e)))?;

            Ok(true)
        }
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        // Build query expression: id in ["id1", "id2", ...]
        let ids_str = ids
            .iter()
            .map(|id| format!("\"{}\"", id))
            .collect::<Vec<_>>()
            .join(", ");
        let expr = format!("{} in [{}]", ID_FIELD, ids_str);

        // Query documents using collection (no output fields parameter in 0.1.0)
        let collection = self.get_collection().await?;

        let results = collection
            .query(&expr, Vec::<String>::new()) // empty partition names
            .await
            .map_err(|e| Error::api(format!("Query failed: {}", e)))?;

        // Parse results into documents
        // Results are Vec<FieldColumn>, need to transpose into per-doc format
        let mut documents = Vec::new();

        // Find each field column
        let id_col = results
            .iter()
            .find(|col| col.name == ID_FIELD)
            .ok_or_else(|| Error::other("ID field not found in query results"))?;

        let text_col = results
            .iter()
            .find(|col| col.name == TEXT_FIELD)
            .ok_or_else(|| Error::other("Text field not found in query results"))?;

        let metadata_col = results
            .iter()
            .find(|col| col.name == METADATA_FIELD)
            .ok_or_else(|| Error::other("Metadata field not found in query results"))?;

        // Extract vectors from each column
        let id_vec: Vec<String> = id_col
            .value
            .clone()
            .try_into()
            .map_err(|e| Error::other(format!("Failed to parse ID field: {e:?}")))?;

        let text_vec: Vec<String> = text_col
            .value
            .clone()
            .try_into()
            .map_err(|e| Error::other(format!("Failed to parse text field: {e:?}")))?;

        let metadata_vec: Vec<String> = metadata_col
            .value
            .clone()
            .try_into()
            .map_err(|e| Error::other(format!("Failed to parse metadata field: {e:?}")))?;

        // Combine into documents
        for ((id, text), metadata_str) in id_vec
            .into_iter()
            .zip(text_vec.into_iter())
            .zip(metadata_vec.into_iter())
        {
            let metadata: HashMap<String, JsonValue> =
                serde_json::from_str(&metadata_str).unwrap_or_default();

            documents.push(Document {
                id: Some(id),
                page_content: text,
                metadata,
            });
        }

        Ok(documents)
    }
}

impl MilvusVectorStore {
    /// Builds a Milvus filter expression from a metadata filter map.
    ///
    /// Converts HashMap<String, JsonValue> to Milvus expression syntax.
    /// Example: {"category": "tech"} -> "metadata['category'] == 'tech'"
    fn build_filter_expression(filter: &HashMap<String, JsonValue>) -> Result<String> {
        let conditions: Vec<String> = filter
            .iter()
            .map(|(key, value)| {
                let val_str = match value {
                    JsonValue::String(s) => format!("'{}'", s),
                    JsonValue::Number(n) => n.to_string(),
                    JsonValue::Bool(b) => b.to_string(),
                    _ => return Err(Error::config("Unsupported filter value type")),
                };
                Ok(format!("{}['{}'] == {}", METADATA_FIELD, key, val_str))
            })
            .collect::<Result<Vec<_>>>()?;

        if conditions.is_empty() {
            Ok(String::new())
        } else {
            Ok(conditions.join(" && "))
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    // ========================================================================
    // FIELD CONSTANTS TESTS
    // ========================================================================

    #[test]
    fn test_id_field_constant() {
        assert_eq!(ID_FIELD, "id");
    }

    #[test]
    fn test_text_field_constant() {
        assert_eq!(TEXT_FIELD, "text");
    }

    #[test]
    fn test_vector_field_constant() {
        assert_eq!(VECTOR_FIELD, "vector");
    }

    #[test]
    fn test_metadata_field_constant() {
        assert_eq!(METADATA_FIELD, "metadata");
    }

    #[test]
    fn test_field_constants_are_unique() {
        let fields = [ID_FIELD, TEXT_FIELD, VECTOR_FIELD, METADATA_FIELD];
        let mut unique = std::collections::HashSet::new();
        for field in fields {
            assert!(unique.insert(field), "Duplicate field name: {}", field);
        }
    }

    #[test]
    fn test_field_constants_are_lowercase() {
        assert_eq!(ID_FIELD, ID_FIELD.to_lowercase());
        assert_eq!(TEXT_FIELD, TEXT_FIELD.to_lowercase());
        assert_eq!(VECTOR_FIELD, VECTOR_FIELD.to_lowercase());
        assert_eq!(METADATA_FIELD, METADATA_FIELD.to_lowercase());
    }

    // ========================================================================
    // BUILD FILTER EXPRESSION TESTS
    // ========================================================================

    #[test]
    fn test_build_filter_expression_empty() {
        let filter = HashMap::new();
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_build_filter_expression_single_string() {
        let mut filter = HashMap::new();
        filter.insert("category".to_string(), JsonValue::String("tech".to_string()));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "metadata['category'] == 'tech'");
    }

    #[test]
    fn test_build_filter_expression_single_number_int() {
        let mut filter = HashMap::new();
        filter.insert("count".to_string(), serde_json::json!(42));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "metadata['count'] == 42");
    }

    #[test]
    fn test_build_filter_expression_single_number_float() {
        let mut filter = HashMap::new();
        filter.insert("score".to_string(), serde_json::json!(3.14));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "metadata['score'] == 3.14");
    }

    #[test]
    fn test_build_filter_expression_single_bool_true() {
        let mut filter = HashMap::new();
        filter.insert("active".to_string(), JsonValue::Bool(true));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "metadata['active'] == true");
    }

    #[test]
    fn test_build_filter_expression_single_bool_false() {
        let mut filter = HashMap::new();
        filter.insert("deleted".to_string(), JsonValue::Bool(false));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "metadata['deleted'] == false");
    }

    #[test]
    fn test_build_filter_expression_multiple_conditions() {
        let mut filter = HashMap::new();
        filter.insert("category".to_string(), JsonValue::String("tech".to_string()));
        filter.insert("priority".to_string(), serde_json::json!(1));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        // Multiple conditions are joined with " && "
        // Order may vary, so check both parts exist
        assert!(result.contains("metadata['category'] == 'tech'"));
        assert!(result.contains("metadata['priority'] == 1"));
        assert!(result.contains(" && "));
    }

    #[test]
    fn test_build_filter_expression_unsupported_array() {
        let mut filter = HashMap::new();
        filter.insert("tags".to_string(), serde_json::json!(["a", "b"]));
        let result = MilvusVectorStore::build_filter_expression(&filter);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_filter_expression_unsupported_object() {
        let mut filter = HashMap::new();
        filter.insert("nested".to_string(), serde_json::json!({"key": "value"}));
        let result = MilvusVectorStore::build_filter_expression(&filter);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_filter_expression_unsupported_null() {
        let mut filter = HashMap::new();
        filter.insert("nullable".to_string(), JsonValue::Null);
        let result = MilvusVectorStore::build_filter_expression(&filter);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_filter_expression_negative_number() {
        let mut filter = HashMap::new();
        filter.insert("offset".to_string(), serde_json::json!(-100));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "metadata['offset'] == -100");
    }

    #[test]
    fn test_build_filter_expression_zero() {
        let mut filter = HashMap::new();
        filter.insert("level".to_string(), serde_json::json!(0));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "metadata['level'] == 0");
    }

    #[test]
    fn test_build_filter_expression_empty_string_value() {
        let mut filter = HashMap::new();
        filter.insert("name".to_string(), JsonValue::String(String::new()));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "metadata['name'] == ''");
    }

    #[test]
    fn test_build_filter_expression_string_with_spaces() {
        let mut filter = HashMap::new();
        filter.insert(
            "title".to_string(),
            JsonValue::String("hello world".to_string()),
        );
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "metadata['title'] == 'hello world'");
    }

    #[test]
    fn test_build_filter_expression_key_with_underscore() {
        let mut filter = HashMap::new();
        filter.insert(
            "user_id".to_string(),
            JsonValue::String("abc123".to_string()),
        );
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "metadata['user_id'] == 'abc123'");
    }

    #[test]
    fn test_build_filter_expression_large_number() {
        let mut filter = HashMap::new();
        filter.insert("big".to_string(), serde_json::json!(i64::MAX));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert!(result.contains("metadata['big'] == "));
    }

    #[test]
    fn test_build_filter_expression_scientific_notation() {
        let mut filter = HashMap::new();
        filter.insert("tiny".to_string(), serde_json::json!(1e-10));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        // Exact format may vary, just verify it parses
        assert!(result.starts_with("metadata['tiny'] == "));
    }

    #[test]
    fn test_build_filter_expression_unicode_key() {
        let mut filter = HashMap::new();
        filter.insert("名前".to_string(), JsonValue::String("value".to_string()));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "metadata['名前'] == 'value'");
    }

    #[test]
    fn test_build_filter_expression_unicode_value() {
        let mut filter = HashMap::new();
        filter.insert(
            "greeting".to_string(),
            JsonValue::String("こんにちは".to_string()),
        );
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "metadata['greeting'] == 'こんにちは'");
    }

    #[test]
    fn test_build_filter_expression_three_conditions() {
        let mut filter = HashMap::new();
        filter.insert("a".to_string(), serde_json::json!(1));
        filter.insert("b".to_string(), serde_json::json!(2));
        filter.insert("c".to_string(), serde_json::json!(3));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        // Count " && " occurrences - should be 2 for 3 conditions
        let count = result.matches(" && ").count();
        assert_eq!(count, 2);
    }

    // ========================================================================
    // SCHEMA CREATION TESTS
    // ========================================================================

    #[test]
    fn test_create_collection_schema_basic() {
        let result = MilvusVectorStore::create_collection_schema("test_collection", 384);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_collection_schema_dimension_1() {
        let result = MilvusVectorStore::create_collection_schema("minimal", 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_collection_schema_large_dimension() {
        let result = MilvusVectorStore::create_collection_schema("large", 4096);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_collection_schema_has_id_field() {
        let schema = MilvusVectorStore::create_collection_schema("test", 128).unwrap();
        let id_field = schema.get_field(ID_FIELD);
        assert!(id_field.is_some(), "Schema should have id field");
    }

    #[test]
    fn test_create_collection_schema_has_text_field() {
        let schema = MilvusVectorStore::create_collection_schema("test", 128).unwrap();
        let text_field = schema.get_field(TEXT_FIELD);
        assert!(text_field.is_some(), "Schema should have text field");
    }

    #[test]
    fn test_create_collection_schema_has_vector_field() {
        let schema = MilvusVectorStore::create_collection_schema("test", 128).unwrap();
        let vector_field = schema.get_field(VECTOR_FIELD);
        assert!(vector_field.is_some(), "Schema should have vector field");
    }

    #[test]
    fn test_create_collection_schema_has_metadata_field() {
        let schema = MilvusVectorStore::create_collection_schema("test", 128).unwrap();
        let metadata_field = schema.get_field(METADATA_FIELD);
        assert!(metadata_field.is_some(), "Schema should have metadata field");
    }

    #[test]
    fn test_create_collection_schema_special_name() {
        let result = MilvusVectorStore::create_collection_schema("my_test_collection_123", 256);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_collection_schema_common_dimensions() {
        // Test common embedding dimensions
        for dim in [128, 256, 384, 512, 768, 1024, 1536, 3072] {
            let result = MilvusVectorStore::create_collection_schema("test", dim);
            assert!(
                result.is_ok(),
                "Failed to create schema with dimension {}",
                dim
            );
        }
    }

    // ========================================================================
    // DISTANCE METRIC TESTS
    // ========================================================================

    #[test]
    fn test_distance_metric_default() {
        // Default metric (L2/Euclidean) maps to MetricType::L2
        // Can't fully test without async constructor, but verify enum variants
        let metrics = [
            DistanceMetric::Euclidean,
            DistanceMetric::Cosine,
            DistanceMetric::DotProduct,
            DistanceMetric::MaxInnerProduct,
        ];
        for metric in metrics {
            // Verify all variants are accessible
            let _ = format!("{:?}", metric);
        }
    }

    #[test]
    fn test_distance_metric_euclidean_to_relevance() {
        let metric = DistanceMetric::Euclidean;
        // L2 distance of 0 should give relevance ~1
        let relevance = metric.distance_to_relevance(0.0);
        assert!((relevance - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_distance_metric_euclidean_large_distance() {
        let metric = DistanceMetric::Euclidean;
        // Large L2 distance should give low relevance
        let relevance = metric.distance_to_relevance(100.0);
        assert!(relevance < 0.5);
    }

    #[test]
    fn test_distance_metric_cosine_to_relevance() {
        let metric = DistanceMetric::Cosine;
        // Perfect similarity (distance 0) should give high relevance
        let relevance = metric.distance_to_relevance(0.0);
        assert!(relevance > 0.5);
    }

    #[test]
    fn test_distance_metric_dot_product() {
        let metric = DistanceMetric::DotProduct;
        // Verify the type exists and can be used
        let _ = metric.distance_to_relevance(1.0);
    }

    #[test]
    fn test_distance_metric_max_inner_product() {
        let metric = DistanceMetric::MaxInnerProduct;
        // Verify the type exists and can be used
        let _ = metric.distance_to_relevance(1.0);
    }

    // ========================================================================
    // ERROR MESSAGE FORMAT TESTS
    // ========================================================================

    #[test]
    fn test_error_message_format_config() {
        let err = Error::config("test error message");
        let msg = format!("{}", err);
        assert!(msg.contains("test error message"));
    }

    #[test]
    fn test_error_message_format_api() {
        let err = Error::api("API connection failed");
        let msg = format!("{}", err);
        assert!(msg.contains("API connection failed"));
    }

    #[test]
    fn test_error_message_format_other() {
        let err = Error::other("generic error");
        let msg = format!("{}", err);
        assert!(msg.contains("generic error"));
    }

    // ========================================================================
    // EDGE CASES AND BOUNDARY TESTS
    // ========================================================================

    #[test]
    fn test_build_filter_special_chars_in_string_value() {
        let mut filter = HashMap::new();
        // String with quotes - these might need escaping in real usage
        filter.insert("note".to_string(), JsonValue::String("it's a test".to_string()));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert!(result.contains("metadata['note'] == 'it's a test'"));
    }

    #[test]
    fn test_build_filter_key_with_brackets() {
        let mut filter = HashMap::new();
        filter.insert("field[0]".to_string(), JsonValue::String("value".to_string()));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        // Key is wrapped in brackets, so this tests nested bracket handling
        assert!(result.contains("metadata['field[0]'] == 'value'"));
    }

    #[test]
    fn test_build_filter_very_long_key() {
        let mut filter = HashMap::new();
        let long_key = "a".repeat(1000);
        filter.insert(long_key.clone(), serde_json::json!(1));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert!(result.contains(&long_key));
    }

    #[test]
    fn test_build_filter_very_long_string_value() {
        let mut filter = HashMap::new();
        let long_value = "x".repeat(10000);
        filter.insert("content".to_string(), JsonValue::String(long_value.clone()));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert!(result.contains(&long_value));
    }

    #[test]
    fn test_build_filter_numeric_string_key() {
        let mut filter = HashMap::new();
        filter.insert("123".to_string(), JsonValue::String("value".to_string()));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert_eq!(result, "metadata['123'] == 'value'");
    }

    #[test]
    fn test_build_filter_expression_float_precision() {
        let mut filter = HashMap::new();
        filter.insert("pi".to_string(), serde_json::json!(3.141592653589793));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        // Should preserve reasonable precision
        assert!(result.contains("3.14159"));
    }

    // ========================================================================
    // METADATA FIELD CONSTANT USAGE TESTS
    // ========================================================================

    #[test]
    fn test_filter_uses_metadata_field_constant() {
        let mut filter = HashMap::new();
        filter.insert("key".to_string(), JsonValue::String("val".to_string()));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        // Verify the filter uses the METADATA_FIELD constant value
        assert!(result.starts_with(&format!("{}['", METADATA_FIELD)));
    }

    // ========================================================================
    // MULTIPLE VALUE TYPE COMBINATIONS
    // ========================================================================

    #[test]
    fn test_build_filter_mixed_types() {
        let mut filter = HashMap::new();
        filter.insert("name".to_string(), JsonValue::String("test".to_string()));
        filter.insert("count".to_string(), serde_json::json!(5));
        filter.insert("enabled".to_string(), JsonValue::Bool(true));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert!(result.contains("'test'"));
        assert!(result.contains("5"));
        assert!(result.contains("true"));
    }

    #[test]
    fn test_build_filter_all_bool_values() {
        let mut filter = HashMap::new();
        filter.insert("a".to_string(), JsonValue::Bool(true));
        filter.insert("b".to_string(), JsonValue::Bool(false));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert!(result.contains("true"));
        assert!(result.contains("false"));
    }

    #[test]
    fn test_build_filter_all_numeric_values() {
        let mut filter = HashMap::new();
        filter.insert("int".to_string(), serde_json::json!(42));
        filter.insert("float".to_string(), serde_json::json!(3.14));
        filter.insert("neg".to_string(), serde_json::json!(-1));
        let result = MilvusVectorStore::build_filter_expression(&filter).unwrap();
        assert!(result.contains("42"));
        assert!(result.contains("3.14"));
        assert!(result.contains("-1"));
    }
}

#[cfg(test)]
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

    async fn create_test_store() -> MilvusVectorStore {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        // Use unique collection name per test to avoid conflicts
        let collection_name = format!("test_{}", uuid::Uuid::new_v4().to_string().replace("-", "_"));
        let url = std::env::var("MILVUS_URL")
            .unwrap_or_else(|_| "http://localhost:19530".to_string());

        MilvusVectorStore::new(&url, &collection_name, embeddings, 3)
            .await
            .expect("Failed to create test store - is Milvus running on localhost:19530?")
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_add_and_search_standard() {
        let mut store = create_test_store().await;
        test_add_and_search(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_search_with_scores_standard() {
        let mut store = create_test_store().await;
        test_search_with_scores(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_metadata_filtering_standard() {
        let mut store = create_test_store().await;
        test_metadata_filtering(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_custom_ids_standard() {
        let mut store = create_test_store().await;
        test_custom_ids(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_delete_standard() {
        let mut store = create_test_store().await;
        test_delete(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_add_documents_standard() {
        let mut store = create_test_store().await;
        test_add_documents(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_empty_search_standard() {
        let store = create_test_store().await;
        test_empty_search(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_search_by_vector_standard() {
        let mut store = create_test_store().await;
        test_search_by_vector(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_mmr_search_standard() {
        let mut store = create_test_store().await;
        test_mmr_search(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_large_batch_standard() {
        let mut store = create_test_store().await;
        test_large_batch(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_validation_standard() {
        let mut store = create_test_store().await;
        test_validation(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_update_document_standard() {
        let mut store = create_test_store().await;
        test_update_document(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_metadata_only_filter_standard() {
        let mut store = create_test_store().await;
        test_metadata_only_filter(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_complex_metadata_standard() {
        let mut store = create_test_store().await;
        test_complex_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_empty_text_standard() {
        let mut store = create_test_store().await;
        test_empty_text(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_special_chars_metadata_standard() {
        let mut store = create_test_store().await;
        test_special_chars_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_concurrent_operations_standard() {
        let mut store = create_test_store().await;
        test_concurrent_operations(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_very_long_text_standard() {
        let mut store = create_test_store().await;
        test_very_long_text(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_duplicate_documents_standard() {
        let mut store = create_test_store().await;
        test_duplicate_documents(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_k_parameter_standard() {
        let mut store = create_test_store().await;
        test_k_parameter(&mut store).await;
    }

    // ========================================================================
    // COMPREHENSIVE TESTS
    // These tests provide deeper coverage beyond standard conformance tests
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_mmr_lambda_zero_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_lambda_zero(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_mmr_lambda_one_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_lambda_one(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_mmr_fetch_k_variations_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_fetch_k_variations(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_complex_metadata_operators_comprehensive() {
        let mut store = create_test_store().await;
        test_complex_metadata_operators(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_nested_metadata_filtering_comprehensive() {
        let mut store = create_test_store().await;
        test_nested_metadata_filtering(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_array_metadata_comprehensive() {
        let mut store = create_test_store().await;
        test_array_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_very_large_batch_comprehensive() {
        let mut store = create_test_store().await;
        test_very_large_batch(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_concurrent_writes_comprehensive() {
        let mut store = create_test_store().await;
        test_concurrent_writes(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_error_handling_network_comprehensive() {
        let mut store = create_test_store().await;
        test_error_handling_network(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_error_handling_invalid_input_comprehensive() {
        let mut store = create_test_store().await;
        test_error_handling_invalid_input(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_bulk_delete_comprehensive() {
        let mut store = create_test_store().await;
        test_bulk_delete(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_update_metadata_comprehensive() {
        let mut store = create_test_store().await;
        test_update_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires Milvus server on localhost:19530"]
    async fn test_search_score_threshold_comprehensive() {
        let mut store = create_test_store().await;
        test_search_score_threshold(&mut store).await;
    }
}
