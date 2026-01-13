//! `ClickHouse` vector store implementation for `DashFlow` Rust.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use clickhouse::{Client, Row};
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::{Error, Result};
use dashflow::{embed, embed_query};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Validates that a SQL identifier (database name, table name) contains only safe characters.
/// Returns an error if the identifier contains potentially dangerous characters.
///
/// Safe characters: alphanumeric (a-z, A-Z, 0-9) and underscore (_)
fn validate_sql_identifier(name: &str, identifier_type: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::config(format!("{identifier_type} cannot be empty")));
    }

    // Must start with letter or underscore, not a digit
    // SAFETY: is_empty() check above guarantees .next() returns Some
    #[allow(clippy::unwrap_used)]
    let first_char = name.chars().next().unwrap();
    if !first_char.is_ascii_alphabetic() && first_char != '_' {
        return Err(Error::config(format!(
            "{identifier_type} must start with a letter or underscore, got: '{}'",
            first_char
        )));
    }

    // All characters must be alphanumeric or underscore
    for ch in name.chars() {
        if !ch.is_ascii_alphanumeric() && ch != '_' {
            return Err(Error::config(format!(
                "{identifier_type} contains invalid character: '{}'. Only alphanumeric and underscore allowed.",
                ch
            )));
        }
    }

    // Reasonable length limit to prevent abuse
    if name.len() > 128 {
        return Err(Error::config(format!(
            "{identifier_type} too long: {} chars (max 128)",
            name.len()
        )));
    }

    Ok(())
}

/// Escapes a string value for use in ClickHouse SQL.
/// Escapes single quotes and backslashes.
fn escape_sql_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

/// `ClickHouse` vector store implementation.
pub struct ClickHouseVectorStore {
    client: Client,
    database: String,
    table_name: String,
    embeddings: Arc<dyn Embeddings>,
    distance_metric: DistanceMetric,
    embedding_dimension: usize,
}

/// Internal row structure for `ClickHouse` table operations.
#[derive(Debug, Clone, Row, Serialize, Deserialize)]
struct VectorRow {
    id: String,
    text: String,
    embedding: Vec<f32>,
    metadata: String, // JSON string
}

/// Row structure for fetching documents without embeddings.
#[derive(Debug, Clone, Row, Deserialize)]
struct DocumentRow {
    id: String,
    text: String,
    metadata: String,
}

/// Row structure for fetching documents with embeddings.
#[derive(Debug, Clone, Row, Deserialize)]
struct DocumentWithEmbeddingRow {
    id: String,
    text: String,
    embedding: Vec<f32>,
    metadata: String,
}

/// Row structure for similarity search results.
#[derive(Debug, Clone, Row, Deserialize)]
struct SimilaritySearchRow {
    id: String,
    text: String,
    metadata: String,
    distance: f32,
}

impl ClickHouseVectorStore {
    /// Creates a new `ClickHouseVectorStore` instance.
    ///
    /// # Arguments
    ///
    /// * `url` - `ClickHouse` HTTP endpoint (e.g., "<http://localhost:8123>")
    /// * `database` - Database name (e.g., "default"). Must contain only alphanumeric characters and underscores.
    /// * `table_name` - Name of the table for storing vectors. Must contain only alphanumeric characters and underscores.
    /// * `embeddings` - Embeddings model to use
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Database or table name contains invalid characters (SQL injection prevention)
    /// - Connection to `ClickHouse` fails
    /// - Database or table creation fails
    pub async fn new(
        url: &str,
        database: &str,
        table_name: &str,
        embeddings: Arc<dyn Embeddings>,
    ) -> Result<Self> {
        // Validate identifiers to prevent SQL injection
        validate_sql_identifier(database, "Database name")?;
        validate_sql_identifier(table_name, "Table name")?;

        let client = Client::default().with_url(url).with_database(database);

        let store = Self {
            client,
            database: database.to_string(),
            table_name: table_name.to_string(),
            embeddings,
            distance_metric: DistanceMetric::Cosine,
            embedding_dimension: 1536, // Default, will be updated on first insert
        };

        // Ensure database and table exist
        store.ensure_database().await?;
        store.ensure_table().await?;

        Ok(store)
    }

    /// Sets the embedding dimension explicitly.
    #[must_use]
    pub fn with_embedding_dimension(mut self, dimension: usize) -> Self {
        self.embedding_dimension = dimension;
        self
    }

    /// Sets the distance metric for similarity search.
    #[must_use]
    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Ensures the database exists.
    async fn ensure_database(&self) -> Result<()> {
        let query = format!("CREATE DATABASE IF NOT EXISTS {}", self.database);
        self.client
            .query(&query)
            .execute()
            .await
            .map_err(|e| Error::other(format!("Failed to create database: {e}")))?;
        Ok(())
    }

    /// Ensures the table exists with proper schema and vector index.
    async fn ensure_table(&self) -> Result<()> {
        // Create table with vector column and HNSW index
        let distance_func = match self.distance_metric {
            DistanceMetric::Cosine => "cosineDistance",
            DistanceMetric::Euclidean
            | DistanceMetric::DotProduct
            | DistanceMetric::MaxInnerProduct => "L2Distance",
        };

        let create_table_query = format!(
            "CREATE TABLE IF NOT EXISTS {}.{} (
                id String,
                text String,
                embedding Array(Float32),
                metadata String,
                INDEX embedding_idx embedding TYPE vector_similarity('hnsw', '{}') GRANULARITY 2
            ) ENGINE = MergeTree()
            ORDER BY id",
            self.database, self.table_name, distance_func
        );

        self.client
            .query(&create_table_query)
            .execute()
            .await
            .map_err(|e| Error::other(format!("Failed to create table: {e}")))?;

        Ok(())
    }

    /// Gets the distance function name for `ClickHouse` SQL.
    fn distance_function(&self) -> &'static str {
        match self.distance_metric {
            DistanceMetric::Cosine => "cosineDistance",
            DistanceMetric::Euclidean => "L2Distance",
            DistanceMetric::DotProduct => "dotProduct",
            DistanceMetric::MaxInnerProduct => "dotProduct", // Use negative for max
        }
    }

    /// Builds a WHERE clause for metadata filtering.
    ///
    /// Both keys and values are properly escaped to prevent SQL injection.
    fn build_where_clause(&self, filter: &HashMap<String, JsonValue>) -> String {
        if filter.is_empty() {
            return String::from("1=1");
        }

        let conditions: Vec<String> = filter
            .iter()
            .map(|(k, v)| {
                // Escape the key to prevent SQL injection via metadata field names
                let escaped_key = escape_sql_string(k);
                let value_str = match v {
                    JsonValue::String(s) => format!("'{}'", escape_sql_string(s)),
                    JsonValue::Number(n) => n.to_string(),
                    JsonValue::Bool(b) => b.to_string(),
                    _ => format!("'{}'", escape_sql_string(&v.to_string())),
                };
                format!("JSONExtractString(metadata, '{escaped_key}') = {value_str}")
            })
            .collect();

        conditions.join(" AND ")
    }
}

#[async_trait]
impl VectorStore for ClickHouseVectorStore {
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

        // Update embedding dimension from first embedding
        if !embeddings_vec.is_empty() {
            self.embedding_dimension = embeddings_vec[0].len();
        }

        // Generate IDs if not provided
        let doc_ids: Vec<String> = if let Some(ids) = ids {
            ids.to_vec()
        } else {
            (0..text_count)
                .map(|_| uuid::Uuid::new_v4().to_string())
                .collect()
        };

        // Prepare rows for insertion
        let mut insert = self
            .client
            .insert::<VectorRow>(&format!("{}.{}", self.database, self.table_name))
            .await
            .map_err(|e| Error::other(format!("Failed to prepare insert: {e}")))?;

        for (i, text) in text_strings.iter().enumerate() {
            let metadata_json = if let Some(metadatas) = metadatas {
                serde_json::to_string(&metadatas[i])
                    .map_err(|e| Error::other(format!("Failed to serialize metadata: {e}")))?
            } else {
                "{}".to_string()
            };

            let row = VectorRow {
                id: doc_ids[i].clone(),
                text: text.clone(),
                embedding: embeddings_vec[i].clone(),
                metadata: metadata_json,
            };

            insert
                .write(&row)
                .await
                .map_err(|e| Error::other(format!("Failed to write row: {e}")))?;
        }

        insert
            .end()
            .await
            .map_err(|e| Error::other(format!("Failed to complete insert: {e}")))?;

        Ok(doc_ids)
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        if let Some(ids) = ids {
            if ids.is_empty() {
                return Ok(true);
            }

            // Build IN clause for delete (properly escaped to prevent SQL injection)
            let id_list: Vec<String> = ids
                .iter()
                .map(|id| format!("'{}'", escape_sql_string(id)))
                .collect();
            let delete_query = format!(
                "ALTER TABLE {}.{} DELETE WHERE id IN ({})",
                self.database,
                self.table_name,
                id_list.join(", ")
            );

            self.client
                .query(&delete_query)
                .execute()
                .await
                .map_err(|e| Error::other(format!("Failed to delete documents: {e}")))?;
        } else {
            // Delete all documents
            let truncate_query = format!("TRUNCATE TABLE {}.{}", self.database, self.table_name);
            self.client
                .query(&truncate_query)
                .execute()
                .await
                .map_err(|e| Error::other(format!("Failed to truncate table: {e}")))?;
        }

        Ok(true)
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        // Properly escape IDs to prevent SQL injection
        let id_list: Vec<String> = ids
            .iter()
            .map(|id| format!("'{}'", escape_sql_string(id)))
            .collect();
        let select_query = format!(
            "SELECT id, text, metadata FROM {}.{} WHERE id IN ({})",
            self.database,
            self.table_name,
            id_list.join(", ")
        );

        let mut cursor = self
            .client
            .query(&select_query)
            .fetch::<DocumentRow>()
            .map_err(|e| Error::other(format!("Failed to fetch documents: {e}")))?;

        let mut documents = Vec::new();
        while let Some(row) = cursor
            .next()
            .await
            .map_err(|e| Error::other(format!("Failed to read row: {e}")))?
        {
            let metadata: HashMap<String, JsonValue> =
                if row.metadata.is_empty() || row.metadata == "{}" {
                    HashMap::new()
                } else {
                    serde_json::from_str(&row.metadata)
                        .map_err(|e| Error::other(format!("Failed to deserialize metadata: {e}")))?
                };

            documents.push(Document {
                id: Some(row.id),
                page_content: row.text,
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
        let where_clause = if let Some(filter) = filter {
            self.build_where_clause(filter)
        } else {
            String::from("1=1")
        };

        // Format embedding as ClickHouse array literal
        let embedding_str = format!(
            "[{}]",
            embedding
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );

        let distance_func = self.distance_function();

        // Query with vector similarity
        let query = format!(
            "SELECT id, text, metadata, {}(embedding, {}) AS distance
             FROM {}.{}
             WHERE {}
             ORDER BY distance ASC
             LIMIT {}",
            distance_func, embedding_str, self.database, self.table_name, where_clause, k
        );

        let mut cursor = self
            .client
            .query(&query)
            .fetch::<SimilaritySearchRow>()
            .map_err(|e| Error::other(format!("Failed to execute similarity search: {e}")))?;

        let mut results = Vec::new();
        while let Some(row) = cursor
            .next()
            .await
            .map_err(|e| Error::other(format!("Failed to read search result: {e}")))?
        {
            let metadata: HashMap<String, JsonValue> =
                if row.metadata.is_empty() || row.metadata == "{}" {
                    HashMap::new()
                } else {
                    serde_json::from_str(&row.metadata)
                        .map_err(|e| Error::other(format!("Failed to deserialize metadata: {e}")))?
                };

            let doc = Document {
                id: Some(row.id),
                page_content: row.text,
                metadata,
            };

            // For cosine distance, smaller is more similar (0 = identical)
            // For L2 distance, smaller is more similar (0 = identical)
            // Return distance as-is since lower = more similar
            results.push((doc, row.distance));
        }

        Ok(results)
    }

    async fn max_marginal_relevance_search(
        &self,
        query: &str,
        k: usize,
        fetch_k: usize,
        lambda_mult: f32,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        // Generate query embedding using graph API
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query).await?;

        // Fetch more candidates than needed
        let candidates = self
            .similarity_search_by_vector_with_score(&query_embedding, fetch_k, filter)
            .await?;

        if candidates.is_empty() {
            return Ok(vec![]);
        }

        // Extract embeddings for MMR calculation
        // We need to re-fetch embeddings since similarity_search doesn't return them
        let candidate_ids: Vec<String> = candidates
            .iter()
            .filter_map(|(doc, _)| doc.id.clone())
            .collect();

        if candidate_ids.is_empty() {
            return Ok(vec![]);
        }

        // Fetch full rows including embeddings (properly escaped to prevent SQL injection)
        let id_list: Vec<String> = candidate_ids
            .iter()
            .map(|id| format!("'{}'", escape_sql_string(id)))
            .collect();
        let query = format!(
            "SELECT id, text, embedding, metadata FROM {}.{} WHERE id IN ({})",
            self.database,
            self.table_name,
            id_list.join(", ")
        );

        let mut cursor = self
            .client
            .query(&query)
            .fetch::<DocumentWithEmbeddingRow>()
            .map_err(|e| Error::other(format!("Failed to fetch candidate embeddings: {e}")))?;

        let mut candidate_data = Vec::new();
        while let Some(row) = cursor
            .next()
            .await
            .map_err(|e| Error::other(format!("Failed to read candidate row: {e}")))?
        {
            let metadata: HashMap<String, JsonValue> =
                if row.metadata.is_empty() || row.metadata == "{}" {
                    HashMap::new()
                } else {
                    serde_json::from_str(&row.metadata)
                        .map_err(|e| Error::other(format!("Failed to deserialize metadata: {e}")))?
                };

            candidate_data.push((
                Document {
                    id: Some(row.id),
                    page_content: row.text,
                    metadata,
                },
                row.embedding,
            ));
        }

        // Perform MMR selection
        let selected = self.mmr_selection(&query_embedding, candidate_data, k, lambda_mult)?;

        Ok(selected)
    }
}

impl ClickHouseVectorStore {
    /// Maximum Marginal Relevance selection algorithm.
    fn mmr_selection(
        &self,
        query_embedding: &[f32],
        candidates: Vec<(Document, Vec<f32>)>,
        k: usize,
        lambda_mult: f32,
    ) -> Result<Vec<Document>> {
        if candidates.is_empty() {
            return Ok(vec![]);
        }

        let mut selected: Vec<(Document, Vec<f32>)> = Vec::new();
        let mut remaining = candidates;

        for _ in 0..k.min(remaining.len()) {
            let mut best_score = f32::NEG_INFINITY;
            let mut best_idx = 0;

            for (i, (_doc, embedding)) in remaining.iter().enumerate() {
                // Relevance to query
                let relevance = self.cosine_similarity(query_embedding, embedding);

                // Max similarity to already selected documents
                let max_similarity = if selected.is_empty() {
                    0.0
                } else {
                    selected
                        .iter()
                        .map(|(_, selected_emb)| self.cosine_similarity(embedding, selected_emb))
                        .fold(f32::NEG_INFINITY, f32::max)
                };

                // MMR score: balance relevance and diversity
                let mmr_score = lambda_mult * relevance - (1.0 - lambda_mult) * max_similarity;

                if mmr_score > best_score {
                    best_score = mmr_score;
                    best_idx = i;
                }
            }

            // Move best candidate to selected
            let best = remaining.remove(best_idx);
            selected.push(best);
        }

        Ok(selected.into_iter().map(|(doc, _)| doc).collect())
    }

    /// Calculates cosine similarity between two vectors.
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot_product / (norm_a * norm_b)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ==================== SQL Identifier Validation Tests ====================

    #[test]
    fn test_validate_sql_identifier_valid() {
        // Valid identifiers should pass
        assert!(validate_sql_identifier("valid_name", "Test").is_ok());
        assert!(validate_sql_identifier("_underscore_start", "Test").is_ok());
        assert!(validate_sql_identifier("name123", "Test").is_ok());
        assert!(validate_sql_identifier("Name_With_Mixed_Case", "Test").is_ok());
        assert!(validate_sql_identifier("a", "Test").is_ok());
        assert!(validate_sql_identifier("default", "Test").is_ok());
    }

    #[test]
    fn test_validate_sql_identifier_empty() {
        let result = validate_sql_identifier("", "Database name");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_validate_sql_identifier_starts_with_digit() {
        let result = validate_sql_identifier("1table", "Table name");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must start with a letter or underscore"));
    }

    #[test]
    fn test_validate_sql_identifier_sql_injection_attempts() {
        // SQL injection via semicolon
        let result = validate_sql_identifier("table; DROP TABLE users;--", "Table name");
        assert!(result.is_err());

        // SQL injection via quotes
        let result = validate_sql_identifier("table'--", "Table name");
        assert!(result.is_err());

        // SQL injection via spaces
        let result = validate_sql_identifier("table name", "Table name");
        assert!(result.is_err());

        // SQL injection via backticks
        let result = validate_sql_identifier("table`injection", "Table name");
        assert!(result.is_err());

        // SQL injection via parentheses
        let result = validate_sql_identifier("table()", "Table name");
        assert!(result.is_err());

        // SQL injection via double dash comment
        let result = validate_sql_identifier("table--comment", "Table name");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_sql_identifier_too_long() {
        let long_name = "a".repeat(129);
        let result = validate_sql_identifier(&long_name, "Table name");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too long"));
    }

    #[test]
    fn test_validate_sql_identifier_max_valid_length() {
        // Exactly 128 chars should be valid
        let max_name = "a".repeat(128);
        assert!(validate_sql_identifier(&max_name, "Table name").is_ok());
    }

    #[test]
    fn test_validate_sql_identifier_special_characters() {
        // Various special characters that should be rejected
        assert!(validate_sql_identifier("table.name", "Test").is_err());
        assert!(validate_sql_identifier("table@name", "Test").is_err());
        assert!(validate_sql_identifier("table#name", "Test").is_err());
        assert!(validate_sql_identifier("table$name", "Test").is_err());
        assert!(validate_sql_identifier("table%name", "Test").is_err());
        assert!(validate_sql_identifier("table^name", "Test").is_err());
        assert!(validate_sql_identifier("table&name", "Test").is_err());
        assert!(validate_sql_identifier("table*name", "Test").is_err());
        assert!(validate_sql_identifier("table+name", "Test").is_err());
        assert!(validate_sql_identifier("table=name", "Test").is_err());
        assert!(validate_sql_identifier("table[name", "Test").is_err());
        assert!(validate_sql_identifier("table]name", "Test").is_err());
        assert!(validate_sql_identifier("table{name", "Test").is_err());
        assert!(validate_sql_identifier("table}name", "Test").is_err());
        assert!(validate_sql_identifier("table|name", "Test").is_err());
        assert!(validate_sql_identifier("table:name", "Test").is_err());
        assert!(validate_sql_identifier("table\"name", "Test").is_err());
        assert!(validate_sql_identifier("table<name", "Test").is_err());
        assert!(validate_sql_identifier("table>name", "Test").is_err());
        assert!(validate_sql_identifier("table?name", "Test").is_err());
        assert!(validate_sql_identifier("table/name", "Test").is_err());
    }

    #[test]
    fn test_validate_sql_identifier_unicode() {
        // Unicode characters should be rejected
        assert!(validate_sql_identifier("täble", "Test").is_err());
        assert!(validate_sql_identifier("表", "Test").is_err());
        assert!(validate_sql_identifier("тест", "Test").is_err());
        assert!(validate_sql_identifier("table_émoji", "Test").is_err());
    }

    #[test]
    fn test_validate_sql_identifier_newlines_tabs() {
        // Whitespace characters
        assert!(validate_sql_identifier("table\nname", "Test").is_err());
        assert!(validate_sql_identifier("table\tname", "Test").is_err());
        assert!(validate_sql_identifier("table\rname", "Test").is_err());
        assert!(validate_sql_identifier(" table", "Test").is_err());
        assert!(validate_sql_identifier("table ", "Test").is_err());
    }

    #[test]
    fn test_validate_sql_identifier_error_message_contains_type() {
        let result = validate_sql_identifier("", "Database name");
        assert!(result.unwrap_err().to_string().contains("Database name"));

        let result = validate_sql_identifier("1table", "Table name");
        assert!(result.unwrap_err().to_string().contains("Table name"));
    }

    // ==================== SQL String Escaping Tests ====================

    #[test]
    fn test_escape_sql_string() {
        // Basic escaping
        assert_eq!(escape_sql_string("hello"), "hello");
        assert_eq!(escape_sql_string("it's"), "it\\'s");
        assert_eq!(escape_sql_string("back\\slash"), "back\\\\slash");

        // Complex escaping with both
        assert_eq!(
            escape_sql_string("it's a back\\slash"),
            "it\\'s a back\\\\slash"
        );

        // SQL injection attempts
        assert_eq!(
            escape_sql_string("'; DROP TABLE users;--"),
            "\\'; DROP TABLE users;--"
        );
        assert_eq!(escape_sql_string("a'b'c"), "a\\'b\\'c");
    }

    #[test]
    fn test_escape_sql_string_injection_via_key() {
        // Verify that metadata keys with SQL injection attempts would be escaped
        let malicious_key = "field'; DELETE FROM table;--";
        let escaped = escape_sql_string(malicious_key);
        assert_eq!(escaped, "field\\'; DELETE FROM table;--");
        // The semicolon and -- remain but the quote is escaped, preventing breakout
    }

    #[test]
    fn test_escape_sql_string_empty() {
        assert_eq!(escape_sql_string(""), "");
    }

    #[test]
    fn test_escape_sql_string_no_escaping_needed() {
        assert_eq!(escape_sql_string("simple text"), "simple text");
        assert_eq!(escape_sql_string("123456"), "123456");
        assert_eq!(escape_sql_string("hello world!"), "hello world!");
    }

    #[test]
    fn test_escape_sql_string_multiple_quotes() {
        assert_eq!(escape_sql_string("'''"), "\\'\\'\\'");
        assert_eq!(escape_sql_string("a''b''c"), "a\\'\\'b\\'\\'c");
    }

    #[test]
    fn test_escape_sql_string_multiple_backslashes() {
        assert_eq!(escape_sql_string("\\\\\\"), "\\\\\\\\\\\\");
        assert_eq!(escape_sql_string("a\\b\\c"), "a\\\\b\\\\c");
    }

    #[test]
    fn test_escape_sql_string_mixed_special_chars() {
        assert_eq!(escape_sql_string("it's\\here"), "it\\'s\\\\here");
        assert_eq!(escape_sql_string("'\\"), "\\'\\\\");
        assert_eq!(escape_sql_string("\\'"), "\\\\\\'");
    }

    #[test]
    fn test_escape_sql_string_unicode() {
        // Unicode should pass through unchanged (only quotes and backslashes escaped)
        assert_eq!(escape_sql_string("héllo"), "héllo");
        assert_eq!(escape_sql_string("日本語"), "日本語");
        assert_eq!(escape_sql_string("emoji 😀"), "emoji 😀");
        assert_eq!(escape_sql_string("café's"), "café\\'s");
    }

    #[test]
    fn test_escape_sql_string_newlines_preserved() {
        assert_eq!(escape_sql_string("line1\nline2"), "line1\nline2");
        assert_eq!(escape_sql_string("tab\there"), "tab\there");
        assert_eq!(escape_sql_string("carriage\rreturn"), "carriage\rreturn");
    }

    // ==================== VectorRow Serialization Tests ====================

    #[test]
    fn test_vector_row_serialize() {
        let row = VectorRow {
            id: "test-id".to_string(),
            text: "test text".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            metadata: r#"{"key":"value"}"#.to_string(),
        };

        let json = serde_json::to_string(&row).unwrap();
        assert!(json.contains("test-id"));
        assert!(json.contains("test text"));
        assert!(json.contains("0.1"));
    }

    #[test]
    fn test_vector_row_deserialize() {
        let json = r#"{"id":"test-id","text":"test text","embedding":[0.1,0.2,0.3],"metadata":"{}"}"#;
        let row: VectorRow = serde_json::from_str(json).unwrap();
        assert_eq!(row.id, "test-id");
        assert_eq!(row.text, "test text");
        assert_eq!(row.embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(row.metadata, "{}");
    }

    #[test]
    fn test_vector_row_roundtrip() {
        let original = VectorRow {
            id: "uuid-123".to_string(),
            text: "Hello world".to_string(),
            embedding: vec![1.0, 2.0, 3.0, 4.0, 5.0],
            metadata: r#"{"author":"test","score":0.95}"#.to_string(),
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: VectorRow = serde_json::from_str(&json).unwrap();

        assert_eq!(original.id, deserialized.id);
        assert_eq!(original.text, deserialized.text);
        assert_eq!(original.embedding, deserialized.embedding);
        assert_eq!(original.metadata, deserialized.metadata);
    }

    #[test]
    fn test_vector_row_clone() {
        let row = VectorRow {
            id: "clone-test".to_string(),
            text: "clone text".to_string(),
            embedding: vec![0.5],
            metadata: "{}".to_string(),
        };
        let cloned = row.clone();
        assert_eq!(row.id, cloned.id);
        assert_eq!(row.text, cloned.text);
        assert_eq!(row.embedding, cloned.embedding);
        assert_eq!(row.metadata, cloned.metadata);
    }

    #[test]
    fn test_vector_row_debug() {
        let row = VectorRow {
            id: "debug-test".to_string(),
            text: "debug text".to_string(),
            embedding: vec![1.0],
            metadata: "{}".to_string(),
        };
        let debug_str = format!("{:?}", row);
        assert!(debug_str.contains("VectorRow"));
        assert!(debug_str.contains("debug-test"));
    }

    #[test]
    fn test_vector_row_empty_embedding() {
        let row = VectorRow {
            id: "empty".to_string(),
            text: "".to_string(),
            embedding: vec![],
            metadata: "{}".to_string(),
        };
        let json = serde_json::to_string(&row).unwrap();
        let deserialized: VectorRow = serde_json::from_str(&json).unwrap();
        assert!(deserialized.embedding.is_empty());
    }

    // ==================== DocumentRow Tests ====================

    #[test]
    fn test_document_row_deserialize() {
        let json = r#"{"id":"doc-id","text":"doc text","metadata":"{\"key\":\"value\"}"}"#;
        let row: DocumentRow = serde_json::from_str(json).unwrap();
        assert_eq!(row.id, "doc-id");
        assert_eq!(row.text, "doc text");
        assert!(row.metadata.contains("key"));
    }

    #[test]
    fn test_document_row_clone() {
        let row = DocumentRow {
            id: "clone".to_string(),
            text: "text".to_string(),
            metadata: "{}".to_string(),
        };
        let cloned = row.clone();
        assert_eq!(row.id, cloned.id);
    }

    #[test]
    fn test_document_row_debug() {
        let row = DocumentRow {
            id: "debug".to_string(),
            text: "text".to_string(),
            metadata: "{}".to_string(),
        };
        let debug_str = format!("{:?}", row);
        assert!(debug_str.contains("DocumentRow"));
    }

    // ==================== DocumentWithEmbeddingRow Tests ====================

    #[test]
    fn test_document_with_embedding_row_deserialize() {
        let json = r#"{"id":"doc-id","text":"doc text","embedding":[0.1,0.2],"metadata":"{}"}"#;
        let row: DocumentWithEmbeddingRow = serde_json::from_str(json).unwrap();
        assert_eq!(row.id, "doc-id");
        assert_eq!(row.embedding, vec![0.1, 0.2]);
    }

    #[test]
    fn test_document_with_embedding_row_clone() {
        let row = DocumentWithEmbeddingRow {
            id: "clone".to_string(),
            text: "text".to_string(),
            embedding: vec![1.0, 2.0],
            metadata: "{}".to_string(),
        };
        let cloned = row.clone();
        assert_eq!(row.embedding, cloned.embedding);
    }

    #[test]
    fn test_document_with_embedding_row_debug() {
        let row = DocumentWithEmbeddingRow {
            id: "debug".to_string(),
            text: "text".to_string(),
            embedding: vec![0.5],
            metadata: "{}".to_string(),
        };
        let debug_str = format!("{:?}", row);
        assert!(debug_str.contains("DocumentWithEmbeddingRow"));
    }

    // ==================== SimilaritySearchRow Tests ====================

    #[test]
    fn test_similarity_search_row_deserialize() {
        let json = r#"{"id":"search-id","text":"search text","metadata":"{}","distance":0.5}"#;
        let row: SimilaritySearchRow = serde_json::from_str(json).unwrap();
        assert_eq!(row.id, "search-id");
        assert_eq!(row.distance, 0.5);
    }

    #[test]
    fn test_similarity_search_row_clone() {
        let row = SimilaritySearchRow {
            id: "clone".to_string(),
            text: "text".to_string(),
            metadata: "{}".to_string(),
            distance: 0.123,
        };
        let cloned = row.clone();
        assert_eq!(row.distance, cloned.distance);
    }

    #[test]
    fn test_similarity_search_row_debug() {
        let row = SimilaritySearchRow {
            id: "debug".to_string(),
            text: "text".to_string(),
            metadata: "{}".to_string(),
            distance: 0.0,
        };
        let debug_str = format!("{:?}", row);
        assert!(debug_str.contains("SimilaritySearchRow"));
    }

    #[test]
    fn test_similarity_search_row_various_distances() {
        // Test various distance values
        for distance in [0.0, 0.5, 1.0, 2.0, -0.5, f32::MAX, f32::MIN] {
            let json = format!(
                r#"{{"id":"id","text":"text","metadata":"{{}}","distance":{}}}"#,
                distance
            );
            let row: SimilaritySearchRow = serde_json::from_str(&json).unwrap();
            // For infinity values, JSON parsing may produce NaN or inf
            if distance.is_finite() {
                assert_eq!(row.distance, distance);
            }
        }
    }

    // ==================== Cosine Similarity Tests ====================

    /// Creates a minimal ClickHouseVectorStore for testing internal methods.
    /// This uses a mock/minimal setup since we're only testing pure functions.
    fn create_test_store() -> ClickHouseVectorStore {
        // Note: This store won't be connected to any real ClickHouse instance.
        // We're only using it to test pure internal methods.
        let client = Client::default().with_url("http://localhost:8123");
        ClickHouseVectorStore {
            client,
            database: "test_db".to_string(),
            table_name: "test_table".to_string(),
            embeddings: std::sync::Arc::new(dashflow::core::embeddings::MockEmbeddings::new(384)),
            distance_metric: DistanceMetric::Cosine,
            embedding_dimension: 384,
        }
    }

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let store = create_test_store();
        let v = vec![1.0, 2.0, 3.0];
        let similarity = store.cosine_similarity(&v, &v);
        // Identical vectors should have similarity ~1.0
        assert!((similarity - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let store = create_test_store();
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let similarity = store.cosine_similarity(&v1, &v2);
        // Orthogonal vectors should have similarity ~0.0
        assert!(similarity.abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        let store = create_test_store();
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![-1.0, 0.0, 0.0];
        let similarity = store.cosine_similarity(&v1, &v2);
        // Opposite vectors should have similarity ~-1.0
        assert!((similarity + 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let store = create_test_store();
        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![1.0, 2.0]; // Different length
        let similarity = store.cosine_similarity(&v1, &v2);
        // Different lengths should return 0.0
        assert_eq!(similarity, 0.0);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let store = create_test_store();
        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![0.0, 0.0, 0.0];
        let similarity = store.cosine_similarity(&v1, &v2);
        // Zero vector should return 0.0
        assert_eq!(similarity, 0.0);
    }

    #[test]
    fn test_cosine_similarity_both_zero() {
        let store = create_test_store();
        let v = vec![0.0, 0.0, 0.0];
        let similarity = store.cosine_similarity(&v, &v);
        assert_eq!(similarity, 0.0);
    }

    #[test]
    fn test_cosine_similarity_empty_vectors() {
        let store = create_test_store();
        let v: Vec<f32> = vec![];
        let similarity = store.cosine_similarity(&v, &v);
        // Empty vectors have different lengths check passes but norm is 0
        // Actually, empty vectors have the same length (0), so it will compute
        // dot_product = 0, norm_a = 0, norm_b = 0, returning 0.0
        assert_eq!(similarity, 0.0);
    }

    #[test]
    fn test_cosine_similarity_single_element() {
        let store = create_test_store();
        let v1 = vec![3.0];
        let v2 = vec![4.0];
        let similarity = store.cosine_similarity(&v1, &v2);
        // Both positive single elements => similarity = 1.0
        assert!((similarity - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_negative_values() {
        let store = create_test_store();
        let v1 = vec![-1.0, -2.0, -3.0];
        let v2 = vec![-1.0, -2.0, -3.0];
        let similarity = store.cosine_similarity(&v1, &v2);
        assert!((similarity - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_mixed_signs() {
        let store = create_test_store();
        let v1 = vec![1.0, -1.0, 1.0];
        let v2 = vec![-1.0, 1.0, -1.0];
        let similarity = store.cosine_similarity(&v1, &v2);
        // These are exactly opposite => -1.0
        assert!((similarity + 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_large_vectors() {
        let store = create_test_store();
        let v1: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        let v2: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        let similarity = store.cosine_similarity(&v1, &v2);
        assert!((similarity - 1.0).abs() < 0.0001);
    }

    // ==================== Distance Function Tests ====================

    #[test]
    fn test_distance_function_cosine() {
        let store = create_test_store();
        assert_eq!(store.distance_function(), "cosineDistance");
    }

    #[test]
    fn test_distance_function_euclidean() {
        let mut store = create_test_store();
        store.distance_metric = DistanceMetric::Euclidean;
        assert_eq!(store.distance_function(), "L2Distance");
    }

    #[test]
    fn test_distance_function_dot_product() {
        let mut store = create_test_store();
        store.distance_metric = DistanceMetric::DotProduct;
        assert_eq!(store.distance_function(), "dotProduct");
    }

    #[test]
    fn test_distance_function_max_inner_product() {
        let mut store = create_test_store();
        store.distance_metric = DistanceMetric::MaxInnerProduct;
        assert_eq!(store.distance_function(), "dotProduct");
    }

    // ==================== Distance Metric Accessor Tests ====================

    #[test]
    fn test_distance_metric_accessor() {
        let store = create_test_store();
        assert_eq!(store.distance_metric(), DistanceMetric::Cosine);
    }

    #[test]
    fn test_embeddings_accessor() {
        let store = create_test_store();
        assert!(store.embeddings().is_some());
    }

    // ==================== Build Where Clause Tests ====================

    #[test]
    fn test_build_where_clause_empty() {
        let store = create_test_store();
        let filter = HashMap::new();
        let clause = store.build_where_clause(&filter);
        assert_eq!(clause, "1=1");
    }

    #[test]
    fn test_build_where_clause_single_string() {
        let store = create_test_store();
        let mut filter = HashMap::new();
        filter.insert("author".to_string(), JsonValue::String("Alice".to_string()));
        let clause = store.build_where_clause(&filter);
        assert!(clause.contains("JSONExtractString(metadata, 'author') = 'Alice'"));
    }

    #[test]
    fn test_build_where_clause_single_number() {
        let store = create_test_store();
        let mut filter = HashMap::new();
        filter.insert(
            "score".to_string(),
            JsonValue::Number(serde_json::Number::from(42)),
        );
        let clause = store.build_where_clause(&filter);
        assert!(clause.contains("JSONExtractString(metadata, 'score') = 42"));
    }

    #[test]
    fn test_build_where_clause_single_bool_true() {
        let store = create_test_store();
        let mut filter = HashMap::new();
        filter.insert("active".to_string(), JsonValue::Bool(true));
        let clause = store.build_where_clause(&filter);
        assert!(clause.contains("JSONExtractString(metadata, 'active') = true"));
    }

    #[test]
    fn test_build_where_clause_single_bool_false() {
        let store = create_test_store();
        let mut filter = HashMap::new();
        filter.insert("active".to_string(), JsonValue::Bool(false));
        let clause = store.build_where_clause(&filter);
        assert!(clause.contains("JSONExtractString(metadata, 'active') = false"));
    }

    #[test]
    fn test_build_where_clause_multiple_conditions() {
        let store = create_test_store();
        let mut filter = HashMap::new();
        filter.insert("author".to_string(), JsonValue::String("Bob".to_string()));
        filter.insert("year".to_string(), JsonValue::Number(2024.into()));
        let clause = store.build_where_clause(&filter);
        // Multiple conditions should be joined with AND
        assert!(clause.contains(" AND "));
        assert!(clause.contains("author"));
        assert!(clause.contains("year"));
    }

    #[test]
    fn test_build_where_clause_escapes_string_values() {
        let store = create_test_store();
        let mut filter = HashMap::new();
        filter.insert(
            "name".to_string(),
            JsonValue::String("O'Brien".to_string()),
        );
        let clause = store.build_where_clause(&filter);
        // Single quote should be escaped
        assert!(clause.contains("\\'"));
    }

    #[test]
    fn test_build_where_clause_escapes_keys() {
        let store = create_test_store();
        let mut filter = HashMap::new();
        filter.insert(
            "key'injection".to_string(),
            JsonValue::String("value".to_string()),
        );
        let clause = store.build_where_clause(&filter);
        // Key should be escaped
        assert!(clause.contains("key\\'injection"));
    }

    #[test]
    fn test_build_where_clause_null_value() {
        let store = create_test_store();
        let mut filter = HashMap::new();
        filter.insert("nullable".to_string(), JsonValue::Null);
        let clause = store.build_where_clause(&filter);
        // Null gets stringified
        assert!(clause.contains("'null'"));
    }

    #[test]
    fn test_build_where_clause_array_value() {
        let store = create_test_store();
        let mut filter = HashMap::new();
        filter.insert(
            "tags".to_string(),
            JsonValue::Array(vec![JsonValue::String("a".to_string())]),
        );
        let clause = store.build_where_clause(&filter);
        // Array gets stringified
        assert!(clause.contains("tags"));
    }

    #[test]
    fn test_build_where_clause_object_value() {
        let store = create_test_store();
        let mut filter = HashMap::new();
        let mut inner = serde_json::Map::new();
        inner.insert("nested".to_string(), JsonValue::String("value".to_string()));
        filter.insert("obj".to_string(), JsonValue::Object(inner));
        let clause = store.build_where_clause(&filter);
        // Object gets stringified
        assert!(clause.contains("obj"));
    }

    #[test]
    fn test_build_where_clause_float_number() {
        let store = create_test_store();
        let mut filter = HashMap::new();
        filter.insert(
            "score".to_string(),
            JsonValue::Number(serde_json::Number::from_f64(0.95).unwrap()),
        );
        let clause = store.build_where_clause(&filter);
        assert!(clause.contains("0.95"));
    }

    #[test]
    fn test_build_where_clause_negative_number() {
        let store = create_test_store();
        let mut filter = HashMap::new();
        filter.insert(
            "offset".to_string(),
            JsonValue::Number(serde_json::Number::from(-10)),
        );
        let clause = store.build_where_clause(&filter);
        assert!(clause.contains("-10"));
    }

    // ==================== MMR Selection Tests ====================

    #[test]
    fn test_mmr_selection_empty_candidates() {
        let store = create_test_store();
        let query_embedding = vec![1.0, 0.0, 0.0];
        let candidates: Vec<(Document, Vec<f32>)> = vec![];
        let result = store.mmr_selection(&query_embedding, candidates, 5, 0.5).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_mmr_selection_single_candidate() {
        let store = create_test_store();
        let query_embedding = vec![1.0, 0.0, 0.0];
        let doc = Document {
            id: Some("doc1".to_string()),
            page_content: "test".to_string(),
            metadata: HashMap::new(),
        };
        let candidates = vec![(doc.clone(), vec![1.0, 0.0, 0.0])];
        let result = store.mmr_selection(&query_embedding, candidates, 5, 0.5).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, Some("doc1".to_string()));
    }

    #[test]
    fn test_mmr_selection_k_greater_than_candidates() {
        let store = create_test_store();
        let query_embedding = vec![1.0, 0.0, 0.0];
        let docs: Vec<(Document, Vec<f32>)> = (0..3)
            .map(|i| {
                (
                    Document {
                        id: Some(format!("doc{}", i)),
                        page_content: format!("content {}", i),
                        metadata: HashMap::new(),
                    },
                    vec![1.0, i as f32 * 0.1, 0.0],
                )
            })
            .collect();
        let result = store.mmr_selection(&query_embedding, docs, 10, 0.5).unwrap();
        // Should return all 3 even though k=10
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_mmr_selection_k_less_than_candidates() {
        let store = create_test_store();
        let query_embedding = vec![1.0, 0.0, 0.0];
        let docs: Vec<(Document, Vec<f32>)> = (0..10)
            .map(|i| {
                (
                    Document {
                        id: Some(format!("doc{}", i)),
                        page_content: format!("content {}", i),
                        metadata: HashMap::new(),
                    },
                    vec![1.0, i as f32 * 0.1, 0.0],
                )
            })
            .collect();
        let result = store.mmr_selection(&query_embedding, docs, 3, 0.5).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_mmr_selection_lambda_one_pure_relevance() {
        let store = create_test_store();
        let query_embedding = vec![1.0, 0.0, 0.0];
        // doc0 is most similar to query, doc1 is second
        let docs = vec![
            (
                Document {
                    id: Some("most_similar".to_string()),
                    page_content: "most similar".to_string(),
                    metadata: HashMap::new(),
                },
                vec![1.0, 0.0, 0.0], // Identical to query
            ),
            (
                Document {
                    id: Some("less_similar".to_string()),
                    page_content: "less similar".to_string(),
                    metadata: HashMap::new(),
                },
                vec![0.5, 0.5, 0.0],
            ),
        ];
        let result = store.mmr_selection(&query_embedding, docs, 1, 1.0).unwrap();
        // With lambda=1.0, pure relevance, should pick most similar
        assert_eq!(result[0].id, Some("most_similar".to_string()));
    }

    #[test]
    fn test_mmr_selection_lambda_zero_pure_diversity() {
        let store = create_test_store();
        let query_embedding = vec![1.0, 0.0, 0.0];
        // All docs similar to query but different from each other
        let docs = vec![
            (
                Document {
                    id: Some("doc0".to_string()),
                    page_content: "".to_string(),
                    metadata: HashMap::new(),
                },
                vec![1.0, 0.0, 0.0],
            ),
            (
                Document {
                    id: Some("doc1".to_string()),
                    page_content: "".to_string(),
                    metadata: HashMap::new(),
                },
                vec![1.0, 0.01, 0.0],
            ),
            (
                Document {
                    id: Some("doc2".to_string()),
                    page_content: "".to_string(),
                    metadata: HashMap::new(),
                },
                vec![0.0, 1.0, 0.0], // Orthogonal
            ),
        ];
        // With lambda=0.0, after selecting first doc, should prefer maximally different doc
        let result = store.mmr_selection(&query_embedding, docs, 2, 0.0).unwrap();
        // First selection has no selected docs yet, so only relevance matters
        // After that, diversity kicks in
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_mmr_selection_preserves_document_metadata() {
        let store = create_test_store();
        let query_embedding = vec![1.0, 0.0, 0.0];
        let mut metadata = HashMap::new();
        metadata.insert(
            "important".to_string(),
            JsonValue::String("data".to_string()),
        );
        let doc = Document {
            id: Some("doc1".to_string()),
            page_content: "content".to_string(),
            metadata: metadata.clone(),
        };
        let candidates = vec![(doc, vec![1.0, 0.0, 0.0])];
        let result = store.mmr_selection(&query_embedding, candidates, 1, 0.5).unwrap();
        assert_eq!(result[0].metadata.get("important"), metadata.get("important"));
    }
}
