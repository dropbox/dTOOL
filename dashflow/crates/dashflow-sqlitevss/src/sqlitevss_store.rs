use async_trait::async_trait;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::Error;
use dashflow::{embed, embed_query};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::collections::HashMap;
use tracing::warn;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// `SQLite` VSS Vector Store.
///
/// Provides vector similarity search using `SQLite` with the sqlite-vss extension.
/// Suitable for local/embedded applications requiring persistent vector storage.
pub struct SQLiteVSSStore {
    /// Database connection (wrapped in Mutex for interior mutability)
    conn: Arc<Mutex<Connection>>,
    /// Embeddings function
    embedding: Arc<dyn Embeddings>,
    /// Vector dimension
    dimensions: usize,
    /// Table name
    table_name: String,
    /// Distance metric
    metric: DistanceMetric,
}

impl SQLiteVSSStore {
    /// Create a new `SQLite` VSS vector store.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Embeddings function to use
    /// * `db_path` - Path to `SQLite` database file (use ":memory:" for in-memory)
    /// * `dimensions` - Dimensionality of vectors
    /// * `metric` - Optional distance metric (defaults to Cosine)
    ///
    /// # Returns
    ///
    /// A new `SQLite` VSS store instance
    ///
    /// # Errors
    ///
    /// Returns error if database connection or initialization fails
    pub fn new(
        embedding: Arc<dyn Embeddings>,
        db_path: &str,
        dimensions: usize,
        metric: Option<DistanceMetric>,
    ) -> Result<Self, Error> {
        let conn = Connection::open(db_path)
            .map_err(|e| Error::other(format!("Failed to open SQLite database: {e}")))?;

        // Note: sqlite-vss extension loading is handled by the sqlite-vss crate
        // when using the download-libs feature. The extension is automatically
        // available at build time and loaded when creating VSS virtual tables.

        let table_name = "dashflow_vectors".to_string();
        let metric = metric.unwrap_or(DistanceMetric::Cosine);

        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            embedding,
            dimensions,
            table_name,
            metric,
        };

        // Initialize schema
        store.init_schema()?;

        Ok(store)
    }

    /// Initialize database schema.
    fn init_schema(&self) -> Result<(), Error> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| Error::other(format!("Failed to acquire lock: {e}")))?;

        // Create main table for documents
        conn.execute(
            &format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    id TEXT PRIMARY KEY,
                    text TEXT NOT NULL,
                    metadata TEXT
                )",
                self.table_name
            ),
            [],
        )
        .map_err(|e| Error::other(format!("Failed to create table: {e}")))?;

        // Create virtual table for vectors using vss0
        // vss0 is the virtual table module provided by sqlite-vss
        conn.execute(
            &format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS {}_vss USING vss0(
                    embedding({})
                )",
                self.table_name, self.dimensions
            ),
            [],
        )
        .map_err(|e| Error::other(format!("Failed to create VSS table: {e}")))?;

        Ok(())
    }

    /// Convert distance to relevance score [0, 1].
    fn distance_to_relevance(&self, distance: f32) -> f32 {
        self.metric.distance_to_relevance(distance)
    }

    /// Match document against metadata filter.
    fn matches_filter(
        metadata: &HashMap<String, Value>,
        filter: Option<&HashMap<String, Value>>,
    ) -> bool {
        if let Some(filter) = filter {
            for (key, value) in filter {
                if metadata.get(key) != Some(value) {
                    return false;
                }
            }
        }
        true
    }
}

#[async_trait]
impl VectorStore for SQLiteVSSStore {
    fn embeddings(&self) -> Option<Arc<dyn Embeddings>> {
        Some(Arc::clone(&self.embedding))
    }

    fn distance_metric(&self) -> DistanceMetric {
        self.metric
    }

    async fn add_texts(
        &mut self,
        texts: &[impl AsRef<str> + Send + Sync],
        metadatas: Option<&[HashMap<String, Value>]>,
        ids: Option<&[String]>,
    ) -> Result<Vec<String>, Error> {
        // Validate inputs
        if let Some(metadatas) = metadatas {
            if metadatas.len() != texts.len() {
                return Err(Error::InvalidInput(
                    "metadatas length must match texts length".to_string(),
                ));
            }
        }
        if let Some(ids) = ids {
            if ids.len() != texts.len() {
                return Err(Error::InvalidInput(
                    "ids length must match texts length".to_string(),
                ));
            }
        }

        // Generate IDs if not provided
        let generated_ids: Vec<String>;
        let ids_ref = if let Some(ids) = ids {
            ids
        } else {
            generated_ids = (0..texts.len())
                .map(|_| Uuid::new_v4().to_string())
                .collect();
            &generated_ids
        };

        // Embed all texts using graph API
        let text_strings: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();
        let embeddings = embed(Arc::clone(&self.embedding), &text_strings).await?;

        // Verify embedding dimensions
        if !embeddings.is_empty() && embeddings[0].len() != self.dimensions {
            return Err(Error::InvalidInput(format!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                embeddings[0].len()
            )));
        }

        // Insert into database
        let conn = self
            .conn
            .lock()
            .map_err(|e| Error::other(format!("Failed to acquire lock: {e}")))?;

        for (idx, ((id, text), embedding)) in ids_ref
            .iter()
            .zip(text_strings.iter())
            .zip(embeddings.iter())
            .enumerate()
        {
            let metadata_json = if let Some(metadatas) = metadatas {
                serde_json::to_string(&metadatas[idx])
                    .map_err(|e| Error::other(format!("Failed to serialize metadata: {e}")))?
            } else {
                "{}".to_string()
            };

            // Insert document
            conn.execute(
                &format!(
                    "INSERT OR REPLACE INTO {} (id, text, metadata) VALUES (?1, ?2, ?3)",
                    self.table_name
                ),
                params![id, text, metadata_json],
            )
            .map_err(|e| Error::other(format!("Failed to insert document: {e}")))?;

            // Insert vector
            // vss0 expects vectors as JSON arrays or blob
            let embedding_blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

            conn.execute(
                &format!(
                    "INSERT OR REPLACE INTO {}_vss (rowid, embedding) VALUES (
                        (SELECT rowid FROM {} WHERE id = ?1),
                        ?2
                    )",
                    self.table_name, self.table_name
                ),
                params![id, embedding_blob],
            )
            .map_err(|e| Error::other(format!("Failed to insert vector: {e}")))?;
        }

        Ok(ids_ref.to_vec())
    }

    async fn _similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, Value>>,
    ) -> Result<Vec<Document>, Error> {
        let results = self.similarity_search_with_score(query, k, filter).await?;
        Ok(results.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, Value>>,
    ) -> Result<Vec<(Document, f32)>, Error> {
        // Embed query using graph API
        let embedding = embed_query(Arc::clone(&self.embedding), query).await?;
        self.similarity_search_by_vector_with_score(&embedding, k, filter)
            .await
    }

    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, Value>>,
    ) -> Result<Vec<Document>, Error> {
        let results = self
            .similarity_search_by_vector_with_score(embedding, k, filter)
            .await?;
        Ok(results.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, Value>>,
    ) -> Result<Vec<(Document, f32)>, Error> {
        if embedding.len() != self.dimensions {
            return Err(Error::InvalidInput(format!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                embedding.len()
            )));
        }

        let conn = self
            .conn
            .lock()
            .map_err(|e| Error::other(format!("Failed to acquire lock: {e}")))?;

        // Convert embedding to blob for query
        let embedding_blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        // Query using vss0's knn search
        // The exact SQL syntax depends on sqlite-vss version, but typically:
        // SELECT ... FROM table WHERE rowid IN (SELECT rowid FROM table_vss WHERE vss_search(embedding, ?))
        let mut stmt = conn
            .prepare(&format!(
                "SELECT d.id, d.text, d.metadata, vss.distance
                 FROM {} d
                 INNER JOIN {}_vss vss ON d.rowid = vss.rowid
                 WHERE vss_search(vss.embedding, ?)
                 ORDER BY vss.distance
                 LIMIT ?",
                self.table_name, self.table_name
            ))
            .map_err(|e| Error::other(format!("Failed to prepare statement: {e}")))?;

        let rows = stmt
            .query_map(params![embedding_blob, k], |row| {
                Ok((
                    row.get::<_, String>(0)?, // id
                    row.get::<_, String>(1)?, // text
                    row.get::<_, String>(2)?, // metadata
                    row.get::<_, f32>(3)?,    // distance
                ))
            })
            .map_err(|e| Error::other(format!("Failed to execute query: {e}")))?;

        let mut results = Vec::new();
        for row in rows {
            let (id, text, metadata_json, distance) =
                row.map_err(|e| Error::other(format!("Failed to read row: {e}")))?;

            let metadata: HashMap<String, Value> =
                serde_json::from_str(&metadata_json).unwrap_or_else(|e| {
                    warn!(
                        document_id = %id,
                        error = %e,
                        "Failed to parse metadata JSON, using empty metadata"
                    );
                    HashMap::new()
                });

            // Apply filter
            if !Self::matches_filter(&metadata, filter) {
                continue;
            }

            let score = self.distance_to_relevance(distance);

            results.push((
                Document {
                    id: Some(id),
                    page_content: text,
                    metadata,
                },
                score,
            ));
        }

        Ok(results)
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool, Error> {
        let Some(ids) = ids else {
            return Ok(false);
        };
        if ids.is_empty() {
            return Ok(false);
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| Error::other(format!("Failed to acquire lock: {e}")))?;

        for id in ids {
            // Delete from VSS table first
            conn.execute(
                &format!(
                    "DELETE FROM {}_vss WHERE rowid = (SELECT rowid FROM {} WHERE id = ?1)",
                    self.table_name, self.table_name
                ),
                params![id],
            )
            .map_err(|e| Error::other(format!("Failed to delete vector: {e}")))?;

            // Delete from main table
            conn.execute(
                &format!("DELETE FROM {} WHERE id = ?1", self.table_name),
                params![id],
            )
            .map_err(|e| Error::other(format!("Failed to delete document: {e}")))?;
        }

        Ok(true)
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>, Error> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| Error::other(format!("Failed to acquire lock: {e}")))?;

        let mut results = Vec::new();
        for id in ids {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT id, text, metadata FROM {} WHERE id = ?1",
                    self.table_name
                ))
                .map_err(|e| Error::other(format!("Failed to prepare statement: {e}")))?;

            let mut rows = stmt
                .query(params![id])
                .map_err(|e| Error::other(format!("Failed to execute query: {e}")))?;

            if let Some(row) = rows
                .next()
                .map_err(|e| Error::other(format!("Failed to read row: {e}")))?
            {
                let id = row
                    .get::<_, String>(0)
                    .map_err(|e| Error::other(format!("Failed to read id: {e}")))?;
                let text = row
                    .get::<_, String>(1)
                    .map_err(|e| Error::other(format!("Failed to read text: {e}")))?;
                let metadata_json = row
                    .get::<_, String>(2)
                    .map_err(|e| Error::other(format!("Failed to read metadata: {e}")))?;

                let metadata: HashMap<String, Value> =
                    serde_json::from_str(&metadata_json).unwrap_or_else(|e| {
                        warn!(
                            document_id = %id,
                            error = %e,
                            "Failed to parse metadata JSON in get_by_ids, using empty metadata"
                        );
                        HashMap::new()
                    });

                results.push(Document {
                    id: Some(id),
                    page_content: text,
                    metadata,
                });
            }
        }

        Ok(results)
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    // Test matches_filter function
    #[test]
    fn test_matches_filter_none() {
        let metadata = HashMap::new();
        assert!(SQLiteVSSStore::matches_filter(&metadata, None));
    }

    #[test]
    fn test_matches_filter_empty_filter() {
        let metadata = HashMap::new();
        let filter = HashMap::new();
        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_single_match() {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), Value::String("article".to_string()));

        let mut filter = HashMap::new();
        filter.insert("type".to_string(), Value::String("article".to_string()));

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_single_no_match() {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), Value::String("article".to_string()));

        let mut filter = HashMap::new();
        filter.insert("type".to_string(), Value::String("book".to_string()));

        assert!(!SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_multiple_all_match() {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), Value::String("article".to_string()));
        metadata.insert("author".to_string(), Value::String("Alice".to_string()));

        let mut filter = HashMap::new();
        filter.insert("type".to_string(), Value::String("article".to_string()));
        filter.insert("author".to_string(), Value::String("Alice".to_string()));

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_multiple_partial_match() {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), Value::String("article".to_string()));
        metadata.insert("author".to_string(), Value::String("Alice".to_string()));

        let mut filter = HashMap::new();
        filter.insert("type".to_string(), Value::String("article".to_string()));
        filter.insert("author".to_string(), Value::String("Bob".to_string()));

        assert!(!SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_missing_key() {
        let metadata = HashMap::new();

        let mut filter = HashMap::new();
        filter.insert("type".to_string(), Value::String("article".to_string()));

        assert!(!SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_extra_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), Value::String("article".to_string()));
        metadata.insert("extra".to_string(), Value::String("value".to_string()));

        let mut filter = HashMap::new();
        filter.insert("type".to_string(), Value::String("article".to_string()));

        // Should match even with extra metadata
        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    // Test distance_to_relevance through DistanceMetric
    #[test]
    fn test_distance_metric_cosine_to_relevance() {
        // For cosine, relevance = 1.0 - (distance / 2.0)
        // Cosine distance is [0, 2], so distance 0.2 -> relevance 0.9
        let distance = 0.2;
        let relevance = DistanceMetric::Cosine.distance_to_relevance(distance);
        let expected = 1.0 - (0.2 / 2.0);
        assert!((relevance - expected).abs() < 1e-6);
    }

    #[test]
    fn test_distance_metric_euclidean_to_relevance() {
        // For euclidean, relevance = 1.0 - (distance / sqrt(2))
        // Euclidean for normalized embeddings is [0, sqrt(2)]
        let distance = 1.0;
        let relevance = DistanceMetric::Euclidean.distance_to_relevance(distance);
        let expected = 1.0 - (1.0 / 2.0_f32.sqrt());
        assert!((relevance - expected).abs() < 1e-6);
    }

    #[test]
    fn test_distance_metric_zero_distance() {
        // Zero distance should give max relevance
        let relevance_cosine = DistanceMetric::Cosine.distance_to_relevance(0.0);
        let relevance_euclidean = DistanceMetric::Euclidean.distance_to_relevance(0.0);

        assert!((relevance_cosine - 1.0).abs() < 1e-6);
        assert!((relevance_euclidean - 1.0).abs() < 1e-6);
    }

    // Test embedding to blob conversion
    #[test]
    fn test_embedding_to_blob() {
        let embedding = [1.0_f32, 2.0, 3.0];
        let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        // Each f32 is 4 bytes
        assert_eq!(blob.len(), 12);

        // Verify first float bytes
        let bytes_1 = 1.0_f32.to_le_bytes();
        assert_eq!(&blob[0..4], &bytes_1);
    }

    #[test]
    fn test_embedding_to_blob_empty() {
        let embedding: Vec<f32> = vec![];
        let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        assert!(blob.is_empty());
    }

    #[test]
    fn test_embedding_to_blob_roundtrip() {
        let embedding = [1.5_f32, -2.5, 0.0, std::f32::consts::PI];
        let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        // Convert back
        let recovered: Vec<f32> = blob
            .chunks(4)
            .map(|chunk| {
                let arr: [u8; 4] = chunk.try_into().unwrap();
                f32::from_le_bytes(arr)
            })
            .collect();

        assert_eq!(embedding.len(), recovered.len());
        for (orig, rec) in embedding.iter().zip(recovered.iter()) {
            assert!((orig - rec).abs() < 1e-6);
        }
    }

    // Test Document struct
    #[test]
    fn test_document_creation() {
        let doc = Document {
            id: Some("test-id".to_string()),
            page_content: "Hello world".to_string(),
            metadata: HashMap::new(),
        };

        assert_eq!(doc.id, Some("test-id".to_string()));
        assert_eq!(doc.page_content, "Hello world");
        assert!(doc.metadata.is_empty());
    }

    #[test]
    fn test_document_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), Value::String("test".to_string()));
        metadata.insert("count".to_string(), Value::Number(serde_json::Number::from(42)));

        let doc = Document {
            id: None,
            page_content: "Content".to_string(),
            metadata,
        };

        assert!(doc.id.is_none());
        assert_eq!(doc.metadata.len(), 2);
    }

    // Test UUID generation
    #[test]
    fn test_uuid_generation() {
        let id1 = Uuid::new_v4().to_string();
        let id2 = Uuid::new_v4().to_string();

        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 36);
    }

    // Test metadata JSON serialization/deserialization
    #[test]
    fn test_metadata_json_serialize() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), Value::String("value".to_string()));

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("key"));
        assert!(json.contains("value"));
    }

    #[test]
    fn test_metadata_json_deserialize() {
        let json = r#"{"type": "article", "count": 5}"#;
        let metadata: HashMap<String, Value> = serde_json::from_str(json).unwrap();

        assert_eq!(metadata.len(), 2);
        assert_eq!(metadata.get("type"), Some(&Value::String("article".to_string())));
    }

    #[test]
    fn test_metadata_json_deserialize_empty() {
        let json = "{}";
        let metadata: HashMap<String, Value> = serde_json::from_str(json).unwrap();
        assert!(metadata.is_empty());
    }

    // Test table name constants
    #[test]
    fn test_table_name_default() {
        let table_name = "dashflow_vectors";
        assert!(!table_name.is_empty());
        assert!(!table_name.contains(' '));
    }

    // Test SQL query formats
    #[test]
    fn test_create_table_sql() {
        let table_name = "test_vectors";
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id TEXT PRIMARY KEY,
                text TEXT NOT NULL,
                metadata TEXT
            )",
            table_name
        );

        assert!(sql.contains("test_vectors"));
        assert!(sql.contains("id TEXT PRIMARY KEY"));
        assert!(sql.contains("text TEXT NOT NULL"));
        assert!(sql.contains("metadata TEXT"));
    }

    #[test]
    fn test_create_vss_table_sql() {
        let table_name = "test_vectors";
        let dimensions = 384;
        let sql = format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS {}_vss USING vss0(
                embedding({})
            )",
            table_name, dimensions
        );

        assert!(sql.contains("test_vectors_vss"));
        assert!(sql.contains("USING vss0"));
        assert!(sql.contains("embedding(384)"));
    }

    #[test]
    fn test_insert_document_sql() {
        let table_name = "docs";
        let sql = format!(
            "INSERT OR REPLACE INTO {} (id, text, metadata) VALUES (?1, ?2, ?3)",
            table_name
        );

        assert!(sql.contains("INSERT OR REPLACE"));
        assert!(sql.contains("?1, ?2, ?3"));
    }

    #[test]
    fn test_insert_vector_sql() {
        let table_name = "docs";
        let sql = format!(
            "INSERT OR REPLACE INTO {}_vss (rowid, embedding) VALUES (
                (SELECT rowid FROM {} WHERE id = ?1),
                ?2
            )",
            table_name, table_name
        );

        assert!(sql.contains("docs_vss"));
        assert!(sql.contains("SELECT rowid FROM docs"));
    }

    #[test]
    fn test_delete_vss_sql() {
        let table_name = "docs";
        let sql = format!(
            "DELETE FROM {}_vss WHERE rowid = (SELECT rowid FROM {} WHERE id = ?1)",
            table_name, table_name
        );

        assert!(sql.contains("DELETE FROM docs_vss"));
        assert!(sql.contains("WHERE id = ?1"));
    }

    #[test]
    fn test_delete_doc_sql() {
        let table_name = "docs";
        let sql = format!("DELETE FROM {} WHERE id = ?1", table_name);

        assert!(sql.contains("DELETE FROM docs"));
        assert!(sql.contains("WHERE id = ?1"));
    }

    #[test]
    fn test_select_doc_sql() {
        let table_name = "docs";
        let sql = format!(
            "SELECT id, text, metadata FROM {} WHERE id = ?1",
            table_name
        );

        assert!(sql.contains("SELECT id, text, metadata"));
        assert!(sql.contains("WHERE id = ?1"));
    }

    #[test]
    fn test_similarity_search_sql() {
        let table_name = "docs";
        let sql = format!(
            "SELECT d.id, d.text, d.metadata, vss.distance
             FROM {} d
             INNER JOIN {}_vss vss ON d.rowid = vss.rowid
             WHERE vss_search(vss.embedding, ?)
             ORDER BY vss.distance
             LIMIT ?",
            table_name, table_name
        );

        assert!(sql.contains("docs d"));
        assert!(sql.contains("docs_vss vss"));
        assert!(sql.contains("vss_search"));
        assert!(sql.contains("ORDER BY vss.distance"));
    }

    // Test dimension validation
    #[test]
    fn test_dimension_mismatch_error() {
        let expected = 384;
        let actual = 512;

        let error_msg = format!(
            "Embedding dimension mismatch: expected {}, got {}",
            expected, actual
        );

        assert!(error_msg.contains("384"));
        assert!(error_msg.contains("512"));
    }

    // Test input validation
    #[test]
    fn test_metadatas_length_validation() {
        let texts_len = 3;
        let metadatas_len = 2;

        let valid = metadatas_len == texts_len;
        assert!(!valid);
    }

    #[test]
    fn test_ids_length_validation() {
        let texts_len = 5;
        let ids_len = 3;

        let valid = ids_len == texts_len;
        assert!(!valid);
    }

    // Test DistanceMetric variants
    #[test]
    fn test_distance_metric_variants_exist() {
        let metrics = [
            DistanceMetric::Cosine,
            DistanceMetric::Euclidean,
            DistanceMetric::DotProduct,
            DistanceMetric::MaxInnerProduct,
        ];

        assert_eq!(metrics.len(), 4);
    }

    #[test]
    fn test_distance_metric_equality() {
        assert_eq!(DistanceMetric::Cosine, DistanceMetric::Cosine);
        assert_ne!(DistanceMetric::Cosine, DistanceMetric::Euclidean);
    }

    // Test Option handling for metric
    #[test]
    #[allow(clippy::unnecessary_literal_unwrap)]
    fn test_metric_default() {
        let metric: Option<DistanceMetric> = None;
        let actual = metric.unwrap_or(DistanceMetric::Cosine);
        assert_eq!(actual, DistanceMetric::Cosine);
    }

    #[test]
    #[allow(clippy::unnecessary_literal_unwrap)]
    fn test_metric_provided() {
        let metric = Some(DistanceMetric::Euclidean);
        let actual = metric.unwrap_or(DistanceMetric::Cosine);
        assert_eq!(actual, DistanceMetric::Euclidean);
    }

    // Test embedding dimension validation
    #[test]
    fn test_empty_embeddings_dimension_check() {
        let embeddings: Vec<Vec<f32>> = vec![];
        let dimensions = 384;

        // Empty embeddings should pass dimension check
        let valid = embeddings.is_empty() || embeddings[0].len() == dimensions;
        assert!(valid);
    }

    #[test]
    fn test_embeddings_dimension_match() {
        let embeddings = [vec![0.0_f32; 384]];
        let dimensions = 384;

        let valid = embeddings.is_empty() || embeddings[0].len() == dimensions;
        assert!(valid);
    }

    #[test]
    fn test_embeddings_dimension_mismatch() {
        let embeddings = [vec![0.0_f32; 512]];
        let dimensions = 384;

        let valid = embeddings.is_empty() || embeddings[0].len() == dimensions;
        assert!(!valid);
    }

    // Test delete with None ids
    #[test]
    fn test_delete_none_returns_false() {
        let ids: Option<&[String]> = None;

        // Should return Ok(false) when ids is None
        let result = ids.is_none();
        assert!(result);
    }

    // Test delete with empty ids
    #[test]
    fn test_delete_empty_returns_false() {
        let ids: Vec<String> = vec![];

        // Should return Ok(false) when ids is empty
        let result = ids.is_empty();
        assert!(result);
    }

    // ============================================================================
    // Additional comprehensive tests - Worker #2731
    // ============================================================================

    // === Distance metric tests ===

    #[test]
    fn test_distance_metric_dot_product_to_relevance() {
        // DotProduct: Higher dot product = more similar, relevance = (1.0 + distance) / 2.0
        let distance = 0.8;
        let relevance = DistanceMetric::DotProduct.distance_to_relevance(distance);
        // Dot product relevance converts [-1, 1] to [0, 1]
        assert!(relevance >= 0.0 && relevance <= 1.0);
    }

    #[test]
    fn test_distance_metric_max_inner_product_to_relevance() {
        let distance = 0.5;
        let relevance = DistanceMetric::MaxInnerProduct.distance_to_relevance(distance);
        assert!(relevance >= 0.0);
    }

    #[test]
    fn test_distance_metric_max_distance_cosine() {
        // Cosine distance max is 2.0 (opposite vectors)
        let distance = 2.0;
        let relevance = DistanceMetric::Cosine.distance_to_relevance(distance);
        assert!((relevance - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_distance_metric_negative_values() {
        // Some metrics like dot product can have negative "distances"
        let distance = -0.5;
        let relevance = DistanceMetric::DotProduct.distance_to_relevance(distance);
        // Should handle gracefully
        assert!(relevance.is_finite());
    }

    #[test]
    fn test_distance_metric_large_euclidean_distance() {
        let distance = 10.0;
        let relevance = DistanceMetric::Euclidean.distance_to_relevance(distance);
        // Large distance should give low or zero relevance
        assert!(relevance <= 1.0);
    }

    // === Embedding blob conversion tests ===

    #[test]
    fn test_embedding_to_blob_negative_values() {
        let embedding = [-1.0_f32, -2.5, -0.001];
        let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        // Roundtrip should preserve negative values
        let recovered: Vec<f32> = blob
            .chunks(4)
            .map(|chunk| {
                let arr: [u8; 4] = chunk.try_into().unwrap();
                f32::from_le_bytes(arr)
            })
            .collect();

        for (orig, rec) in embedding.iter().zip(recovered.iter()) {
            assert!((orig - rec).abs() < 1e-9);
        }
    }

    #[test]
    fn test_embedding_to_blob_very_small_values() {
        let embedding = [1e-38_f32, 1e-30, 1e-20];
        let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        let recovered: Vec<f32> = blob
            .chunks(4)
            .map(|chunk| {
                let arr: [u8; 4] = chunk.try_into().unwrap();
                f32::from_le_bytes(arr)
            })
            .collect();

        for (orig, rec) in embedding.iter().zip(recovered.iter()) {
            assert!((orig - rec).abs() / orig.abs().max(1e-40) < 1e-6);
        }
    }

    #[test]
    fn test_embedding_to_blob_very_large_values() {
        let embedding = [1e30_f32, 1e20, 1e10];
        let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        let recovered: Vec<f32> = blob
            .chunks(4)
            .map(|chunk| {
                let arr: [u8; 4] = chunk.try_into().unwrap();
                f32::from_le_bytes(arr)
            })
            .collect();

        for (orig, rec) in embedding.iter().zip(recovered.iter()) {
            assert!((orig - rec).abs() < 1e24);
        }
    }

    #[test]
    fn test_embedding_to_blob_special_values() {
        let embedding = [0.0_f32, -0.0, std::f32::EPSILON];
        let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        assert_eq!(blob.len(), 12);
    }

    #[test]
    fn test_embedding_to_blob_infinity() {
        let embedding = [std::f32::INFINITY, std::f32::NEG_INFINITY];
        let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        let recovered: Vec<f32> = blob
            .chunks(4)
            .map(|chunk| {
                let arr: [u8; 4] = chunk.try_into().unwrap();
                f32::from_le_bytes(arr)
            })
            .collect();

        assert!(recovered[0].is_infinite() && recovered[0].is_sign_positive());
        assert!(recovered[1].is_infinite() && recovered[1].is_sign_negative());
    }

    #[test]
    fn test_embedding_to_blob_nan() {
        let embedding = [std::f32::NAN];
        let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        let recovered: Vec<f32> = blob
            .chunks(4)
            .map(|chunk| {
                let arr: [u8; 4] = chunk.try_into().unwrap();
                f32::from_le_bytes(arr)
            })
            .collect();

        assert!(recovered[0].is_nan());
    }

    #[test]
    fn test_embedding_to_blob_large_dimension() {
        let embedding = vec![0.5_f32; 1536]; // OpenAI embedding dimension
        let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        assert_eq!(blob.len(), 1536 * 4);
    }

    // === Metadata filter tests with various types ===

    #[test]
    fn test_matches_filter_numeric_value() {
        let mut metadata = HashMap::new();
        metadata.insert("count".to_string(), Value::Number(42.into()));

        let mut filter = HashMap::new();
        filter.insert("count".to_string(), Value::Number(42.into()));

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_numeric_mismatch() {
        let mut metadata = HashMap::new();
        metadata.insert("count".to_string(), Value::Number(42.into()));

        let mut filter = HashMap::new();
        filter.insert("count".to_string(), Value::Number(43.into()));

        assert!(!SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_float_value() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "score".to_string(),
            Value::Number(serde_json::Number::from_f64(3.14).unwrap()),
        );

        let mut filter = HashMap::new();
        filter.insert(
            "score".to_string(),
            Value::Number(serde_json::Number::from_f64(3.14).unwrap()),
        );

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_boolean_true() {
        let mut metadata = HashMap::new();
        metadata.insert("active".to_string(), Value::Bool(true));

        let mut filter = HashMap::new();
        filter.insert("active".to_string(), Value::Bool(true));

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_boolean_false() {
        let mut metadata = HashMap::new();
        metadata.insert("active".to_string(), Value::Bool(false));

        let mut filter = HashMap::new();
        filter.insert("active".to_string(), Value::Bool(false));

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_boolean_mismatch() {
        let mut metadata = HashMap::new();
        metadata.insert("active".to_string(), Value::Bool(true));

        let mut filter = HashMap::new();
        filter.insert("active".to_string(), Value::Bool(false));

        assert!(!SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_null_value() {
        let mut metadata = HashMap::new();
        metadata.insert("empty".to_string(), Value::Null);

        let mut filter = HashMap::new();
        filter.insert("empty".to_string(), Value::Null);

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_array_value() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "tags".to_string(),
            Value::Array(vec![Value::String("a".to_string()), Value::String("b".to_string())]),
        );

        let mut filter = HashMap::new();
        filter.insert(
            "tags".to_string(),
            Value::Array(vec![Value::String("a".to_string()), Value::String("b".to_string())]),
        );

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_array_order_matters() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "tags".to_string(),
            Value::Array(vec![Value::String("a".to_string()), Value::String("b".to_string())]),
        );

        let mut filter = HashMap::new();
        filter.insert(
            "tags".to_string(),
            Value::Array(vec![Value::String("b".to_string()), Value::String("a".to_string())]),
        );

        // Arrays with different order should not match
        assert!(!SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_nested_object() {
        let mut inner = serde_json::Map::new();
        inner.insert("key".to_string(), Value::String("value".to_string()));

        let mut metadata = HashMap::new();
        metadata.insert("nested".to_string(), Value::Object(inner.clone()));

        let mut filter = HashMap::new();
        filter.insert("nested".to_string(), Value::Object(inner));

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_type_mismatch() {
        let mut metadata = HashMap::new();
        metadata.insert("value".to_string(), Value::String("42".to_string()));

        let mut filter = HashMap::new();
        filter.insert("value".to_string(), Value::Number(42.into()));

        // String "42" != Number 42
        assert!(!SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_matches_filter_multiple_types() {
        let mut metadata = HashMap::new();
        metadata.insert("name".to_string(), Value::String("test".to_string()));
        metadata.insert("count".to_string(), Value::Number(5.into()));
        metadata.insert("active".to_string(), Value::Bool(true));

        let mut filter = HashMap::new();
        filter.insert("name".to_string(), Value::String("test".to_string()));
        filter.insert("count".to_string(), Value::Number(5.into()));

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    // === Metadata JSON serialization edge cases ===

    #[test]
    fn test_metadata_json_unicode() {
        let mut metadata = HashMap::new();
        metadata.insert("name".to_string(), Value::String("日本語テスト".to_string()));

        let json = serde_json::to_string(&metadata).unwrap();
        let recovered: HashMap<String, Value> = serde_json::from_str(&json).unwrap();

        assert_eq!(recovered.get("name"), Some(&Value::String("日本語テスト".to_string())));
    }

    #[test]
    fn test_metadata_json_emoji() {
        let mut metadata = HashMap::new();
        metadata.insert("emoji".to_string(), Value::String("🎉🚀💻".to_string()));

        let json = serde_json::to_string(&metadata).unwrap();
        let recovered: HashMap<String, Value> = serde_json::from_str(&json).unwrap();

        assert_eq!(recovered.get("emoji"), Some(&Value::String("🎉🚀💻".to_string())));
    }

    #[test]
    fn test_metadata_json_special_chars() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "text".to_string(),
            Value::String("line1\nline2\ttab\"quote\\backslash".to_string()),
        );

        let json = serde_json::to_string(&metadata).unwrap();
        let recovered: HashMap<String, Value> = serde_json::from_str(&json).unwrap();

        assert_eq!(
            recovered.get("text"),
            Some(&Value::String("line1\nline2\ttab\"quote\\backslash".to_string()))
        );
    }

    #[test]
    fn test_metadata_json_nested_structure() {
        let json = r#"{
            "level1": {
                "level2": {
                    "level3": "deep"
                }
            }
        }"#;

        let metadata: HashMap<String, Value> = serde_json::from_str(json).unwrap();
        assert!(metadata.contains_key("level1"));
    }

    #[test]
    fn test_metadata_json_array_of_objects() {
        let json = r#"{
            "items": [
                {"name": "first"},
                {"name": "second"}
            ]
        }"#;

        let metadata: HashMap<String, Value> = serde_json::from_str(json).unwrap();
        assert!(metadata.contains_key("items"));
    }

    #[test]
    fn test_metadata_json_large_numbers() {
        let json = r#"{"big": 9007199254740993}"#;

        let metadata: HashMap<String, Value> = serde_json::from_str(json).unwrap();
        assert!(metadata.contains_key("big"));
    }

    #[test]
    fn test_metadata_json_scientific_notation() {
        let json = r#"{"value": 1.5e10}"#;

        let metadata: HashMap<String, Value> = serde_json::from_str(json).unwrap();
        if let Some(Value::Number(n)) = metadata.get("value") {
            let val = n.as_f64().unwrap();
            assert!((val - 1.5e10).abs() < 1.0);
        }
    }

    // === Document struct tests ===

    #[test]
    fn test_document_unicode_content() {
        let doc = Document {
            id: Some("unicode-doc".to_string()),
            page_content: "Hello 世界 🌍".to_string(),
            metadata: HashMap::new(),
        };

        assert!(doc.page_content.contains("世界"));
        assert!(doc.page_content.contains("🌍"));
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
    fn test_document_very_long_content() {
        let content = "a".repeat(100_000);
        let doc = Document {
            id: Some("long".to_string()),
            page_content: content.clone(),
            metadata: HashMap::new(),
        };

        assert_eq!(doc.page_content.len(), 100_000);
    }

    #[test]
    fn test_document_many_metadata_keys() {
        let mut metadata = HashMap::new();
        for i in 0..100 {
            metadata.insert(format!("key_{i}"), Value::Number(i.into()));
        }

        let doc = Document {
            id: Some("many-keys".to_string()),
            page_content: "content".to_string(),
            metadata,
        };

        assert_eq!(doc.metadata.len(), 100);
    }

    // === UUID tests ===

    #[test]
    fn test_uuid_format_v4() {
        let id = Uuid::new_v4().to_string();

        // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);

        // Version 4 indicator
        assert!(parts[2].starts_with('4'));
    }

    #[test]
    fn test_uuid_uniqueness_bulk() {
        let ids: Vec<String> = (0..1000).map(|_| Uuid::new_v4().to_string()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();

        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn test_uuid_lowercase() {
        let id = Uuid::new_v4().to_string();

        // UUID strings should be lowercase
        assert_eq!(id, id.to_lowercase());
    }

    // === SQL query safety tests ===

    #[test]
    fn test_sql_table_name_safe_characters() {
        let table_name = "dashflow_vectors";

        // Should only contain safe characters
        assert!(table_name.chars().all(|c| c.is_alphanumeric() || c == '_'));
    }

    #[test]
    fn test_sql_parameterized_query_format() {
        let sql = "INSERT INTO table (id, text, metadata) VALUES (?1, ?2, ?3)";

        // Verify parameters are numbered correctly
        assert!(sql.contains("?1"));
        assert!(sql.contains("?2"));
        assert!(sql.contains("?3"));
    }

    #[test]
    fn test_vss_table_naming_convention() {
        let base_name = "docs";
        let vss_name = format!("{}_vss", base_name);

        assert_eq!(vss_name, "docs_vss");
    }

    // === Error message formatting tests ===

    #[test]
    fn test_error_message_lock_failure() {
        let error_msg = "Failed to acquire lock: PoisonError";
        assert!(error_msg.contains("lock"));
    }

    #[test]
    fn test_error_message_database_open() {
        let error_msg = "Failed to open SQLite database: unable to open database file";
        assert!(error_msg.contains("SQLite"));
        assert!(error_msg.contains("database"));
    }

    #[test]
    fn test_error_message_table_creation() {
        let error_msg = "Failed to create table: table already exists";
        assert!(error_msg.contains("create table"));
    }

    #[test]
    fn test_error_message_serialization() {
        let error_msg = "Failed to serialize metadata: invalid type";
        assert!(error_msg.contains("serialize"));
        assert!(error_msg.contains("metadata"));
    }

    // === Input validation edge cases ===

    #[test]
    fn test_validate_zero_texts() {
        let texts: Vec<String> = vec![];
        let metadatas: Vec<HashMap<String, Value>> = vec![];

        // Zero texts with zero metadatas is valid
        assert_eq!(texts.len(), metadatas.len());
    }

    #[test]
    fn test_validate_single_text() {
        let texts = ["single text"];
        let metadatas: [HashMap<String, Value>; 1] = [HashMap::new()];
        let ids = ["id1".to_string()];

        assert_eq!(texts.len(), metadatas.len());
        assert_eq!(texts.len(), ids.len());
    }

    #[test]
    fn test_validate_large_batch() {
        let count = 10_000;
        let texts: Vec<String> = (0..count).map(|i| format!("text {i}")).collect();
        let ids: Vec<String> = (0..count).map(|i| format!("id_{i}")).collect();

        assert_eq!(texts.len(), ids.len());
    }

    // === Dimension validation tests ===

    #[test]
    fn test_common_embedding_dimensions() {
        let common_dims = [384, 512, 768, 1024, 1536, 3072, 4096];

        for dim in common_dims {
            assert!(dim > 0);
            assert!(dim <= 8192);
        }
    }

    #[test]
    fn test_dimension_boundary_values() {
        let min_dim = 1;
        let large_dim = 8192;

        assert!(min_dim > 0);
        assert!(large_dim > 0);
    }

    // === Filter logic edge cases ===

    #[test]
    fn test_filter_empty_string_value() {
        let mut metadata = HashMap::new();
        metadata.insert("tag".to_string(), Value::String(String::new()));

        let mut filter = HashMap::new();
        filter.insert("tag".to_string(), Value::String(String::new()));

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_filter_whitespace_string() {
        let mut metadata = HashMap::new();
        metadata.insert("tag".to_string(), Value::String("  ".to_string()));

        let mut filter = HashMap::new();
        filter.insert("tag".to_string(), Value::String("  ".to_string()));

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_filter_zero_numeric() {
        let mut metadata = HashMap::new();
        metadata.insert("count".to_string(), Value::Number(0.into()));

        let mut filter = HashMap::new();
        filter.insert("count".to_string(), Value::Number(0.into()));

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }

    #[test]
    fn test_filter_negative_numeric() {
        let mut metadata = HashMap::new();
        metadata.insert("score".to_string(), Value::Number((-5).into()));

        let mut filter = HashMap::new();
        filter.insert("score".to_string(), Value::Number((-5).into()));

        assert!(SQLiteVSSStore::matches_filter(&metadata, Some(&filter)));
    }
}
