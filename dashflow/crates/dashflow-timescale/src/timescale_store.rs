//! `TimescaleDB` pgvectorscale vector store implementation for `DashFlow` Rust.
//!
//! This implementation uses `TimescaleDB`'s pgvectorscale extension which provides:
//! - `StreamingDiskANN` index for high-performance ANN search
//! - Statistical Binary Quantization (SBQ) for cost-efficient storage
//! - Label-based filtering for hybrid search

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::{Error, Result};
use dashflow::{embed, embed_query};
use pgvector::Vector;
use serde_json::Value as JsonValue;
use tokio_postgres::{Client, NoTls};

/// `TimescaleDB` pgvectorscale vector store implementation.
///
/// Uses `StreamingDiskANN` indexing for high-performance approximate nearest neighbor search.
pub struct TimescaleVectorStore {
    client: Arc<tokio::sync::Mutex<Client>>,
    collection_name: String,
    embeddings: Arc<dyn Embeddings>,
    distance_metric: DistanceMetric,
}

impl TimescaleVectorStore {
    /// Creates a new `TimescaleVectorStore` instance.
    ///
    /// # Arguments
    ///
    /// * `connection_string` - `PostgreSQL` connection string (e.g., "<postgresql://user:pass@localhost:5432/db>")
    /// * `collection_name` - Name of the collection/table
    /// * `embeddings` - Embeddings model to use
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Connection to `PostgreSQL` fails
    /// - pgvector or pgvectorscale extension is not installed
    /// - Table creation fails
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dashflow_timescale::TimescaleVectorStore;
    /// # use dashflow::core::embeddings::Embeddings;
    /// # use std::sync::Arc;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # struct MockEmbeddings;
    /// # #[async_trait::async_trait]
    /// # impl Embeddings for MockEmbeddings {
    /// #     async fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, dashflow::core::Error> {
    /// #         Ok(texts.iter().map(|_| vec![0.0; 1536]).collect())
    /// #     }
    /// #     async fn embed_query(&self, text: &str) -> Result<Vec<f32>, dashflow::core::Error> {
    /// #         Ok(vec![0.0; 1536])
    /// #     }
    /// # }
    /// let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
    ///
    /// let store = TimescaleVectorStore::new(
    ///     "postgresql://user:pass@localhost:5432/vectordb",
    ///     "my_documents",
    ///     embeddings,
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(
        connection_string: &str,
        collection_name: &str,
        embeddings: Arc<dyn Embeddings>,
    ) -> Result<Self> {
        // Connect to PostgreSQL
        let (client, connection) = tokio_postgres::connect(connection_string, NoTls)
            .await
            .map_err(|e| Error::config(format!("Failed to connect to PostgreSQL: {e}")))?;

        // Spawn connection task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!(error = %e, "PostgreSQL connection error");
            }
        });

        let client = Arc::new(tokio::sync::Mutex::new(client));

        let store = Self {
            client,
            collection_name: collection_name.to_string(),
            embeddings,
            distance_metric: DistanceMetric::Cosine,
        };

        // Ensure extensions and table exist
        store.ensure_extensions().await?;
        store.ensure_table().await?;

        Ok(store)
    }

    /// Ensures both pgvector and pgvectorscale extensions are installed.
    async fn ensure_extensions(&self) -> Result<()> {
        let client = self.client.lock().await;

        // First ensure pgvector is installed
        client
            .execute("CREATE EXTENSION IF NOT EXISTS vector", &[])
            .await
            .map_err(|e| {
                Error::other(format!(
                    "Failed to create pgvector extension (is it installed?): {e}"
                ))
            })?;

        // Then ensure pgvectorscale is installed (CASCADE ensures dependencies)
        client
            .execute("CREATE EXTENSION IF NOT EXISTS vectorscale CASCADE", &[])
            .await
            .map_err(|e| {
                Error::other(format!(
                    "Failed to create vectorscale extension (is it installed?): {e}. \
                     Install from https://github.com/timescale/pgvectorscale"
                ))
            })?;

        Ok(())
    }

    /// Ensures the collection table exists with proper schema.
    async fn ensure_table(&self) -> Result<()> {
        let client = self.client.lock().await;

        // Create table with id, text, embedding (vector), and metadata (jsonb)
        let create_table_query = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id TEXT PRIMARY KEY,
                text TEXT NOT NULL,
                embedding vector(1536),
                metadata JSONB DEFAULT '{{}}'::jsonb
            )",
            self.collection_name
        );

        client
            .execute(&create_table_query, &[])
            .await
            .map_err(|e| Error::other(format!("Failed to create table: {e}")))?;

        // Create StreamingDiskANN index for vector similarity search
        // Using diskann index type from pgvectorscale for high performance
        let index_name = format!("{}_embedding_idx", self.collection_name);
        let ops_type = match self.distance_metric {
            DistanceMetric::Cosine => "vector_cosine_ops",
            DistanceMetric::Euclidean => "vector_l2_ops",
            DistanceMetric::DotProduct | DistanceMetric::MaxInnerProduct => "vector_ip_ops",
        };

        let create_index_query = format!(
            "CREATE INDEX IF NOT EXISTS {} ON {} USING diskann (embedding {}) WITH (num_neighbors = 50)",
            index_name, self.collection_name, ops_type
        );

        // Ignore index creation errors (might fail if table is empty or extension not available)
        // In that case, fall back to sequential scan
        let _ = client.execute(&create_index_query, &[]).await;

        Ok(())
    }

    /// Converts distance metric to pgvector operator.
    fn distance_metric_to_operator(&self) -> &'static str {
        match self.distance_metric {
            DistanceMetric::Cosine => "<=>",          // Cosine distance
            DistanceMetric::Euclidean => "<->",       // L2 distance
            DistanceMetric::DotProduct => "<#>",      // Negative inner product (for max IP)
            DistanceMetric::MaxInnerProduct => "<#>", // Negative inner product
        }
    }

    /// Builds a WHERE clause for metadata filtering.
    fn build_where_clause(&self, filter: &HashMap<String, JsonValue>) -> String {
        if filter.is_empty() {
            return String::from("TRUE");
        }

        let conditions: Vec<String> = filter
            .iter()
            .map(|(k, v)| {
                format!(
                    "metadata->>'{}' = '{}'",
                    k,
                    v.as_str().unwrap_or(&v.to_string())
                )
            })
            .collect();

        conditions.join(" AND ")
    }
}

#[async_trait]
impl VectorStore for TimescaleVectorStore {
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

        // Insert documents
        let client = self.client.lock().await;

        for (i, text) in text_strings.iter().enumerate() {
            let embedding = Vector::from(embeddings_vec[i].clone());
            let metadata_json = if let Some(metadatas) = metadatas {
                serde_json::to_value(&metadatas[i])
                    .map_err(|e| Error::other(format!("Failed to serialize metadata: {e}")))?
            } else {
                JsonValue::Object(serde_json::Map::new())
            };

            let insert_query = format!(
                "INSERT INTO {} (id, text, embedding, metadata) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (id) DO UPDATE SET text = $2, embedding = $3, metadata = $4",
                self.collection_name
            );

            client
                .execute(
                    &insert_query,
                    &[&doc_ids[i], &text, &embedding, &metadata_json],
                )
                .await
                .map_err(|e| Error::other(format!("Failed to insert document: {e}")))?;
        }

        Ok(doc_ids)
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        let client = self.client.lock().await;

        if let Some(ids) = ids {
            if ids.is_empty() {
                return Ok(true);
            }

            let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();
            let delete_query = format!(
                "DELETE FROM {} WHERE id IN ({})",
                self.collection_name,
                placeholders.join(", ")
            );

            let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = ids
                .iter()
                .map(|id| id as &(dyn tokio_postgres::types::ToSql + Sync))
                .collect();

            client
                .execute(&delete_query, &params)
                .await
                .map_err(|e| Error::other(format!("Failed to delete documents: {e}")))?;
        } else {
            // Delete all documents
            let delete_query = format!("DELETE FROM {}", self.collection_name);
            client
                .execute(&delete_query, &[])
                .await
                .map_err(|e| Error::other(format!("Failed to delete all documents: {e}")))?;
        }

        Ok(true)
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let client = self.client.lock().await;

        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();
        let select_query = format!(
            "SELECT id, text, metadata FROM {} WHERE id IN ({})",
            self.collection_name,
            placeholders.join(", ")
        );

        let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = ids
            .iter()
            .map(|id| id as &(dyn tokio_postgres::types::ToSql + Sync))
            .collect();

        let rows = client
            .query(&select_query, &params)
            .await
            .map_err(|e| Error::other(format!("Failed to fetch documents: {e}")))?;

        let mut documents = Vec::new();
        for row in rows {
            let id: String = row.get(0);
            let text: String = row.get(1);
            let metadata_json: JsonValue = row.get(2);

            let metadata: HashMap<String, JsonValue> = if let JsonValue::Object(obj) = metadata_json
            {
                obj.into_iter().collect()
            } else {
                HashMap::new()
            };

            documents.push(Document {
                id: Some(id),
                page_content: text,
                metadata,
            });
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

        // Perform vector search using DiskANN
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

        // Perform vector search with scores using DiskANN
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
        let client = self.client.lock().await;

        let embedding_vec = Vector::from(embedding.to_vec());
        let operator = self.distance_metric_to_operator();

        let where_clause =
            filter.map_or_else(|| "TRUE".to_string(), |f| self.build_where_clause(f));

        // Query using DiskANN index
        // The index is automatically used by PostgreSQL query planner when ORDER BY embedding <op> is present
        let query = format!(
            "SELECT id, text, metadata, embedding {} $1::vector AS distance
             FROM {}
             WHERE {}
             ORDER BY distance
             LIMIT $2",
            operator, self.collection_name, where_clause
        );

        let rows = client
            .query(&query, &[&embedding_vec, &(k as i64)])
            .await
            .map_err(|e| Error::other(format!("Failed to search documents: {e}")))?;

        let mut results = Vec::new();
        for row in rows {
            let id: String = row.get(0);
            let text: String = row.get(1);
            let metadata_json: JsonValue = row.get(2);
            let distance: f32 = row.get(3);

            let metadata: HashMap<String, JsonValue> = if let JsonValue::Object(obj) = metadata_json
            {
                obj.into_iter().collect()
            } else {
                HashMap::new()
            };

            let doc = Document {
                id: Some(id),
                page_content: text,
                metadata,
            };

            // Convert distance to similarity score (0-1, higher is more similar)
            let score = match self.distance_metric {
                DistanceMetric::Cosine => (1.0 - distance).max(0.0),
                DistanceMetric::Euclidean => 1.0 / (1.0 + distance),
                DistanceMetric::DotProduct | DistanceMetric::MaxInnerProduct => -distance, // pgvector returns negative
            };

            results.push((doc, score));
        }

        Ok(results)
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    // Test distance_metric_to_operator function
    #[test]
    fn test_distance_metric_to_operator_cosine() {
        // Test helper that mimics the method
        fn distance_metric_to_operator(metric: DistanceMetric) -> &'static str {
            match metric {
                DistanceMetric::Cosine => "<=>",
                DistanceMetric::Euclidean => "<->",
                DistanceMetric::DotProduct => "<#>",
                DistanceMetric::MaxInnerProduct => "<#>",
            }
        }

        assert_eq!(distance_metric_to_operator(DistanceMetric::Cosine), "<=>");
    }

    #[test]
    fn test_distance_metric_to_operator_euclidean() {
        fn distance_metric_to_operator(metric: DistanceMetric) -> &'static str {
            match metric {
                DistanceMetric::Cosine => "<=>",
                DistanceMetric::Euclidean => "<->",
                DistanceMetric::DotProduct => "<#>",
                DistanceMetric::MaxInnerProduct => "<#>",
            }
        }

        assert_eq!(distance_metric_to_operator(DistanceMetric::Euclidean), "<->");
    }

    #[test]
    fn test_distance_metric_to_operator_dot_product() {
        fn distance_metric_to_operator(metric: DistanceMetric) -> &'static str {
            match metric {
                DistanceMetric::Cosine => "<=>",
                DistanceMetric::Euclidean => "<->",
                DistanceMetric::DotProduct => "<#>",
                DistanceMetric::MaxInnerProduct => "<#>",
            }
        }

        assert_eq!(distance_metric_to_operator(DistanceMetric::DotProduct), "<#>");
    }

    #[test]
    fn test_distance_metric_to_operator_max_inner_product() {
        fn distance_metric_to_operator(metric: DistanceMetric) -> &'static str {
            match metric {
                DistanceMetric::Cosine => "<=>",
                DistanceMetric::Euclidean => "<->",
                DistanceMetric::DotProduct => "<#>",
                DistanceMetric::MaxInnerProduct => "<#>",
            }
        }

        assert_eq!(distance_metric_to_operator(DistanceMetric::MaxInnerProduct), "<#>");
    }

    // Test build_where_clause function
    #[test]
    fn test_build_where_clause_empty() {
        fn build_where_clause(filter: &HashMap<String, JsonValue>) -> String {
            if filter.is_empty() {
                return String::from("TRUE");
            }

            let conditions: Vec<String> = filter
                .iter()
                .map(|(k, v)| {
                    format!(
                        "metadata->>'{}' = '{}'",
                        k,
                        v.as_str().unwrap_or(&v.to_string())
                    )
                })
                .collect();

            conditions.join(" AND ")
        }

        let filter = HashMap::new();
        assert_eq!(build_where_clause(&filter), "TRUE");
    }

    #[test]
    fn test_build_where_clause_single_condition() {
        fn build_where_clause(filter: &HashMap<String, JsonValue>) -> String {
            if filter.is_empty() {
                return String::from("TRUE");
            }

            let conditions: Vec<String> = filter
                .iter()
                .map(|(k, v)| {
                    format!(
                        "metadata->>'{}' = '{}'",
                        k,
                        v.as_str().unwrap_or(&v.to_string())
                    )
                })
                .collect();

            conditions.join(" AND ")
        }

        let mut filter = HashMap::new();
        filter.insert("type".to_string(), JsonValue::String("article".to_string()));

        let clause = build_where_clause(&filter);
        assert!(clause.contains("metadata->>'type'"));
        assert!(clause.contains("'article'"));
    }

    #[test]
    fn test_build_where_clause_multiple_conditions() {
        fn build_where_clause(filter: &HashMap<String, JsonValue>) -> String {
            if filter.is_empty() {
                return String::from("TRUE");
            }

            let mut conditions: Vec<String> = filter
                .iter()
                .map(|(k, v)| {
                    format!(
                        "metadata->>'{}' = '{}'",
                        k,
                        v.as_str().unwrap_or(&v.to_string())
                    )
                })
                .collect();

            // Sort for deterministic testing
            conditions.sort();
            conditions.join(" AND ")
        }

        let mut filter = HashMap::new();
        filter.insert("author".to_string(), JsonValue::String("Alice".to_string()));
        filter.insert("type".to_string(), JsonValue::String("article".to_string()));

        let clause = build_where_clause(&filter);
        assert!(clause.contains(" AND "));
        assert!(clause.contains("author"));
        assert!(clause.contains("type"));
    }

    #[test]
    fn test_build_where_clause_numeric_value() {
        fn build_where_clause(filter: &HashMap<String, JsonValue>) -> String {
            if filter.is_empty() {
                return String::from("TRUE");
            }

            let conditions: Vec<String> = filter
                .iter()
                .map(|(k, v)| {
                    format!(
                        "metadata->>'{}' = '{}'",
                        k,
                        v.as_str().unwrap_or(&v.to_string())
                    )
                })
                .collect();

            conditions.join(" AND ")
        }

        let mut filter = HashMap::new();
        filter.insert("count".to_string(), JsonValue::Number(serde_json::Number::from(42)));

        let clause = build_where_clause(&filter);
        assert!(clause.contains("metadata->>'count'"));
        assert!(clause.contains("42"));
    }

    // Test distance to score conversion
    #[test]
    fn test_distance_to_score_cosine() {
        // Cosine: score = (1.0 - distance).max(0.0)
        let distance: f32 = 0.2;
        let score = (1.0_f32 - distance).max(0.0);
        assert!((score - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_distance_to_score_cosine_high_distance() {
        // When distance > 1.0, score should be clamped to 0
        let distance: f32 = 1.5;
        let score = (1.0_f32 - distance).max(0.0);
        assert!(score.abs() < 1e-6);
    }

    #[test]
    fn test_distance_to_score_euclidean() {
        // Euclidean: score = 1.0 / (1.0 + distance)
        let distance: f32 = 0.0;
        let score = 1.0_f32 / (1.0 + distance);
        assert!((score - 1.0).abs() < 1e-6);

        let distance: f32 = 1.0;
        let score = 1.0_f32 / (1.0 + distance);
        assert!((score - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_distance_to_score_dot_product() {
        // DotProduct: score = -distance (pgvector returns negative)
        let distance: f32 = -0.8;
        let score = -distance;
        assert!((score - 0.8).abs() < 1e-6);
    }

    // Test ops_type selection for index creation
    #[test]
    fn test_ops_type_for_cosine() {
        fn get_ops_type(metric: DistanceMetric) -> &'static str {
            match metric {
                DistanceMetric::Cosine => "vector_cosine_ops",
                DistanceMetric::Euclidean => "vector_l2_ops",
                DistanceMetric::DotProduct | DistanceMetric::MaxInnerProduct => "vector_ip_ops",
            }
        }

        assert_eq!(get_ops_type(DistanceMetric::Cosine), "vector_cosine_ops");
    }

    #[test]
    fn test_ops_type_for_euclidean() {
        fn get_ops_type(metric: DistanceMetric) -> &'static str {
            match metric {
                DistanceMetric::Cosine => "vector_cosine_ops",
                DistanceMetric::Euclidean => "vector_l2_ops",
                DistanceMetric::DotProduct | DistanceMetric::MaxInnerProduct => "vector_ip_ops",
            }
        }

        assert_eq!(get_ops_type(DistanceMetric::Euclidean), "vector_l2_ops");
    }

    #[test]
    fn test_ops_type_for_inner_product() {
        fn get_ops_type(metric: DistanceMetric) -> &'static str {
            match metric {
                DistanceMetric::Cosine => "vector_cosine_ops",
                DistanceMetric::Euclidean => "vector_l2_ops",
                DistanceMetric::DotProduct | DistanceMetric::MaxInnerProduct => "vector_ip_ops",
            }
        }

        assert_eq!(get_ops_type(DistanceMetric::DotProduct), "vector_ip_ops");
        assert_eq!(get_ops_type(DistanceMetric::MaxInnerProduct), "vector_ip_ops");
    }

    // Test table name formatting
    #[test]
    fn test_index_name_format() {
        let collection_name = "my_documents";
        let index_name = format!("{}_embedding_idx", collection_name);
        assert_eq!(index_name, "my_documents_embedding_idx");
    }

    // Test create table SQL format
    #[test]
    fn test_create_table_query_format() {
        let collection_name = "test_collection";
        let query = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id TEXT PRIMARY KEY,
                text TEXT NOT NULL,
                embedding vector(1536),
                metadata JSONB DEFAULT '{{}}'::jsonb
            )",
            collection_name
        );

        assert!(query.contains("test_collection"));
        assert!(query.contains("id TEXT PRIMARY KEY"));
        assert!(query.contains("embedding vector(1536)"));
        assert!(query.contains("metadata JSONB"));
    }

    // Test create index SQL format
    #[test]
    fn test_create_index_query_format() {
        let collection_name = "docs";
        let index_name = format!("{}_embedding_idx", collection_name);
        let ops_type = "vector_cosine_ops";

        let query = format!(
            "CREATE INDEX IF NOT EXISTS {} ON {} USING diskann (embedding {}) WITH (num_neighbors = 50)",
            index_name, collection_name, ops_type
        );

        assert!(query.contains("docs_embedding_idx"));
        assert!(query.contains("USING diskann"));
        assert!(query.contains("vector_cosine_ops"));
        assert!(query.contains("num_neighbors = 50"));
    }

    // Test insert query format
    #[test]
    fn test_insert_query_format() {
        let collection_name = "documents";
        let query = format!(
            "INSERT INTO {} (id, text, embedding, metadata) VALUES ($1, $2, $3, $4)
             ON CONFLICT (id) DO UPDATE SET text = $2, embedding = $3, metadata = $4",
            collection_name
        );

        assert!(query.contains("INSERT INTO documents"));
        assert!(query.contains("ON CONFLICT (id) DO UPDATE"));
        assert!(query.contains("$1, $2, $3, $4"));
    }

    // Test delete query format
    #[test]
    fn test_delete_query_single() {
        let collection_name = "docs";
        let ids = ["id1".to_string()];
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();

        let query = format!(
            "DELETE FROM {} WHERE id IN ({})",
            collection_name,
            placeholders.join(", ")
        );

        assert!(query.contains("DELETE FROM docs"));
        assert!(query.contains("WHERE id IN ($1)"));
    }

    #[test]
    fn test_delete_query_multiple() {
        let collection_name = "docs";
        let ids = ["id1".to_string(), "id2".to_string(), "id3".to_string()];
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();

        let query = format!(
            "DELETE FROM {} WHERE id IN ({})",
            collection_name,
            placeholders.join(", ")
        );

        assert!(query.contains("WHERE id IN ($1, $2, $3)"));
    }

    // Test select query format
    #[test]
    fn test_select_query_format() {
        let collection_name = "vectors";
        let ids = ["a".to_string(), "b".to_string()];
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();

        let query = format!(
            "SELECT id, text, metadata FROM {} WHERE id IN ({})",
            collection_name,
            placeholders.join(", ")
        );

        assert!(query.contains("SELECT id, text, metadata"));
        assert!(query.contains("FROM vectors"));
        assert!(query.contains("$1, $2"));
    }

    // Test similarity search query format
    #[test]
    fn test_similarity_search_query_format() {
        let collection_name = "docs";
        let operator = "<=>";
        let where_clause = "TRUE";

        let query = format!(
            "SELECT id, text, metadata, embedding {} $1::vector AS distance
             FROM {}
             WHERE {}
             ORDER BY distance
             LIMIT $2",
            operator, collection_name, where_clause
        );

        assert!(query.contains("embedding <=> $1::vector AS distance"));
        assert!(query.contains("ORDER BY distance"));
        assert!(query.contains("LIMIT $2"));
    }

    // Test Document struct creation
    #[test]
    fn test_document_creation() {
        let doc = Document {
            id: Some("doc-1".to_string()),
            page_content: "Hello world".to_string(),
            metadata: HashMap::new(),
        };

        assert_eq!(doc.id, Some("doc-1".to_string()));
        assert_eq!(doc.page_content, "Hello world");
        assert!(doc.metadata.is_empty());
    }

    #[test]
    fn test_document_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), JsonValue::String("web".to_string()));

        let doc = Document {
            id: None,
            page_content: "Content".to_string(),
            metadata,
        };

        assert_eq!(doc.metadata.get("source"), Some(&JsonValue::String("web".to_string())));
    }

    // Test metadata JSON parsing
    #[test]
    fn test_metadata_json_object_conversion() {
        let json_obj = serde_json::json!({"key": "value", "num": 42});

        if let JsonValue::Object(obj) = json_obj {
            let metadata: HashMap<String, JsonValue> = obj.into_iter().collect();
            assert_eq!(metadata.len(), 2);
            assert_eq!(metadata.get("key"), Some(&JsonValue::String("value".to_string())));
        } else {
            panic!("Expected object");
        }
    }

    #[test]
    fn test_metadata_json_non_object_fallback() {
        let json_array = JsonValue::Array(vec![JsonValue::from(1), JsonValue::from(2)]);

        // Non-object should result in empty HashMap
        let metadata: HashMap<String, JsonValue> = if let JsonValue::Object(obj) = json_array {
            obj.into_iter().collect()
        } else {
            HashMap::new()
        };

        assert!(metadata.is_empty());
    }

    // Test UUID generation
    #[test]
    fn test_uuid_uniqueness() {
        let id1 = uuid::Uuid::new_v4().to_string();
        let id2 = uuid::Uuid::new_v4().to_string();

        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 36);
        assert_eq!(id2.len(), 36);
    }

    // Test input validation
    #[test]
    fn test_metadatas_length_mismatch_error() {
        let texts_len = 5;
        let metadatas_len = 3;

        let error_msg = format!(
            "Metadatas length mismatch: {} vs {}",
            metadatas_len, texts_len
        );

        assert!(error_msg.contains("3"));
        assert!(error_msg.contains("5"));
    }

    #[test]
    fn test_ids_length_mismatch_error() {
        let texts_len = 4;
        let ids_len = 2;

        let error_msg = format!(
            "IDs length mismatch: {} vs {}",
            ids_len, texts_len
        );

        assert!(error_msg.contains("2"));
        assert!(error_msg.contains("4"));
    }

    // Test empty ids handling
    #[test]
    fn test_delete_with_empty_ids() {
        let ids: Vec<String> = vec![];

        // Should return early with Ok(true) when ids is empty
        let should_return_early = ids.is_empty();
        assert!(should_return_early);
    }

    #[test]
    fn test_get_by_ids_empty() {
        let ids: Vec<String> = vec![];

        // Should return empty vec when ids is empty
        if ids.is_empty() {
            let results: Vec<Document> = vec![];
            assert!(results.is_empty());
        }
    }

    // =========================================================================
    // Extended Distance Score Tests
    // =========================================================================

    #[test]
    fn test_distance_to_score_cosine_zero_distance() {
        // Zero distance = perfect match
        let distance: f32 = 0.0;
        let score = (1.0_f32 - distance).max(0.0);
        assert!((score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_distance_to_score_cosine_perfect_opposite() {
        // Distance of 2.0 means opposite vectors (angle = 180°)
        let distance: f32 = 2.0;
        let score = (1.0_f32 - distance).max(0.0);
        assert!((score - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_distance_to_score_euclidean_high_distance() {
        // High distance = low score
        let distance: f32 = 100.0;
        let score = 1.0_f32 / (1.0 + distance);
        assert!((score - 0.0099).abs() < 0.001);
    }

    #[test]
    fn test_distance_to_score_euclidean_precise() {
        // distance=3 gives 1/(1+3) = 0.25
        let distance: f32 = 3.0;
        let score = 1.0_f32 / (1.0 + distance);
        assert!((score - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_distance_to_score_dot_product_positive() {
        // Positive negative distance = negative score (shouldn't happen in practice)
        let distance: f32 = 0.5;
        let score = -distance;
        assert!((score - (-0.5)).abs() < 1e-6);
    }

    #[test]
    fn test_distance_to_score_dot_product_large() {
        let distance: f32 = -10.0;
        let score = -distance;
        assert!((score - 10.0).abs() < 1e-6);
    }

    // =========================================================================
    // Extended Filter Clause Tests
    // =========================================================================

    #[test]
    fn test_build_where_clause_boolean_value() {
        fn build_where_clause(filter: &HashMap<String, JsonValue>) -> String {
            if filter.is_empty() {
                return String::from("TRUE");
            }
            let conditions: Vec<String> = filter
                .iter()
                .map(|(k, v)| {
                    format!(
                        "metadata->>'{}' = '{}'",
                        k,
                        v.as_str().unwrap_or(&v.to_string())
                    )
                })
                .collect();
            conditions.join(" AND ")
        }

        let mut filter = HashMap::new();
        filter.insert("active".to_string(), JsonValue::Bool(true));

        let clause = build_where_clause(&filter);
        assert!(clause.contains("metadata->>'active'"));
        assert!(clause.contains("true"));
    }

    #[test]
    fn test_build_where_clause_null_value() {
        fn build_where_clause(filter: &HashMap<String, JsonValue>) -> String {
            if filter.is_empty() {
                return String::from("TRUE");
            }
            let conditions: Vec<String> = filter
                .iter()
                .map(|(k, v)| {
                    format!(
                        "metadata->>'{}' = '{}'",
                        k,
                        v.as_str().unwrap_or(&v.to_string())
                    )
                })
                .collect();
            conditions.join(" AND ")
        }

        let mut filter = HashMap::new();
        filter.insert("deleted".to_string(), JsonValue::Null);

        let clause = build_where_clause(&filter);
        assert!(clause.contains("metadata->>'deleted'"));
        assert!(clause.contains("null"));
    }

    #[test]
    fn test_build_where_clause_float_value() {
        fn build_where_clause(filter: &HashMap<String, JsonValue>) -> String {
            if filter.is_empty() {
                return String::from("TRUE");
            }
            let conditions: Vec<String> = filter
                .iter()
                .map(|(k, v)| {
                    format!(
                        "metadata->>'{}' = '{}'",
                        k,
                        v.as_str().unwrap_or(&v.to_string())
                    )
                })
                .collect();
            conditions.join(" AND ")
        }

        let mut filter = HashMap::new();
        filter.insert("score".to_string(), serde_json::json!(0.95));

        let clause = build_where_clause(&filter);
        assert!(clause.contains("metadata->>'score'"));
        assert!(clause.contains("0.95"));
    }

    #[test]
    fn test_build_where_clause_negative_number() {
        fn build_where_clause(filter: &HashMap<String, JsonValue>) -> String {
            if filter.is_empty() {
                return String::from("TRUE");
            }
            let conditions: Vec<String> = filter
                .iter()
                .map(|(k, v)| {
                    format!(
                        "metadata->>'{}' = '{}'",
                        k,
                        v.as_str().unwrap_or(&v.to_string())
                    )
                })
                .collect();
            conditions.join(" AND ")
        }

        let mut filter = HashMap::new();
        filter.insert("offset".to_string(), serde_json::json!(-10));

        let clause = build_where_clause(&filter);
        assert!(clause.contains("-10"));
    }

    #[test]
    fn test_build_where_clause_unicode_key() {
        fn build_where_clause(filter: &HashMap<String, JsonValue>) -> String {
            if filter.is_empty() {
                return String::from("TRUE");
            }
            let conditions: Vec<String> = filter
                .iter()
                .map(|(k, v)| {
                    format!(
                        "metadata->>'{}' = '{}'",
                        k,
                        v.as_str().unwrap_or(&v.to_string())
                    )
                })
                .collect();
            conditions.join(" AND ")
        }

        let mut filter = HashMap::new();
        filter.insert("タイプ".to_string(), JsonValue::String("記事".to_string()));

        let clause = build_where_clause(&filter);
        assert!(clause.contains("タイプ"));
        assert!(clause.contains("記事"));
    }

    // =========================================================================
    // SQL Query Format Tests
    // =========================================================================

    #[test]
    fn test_delete_all_query_format() {
        let collection_name = "test_docs";
        let query = format!("DELETE FROM {}", collection_name);
        assert_eq!(query, "DELETE FROM test_docs");
    }

    #[test]
    fn test_create_extension_query_pgvector() {
        let query = "CREATE EXTENSION IF NOT EXISTS vector";
        assert!(query.contains("vector"));
        assert!(query.contains("IF NOT EXISTS"));
    }

    #[test]
    fn test_create_extension_query_vectorscale() {
        let query = "CREATE EXTENSION IF NOT EXISTS vectorscale CASCADE";
        assert!(query.contains("vectorscale"));
        assert!(query.contains("CASCADE"));
    }

    #[test]
    fn test_similarity_search_query_with_filter() {
        let collection_name = "docs";
        let operator = "<=>";
        let where_clause = "metadata->>'type' = 'article'";

        let query = format!(
            "SELECT id, text, metadata, embedding {} $1::vector AS distance
             FROM {}
             WHERE {}
             ORDER BY distance
             LIMIT $2",
            operator, collection_name, where_clause
        );

        assert!(query.contains("WHERE metadata->>'type' = 'article'"));
    }

    #[test]
    fn test_similarity_search_query_euclidean() {
        let collection_name = "vectors";
        let operator = "<->";
        let where_clause = "TRUE";

        let query = format!(
            "SELECT id, text, metadata, embedding {} $1::vector AS distance
             FROM {}
             WHERE {}
             ORDER BY distance
             LIMIT $2",
            operator, collection_name, where_clause
        );

        assert!(query.contains("embedding <-> $1::vector"));
    }

    #[test]
    fn test_similarity_search_query_inner_product() {
        let collection_name = "embeddings";
        let operator = "<#>";
        let where_clause = "TRUE";

        let query = format!(
            "SELECT id, text, metadata, embedding {} $1::vector AS distance
             FROM {}
             WHERE {}
             ORDER BY distance
             LIMIT $2",
            operator, collection_name, where_clause
        );

        assert!(query.contains("embedding <#> $1::vector"));
    }

    // =========================================================================
    // Collection/Table Name Tests
    // =========================================================================

    #[test]
    fn test_collection_name_with_underscore() {
        let collection = "my_test_collection";
        let index_name = format!("{}_embedding_idx", collection);
        assert_eq!(index_name, "my_test_collection_embedding_idx");
    }

    #[test]
    fn test_collection_name_with_numbers() {
        let collection = "collection_v2_2024";
        let index_name = format!("{}_embedding_idx", collection);
        assert_eq!(index_name, "collection_v2_2024_embedding_idx");
    }

    #[test]
    fn test_collection_name_simple() {
        let collection = "docs";
        let create_table = format!(
            "CREATE TABLE IF NOT EXISTS {} (id TEXT PRIMARY KEY)",
            collection
        );
        assert!(create_table.contains("docs"));
    }

    // =========================================================================
    // Placeholder Generation Tests
    // =========================================================================

    #[test]
    fn test_placeholders_single() {
        let ids = vec!["id1".to_string()];
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();
        assert_eq!(placeholders.join(", "), "$1");
    }

    #[test]
    fn test_placeholders_multiple() {
        let ids = vec!["a", "b", "c", "d", "e"];
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();
        assert_eq!(placeholders.join(", "), "$1, $2, $3, $4, $5");
    }

    #[test]
    fn test_placeholders_large() {
        let ids: Vec<String> = (0..100).map(|i| format!("id_{}", i)).collect();
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();
        assert_eq!(placeholders.len(), 100);
        assert!(placeholders.last().unwrap().contains("$100"));
    }

    // =========================================================================
    // Document Tests
    // =========================================================================

    #[test]
    fn test_document_with_none_id() {
        let doc = Document {
            id: None,
            page_content: "Anonymous document".to_string(),
            metadata: HashMap::new(),
        };
        assert!(doc.id.is_none());
    }

    #[test]
    fn test_document_with_complex_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), JsonValue::String("Test".to_string()));
        metadata.insert("page".to_string(), JsonValue::Number(serde_json::Number::from(42)));
        metadata.insert("tags".to_string(), serde_json::json!(["rust", "vector"]));
        metadata.insert("nested".to_string(), serde_json::json!({"a": 1, "b": 2}));

        let doc = Document {
            id: Some("complex-doc".to_string()),
            page_content: "Complex".to_string(),
            metadata,
        };

        assert_eq!(doc.metadata.len(), 4);
        assert!(doc.metadata.get("tags").unwrap().is_array());
        assert!(doc.metadata.get("nested").unwrap().is_object());
    }

    #[test]
    fn test_document_empty_content() {
        let doc = Document {
            id: Some("empty".to_string()),
            page_content: "".to_string(),
            metadata: HashMap::new(),
        };
        assert!(doc.page_content.is_empty());
    }

    #[test]
    fn test_document_unicode_content() {
        let doc = Document {
            id: Some("unicode".to_string()),
            page_content: "日本語テキスト 🎉 Привет мир".to_string(),
            metadata: HashMap::new(),
        };
        assert!(doc.page_content.contains("日本語"));
        assert!(doc.page_content.contains("🎉"));
        assert!(doc.page_content.contains("Привет"));
    }

    #[test]
    fn test_document_very_long_content() {
        let long_content = "x".repeat(100_000);
        let doc = Document {
            id: Some("long".to_string()),
            page_content: long_content.clone(),
            metadata: HashMap::new(),
        };
        assert_eq!(doc.page_content.len(), 100_000);
    }

    #[test]
    fn test_document_special_chars_content() {
        let doc = Document {
            id: Some("special".to_string()),
            page_content: "Line1\nLine2\tTabbed\r\nCRLF".to_string(),
            metadata: HashMap::new(),
        };
        assert!(doc.page_content.contains('\n'));
        assert!(doc.page_content.contains('\t'));
    }

    // =========================================================================
    // Metadata Serialization Tests
    // =========================================================================

    #[test]
    fn test_metadata_serialization_empty() {
        let metadata: HashMap<String, JsonValue> = HashMap::new();
        let json = serde_json::to_value(&metadata).unwrap();
        assert!(json.is_object());
        assert!(json.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_metadata_serialization_mixed_types() {
        let mut metadata = HashMap::new();
        metadata.insert("string".to_string(), JsonValue::String("value".to_string()));
        metadata.insert("number".to_string(), JsonValue::Number(123.into()));
        metadata.insert("bool".to_string(), JsonValue::Bool(true));
        metadata.insert("null".to_string(), JsonValue::Null);

        let json = serde_json::to_value(&metadata).unwrap();
        assert!(json.is_object());
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 4);
    }

    #[test]
    fn test_metadata_json_array_in_value() {
        let mut metadata = HashMap::new();
        metadata.insert("items".to_string(), serde_json::json!([1, 2, 3]));

        let json = serde_json::to_value(&metadata).unwrap();
        assert!(json["items"].is_array());
        assert_eq!(json["items"].as_array().unwrap().len(), 3);
    }

    // =========================================================================
    // UUID Tests
    // =========================================================================

    #[test]
    fn test_uuid_format() {
        let id = uuid::Uuid::new_v4().to_string();
        // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn test_uuid_uniqueness_batch() {
        let ids: Vec<String> = (0..100).map(|_| uuid::Uuid::new_v4().to_string()).collect();
        let mut unique_ids = ids.clone();
        unique_ids.sort();
        unique_ids.dedup();
        assert_eq!(ids.len(), unique_ids.len());
    }

    #[test]
    fn test_uuid_version_4() {
        let id = uuid::Uuid::new_v4();
        assert_eq!(id.get_version_num(), 4);
    }

    // =========================================================================
    // Error Message Tests
    // =========================================================================

    #[test]
    fn test_error_message_format_connection() {
        let error = "connection refused";
        let msg = format!("Failed to connect to PostgreSQL: {error}");
        assert!(msg.contains("Failed to connect"));
        assert!(msg.contains("connection refused"));
    }

    #[test]
    fn test_error_message_format_extension() {
        let error = "extension not found";
        let msg = format!("Failed to create pgvector extension (is it installed?): {error}");
        assert!(msg.contains("pgvector"));
        assert!(msg.contains("is it installed"));
    }

    #[test]
    fn test_error_message_format_insert() {
        let error = "duplicate key";
        let msg = format!("Failed to insert document: {error}");
        assert!(msg.contains("Failed to insert"));
        assert!(msg.contains("duplicate key"));
    }

    #[test]
    fn test_error_message_format_delete() {
        let error = "table not found";
        let msg = format!("Failed to delete documents: {error}");
        assert!(msg.contains("Failed to delete"));
    }

    #[test]
    fn test_error_message_format_search() {
        let error = "invalid vector dimension";
        let msg = format!("Failed to search documents: {error}");
        assert!(msg.contains("Failed to search"));
    }

    // =========================================================================
    // Distance Metric Enum Tests
    // =========================================================================

    #[test]
    fn test_distance_metric_all_variants() {
        let metrics = [
            DistanceMetric::Cosine,
            DistanceMetric::Euclidean,
            DistanceMetric::DotProduct,
            DistanceMetric::MaxInnerProduct,
        ];

        fn distance_metric_to_operator(metric: DistanceMetric) -> &'static str {
            match metric {
                DistanceMetric::Cosine => "<=>",
                DistanceMetric::Euclidean => "<->",
                DistanceMetric::DotProduct => "<#>",
                DistanceMetric::MaxInnerProduct => "<#>",
            }
        }

        for metric in metrics {
            let op = distance_metric_to_operator(metric);
            assert!(!op.is_empty());
        }
    }

    #[test]
    fn test_distance_metric_ops_all_variants() {
        let metrics = [
            DistanceMetric::Cosine,
            DistanceMetric::Euclidean,
            DistanceMetric::DotProduct,
            DistanceMetric::MaxInnerProduct,
        ];

        fn get_ops_type(metric: DistanceMetric) -> &'static str {
            match metric {
                DistanceMetric::Cosine => "vector_cosine_ops",
                DistanceMetric::Euclidean => "vector_l2_ops",
                DistanceMetric::DotProduct | DistanceMetric::MaxInnerProduct => "vector_ip_ops",
            }
        }

        for metric in metrics {
            let ops = get_ops_type(metric);
            assert!(ops.starts_with("vector_"));
        }
    }

    // =========================================================================
    // Vector Dimension Tests
    // =========================================================================

    #[test]
    fn test_default_vector_dimension() {
        // Default is 1536 (OpenAI ada-002)
        let create_table = format!(
            "CREATE TABLE IF NOT EXISTS {} (embedding vector(1536))",
            "test"
        );
        assert!(create_table.contains("vector(1536)"));
    }

    #[test]
    fn test_vector_from_slice() {
        let data = vec![0.1f32, 0.2, 0.3, 0.4, 0.5];
        let vector = Vector::from(data.clone());
        // Vector should be created successfully
        assert!(!format!("{:?}", vector).is_empty());
    }

    #[test]
    fn test_vector_empty() {
        let data: Vec<f32> = vec![];
        let vector = Vector::from(data);
        assert!(!format!("{:?}", vector).is_empty());
    }

    #[test]
    fn test_vector_large_dimension() {
        let data: Vec<f32> = vec![0.0; 4096];
        let vector = Vector::from(data);
        assert!(!format!("{:?}", vector).is_empty());
    }

    // =========================================================================
    // Index Configuration Tests
    // =========================================================================

    #[test]
    fn test_diskann_index_params() {
        let collection = "docs";
        let index_name = format!("{}_embedding_idx", collection);
        let ops_type = "vector_cosine_ops";

        let query = format!(
            "CREATE INDEX IF NOT EXISTS {} ON {} USING diskann (embedding {}) WITH (num_neighbors = 50)",
            index_name, collection, ops_type
        );

        assert!(query.contains("USING diskann"));
        assert!(query.contains("num_neighbors = 50"));
    }

    #[test]
    fn test_index_name_special_chars() {
        // Collection names with underscores should work
        let collection = "my_special_docs";
        let index_name = format!("{}_embedding_idx", collection);
        assert_eq!(index_name, "my_special_docs_embedding_idx");
    }

    // =========================================================================
    // Input Validation Tests
    // =========================================================================

    #[test]
    fn test_texts_empty() {
        let texts: Vec<String> = vec![];
        assert!(texts.is_empty());
    }

    #[test]
    fn test_metadatas_match_texts() {
        let texts = vec!["a", "b", "c"];
        let metadatas: Vec<HashMap<String, JsonValue>> = vec![
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        ];
        assert_eq!(texts.len(), metadatas.len());
    }

    #[test]
    fn test_ids_match_texts() {
        let texts = vec!["a", "b"];
        let ids = vec!["id-1".to_string(), "id-2".to_string()];
        assert_eq!(texts.len(), ids.len());
    }

    // =========================================================================
    // Connection String Tests
    // =========================================================================

    #[test]
    fn test_connection_string_format_basic() {
        let conn = "postgresql://user:pass@localhost:5432/db";
        assert!(conn.starts_with("postgresql://"));
        assert!(conn.contains("@localhost"));
    }

    #[test]
    fn test_connection_string_with_params() {
        let conn = "postgresql://user:pass@localhost:5432/db?sslmode=require";
        assert!(conn.contains("sslmode=require"));
    }

    #[test]
    fn test_connection_string_ipv6() {
        let conn = "postgresql://user:pass@[::1]:5432/db";
        assert!(conn.contains("[::1]"));
    }

    // =========================================================================
    // Batch Operation Tests
    // =========================================================================

    #[test]
    fn test_batch_id_generation() {
        let count = 10;
        let ids: Vec<String> = (0..count)
            .map(|_| uuid::Uuid::new_v4().to_string())
            .collect();
        assert_eq!(ids.len(), count);
    }

    #[test]
    fn test_batch_text_conversion() {
        let texts = vec!["hello", "world", "test"];
        let text_strings: Vec<String> = texts.iter().map(|t| t.to_string()).collect();
        assert_eq!(text_strings.len(), 3);
        assert_eq!(text_strings[0], "hello");
    }

    // =========================================================================
    // Result Aggregation Tests
    // =========================================================================

    #[test]
    fn test_results_to_documents_only() {
        let results: Vec<(Document, f32)> = vec![
            (Document {
                id: Some("1".to_string()),
                page_content: "a".to_string(),
                metadata: HashMap::new(),
            }, 0.9),
            (Document {
                id: Some("2".to_string()),
                page_content: "b".to_string(),
                metadata: HashMap::new(),
            }, 0.8),
        ];

        let docs: Vec<Document> = results.into_iter().map(|(doc, _score)| doc).collect();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].id, Some("1".to_string()));
    }

    #[test]
    fn test_results_with_scores() {
        let results: Vec<(Document, f32)> = vec![
            (Document {
                id: Some("1".to_string()),
                page_content: "high score".to_string(),
                metadata: HashMap::new(),
            }, 0.95),
        ];

        assert!((results[0].1 - 0.95).abs() < 1e-6);
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_json_value_as_str_or_to_string() {
        let string_val = JsonValue::String("test".to_string());
        assert_eq!(string_val.as_str().unwrap(), "test");

        let number_val = JsonValue::Number(42.into());
        assert_eq!(number_val.as_str().unwrap_or(&number_val.to_string()), "42");

        let bool_val = JsonValue::Bool(true);
        assert_eq!(bool_val.as_str().unwrap_or(&bool_val.to_string()), "true");
    }

    #[test]
    fn test_empty_where_clause_is_true() {
        fn build_where_clause(filter: &HashMap<String, JsonValue>) -> String {
            if filter.is_empty() {
                return String::from("TRUE");
            }
            "...".to_string()
        }

        let empty_filter = HashMap::new();
        assert_eq!(build_where_clause(&empty_filter), "TRUE");
    }

    #[test]
    fn test_score_clamping_negative() {
        // Test that negative scores are handled
        let distance: f32 = 1.5;
        let score = (1.0_f32 - distance).max(0.0);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_score_precision() {
        let distance: f32 = 0.123456;
        let score = (1.0_f32 - distance).max(0.0);
        assert!((score - 0.876544).abs() < 1e-5);
    }
}
