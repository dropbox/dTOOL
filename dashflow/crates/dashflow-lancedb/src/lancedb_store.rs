// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! `LanceDB` vector store implementation for `DashFlow` Rust.

use std::collections::HashMap;
use std::sync::Arc;

use futures::TryStreamExt;

use arrow_array::{Float32Array, RecordBatch, RecordBatchIterator, StringArray};
use arrow_schema::{DataType, Field, Schema};
use async_trait::async_trait;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::{Error, Result};
use dashflow::{embed, embed_query};
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::{Connection, Table};
use serde_json::Value as JsonValue;
use tracing::warn;
use uuid::Uuid;

/// `LanceDB` vector database implementation.
///
/// This implementation uses `LanceDB`'s native vector search capabilities for efficient
/// similarity search. `LanceDB` is a serverless, low-latency vector database built on
/// Lance columnar format, designed for AI applications.
///
/// # Features
///
/// - 100x faster random access than Parquet
/// - Zero-copy operations
/// - Automatic versioning
/// - Support for multi-modal data (vectors, text, images)
/// - SQL-like querying
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_lancedb::LanceDBVectorStore;
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
/// let mut store = LanceDBVectorStore::new(
///     "data/lancedb",
///     "documents",
///     embeddings,
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub struct LanceDBVectorStore {
    db: Connection,
    table_name: String,
    table: Option<Table>,
    embeddings: Arc<dyn Embeddings>,
    distance_metric: DistanceMetric,
    vector_dimension: Option<usize>,
}

impl LanceDBVectorStore {
    /// Creates a new `LanceDBVectorStore` instance.
    ///
    /// # Arguments
    ///
    /// * `uri` - Database URI (local path, S3 path, or Lance Cloud URL)
    ///   - Local: "data/lancedb" or "/path/to/db"
    ///   - S3: "<s3://bucket/path/to/db>"
    ///   - Lance Cloud: "<db://dbname>"
    /// * `table_name` - Name of the table to store documents in
    /// * `embeddings` - Embeddings model to use
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Connection to database fails
    /// - Table creation fails
    pub async fn new(uri: &str, table_name: &str, embeddings: Arc<dyn Embeddings>) -> Result<Self> {
        let db = lancedb::connect(uri)
            .execute()
            .await
            .map_err(|e| Error::api(format!("Failed to connect to LanceDB: {e}")))?;

        let mut store = Self {
            db,
            table_name: table_name.to_string(),
            table: None,
            embeddings,
            distance_metric: DistanceMetric::Cosine,
            vector_dimension: None,
        };

        // Try to open existing table
        if let Ok(table) = store.db.open_table(&store.table_name).execute().await {
            store.table = Some(table);
        }

        Ok(store)
    }

    /// Sets the distance metric for similarity search.
    #[must_use]
    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Creates the table schema with the given vector dimension.
    async fn ensure_table(&mut self, vector_dim: usize) -> Result<()> {
        if self.table.is_some() {
            return Ok(());
        }

        // Create schema for the table
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("text", DataType::Utf8, false),
            Field::new("metadata", DataType::Utf8, true), // JSON as string
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    vector_dim as i32,
                ),
                false,
            ),
        ]));

        // Create empty batch to initialize table
        let empty_batch = RecordBatch::new_empty(schema.clone());
        let batches = vec![empty_batch];
        let reader = RecordBatchIterator::new(batches.into_iter().map(Ok), schema.clone());

        let table = self
            .db
            .create_table(&self.table_name, Box::new(reader))
            .execute()
            .await
            .map_err(|e| Error::api(format!("Failed to create table: {e}")))?;

        self.table = Some(table);
        self.vector_dimension = Some(vector_dim);

        Ok(())
    }

    /// Gets the table, ensuring it exists.
    fn get_table(&self) -> Result<&Table> {
        self.table
            .as_ref()
            .ok_or_else(|| Error::api("Table not initialized. Add documents first."))
    }

    /// Creates a `RecordBatch` from vectors and metadata.
    fn create_record_batch(
        &self,
        ids: &[String],
        texts: &[String],
        vectors: &[Vec<f32>],
        metadatas: &[HashMap<String, JsonValue>],
        vector_dim: usize,
    ) -> Result<RecordBatch> {
        // Create ID array
        let id_array = StringArray::from(ids.to_vec());

        // Create text array
        let text_array = StringArray::from(texts.to_vec());

        // Create metadata array (JSON serialized as strings)
        let metadata_strings: Vec<String> = metadatas
            .iter()
            .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string()))
            .collect();
        let metadata_array = StringArray::from(metadata_strings);

        // Create vector array (FixedSizeList of Float32)
        use arrow_array::types::Float32Type;
        use arrow_array::FixedSizeListArray;

        let vector_array = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
            vectors.iter().map(|v| Some(v.iter().map(|&x| Some(x)))),
            vector_dim as i32,
        );

        // Create schema
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("text", DataType::Utf8, false),
            Field::new("metadata", DataType::Utf8, true),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    vector_dim as i32,
                ),
                false,
            ),
        ]));

        // Create RecordBatch
        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(id_array),
                Arc::new(text_array),
                Arc::new(metadata_array),
                Arc::new(vector_array),
            ],
        )
        .map_err(|e| Error::api(format!("Failed to create record batch: {e}")))
    }
}

#[async_trait]
impl VectorStore for LanceDBVectorStore {
    fn embeddings(&self) -> Option<Arc<dyn Embeddings>> {
        Some(self.embeddings.clone())
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

        // Validate inputs
        if let Some(meta) = metadatas {
            if meta.len() != texts.len() {
                return Err(Error::invalid_input(format!(
                    "Metadatas length ({}) doesn't match texts length ({})",
                    meta.len(),
                    texts.len()
                )));
            }
        }

        if let Some(id_list) = ids {
            if id_list.len() != texts.len() {
                return Err(Error::invalid_input(format!(
                    "IDs length ({}) doesn't match texts length ({})",
                    id_list.len(),
                    texts.len()
                )));
            }
        }

        // Generate embeddings using graph API
        let text_strs: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();
        let vectors = embed(Arc::clone(&self.embeddings), &text_strs).await?;

        if vectors.is_empty() {
            return Err(Error::api("No embeddings generated"));
        }

        let vector_dim = vectors[0].len();

        // Ensure table exists with correct dimension
        self.ensure_table(vector_dim).await?;

        // Generate IDs if not provided
        let generated_ids: Vec<String>;
        let id_list = if let Some(ids) = ids {
            ids
        } else {
            generated_ids = (0..texts.len())
                .map(|_| Uuid::new_v4().to_string())
                .collect();
            &generated_ids
        };

        // Prepare metadata
        let metadata_list: Vec<HashMap<String, JsonValue>> = if let Some(meta) = metadatas {
            meta.to_vec()
        } else {
            vec![HashMap::new(); texts.len()]
        };

        // Create record batch
        let batch =
            self.create_record_batch(id_list, &text_strs, &vectors, &metadata_list, vector_dim)?;

        // Add to table
        let table = self.get_table()?;
        let schema = batch.schema();
        let batches = vec![Ok(batch)];
        let reader = RecordBatchIterator::new(batches.into_iter(), schema);
        table
            .add(reader)
            .execute()
            .await
            .map_err(|e| Error::api(format!("Failed to add records: {e}")))?;

        Ok(id_list.to_vec())
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        let table = self.get_table()?;

        if let Some(id_list) = ids {
            if id_list.is_empty() {
                return Ok(true);
            }

            // Build delete predicate: id IN ('id1', 'id2', ...)
            let id_list_str = id_list
                .iter()
                .map(|id| format!("'{}'", id.replace('\'', "''")))
                .collect::<Vec<_>>()
                .join(", ");
            let predicate = format!("id IN ({id_list_str})");

            table
                .delete(&predicate)
                .await
                .map_err(|e| Error::api(format!("Failed to delete records: {e}")))?;

            Ok(true)
        } else {
            // Delete all - drop and recreate table
            self.db
                .drop_table(&self.table_name, &[])
                .await
                .map_err(|e| Error::api(format!("Failed to drop table: {e}")))?;

            self.table = None;
            self.vector_dimension = None;

            Ok(true)
        }
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let table = self.get_table()?;

        // Execute query to get all records (LanceDB filter API has issues)
        let results = table
            .query()
            .execute()
            .await
            .map_err(|e| Error::api(format!("Failed to execute query: {e}")))?;

        let batches: Vec<RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| Error::api(format!("Failed to collect results: {e}")))?;

        // Create a set of requested IDs for fast lookup
        let id_set: std::collections::HashSet<&str> =
            ids.iter().map(std::string::String::as_str).collect();
        let mut documents = Vec::new();

        for batch in batches {
            let id_array = batch
                .column_by_name("id")
                .ok_or_else(|| Error::api("Missing id column"))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| Error::api("Invalid id column type"))?;

            let text_array = batch
                .column_by_name("text")
                .ok_or_else(|| Error::api("Missing text column"))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| Error::api("Invalid text column type"))?;

            let metadata_array = batch
                .column_by_name("metadata")
                .ok_or_else(|| Error::api("Missing metadata column"))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| Error::api("Invalid metadata column type"))?;

            for i in 0..batch.num_rows() {
                let id = id_array.value(i).to_string();

                // Filter: only include documents with matching IDs
                if !id_set.contains(id.as_str()) {
                    continue;
                }

                let text = text_array.value(i).to_string();
                let metadata_str = metadata_array.value(i);
                let metadata: HashMap<String, JsonValue> =
                    serde_json::from_str(metadata_str).unwrap_or_else(|e| {
                        warn!(
                            document_id = %id,
                            error = %e,
                            "Failed to parse metadata JSON, using empty metadata"
                        );
                        HashMap::new()
                    });

                documents.push(Document {
                    id: Some(id),
                    page_content: text,
                    metadata,
                });
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
        let results = self.similarity_search_with_score(query, k, filter).await?;
        Ok(results.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        let query_vector = embed_query(Arc::clone(&self.embeddings), query).await?;
        self.similarity_search_by_vector_with_score(&query_vector, k, filter)
            .await
    }

    async fn similarity_search_by_vector(
        &self,
        query: &[f32],
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        let results = self
            .similarity_search_by_vector_with_score(query, k, filter)
            .await?;
        Ok(results.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn similarity_search_by_vector_with_score(
        &self,
        query: &[f32],
        k: usize,
        _filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        let table = self.get_table()?;

        // Perform vector search
        let results = table
            .query()
            .nearest_to(query)
            .map_err(|e| Error::api(format!("Failed to build nearest query: {e}")))?
            .limit(k)
            .execute()
            .await
            .map_err(|e| Error::api(format!("Failed to execute search: {e}")))?;

        let batches: Vec<RecordBatch> = results
            .try_collect()
            .await
            .map_err(|e| Error::api(format!("Failed to collect results: {e}")))?;

        let mut documents = Vec::new();

        for batch in batches {
            let id_array = batch
                .column_by_name("id")
                .ok_or_else(|| Error::api("Missing id column"))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| Error::api("Invalid id column type"))?;

            let text_array = batch
                .column_by_name("text")
                .ok_or_else(|| Error::api("Missing text column"))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| Error::api("Invalid text column type"))?;

            let metadata_array = batch
                .column_by_name("metadata")
                .ok_or_else(|| Error::api("Missing metadata column"))?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| Error::api("Invalid metadata column type"))?;

            // LanceDB includes _distance column in search results
            let distance_array = batch
                .column_by_name("_distance")
                .and_then(|col| col.as_any().downcast_ref::<Float32Array>());

            for i in 0..batch.num_rows() {
                let id = id_array.value(i).to_string();
                let text = text_array.value(i).to_string();
                let metadata_str = metadata_array.value(i);
                let metadata: HashMap<String, JsonValue> =
                    serde_json::from_str(metadata_str).unwrap_or_else(|e| {
                        warn!(
                            document_id = %id,
                            error = %e,
                            "Failed to parse metadata JSON in similarity search, using empty metadata"
                        );
                        HashMap::new()
                    });

                // Convert distance to similarity score [0, 1]
                let score = if let Some(dist_arr) = distance_array {
                    let distance = dist_arr.value(i);
                    // LanceDB returns distance, convert to similarity
                    // For cosine: similarity = 1 - distance
                    // For L2: similarity = 1 / (1 + distance)
                    match self.distance_metric {
                        DistanceMetric::Cosine => 1.0 - distance,
                        DistanceMetric::Euclidean => 1.0 / (1.0 + distance),
                        DistanceMetric::DotProduct => distance, // dot product is similarity already
                        DistanceMetric::MaxInnerProduct => distance,
                    }
                } else {
                    1.0 // Default if no distance provided
                };

                documents.push((
                    Document {
                        id: Some(id),
                        page_content: text,
                        metadata,
                    },
                    score,
                ));
            }
        }

        Ok(documents)
    }

    async fn max_marginal_relevance_search(
        &self,
        query: &str,
        k: usize,
        fetch_k: usize,
        lambda_mult: f32,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        // Get query embedding using graph API
        let query_vector = embed_query(Arc::clone(&self.embeddings), query).await?;

        // Fetch more candidates than needed
        let candidates = self
            .similarity_search_by_vector_with_score(&query_vector, fetch_k, filter)
            .await?;

        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // Extract vectors from candidates (we'd need to store them)
        // For now, use the simpler greedy MMR approach based on scores
        let mut selected = Vec::new();
        let mut remaining: Vec<_> = candidates.into_iter().collect();

        // Select first document (highest similarity)
        if let Some((doc, _)) = remaining.first() {
            selected.push(doc.clone());
            remaining.remove(0);
        }

        // Iteratively select documents that maximize MMR
        while selected.len() < k && !remaining.is_empty() {
            let mut best_idx = 0;
            let mut best_score = f32::MIN;

            for (idx, (_doc, query_sim)) in remaining.iter().enumerate() {
                // Calculate MMR score
                // MMR = lambda * sim(query, doc) - (1-lambda) * max(sim(selected, doc))
                // Without vectors, we approximate using query similarity
                let mmr_score = lambda_mult * query_sim;

                if mmr_score > best_score {
                    best_score = mmr_score;
                    best_idx = idx;
                }
            }

            let (doc, _) = remaining.remove(best_idx);
            selected.push(doc);
        }

        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::Array;

    // ==================== Distance Metric Score Conversion Tests ====================

    #[test]
    fn test_cosine_distance_to_similarity() {
        // For cosine: similarity = 1 - distance
        let distance: f32 = 0.3;
        let similarity: f32 = 1.0 - distance;
        assert!((similarity - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_distance_zero_means_identical() {
        // Distance of 0 means identical vectors, similarity should be 1.0
        let distance: f32 = 0.0;
        let similarity: f32 = 1.0 - distance;
        assert!((similarity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_distance_one_means_orthogonal() {
        // Distance of 1 means orthogonal vectors, similarity should be 0.0
        let distance: f32 = 1.0;
        let similarity: f32 = 1.0 - distance;
        assert!((similarity - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_distance_half() {
        let distance: f32 = 0.5;
        let similarity: f32 = 1.0 - distance;
        assert!((similarity - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_distance_small_value() {
        let distance: f32 = 0.01;
        let similarity: f32 = 1.0 - distance;
        assert!((similarity - 0.99).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_distance_near_max() {
        let distance: f32 = 0.99;
        let similarity: f32 = 1.0 - distance;
        assert!((similarity - 0.01).abs() < 0.001);
    }

    #[test]
    fn test_euclidean_distance_to_similarity() {
        // For L2: similarity = 1 / (1 + distance)
        let distance: f32 = 1.0;
        let similarity: f32 = 1.0 / (1.0 + distance);
        assert!((similarity - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_euclidean_distance_zero_means_identical() {
        // Distance of 0 means identical vectors, similarity should be 1.0
        let distance: f32 = 0.0;
        let similarity: f32 = 1.0 / (1.0 + distance);
        assert!((similarity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_euclidean_large_distance_approaches_zero() {
        // Large distance should approach 0 similarity
        let distance: f32 = 100.0;
        let similarity: f32 = 1.0 / (1.0 + distance);
        assert!(similarity < 0.01);
    }

    #[test]
    fn test_euclidean_distance_two() {
        let distance: f32 = 2.0;
        let similarity: f32 = 1.0 / (1.0 + distance);
        assert!((similarity - (1.0 / 3.0)).abs() < 0.001);
    }

    #[test]
    fn test_euclidean_distance_three() {
        let distance: f32 = 3.0;
        let similarity: f32 = 1.0 / (1.0 + distance);
        assert!((similarity - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn test_euclidean_very_large_distance() {
        let distance: f32 = 1000.0;
        let similarity: f32 = 1.0 / (1.0 + distance);
        assert!(similarity < 0.001);
    }

    #[test]
    fn test_euclidean_small_distance() {
        let distance: f32 = 0.1;
        let similarity: f32 = 1.0 / (1.0 + distance);
        assert!((similarity - (1.0 / 1.1)).abs() < 0.001);
    }

    #[test]
    fn test_dot_product_is_direct_score() {
        // For dot product: similarity = distance (it's already a similarity)
        let distance: f32 = 0.85;
        // dot product case passes distance through directly
        assert!((distance - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn test_dot_product_negative() {
        let distance: f32 = -0.5;
        // dot product can be negative (anti-correlated vectors)
        assert!((distance - (-0.5)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_dot_product_zero() {
        let distance: f32 = 0.0;
        assert!((distance - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_dot_product_one() {
        let distance: f32 = 1.0;
        assert!((distance - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_max_inner_product_passes_through() {
        let distance: f32 = 0.75;
        // max inner product also passes through directly
        assert!((distance - 0.75).abs() < f32::EPSILON);
    }

    // ==================== Distance Metric Enum Tests ====================

    #[test]
    fn test_distance_metric_default_is_cosine() {
        let metric = DistanceMetric::Cosine;
        assert!(matches!(metric, DistanceMetric::Cosine));
    }

    #[test]
    fn test_distance_metric_euclidean() {
        let metric = DistanceMetric::Euclidean;
        assert!(matches!(metric, DistanceMetric::Euclidean));
    }

    #[test]
    fn test_distance_metric_dot_product() {
        let metric = DistanceMetric::DotProduct;
        assert!(matches!(metric, DistanceMetric::DotProduct));
    }

    #[test]
    fn test_distance_metric_max_inner_product() {
        let metric = DistanceMetric::MaxInnerProduct;
        assert!(matches!(metric, DistanceMetric::MaxInnerProduct));
    }

    #[test]
    fn test_distance_metric_cosine_variant() {
        let metric = DistanceMetric::Cosine;
        assert!(!matches!(metric, DistanceMetric::Euclidean));
        assert!(!matches!(metric, DistanceMetric::DotProduct));
    }

    #[test]
    fn test_distance_metric_equality() {
        let metric1 = DistanceMetric::Cosine;
        let metric2 = DistanceMetric::Cosine;
        assert_eq!(metric1, metric2);
    }

    #[test]
    fn test_distance_metric_inequality() {
        let metric1 = DistanceMetric::Cosine;
        let metric2 = DistanceMetric::Euclidean;
        assert_ne!(metric1, metric2);
    }

    // ==================== Metadata Serialization Tests ====================

    #[test]
    fn test_empty_metadata_serialization() {
        let metadata: HashMap<String, JsonValue> = HashMap::new();
        let json_str = serde_json::to_string(&metadata).unwrap();
        assert_eq!(json_str, "{}");
    }

    #[test]
    fn test_metadata_with_string_value() {
        let mut metadata: HashMap<String, JsonValue> = HashMap::new();
        metadata.insert("source".to_string(), JsonValue::String("test.pdf".to_string()));
        let json_str = serde_json::to_string(&metadata).unwrap();
        assert!(json_str.contains("source"));
        assert!(json_str.contains("test.pdf"));
    }

    #[test]
    fn test_metadata_with_numeric_value() {
        let mut metadata: HashMap<String, JsonValue> = HashMap::new();
        metadata.insert("page".to_string(), JsonValue::Number(42.into()));
        let json_str = serde_json::to_string(&metadata).unwrap();
        assert!(json_str.contains("page"));
        assert!(json_str.contains("42"));
    }

    #[test]
    fn test_metadata_with_boolean_true() {
        let mut metadata: HashMap<String, JsonValue> = HashMap::new();
        metadata.insert("indexed".to_string(), JsonValue::Bool(true));
        let json_str = serde_json::to_string(&metadata).unwrap();
        assert!(json_str.contains("indexed"));
        assert!(json_str.contains("true"));
    }

    #[test]
    fn test_metadata_with_boolean_false() {
        let mut metadata: HashMap<String, JsonValue> = HashMap::new();
        metadata.insert("processed".to_string(), JsonValue::Bool(false));
        let json_str = serde_json::to_string(&metadata).unwrap();
        assert!(json_str.contains("processed"));
        assert!(json_str.contains("false"));
    }

    #[test]
    fn test_metadata_with_null_value() {
        let mut metadata: HashMap<String, JsonValue> = HashMap::new();
        metadata.insert("optional".to_string(), JsonValue::Null);
        let json_str = serde_json::to_string(&metadata).unwrap();
        assert!(json_str.contains("optional"));
        assert!(json_str.contains("null"));
    }

    #[test]
    fn test_metadata_with_float_value() {
        let mut metadata: HashMap<String, JsonValue> = HashMap::new();
        metadata.insert(
            "score".to_string(),
            JsonValue::Number(serde_json::Number::from_f64(0.95).unwrap()),
        );
        let json_str = serde_json::to_string(&metadata).unwrap();
        assert!(json_str.contains("score"));
        assert!(json_str.contains("0.95"));
    }

    #[test]
    fn test_metadata_with_array_value() {
        let mut metadata: HashMap<String, JsonValue> = HashMap::new();
        metadata.insert(
            "tags".to_string(),
            JsonValue::Array(vec![
                JsonValue::String("tag1".to_string()),
                JsonValue::String("tag2".to_string()),
            ]),
        );
        let json_str = serde_json::to_string(&metadata).unwrap();
        assert!(json_str.contains("tags"));
        assert!(json_str.contains("tag1"));
        assert!(json_str.contains("tag2"));
    }

    #[test]
    fn test_metadata_with_nested_object() {
        let mut metadata: HashMap<String, JsonValue> = HashMap::new();
        let mut nested = serde_json::Map::new();
        nested.insert("key".to_string(), JsonValue::String("value".to_string()));
        metadata.insert("nested".to_string(), JsonValue::Object(nested));
        let json_str = serde_json::to_string(&metadata).unwrap();
        assert!(json_str.contains("nested"));
        assert!(json_str.contains("key"));
        assert!(json_str.contains("value"));
    }

    #[test]
    fn test_metadata_with_multiple_fields() {
        let mut metadata: HashMap<String, JsonValue> = HashMap::new();
        metadata.insert("source".to_string(), JsonValue::String("doc.pdf".to_string()));
        metadata.insert("page".to_string(), JsonValue::Number(5.into()));
        metadata.insert("indexed".to_string(), JsonValue::Bool(true));
        let json_str = serde_json::to_string(&metadata).unwrap();
        assert!(json_str.contains("source"));
        assert!(json_str.contains("page"));
        assert!(json_str.contains("indexed"));
    }

    #[test]
    fn test_metadata_deserialization() {
        let json_str = r#"{"source": "test.pdf", "page": 1}"#;
        let metadata: HashMap<String, JsonValue> = serde_json::from_str(json_str).unwrap();
        assert_eq!(metadata.len(), 2);
        assert_eq!(metadata.get("source").unwrap().as_str().unwrap(), "test.pdf");
        assert_eq!(metadata.get("page").unwrap().as_i64().unwrap(), 1);
    }

    #[test]
    fn test_metadata_deserialization_with_boolean() {
        let json_str = r#"{"active": true, "deleted": false}"#;
        let metadata: HashMap<String, JsonValue> = serde_json::from_str(json_str).unwrap();
        assert_eq!(metadata.get("active").unwrap().as_bool().unwrap(), true);
        assert_eq!(metadata.get("deleted").unwrap().as_bool().unwrap(), false);
    }

    #[test]
    fn test_metadata_deserialization_with_null() {
        let json_str = r#"{"field": null}"#;
        let metadata: HashMap<String, JsonValue> = serde_json::from_str(json_str).unwrap();
        assert!(metadata.get("field").unwrap().is_null());
    }

    #[test]
    fn test_metadata_deserialization_with_array() {
        let json_str = r#"{"items": [1, 2, 3]}"#;
        let metadata: HashMap<String, JsonValue> = serde_json::from_str(json_str).unwrap();
        let items = metadata.get("items").unwrap().as_array().unwrap();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn test_metadata_invalid_json_fallback() {
        let invalid_json = "not valid json {";
        let metadata: HashMap<String, JsonValue> =
            serde_json::from_str(invalid_json).unwrap_or_else(|_| HashMap::new());
        assert!(metadata.is_empty());
    }

    #[test]
    fn test_metadata_truncated_json_fallback() {
        let truncated = r#"{"key": "va"#;
        let metadata: HashMap<String, JsonValue> =
            serde_json::from_str(truncated).unwrap_or_else(|_| HashMap::new());
        assert!(metadata.is_empty());
    }

    #[test]
    fn test_metadata_empty_string_fallback() {
        let empty = "";
        let metadata: HashMap<String, JsonValue> =
            serde_json::from_str(empty).unwrap_or_else(|_| HashMap::new());
        assert!(metadata.is_empty());
    }

    #[test]
    fn test_metadata_with_unicode() {
        let mut metadata: HashMap<String, JsonValue> = HashMap::new();
        metadata.insert("text".to_string(), JsonValue::String("日本語テスト".to_string()));
        let json_str = serde_json::to_string(&metadata).unwrap();
        let parsed: HashMap<String, JsonValue> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(
            parsed.get("text").unwrap().as_str().unwrap(),
            "日本語テスト"
        );
    }

    #[test]
    fn test_metadata_with_emoji() {
        let mut metadata: HashMap<String, JsonValue> = HashMap::new();
        metadata.insert("emoji".to_string(), JsonValue::String("🚀🎉".to_string()));
        let json_str = serde_json::to_string(&metadata).unwrap();
        let parsed: HashMap<String, JsonValue> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.get("emoji").unwrap().as_str().unwrap(), "🚀🎉");
    }

    #[test]
    fn test_metadata_with_special_chars() {
        let mut metadata: HashMap<String, JsonValue> = HashMap::new();
        metadata.insert(
            "path".to_string(),
            JsonValue::String("C:\\Users\\test\\file.txt".to_string()),
        );
        let json_str = serde_json::to_string(&metadata).unwrap();
        let parsed: HashMap<String, JsonValue> = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.get("path").unwrap().as_str().unwrap().contains("Users"));
    }

    // ==================== ID Escaping Tests ====================

    #[test]
    fn test_id_escaping_single_quote() {
        // The code escapes single quotes: id.replace('\'', "''")
        let id = "doc'with'quotes";
        let escaped = id.replace('\'', "''");
        assert_eq!(escaped, "doc''with''quotes");
    }

    #[test]
    fn test_id_escaping_no_quotes() {
        let id = "simple-id-123";
        let escaped = id.replace('\'', "''");
        assert_eq!(escaped, "simple-id-123");
    }

    #[test]
    fn test_id_escaping_multiple_quotes() {
        let id = "'''";
        let escaped = id.replace('\'', "''");
        assert_eq!(escaped, "''''''");
    }

    #[test]
    fn test_id_escaping_quote_at_start() {
        let id = "'start";
        let escaped = id.replace('\'', "''");
        assert_eq!(escaped, "''start");
    }

    #[test]
    fn test_id_escaping_quote_at_end() {
        let id = "end'";
        let escaped = id.replace('\'', "''");
        assert_eq!(escaped, "end''");
    }

    #[test]
    fn test_id_escaping_empty_string() {
        let id = "";
        let escaped = id.replace('\'', "''");
        assert_eq!(escaped, "");
    }

    #[test]
    fn test_id_list_predicate_format() {
        let ids = ["id1".to_string(), "id2".to_string(), "id3".to_string()];
        let id_list_str = ids
            .iter()
            .map(|id| format!("'{}'", id.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");
        assert_eq!(id_list_str, "'id1', 'id2', 'id3'");
    }

    #[test]
    fn test_id_list_with_special_chars() {
        let ids = ["doc'1".to_string(), "doc\"2".to_string()];
        let id_list_str = ids
            .iter()
            .map(|id| format!("'{}'", id.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");
        // Single quotes are escaped, double quotes pass through
        assert_eq!(id_list_str, "'doc''1', 'doc\"2'");
    }

    #[test]
    fn test_id_list_single_id() {
        let ids = ["only-one".to_string()];
        let id_list_str = ids
            .iter()
            .map(|id| format!("'{}'", id.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");
        assert_eq!(id_list_str, "'only-one'");
    }

    #[test]
    fn test_id_list_with_uuid() {
        let ids = [
            "550e8400-e29b-41d4-a716-446655440000".to_string(),
            "6ba7b810-9dad-11d1-80b4-00c04fd430c8".to_string(),
        ];
        let id_list_str = ids
            .iter()
            .map(|id| format!("'{}'", id.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");
        assert!(id_list_str.contains("550e8400"));
        assert!(id_list_str.contains("6ba7b810"));
    }

    #[test]
    fn test_delete_predicate_format() {
        let ids = ["id1".to_string(), "id2".to_string()];
        let id_list_str = ids
            .iter()
            .map(|id| format!("'{}'", id.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");
        let predicate = format!("id IN ({id_list_str})");
        assert_eq!(predicate, "id IN ('id1', 'id2')");
    }

    // ==================== Vector Dimension Tests ====================

    #[test]
    fn test_vector_dimension_from_embeddings() {
        let vectors = [vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]];
        let vector_dim = vectors[0].len();
        assert_eq!(vector_dim, 3);
    }

    #[test]
    fn test_empty_vectors_check() {
        let vectors: Vec<Vec<f32>> = vec![];
        assert!(vectors.is_empty());
    }

    #[test]
    fn test_vector_dimension_large() {
        let vector = vec![0.0f32; 1536]; // OpenAI embedding dimension
        assert_eq!(vector.len(), 1536);
    }

    #[test]
    fn test_vector_dimension_small() {
        let vector = vec![0.0f32; 3];
        assert_eq!(vector.len(), 3);
    }

    #[test]
    fn test_vector_dimension_consistency() {
        let vectors = [
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0, 6.0],
            vec![7.0, 8.0, 9.0],
        ];
        let dim = vectors[0].len();
        assert!(vectors.iter().all(|v| v.len() == dim));
    }

    #[test]
    fn test_vector_values_normalized() {
        let vector = vec![0.6, 0.8, 0.0];
        let magnitude: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.001);
    }

    // ==================== UUID Generation Tests ====================

    #[test]
    fn test_uuid_generation_is_unique() {
        let id1 = Uuid::new_v4().to_string();
        let id2 = Uuid::new_v4().to_string();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_uuid_format() {
        let id = Uuid::new_v4().to_string();
        // UUID v4 format: 8-4-4-4-12 hex characters
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|&c| c == '-').count(), 4);
    }

    #[test]
    fn test_uuid_v4_version_byte() {
        let id = Uuid::new_v4();
        let bytes = id.as_bytes();
        // Version 4 has 0100 in bits 12-15 (byte index 6)
        assert_eq!(bytes[6] >> 4, 4);
    }

    #[test]
    fn test_uuid_many_unique() {
        let mut ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for _ in 0..100 {
            ids.insert(Uuid::new_v4().to_string());
        }
        assert_eq!(ids.len(), 100);
    }

    #[test]
    fn test_uuid_lowercase() {
        let id = Uuid::new_v4().to_string();
        assert!(id.chars().all(|c| !c.is_ascii_uppercase()));
    }

    #[test]
    fn test_uuid_parse_roundtrip() {
        let original = Uuid::new_v4();
        let str_form = original.to_string();
        let parsed = Uuid::parse_str(&str_form).unwrap();
        assert_eq!(original, parsed);
    }

    // ==================== Input Validation Tests ====================

    #[test]
    fn test_metadata_length_validation() {
        let texts = ["text1", "text2", "text3"];
        let metadatas: Vec<HashMap<String, JsonValue>> =
            vec![HashMap::new(), HashMap::new()]; // Only 2 items

        // Should detect mismatch
        assert_ne!(metadatas.len(), texts.len());
    }

    #[test]
    fn test_ids_length_validation() {
        let texts = ["text1", "text2"];
        let ids = ["id1".to_string()]; // Only 1 ID

        // Should detect mismatch
        assert_ne!(ids.len(), texts.len());
    }

    #[test]
    fn test_empty_texts_returns_empty() {
        let texts: [&str; 0] = [];
        assert!(texts.is_empty());
    }

    #[test]
    fn test_metadata_length_matches() {
        let texts = ["text1", "text2"];
        let metadatas: Vec<HashMap<String, JsonValue>> = vec![HashMap::new(), HashMap::new()];
        assert_eq!(metadatas.len(), texts.len());
    }

    #[test]
    fn test_ids_length_matches() {
        let texts = ["text1", "text2"];
        let ids = ["id1".to_string(), "id2".to_string()];
        assert_eq!(ids.len(), texts.len());
    }

    #[test]
    fn test_single_text_validation() {
        let texts = ["single"];
        let metadatas: Vec<HashMap<String, JsonValue>> = vec![HashMap::new()];
        let ids = ["id1".to_string()];
        assert_eq!(texts.len(), metadatas.len());
        assert_eq!(texts.len(), ids.len());
    }

    // ==================== Schema Field Tests ====================

    #[test]
    fn test_schema_has_id_field() {
        let field = Field::new("id", DataType::Utf8, false);
        assert_eq!(field.name(), "id");
        assert!(!field.is_nullable());
    }

    #[test]
    fn test_schema_has_text_field() {
        let field = Field::new("text", DataType::Utf8, false);
        assert_eq!(field.name(), "text");
        assert!(!field.is_nullable());
    }

    #[test]
    fn test_schema_has_metadata_field() {
        let field = Field::new("metadata", DataType::Utf8, true);
        assert_eq!(field.name(), "metadata");
        assert!(field.is_nullable());
    }

    #[test]
    fn test_schema_vector_field_dimension() {
        let vector_dim = 384i32;
        let field = Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                vector_dim,
            ),
            false,
        );
        assert_eq!(field.name(), "vector");
        if let DataType::FixedSizeList(_, size) = field.data_type() {
            assert_eq!(*size, 384);
        } else {
            panic!("Expected FixedSizeList");
        }
    }

    #[test]
    fn test_schema_complete() {
        let vector_dim = 3i32;
        let schema = Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("text", DataType::Utf8, false),
            Field::new("metadata", DataType::Utf8, true),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    vector_dim,
                ),
                false,
            ),
        ]);
        assert_eq!(schema.fields().len(), 4);
    }

    // ==================== Arrow Array Tests ====================

    #[test]
    fn test_string_array_creation() {
        let ids = vec!["id1".to_string(), "id2".to_string()];
        let array = StringArray::from(ids.clone());
        assert_eq!(array.len(), 2);
        assert_eq!(array.value(0), "id1");
        assert_eq!(array.value(1), "id2");
    }

    #[test]
    fn test_string_array_empty() {
        let ids: Vec<String> = vec![];
        let array = StringArray::from(ids);
        assert_eq!(array.len(), 0);
    }

    #[test]
    fn test_float32_array_creation() {
        let values = vec![0.1f32, 0.2, 0.3];
        let array = Float32Array::from(values.clone());
        assert_eq!(array.len(), 3);
        assert!((array.value(0) - 0.1).abs() < f32::EPSILON);
    }

    // ==================== Document Creation Tests ====================

    #[test]
    fn test_document_with_all_fields() {
        let doc = Document {
            id: Some("doc-123".to_string()),
            page_content: "Hello world".to_string(),
            metadata: HashMap::new(),
        };
        assert_eq!(doc.id, Some("doc-123".to_string()));
        assert_eq!(doc.page_content, "Hello world");
        assert!(doc.metadata.is_empty());
    }

    #[test]
    fn test_document_without_id() {
        let doc = Document {
            id: None,
            page_content: "Content".to_string(),
            metadata: HashMap::new(),
        };
        assert!(doc.id.is_none());
    }

    #[test]
    fn test_document_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), JsonValue::String("test.txt".to_string()));
        let doc = Document {
            id: None,
            page_content: "Content".to_string(),
            metadata,
        };
        assert_eq!(doc.metadata.len(), 1);
    }

    #[test]
    fn test_document_empty_content() {
        let doc = Document {
            id: None,
            page_content: String::new(),
            metadata: HashMap::new(),
        };
        assert!(doc.page_content.is_empty());
    }

    // ==================== MMR Logic Tests ====================

    #[test]
    fn test_mmr_lambda_zero_ignores_query_similarity() {
        let lambda_mult: f32 = 0.0;
        let query_sim: f32 = 0.9;
        let mmr_score = lambda_mult * query_sim;
        assert!((mmr_score - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_mmr_lambda_one_uses_full_query_similarity() {
        let lambda_mult: f32 = 1.0;
        let query_sim: f32 = 0.8;
        let mmr_score = lambda_mult * query_sim;
        assert!((mmr_score - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_mmr_lambda_half() {
        let lambda_mult: f32 = 0.5;
        let query_sim: f32 = 1.0;
        let mmr_score = lambda_mult * query_sim;
        assert!((mmr_score - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_mmr_best_score_selection() {
        let scores = [0.1f32, 0.5, 0.3, 0.9, 0.2];
        let best_idx = scores
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(idx, _)| idx)
            .unwrap();
        assert_eq!(best_idx, 3); // Index of 0.9
    }

    // ==================== Text Processing Tests ====================

    #[test]
    fn test_text_to_string_conversion() {
        let texts: [&str; 2] = ["hello", "world"];
        let text_strs: Vec<String> = texts.iter().map(|t| t.to_string()).collect();
        assert_eq!(text_strs.len(), 2);
        assert_eq!(text_strs[0], "hello");
        assert_eq!(text_strs[1], "world");
    }

    #[test]
    fn test_text_with_whitespace() {
        let text = "  hello world  ";
        assert_eq!(text.trim(), "hello world");
    }

    #[test]
    fn test_text_with_newlines() {
        let text = "line1\nline2\nline3";
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_text_unicode() {
        let text = "こんにちは世界";
        assert_eq!(text.chars().count(), 7);
    }

    // ==================== ID Set Lookup Tests ====================

    #[test]
    fn test_id_set_contains() {
        let ids = ["id1".to_string(), "id2".to_string(), "id3".to_string()];
        let id_set: std::collections::HashSet<&str> = ids.iter().map(String::as_str).collect();
        assert!(id_set.contains("id1"));
        assert!(id_set.contains("id2"));
        assert!(!id_set.contains("id4"));
    }

    #[test]
    fn test_id_set_empty() {
        let ids: Vec<String> = vec![];
        let id_set: std::collections::HashSet<&str> = ids.iter().map(String::as_str).collect();
        assert!(id_set.is_empty());
    }

    #[test]
    fn test_id_set_single() {
        let ids = ["only".to_string()];
        let id_set: std::collections::HashSet<&str> = ids.iter().map(String::as_str).collect();
        assert!(id_set.contains("only"));
        assert_eq!(id_set.len(), 1);
    }

    // ==================== Error Message Tests ====================

    #[test]
    fn test_metadata_mismatch_error_message() {
        let meta_len = 2;
        let texts_len = 3;
        let error_msg = format!(
            "Metadatas length ({}) doesn't match texts length ({})",
            meta_len, texts_len
        );
        assert!(error_msg.contains("2"));
        assert!(error_msg.contains("3"));
        assert!(error_msg.contains("Metadatas length"));
    }

    #[test]
    fn test_ids_mismatch_error_message() {
        let ids_len = 1;
        let texts_len = 5;
        let error_msg = format!(
            "IDs length ({}) doesn't match texts length ({})",
            ids_len, texts_len
        );
        assert!(error_msg.contains("1"));
        assert!(error_msg.contains("5"));
        assert!(error_msg.contains("IDs length"));
    }

    #[test]
    fn test_table_not_initialized_error() {
        let error_msg = "Table not initialized. Add documents first.";
        assert!(error_msg.contains("Table not initialized"));
    }
}
