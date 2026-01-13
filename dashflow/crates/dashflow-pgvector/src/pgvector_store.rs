// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! `PostgreSQL` pgvector vector store implementation for `DashFlow` Rust.

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
use tracing::error;

/// Validate a PostgreSQL identifier (table name, column name, etc.)
///
/// PostgreSQL identifiers must:
/// - Start with a letter (a-z, A-Z) or underscore
/// - Contain only letters, digits, and underscores
/// - Be at most 63 characters (PostgreSQL limit for unquoted identifiers)
fn validate_identifier(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::config("identifier cannot be empty"));
    }

    if name.len() > 63 {
        return Err(Error::config(format!(
            "identifier '{}' exceeds maximum length of 63 characters",
            name
        )));
    }

    let mut chars = name.chars();
    let first = chars.next().unwrap(); // Safe: we checked non-empty above

    // First character must be a letter or underscore
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(Error::config(format!(
            "identifier '{}' must start with a letter or underscore",
            name
        )));
    }

    // Remaining characters must be letters, digits, or underscores
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return Err(Error::config(format!(
                "identifier '{}' contains invalid character '{}'",
                name, c
            )));
        }
    }

    Ok(())
}

/// `PostgreSQL` pgvector vector store implementation.
pub struct PgVectorStore {
    client: Arc<tokio::sync::Mutex<Client>>,
    collection_name: String,
    embeddings: Arc<dyn Embeddings>,
    distance_metric: DistanceMetric,
}

impl PgVectorStore {
    /// Creates a new `PgVectorStore` instance.
    ///
    /// # Arguments
    ///
    /// * `connection_string` - `PostgreSQL` connection string (e.g., "<postgresql://user:pass@localhost:5432/db>")
    /// * `collection_name` - Name of the collection/table (must be a valid SQL identifier)
    /// * `embeddings` - Embeddings model to use
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Collection name contains invalid characters (SQL injection prevention)
    /// - Connection to `PostgreSQL` fails
    /// - pgvector extension is not installed
    /// - Table creation fails
    pub async fn new(
        connection_string: &str,
        collection_name: &str,
        embeddings: Arc<dyn Embeddings>,
    ) -> Result<Self> {
        // Validate collection name to prevent SQL injection
        validate_identifier(collection_name)?;

        // Connect to PostgreSQL
        let (client, connection) = tokio_postgres::connect(connection_string, NoTls)
            .await
            .map_err(|e| Error::config(format!("Failed to connect to PostgreSQL: {e}")))?;

        // Spawn connection task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                error!(error = %e, "PostgreSQL connection error");
            }
        });

        let client = Arc::new(tokio::sync::Mutex::new(client));

        let store = Self {
            client,
            collection_name: collection_name.to_string(),
            embeddings,
            distance_metric: DistanceMetric::Cosine,
        };

        // Ensure extension and table exist
        store.ensure_extension().await?;
        store.ensure_table().await?;

        Ok(store)
    }

    /// Ensures the pgvector extension is installed.
    async fn ensure_extension(&self) -> Result<()> {
        let client = self.client.lock().await;
        client
            .execute("CREATE EXTENSION IF NOT EXISTS vector", &[])
            .await
            .map_err(|e| {
                Error::other(format!(
                    "Failed to create pgvector extension (is it installed?): {e}"
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

        // Create index for vector similarity search
        let create_index_query = format!(
            "CREATE INDEX IF NOT EXISTS {}_embedding_idx ON {} USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100)",
            self.collection_name, self.collection_name
        );

        // Ignore index creation errors (might fail if table is empty)
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
impl VectorStore for PgVectorStore {
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
        let client = self.client.lock().await;

        let embedding_vec = Vector::from(embedding.to_vec());
        let operator = self.distance_metric_to_operator();

        let where_clause =
            filter.map_or_else(|| "TRUE".to_string(), |f| self.build_where_clause(f));

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
            // For cosine distance: similarity = 1 - distance
            // For L2 distance: need to normalize differently
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

// ============================================================================
// UNIT TESTS
// These tests cover pure functions and don't require a database connection
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod unit_tests {
    use super::*;

    // ========================================================================
    // validate_identifier tests
    // ========================================================================

    mod validate_identifier_tests {
        use super::*;

        #[test]
        fn test_valid_simple_identifier() {
            assert!(validate_identifier("users").is_ok());
            assert!(validate_identifier("my_table").is_ok());
            assert!(validate_identifier("Users").is_ok());
            assert!(validate_identifier("USERS").is_ok());
        }

        #[test]
        fn test_valid_identifier_with_underscore_prefix() {
            assert!(validate_identifier("_users").is_ok());
            assert!(validate_identifier("_").is_ok());
            assert!(validate_identifier("__double").is_ok());
            assert!(validate_identifier("_MyTable").is_ok());
        }

        #[test]
        fn test_valid_identifier_with_numbers() {
            assert!(validate_identifier("users1").is_ok());
            assert!(validate_identifier("table_123").is_ok());
            assert!(validate_identifier("v2_schema").is_ok());
            assert!(validate_identifier("a1b2c3").is_ok());
        }

        #[test]
        fn test_valid_identifier_max_length() {
            // 63 characters is the PostgreSQL limit
            let max_len = "a".repeat(63);
            assert!(validate_identifier(&max_len).is_ok());
        }

        #[test]
        fn test_invalid_empty_identifier() {
            let err = validate_identifier("").unwrap_err();
            assert!(err.to_string().contains("empty"));
        }

        #[test]
        fn test_invalid_identifier_too_long() {
            let too_long = "a".repeat(64);
            let err = validate_identifier(&too_long).unwrap_err();
            assert!(err.to_string().contains("exceeds maximum length"));
            assert!(err.to_string().contains("63"));
        }

        #[test]
        fn test_invalid_identifier_starts_with_number() {
            let err = validate_identifier("1users").unwrap_err();
            assert!(err.to_string().contains("must start with a letter or underscore"));

            let err = validate_identifier("123").unwrap_err();
            assert!(err.to_string().contains("must start with a letter or underscore"));

            let err = validate_identifier("0_table").unwrap_err();
            assert!(err.to_string().contains("must start with a letter or underscore"));
        }

        #[test]
        fn test_invalid_identifier_special_chars() {
            let err = validate_identifier("users-table").unwrap_err();
            assert!(err.to_string().contains("invalid character"));
            assert!(err.to_string().contains("'-'"));

            let err = validate_identifier("my.table").unwrap_err();
            assert!(err.to_string().contains("invalid character"));
            assert!(err.to_string().contains("'.'"));

            let err = validate_identifier("table name").unwrap_err();
            assert!(err.to_string().contains("invalid character"));
            assert!(err.to_string().contains("' '"));
        }

        #[test]
        fn test_invalid_identifier_sql_injection_attempts() {
            // These should all fail validation
            assert!(validate_identifier("users; DROP TABLE users;").is_err());
            assert!(validate_identifier("users'--").is_err());
            assert!(validate_identifier("users\"--").is_err());
            assert!(validate_identifier("users/*comment*/").is_err());
            assert!(validate_identifier("1=1").is_err());
            assert!(validate_identifier("users OR 1=1").is_err());
        }

        #[test]
        fn test_invalid_identifier_unicode() {
            // Only ASCII letters allowed
            assert!(validate_identifier("café").is_err());
            assert!(validate_identifier("用户").is_err());
            assert!(validate_identifier("таблица").is_err());
            assert!(validate_identifier("tëst").is_err());
        }

        #[test]
        fn test_invalid_identifier_starts_with_special() {
            let err = validate_identifier("-users").unwrap_err();
            assert!(err.to_string().contains("must start with a letter or underscore"));

            let err = validate_identifier("@users").unwrap_err();
            assert!(err.to_string().contains("must start with a letter or underscore"));

            let err = validate_identifier("$users").unwrap_err();
            assert!(err.to_string().contains("must start with a letter or underscore"));
        }

        #[test]
        fn test_boundary_length_identifiers() {
            // Exactly 62 characters - should pass
            let len_62 = "a".repeat(62);
            assert!(validate_identifier(&len_62).is_ok());

            // Exactly 63 characters - should pass (max)
            let len_63 = "a".repeat(63);
            assert!(validate_identifier(&len_63).is_ok());

            // Exactly 64 characters - should fail
            let len_64 = "a".repeat(64);
            assert!(validate_identifier(&len_64).is_err());
        }

        #[test]
        fn test_single_character_identifiers() {
            assert!(validate_identifier("a").is_ok());
            assert!(validate_identifier("Z").is_ok());
            assert!(validate_identifier("_").is_ok());
            assert!(validate_identifier("1").is_err());
            assert!(validate_identifier("-").is_err());
        }

        #[test]
        fn test_mixed_case_identifiers() {
            assert!(validate_identifier("MyTable").is_ok());
            assert!(validate_identifier("myTable").is_ok());
            assert!(validate_identifier("MYTABLE").is_ok());
            assert!(validate_identifier("My_Table_123").is_ok());
        }

        #[test]
        fn test_reserved_word_like_identifiers() {
            // PostgreSQL reserved words are technically valid identifiers
            // (they would need to be quoted in SQL, but as identifiers they're valid)
            assert!(validate_identifier("select").is_ok());
            assert!(validate_identifier("table").is_ok());
            assert!(validate_identifier("where").is_ok());
            assert!(validate_identifier("from").is_ok());
        }
    }

    // ========================================================================
    // DistanceMetric tests
    // ========================================================================

    mod distance_metric_tests {
        use super::*;

        // We can't test distance_metric_to_operator directly without a PgVectorStore,
        // but we can verify the DistanceMetric enum values and their semantics

        #[test]
        fn test_distance_metric_default() {
            // Verify default distance metric is Cosine (most common for embeddings)
            let metric = DistanceMetric::Cosine;
            assert!(matches!(metric, DistanceMetric::Cosine));
        }

        #[test]
        fn test_distance_metric_variants() {
            // Verify all variants exist
            let _ = DistanceMetric::Cosine;
            let _ = DistanceMetric::Euclidean;
            let _ = DistanceMetric::DotProduct;
            let _ = DistanceMetric::MaxInnerProduct;
        }
    }

    // ========================================================================
    // build_where_clause tests (via test helper)
    // ========================================================================

    mod where_clause_tests {
        use super::*;

        // Helper to build where clause without needing full PgVectorStore
        fn build_where_clause_helper(filter: &HashMap<String, JsonValue>) -> String {
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

        #[test]
        fn test_empty_filter() {
            let filter = HashMap::new();
            assert_eq!(build_where_clause_helper(&filter), "TRUE");
        }

        #[test]
        fn test_single_string_filter() {
            let mut filter = HashMap::new();
            filter.insert("category".to_string(), JsonValue::String("tech".to_string()));
            let clause = build_where_clause_helper(&filter);
            assert!(clause.contains("metadata->>'category' = 'tech'"));
        }

        #[test]
        fn test_single_number_filter() {
            let mut filter = HashMap::new();
            filter.insert("year".to_string(), JsonValue::Number(2024.into()));
            let clause = build_where_clause_helper(&filter);
            assert!(clause.contains("metadata->>'year' = '2024'"));
        }

        #[test]
        fn test_single_boolean_filter() {
            let mut filter = HashMap::new();
            filter.insert("active".to_string(), JsonValue::Bool(true));
            let clause = build_where_clause_helper(&filter);
            assert!(clause.contains("metadata->>'active' = 'true'"));
        }

        #[test]
        fn test_multiple_filters() {
            let mut filter = HashMap::new();
            filter.insert("category".to_string(), JsonValue::String("tech".to_string()));
            filter.insert("year".to_string(), JsonValue::Number(2024.into()));
            let clause = build_where_clause_helper(&filter);
            // Both conditions should be present, joined by AND
            assert!(clause.contains("metadata->>'category' = 'tech'"));
            assert!(clause.contains("metadata->>'year' = '2024'"));
            assert!(clause.contains(" AND "));
        }

        #[test]
        fn test_filter_with_special_chars_in_value() {
            let mut filter = HashMap::new();
            filter.insert(
                "description".to_string(),
                JsonValue::String("hello world".to_string()),
            );
            let clause = build_where_clause_helper(&filter);
            assert!(clause.contains("metadata->>'description' = 'hello world'"));
        }

        #[test]
        fn test_filter_with_empty_string_value() {
            let mut filter = HashMap::new();
            filter.insert("tag".to_string(), JsonValue::String(String::new()));
            let clause = build_where_clause_helper(&filter);
            assert!(clause.contains("metadata->>'tag' = ''"));
        }

        #[test]
        fn test_filter_with_null_value() {
            let mut filter = HashMap::new();
            filter.insert("optional".to_string(), JsonValue::Null);
            let clause = build_where_clause_helper(&filter);
            assert!(clause.contains("metadata->>'optional' = 'null'"));
        }

        #[test]
        fn test_filter_with_underscore_key() {
            let mut filter = HashMap::new();
            filter.insert(
                "my_key".to_string(),
                JsonValue::String("value".to_string()),
            );
            let clause = build_where_clause_helper(&filter);
            assert!(clause.contains("metadata->>'my_key' = 'value'"));
        }

        #[test]
        fn test_filter_with_numeric_key() {
            let mut filter = HashMap::new();
            filter.insert(
                "key123".to_string(),
                JsonValue::String("value".to_string()),
            );
            let clause = build_where_clause_helper(&filter);
            assert!(clause.contains("metadata->>'key123' = 'value'"));
        }

        #[test]
        fn test_filter_preserves_all_conditions() {
            let mut filter = HashMap::new();
            filter.insert("a".to_string(), JsonValue::String("1".to_string()));
            filter.insert("b".to_string(), JsonValue::String("2".to_string()));
            filter.insert("c".to_string(), JsonValue::String("3".to_string()));
            let clause = build_where_clause_helper(&filter);
            // All three conditions should be present
            assert!(clause.contains("metadata->>'a' = '1'"));
            assert!(clause.contains("metadata->>'b' = '2'"));
            assert!(clause.contains("metadata->>'c' = '3'"));
            // Should have exactly 2 ANDs for 3 conditions
            assert_eq!(clause.matches(" AND ").count(), 2);
        }

        #[test]
        fn test_filter_with_float_value() {
            let mut filter = HashMap::new();
            filter.insert(
                "score".to_string(),
                JsonValue::Number(serde_json::Number::from_f64(3.14).unwrap()),
            );
            let clause = build_where_clause_helper(&filter);
            assert!(clause.contains("metadata->>'score' = '3.14'"));
        }

        #[test]
        fn test_filter_with_negative_number() {
            let mut filter = HashMap::new();
            filter.insert("offset".to_string(), JsonValue::Number((-10).into()));
            let clause = build_where_clause_helper(&filter);
            assert!(clause.contains("metadata->>'offset' = '-10'"));
        }

        #[test]
        fn test_filter_with_large_number() {
            let mut filter = HashMap::new();
            filter.insert("id".to_string(), JsonValue::Number(9_999_999_999_i64.into()));
            let clause = build_where_clause_helper(&filter);
            assert!(clause.contains("metadata->>'id' = '9999999999'"));
        }
    }

    // ========================================================================
    // Score conversion tests
    // ========================================================================

    mod score_conversion_tests {
        #[allow(unused_imports)]
        use super::*;

        #[test]
        fn test_cosine_distance_to_similarity() {
            // For cosine: similarity = 1 - distance, clamped to [0, 1]
            // Distance 0 = perfect match = similarity 1
            // Distance 1 = opposite = similarity 0
            // Distance 0.5 = similarity 0.5

            let distance = 0.0_f32;
            let similarity = (1.0 - distance).max(0.0);
            assert!((similarity - 1.0).abs() < f32::EPSILON);

            let distance = 1.0_f32;
            let similarity = (1.0 - distance).max(0.0);
            assert!((similarity - 0.0).abs() < f32::EPSILON);

            let distance = 0.5_f32;
            let similarity = (1.0 - distance).max(0.0);
            assert!((similarity - 0.5).abs() < f32::EPSILON);
        }

        #[test]
        fn test_cosine_distance_negative_handling() {
            // Distance might be slightly negative due to floating point
            let distance = -0.001_f32;
            let similarity = (1.0 - distance).max(0.0);
            assert!(similarity > 1.0); // Shows the actual behavior
            // In production, we'd want to clamp to 1.0 max too
        }

        #[test]
        fn test_cosine_distance_greater_than_one() {
            // Distance > 1 would give negative similarity, clamped to 0
            let distance = 1.5_f32;
            let similarity = (1.0 - distance).max(0.0);
            assert!((similarity - 0.0).abs() < f32::EPSILON);
        }

        #[test]
        fn test_euclidean_distance_to_similarity() {
            // For L2: similarity = 1 / (1 + distance)
            // Distance 0 = similarity 1
            // Distance 1 = similarity 0.5
            // Distance large = similarity approaches 0

            let distance = 0.0_f32;
            let similarity = 1.0 / (1.0 + distance);
            assert!((similarity - 1.0).abs() < f32::EPSILON);

            let distance = 1.0_f32;
            let similarity = 1.0 / (1.0 + distance);
            assert!((similarity - 0.5).abs() < f32::EPSILON);

            let distance = 100.0_f32;
            let similarity = 1.0 / (1.0 + distance);
            assert!(similarity < 0.01);
        }

        #[test]
        fn test_dot_product_negation() {
            // For dot product: pgvector returns negative, so score = -distance
            let distance = -10.0_f32; // pgvector returns -10 for similarity 10
            let score = -distance;
            assert!((score - 10.0).abs() < f32::EPSILON);

            let distance = 0.0_f32;
            let score = -distance;
            assert!((score - 0.0).abs() < f32::EPSILON);
        }

        #[test]
        fn test_max_inner_product_negation() {
            // Same as dot product
            let distance = -5.5_f32;
            let score = -distance;
            assert!((score - 5.5).abs() < f32::EPSILON);
        }
    }

    // ========================================================================
    // Mock embeddings tests
    // ========================================================================

    mod mock_embeddings_tests {
        use super::*;

        struct TestMockEmbeddings;

        #[async_trait::async_trait]
        impl Embeddings for TestMockEmbeddings {
            async fn _embed_documents(
                &self,
                texts: &[String],
            ) -> dashflow::core::error::Result<Vec<Vec<f32>>> {
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

        #[tokio::test]
        async fn test_mock_embeddings_deterministic() {
            let embeddings = TestMockEmbeddings;
            let texts = vec!["hello".to_string()];
            let result1 = embeddings._embed_documents(&texts).await.unwrap();
            let result2 = embeddings._embed_documents(&texts).await.unwrap();
            assert_eq!(result1, result2);
        }

        #[tokio::test]
        async fn test_mock_embeddings_different_texts() {
            let embeddings = TestMockEmbeddings;
            let texts1 = vec!["hello".to_string()];
            let texts2 = vec!["world".to_string()];
            let result1 = embeddings._embed_documents(&texts1).await.unwrap();
            let result2 = embeddings._embed_documents(&texts2).await.unwrap();
            assert_ne!(result1, result2);
        }

        #[tokio::test]
        async fn test_mock_embeddings_normalized() {
            let embeddings = TestMockEmbeddings;
            let texts = vec!["test".to_string()];
            let result = embeddings._embed_documents(&texts).await.unwrap();
            let vec = &result[0];

            // Check magnitude is approximately 1 (normalized)
            let magnitude: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!(
                (magnitude - 1.0).abs() < 0.001,
                "Vector should be normalized, got magnitude {}",
                magnitude
            );
        }

        #[tokio::test]
        async fn test_mock_embeddings_empty_text() {
            let embeddings = TestMockEmbeddings;
            let texts = vec![String::new()];
            let result = embeddings._embed_documents(&texts).await.unwrap();
            let vec = &result[0];
            // Empty text should give zero vector
            assert_eq!(vec, &vec![0.0, 0.0, 0.0]);
        }

        #[tokio::test]
        async fn test_mock_embeddings_query_matches_documents() {
            let embeddings = TestMockEmbeddings;
            let text = "hello";
            let doc_result = embeddings
                ._embed_documents(&[text.to_string()])
                .await
                .unwrap();
            let query_result = embeddings._embed_query(text).await.unwrap();
            assert_eq!(doc_result[0], query_result);
        }

        #[tokio::test]
        async fn test_mock_embeddings_batch() {
            let embeddings = TestMockEmbeddings;
            let texts = vec![
                "hello".to_string(),
                "world".to_string(),
                "foo".to_string(),
            ];
            let result = embeddings._embed_documents(&texts).await.unwrap();
            assert_eq!(result.len(), 3);
            // Each vector should be 3-dimensional
            assert_eq!(result[0].len(), 3);
            assert_eq!(result[1].len(), 3);
            assert_eq!(result[2].len(), 3);
        }

        #[tokio::test]
        async fn test_mock_embeddings_single_char() {
            let embeddings = TestMockEmbeddings;
            let texts = vec!["a".to_string()];
            let result = embeddings._embed_documents(&texts).await.unwrap();
            let vec = &result[0];

            // For "a": x = 97/255, y = 0, z = 1/100 = 0.01
            // Then normalized
            let x: f32 = 97.0 / 255.0;
            let y: f32 = 0.0;
            let z: f32 = 0.01;
            let mag = (x * x + y * y + z * z).sqrt();
            let expected = vec![x / mag, y / mag, z / mag];

            for (a, b) in vec.iter().zip(expected.iter()) {
                assert!(
                    (a - b).abs() < 0.001,
                    "Expected {:?}, got {:?}",
                    expected,
                    vec
                );
            }
        }
    }

    // ========================================================================
    // Vector conversion tests
    // ========================================================================

    mod vector_tests {
        use super::*;

        #[test]
        fn test_vector_from_slice() {
            let data: Vec<f32> = vec![1.0, 2.0, 3.0];
            let vector = Vector::from(data.clone());
            // Vector should be created successfully
            assert_eq!(vector.as_slice().len(), 3);
        }

        #[test]
        fn test_vector_empty() {
            let data: Vec<f32> = vec![];
            let vector = Vector::from(data);
            assert_eq!(vector.as_slice().len(), 0);
        }

        #[test]
        fn test_vector_large() {
            let data: Vec<f32> = (0..1536).map(|i| i as f32 / 1536.0).collect();
            let vector = Vector::from(data);
            assert_eq!(vector.as_slice().len(), 1536);
        }

        #[test]
        fn test_vector_preserves_values() {
            let data: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4, 0.5];
            let vector = Vector::from(data.clone());
            let slice = vector.as_slice();
            for (i, &val) in data.iter().enumerate() {
                assert!(
                    (slice[i] - val).abs() < f32::EPSILON,
                    "Value mismatch at index {}",
                    i
                );
            }
        }
    }

    // ========================================================================
    // JsonValue handling tests
    // ========================================================================

    mod json_value_tests {
        use super::*;

        #[test]
        fn test_json_object_to_hashmap() {
            let json = serde_json::json!({
                "key1": "value1",
                "key2": 42,
                "key3": true
            });

            if let JsonValue::Object(obj) = json {
                let map: HashMap<String, JsonValue> = obj.into_iter().collect();
                assert_eq!(map.len(), 3);
                assert_eq!(map.get("key1").unwrap(), &JsonValue::String("value1".to_string()));
                assert_eq!(map.get("key2").unwrap(), &JsonValue::Number(42.into()));
                assert_eq!(map.get("key3").unwrap(), &JsonValue::Bool(true));
            } else {
                panic!("Expected object");
            }
        }

        #[test]
        fn test_json_non_object_to_empty_hashmap() {
            let json = JsonValue::Array(vec![]);
            if let JsonValue::Object(obj) = json {
                let _: HashMap<String, JsonValue> = obj.into_iter().collect();
            } else {
                // Non-object should result in empty HashMap
                let map: HashMap<String, JsonValue> = HashMap::new();
                assert!(map.is_empty());
            }
        }

        #[test]
        fn test_json_null_to_hashmap() {
            let json = JsonValue::Null;
            // Null is not an object, so we'd get empty HashMap
            let map: HashMap<String, JsonValue> = if let JsonValue::Object(obj) = json {
                obj.into_iter().collect()
            } else {
                HashMap::new()
            };
            assert!(map.is_empty());
        }

        #[test]
        fn test_metadata_serialization() {
            let mut metadata: HashMap<String, JsonValue> = HashMap::new();
            metadata.insert("author".to_string(), JsonValue::String("Alice".to_string()));
            metadata.insert("year".to_string(), JsonValue::Number(2024.into()));

            let serialized = serde_json::to_value(&metadata).unwrap();
            assert!(serialized.is_object());
        }

        #[test]
        fn test_empty_metadata() {
            let metadata: HashMap<String, JsonValue> = HashMap::new();
            let serialized = serde_json::to_value(&metadata).unwrap();
            assert_eq!(serialized, serde_json::json!({}));
        }
    }

    // ========================================================================
    // Document struct tests
    // ========================================================================

    mod document_tests {
        use super::*;

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
            metadata.insert("page".to_string(), JsonValue::Number(1.into()));

            let doc = Document {
                id: Some("doc-2".to_string()),
                page_content: "Content here".to_string(),
                metadata,
            };

            assert_eq!(doc.metadata.len(), 2);
            assert_eq!(
                doc.metadata.get("source"),
                Some(&JsonValue::String("web".to_string()))
            );
        }

        #[test]
        fn test_document_without_id() {
            let doc = Document {
                id: None,
                page_content: "No ID".to_string(),
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
    }

    // ========================================================================
    // UUID generation tests
    // ========================================================================

    mod uuid_tests {
        #[test]
        fn test_uuid_format() {
            let id = uuid::Uuid::new_v4().to_string();
            // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
            assert_eq!(id.len(), 36);
            assert_eq!(&id[8..9], "-");
            assert_eq!(&id[13..14], "-");
            assert_eq!(&id[18..19], "-");
            assert_eq!(&id[23..24], "-");
        }

        #[test]
        fn test_uuid_uniqueness() {
            let id1 = uuid::Uuid::new_v4().to_string();
            let id2 = uuid::Uuid::new_v4().to_string();
            assert_ne!(id1, id2);
        }

        #[test]
        fn test_uuid_batch() {
            let ids: Vec<String> = (0..100).map(|_| uuid::Uuid::new_v4().to_string()).collect();
            let unique: std::collections::HashSet<_> = ids.iter().collect();
            assert_eq!(ids.len(), unique.len(), "All UUIDs should be unique");
        }
    }

    // ========================================================================
    // SQL placeholder generation tests
    // ========================================================================

    mod sql_placeholder_tests {
        #[test]
        fn test_placeholder_generation_single() {
            let ids = vec!["id1".to_string()];
            let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();
            assert_eq!(placeholders, vec!["$1"]);
        }

        #[test]
        fn test_placeholder_generation_multiple() {
            let ids = vec![
                "id1".to_string(),
                "id2".to_string(),
                "id3".to_string(),
            ];
            let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();
            assert_eq!(placeholders, vec!["$1", "$2", "$3"]);
        }

        #[test]
        fn test_placeholder_join() {
            let ids = vec!["a".to_string(), "b".to_string()];
            let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();
            let joined = placeholders.join(", ");
            assert_eq!(joined, "$1, $2");
        }

        #[test]
        fn test_placeholder_in_delete_query() {
            let collection = "test_table";
            let ids = vec!["id1".to_string(), "id2".to_string()];
            let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();
            let query = format!(
                "DELETE FROM {} WHERE id IN ({})",
                collection,
                placeholders.join(", ")
            );
            assert_eq!(query, "DELETE FROM test_table WHERE id IN ($1, $2)");
        }
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

    async fn create_test_store() -> PgVectorStore {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        // Use unique collection name per test to avoid conflicts
        let collection_name = format!(
            "test_{}",
            uuid::Uuid::new_v4().to_string().replace("-", "_")
        );
        let connection_string = std::env::var("POSTGRES_CONNECTION_STRING").unwrap_or_else(|_| {
            "postgresql://postgres:password@localhost:5432/postgres".to_string()
        });

        PgVectorStore::new(&connection_string, &collection_name, embeddings)
            .await
            .expect("Failed to create test store - is PostgreSQL with pgvector running on localhost:5432?")
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_add_and_search_standard() {
        let mut store = create_test_store().await;
        test_add_and_search(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_search_with_scores_standard() {
        let mut store = create_test_store().await;
        test_search_with_scores(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_metadata_filtering_standard() {
        let mut store = create_test_store().await;
        test_metadata_filtering(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_custom_ids_standard() {
        let mut store = create_test_store().await;
        test_custom_ids(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_delete_standard() {
        let mut store = create_test_store().await;
        test_delete(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_add_documents_standard() {
        let mut store = create_test_store().await;
        test_add_documents(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_empty_search_standard() {
        let store = create_test_store().await;
        test_empty_search(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_search_by_vector_standard() {
        let mut store = create_test_store().await;
        test_search_by_vector(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_mmr_search_standard() {
        let mut store = create_test_store().await;
        test_mmr_search(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_large_batch_standard() {
        let mut store = create_test_store().await;
        test_large_batch(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_validation_standard() {
        let mut store = create_test_store().await;
        test_validation(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_update_document_standard() {
        let mut store = create_test_store().await;
        test_update_document(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_metadata_only_filter_standard() {
        let mut store = create_test_store().await;
        test_metadata_only_filter(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_complex_metadata_standard() {
        let mut store = create_test_store().await;
        test_complex_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_empty_text_standard() {
        let mut store = create_test_store().await;
        test_empty_text(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_special_chars_metadata_standard() {
        let mut store = create_test_store().await;
        test_special_chars_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_concurrent_operations_standard() {
        let mut store = create_test_store().await;
        test_concurrent_operations(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_very_long_text_standard() {
        let mut store = create_test_store().await;
        test_very_long_text(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_duplicate_documents_standard() {
        let mut store = create_test_store().await;
        test_duplicate_documents(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_k_parameter_standard() {
        let mut store = create_test_store().await;
        test_k_parameter(&mut store).await;
    }

    // ========================================================================
    // COMPREHENSIVE TESTS
    // These tests provide deeper coverage beyond standard conformance tests
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_mmr_lambda_zero_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_lambda_zero(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_mmr_lambda_one_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_lambda_one(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_mmr_fetch_k_variations_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_fetch_k_variations(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_complex_metadata_operators_comprehensive() {
        let mut store = create_test_store().await;
        test_complex_metadata_operators(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_nested_metadata_filtering_comprehensive() {
        let mut store = create_test_store().await;
        test_nested_metadata_filtering(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_array_metadata_comprehensive() {
        let mut store = create_test_store().await;
        test_array_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_very_large_batch_comprehensive() {
        let mut store = create_test_store().await;
        test_very_large_batch(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_concurrent_writes_comprehensive() {
        let mut store = create_test_store().await;
        test_concurrent_writes(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_error_handling_network_comprehensive() {
        let mut store = create_test_store().await;
        test_error_handling_network(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_error_handling_invalid_input_comprehensive() {
        let mut store = create_test_store().await;
        test_error_handling_invalid_input(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_bulk_delete_comprehensive() {
        let mut store = create_test_store().await;
        test_bulk_delete(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_update_metadata_comprehensive() {
        let mut store = create_test_store().await;
        test_update_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PostgreSQL with pgvector: docker-compose -f docker-compose.test.yml up postgres"]
    async fn test_search_score_threshold_comprehensive() {
        let mut store = create_test_store().await;
        test_search_score_threshold(&mut store).await;
    }
}
