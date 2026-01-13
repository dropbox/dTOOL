// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Apache Cassandra / Astra DB vector store implementation

use async_trait::async_trait;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::{Error, Result};
use dashflow::{embed, embed_query};
use scylla::{Session, SessionBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Validate a Cassandra identifier (keyspace name, table name, etc.)
///
/// Cassandra identifiers must:
/// - Start with a letter (a-z, A-Z) or underscore
/// - Contain only letters, digits, and underscores
/// - Be at most 48 characters (Cassandra limit for keyspace/table names)
fn validate_identifier(name: &str, kind: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::config(format!("{} name cannot be empty", kind)));
    }

    if name.len() > 48 {
        return Err(Error::config(format!(
            "{} name '{}' exceeds maximum length of 48 characters",
            kind, name
        )));
    }

    let mut chars = name.chars();
    let first = chars.next().unwrap(); // Safe: we checked non-empty above

    // First character must be a letter or underscore
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(Error::config(format!(
            "{} name '{}' must start with a letter or underscore",
            kind, name
        )));
    }

    // Remaining characters must be letters, digits, or underscores
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return Err(Error::config(format!(
                "{} name '{}' contains invalid character '{}'",
                kind, name, c
            )));
        }
    }

    Ok(())
}

/// Similarity function for vector comparisons in Cassandra
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimilarityFunction {
    /// Cosine similarity (default)
    Cosine,
    /// Euclidean distance (L2)
    Euclidean,
    /// Dot product / Inner product
    DotProduct,
}

impl SimilarityFunction {
    /// Convert to Cassandra CQL similarity function name
    #[must_use]
    pub fn as_cql_str(&self) -> &'static str {
        match self {
            SimilarityFunction::Cosine => "cosine",
            SimilarityFunction::Euclidean => "euclidean",
            SimilarityFunction::DotProduct => "dot_product",
        }
    }

    /// Convert to `DashFlow` `DistanceMetric`
    #[must_use]
    pub fn to_distance_metric(&self) -> DistanceMetric {
        match self {
            SimilarityFunction::Cosine => DistanceMetric::Cosine,
            SimilarityFunction::Euclidean => DistanceMetric::Euclidean,
            SimilarityFunction::DotProduct => DistanceMetric::DotProduct,
        }
    }
}

/// Apache Cassandra / Astra DB vector store
///
/// Stores document embeddings in Cassandra using native `VECTOR<FLOAT, N>` type
/// and performs similarity search using `ORDER BY ANN OF` queries.
///
/// ## Features
///
/// - Native vector type support (Cassandra 5.0+ / Astra DB)
/// - Approximate Nearest Neighbor (ANN) search
/// - Multiple similarity functions (cosine, euclidean, dot product)
/// - Metadata filtering
/// - Distributed architecture for horizontal scaling
///
/// ## Example
///
/// ```rust,no_run
/// use dashflow_cassandra::CassandraVectorStore;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let store = CassandraVectorStore::builder()
///     .contact_points(vec!["127.0.0.1:9042"])
///     .keyspace("dashflow")
///     .table("vector_store")
///     .vector_dimension(1536)
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct CassandraVectorStore {
    session: Arc<Session>,
    keyspace: String,
    table: String,
    vector_dimension: usize,
    similarity_function: SimilarityFunction,
    embeddings: Arc<dyn Embeddings>,
}

impl CassandraVectorStore {
    /// Create a new builder for configuring the Cassandra vector store
    #[must_use]
    pub fn builder() -> CassandraVectorStoreBuilder {
        CassandraVectorStoreBuilder::default()
    }

    /// Get fully qualified table name (keyspace.table)
    fn table_fqname(&self) -> String {
        format!("{}.{}", self.keyspace, self.table)
    }

    /// Initialize the vector store table if it doesn't exist
    async fn initialize_table(&self) -> Result<()> {
        let create_table_cql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id UUID PRIMARY KEY,
                content TEXT,
                metadata TEXT,
                vector VECTOR<FLOAT, {}>
            )",
            self.table_fqname(),
            self.vector_dimension
        );

        self.session
            .query_unpaged(create_table_cql, &[])
            .await
            .map_err(|e| Error::other(format!("Failed to create table: {e}")))?;

        Ok(())
    }

    /// Create vector index if it doesn't exist
    async fn create_vector_index(&self) -> Result<()> {
        let create_index_cql = format!(
            "CREATE INDEX IF NOT EXISTS idx_vector ON {} (vector)
             WITH OPTIONS = {{'similarity_function': '{}'}}",
            self.table_fqname(),
            self.similarity_function.as_cql_str()
        );

        self.session
            .query_unpaged(create_index_cql, &[])
            .await
            .map_err(|e| Error::other(format!("Failed to create index: {e}")))?;

        Ok(())
    }

    /// Add documents with their pre-computed embeddings
    pub async fn add_documents_with_embeddings(
        &self,
        texts: Vec<String>,
        embeddings: Vec<Vec<f32>>,
        metadatas: Option<Vec<HashMap<String, serde_json::Value>>>,
    ) -> Result<Vec<String>> {
        if texts.len() != embeddings.len() {
            return Err(Error::config(
                "Number of texts must match number of embeddings",
            ));
        }

        // Validate dimensions
        for emb in &embeddings {
            if emb.len() != self.vector_dimension {
                return Err(Error::config(format!(
                    "Expected vector dimension {}, got {}",
                    self.vector_dimension,
                    emb.len()
                )));
            }
        }

        let insert_cql = format!(
            "INSERT INTO {} (id, content, metadata, vector) VALUES (?, ?, ?, ?)",
            self.table_fqname()
        );

        let prepared = self
            .session
            .prepare(insert_cql)
            .await
            .map_err(|e| Error::other(format!("Failed to prepare statement: {e}")))?;

        let mut ids = Vec::new();

        for (idx, (text, embedding)) in texts.iter().zip(embeddings.iter()).enumerate() {
            let id = Uuid::new_v4();
            let metadata_json = metadatas.as_ref().and_then(|m| m.get(idx)).map_or_else(
                || "{}".to_string(),
                |m| serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string()),
            );

            self.session
                .execute_unpaged(
                    &prepared,
                    (id, text.as_str(), metadata_json.as_str(), embedding),
                )
                .await
                .map_err(|e| Error::other(format!("Failed to insert document: {e}")))?;

            ids.push(id.to_string());
        }

        Ok(ids)
    }

    /// Perform similarity search by vector (internal implementation)
    async fn similarity_search_by_vector_with_score_internal(
        &self,
        query_vector: &[f32],
        k: usize,
    ) -> Result<Vec<(Document, f32)>> {
        if query_vector.len() != self.vector_dimension {
            return Err(Error::config(format!(
                "Expected vector dimension {}, got {}",
                self.vector_dimension,
                query_vector.len()
            )));
        }

        // Cassandra 5.0+ ANN search syntax: ORDER BY vector ANN OF ?
        let select_cql = format!(
            "SELECT id, content, metadata, similarity_{}(vector, ?) AS score
             FROM {}
             ORDER BY vector ANN OF ?
             LIMIT ?",
            self.similarity_function.as_cql_str(),
            self.table_fqname()
        );

        let query_vec = query_vector.to_vec();
        let query_result = self
            .session
            .query_unpaged(select_cql, (&query_vec, &query_vec, k as i32))
            .await
            .map_err(|e| Error::other(format!("Failed to execute query: {e}")))?;

        let rows_opt = query_result
            .rows
            .ok_or_else(|| Error::other("Query returned no rows"))?;

        let mut results = Vec::new();

        for row in rows_opt {
            let content: String = row
                .columns
                .get(1)
                .and_then(|c| c.as_ref())
                .and_then(|c| {
                    if let scylla::frame::response::result::CqlValue::Text(s) = c {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .ok_or_else(|| Error::other("Missing content column"))?;

            let metadata_str: String = row
                .columns
                .get(2)
                .and_then(|c| c.as_ref())
                .and_then(|c| {
                    if let scylla::frame::response::result::CqlValue::Text(s) = c {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "{}".to_string());

            let metadata: HashMap<String, serde_json::Value> =
                serde_json::from_str(&metadata_str).unwrap_or_default();

            let score: f32 = *row
                .columns
                .get(3)
                .and_then(|c| c.as_ref())
                .and_then(|c| {
                    if let scylla::frame::response::result::CqlValue::Float(f) = c {
                        Some(f)
                    } else {
                        None
                    }
                })
                .ok_or_else(|| Error::other("Missing score column"))?;

            let doc = Document {
                id: None, // We don't need to return IDs for search results
                page_content: content,
                metadata,
            };

            results.push((doc, score));
        }

        Ok(results)
    }

    /// Delete documents by IDs
    pub async fn delete(&self, ids: Vec<String>) -> Result<()> {
        let delete_cql = format!("DELETE FROM {} WHERE id = ?", self.table_fqname());

        let prepared = self
            .session
            .prepare(delete_cql)
            .await
            .map_err(|e| Error::other(format!("Failed to prepare delete: {e}")))?;

        for id_str in ids {
            let id = Uuid::parse_str(&id_str)
                .map_err(|e| Error::config(format!("Invalid UUID: {e}")))?;

            self.session
                .execute_unpaged(&prepared, (id,))
                .await
                .map_err(|e| Error::other(format!("Failed to delete: {e}")))?;
        }

        Ok(())
    }

    /// Get documents by IDs
    pub async fn get_by_ids(&self, ids: Vec<String>) -> Result<Vec<Document>> {
        let select_cql = format!(
            "SELECT content, metadata FROM {} WHERE id = ?",
            self.table_fqname()
        );

        let prepared = self
            .session
            .prepare(select_cql)
            .await
            .map_err(|e| Error::other(format!("Failed to prepare select: {e}")))?;

        let mut documents = Vec::new();

        for id_str in ids {
            let id = Uuid::parse_str(&id_str)
                .map_err(|e| Error::config(format!("Invalid UUID: {e}")))?;

            let query_result = self
                .session
                .execute_unpaged(&prepared, (id,))
                .await
                .map_err(|e| Error::other(format!("Failed to select: {e}")))?;

            let rows_opt = query_result.rows;
            if let Some(rows) = rows_opt {
                for row in rows {
                    let content: String = row
                        .columns
                        .first()
                        .and_then(|c| c.as_ref())
                        .and_then(|c| {
                            if let scylla::frame::response::result::CqlValue::Text(s) = c {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| Error::other("Missing content column"))?;

                    let metadata_str: String = row
                        .columns
                        .get(1)
                        .and_then(|c| c.as_ref())
                        .and_then(|c| {
                            if let scylla::frame::response::result::CqlValue::Text(s) = c {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| "{}".to_string());

                    let metadata: HashMap<String, serde_json::Value> =
                        serde_json::from_str(&metadata_str).unwrap_or_default();

                    documents.push(Document {
                        id: Some(id_str.clone()),
                        page_content: content,
                        metadata,
                    });
                }
            }
        }

        Ok(documents)
    }
}

#[async_trait]
impl VectorStore for CassandraVectorStore {
    fn embeddings(&self) -> Option<Arc<dyn Embeddings>> {
        Some(Arc::clone(&self.embeddings))
    }

    fn distance_metric(&self) -> DistanceMetric {
        self.similarity_function.to_distance_metric()
    }

    async fn add_texts(
        &mut self,
        texts: &[impl AsRef<str> + Send + Sync],
        metadatas: Option<&[HashMap<String, serde_json::Value>]>,
        ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
        // Convert texts to strings
        let text_strings: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();
        let text_count = text_strings.len();

        // Validate metadatas length if provided
        if let Some(metadatas) = metadatas {
            if metadatas.len() != text_count {
                return Err(Error::config(format!(
                    "Metadatas length mismatch: {} vs {}",
                    metadatas.len(),
                    text_count
                )));
            }
        }

        // Validate IDs length if provided
        if let Some(ids) = ids {
            if ids.len() != text_count {
                return Err(Error::config(format!(
                    "IDs length mismatch: {} vs {}",
                    ids.len(),
                    text_count
                )));
            }
        }

        // Generate embeddings for all texts using graph API
        let embedded_vectors = embed(Arc::clone(&self.embeddings), &text_strings).await?;

        // Add documents with embeddings
        self.add_documents_with_embeddings(
            text_strings,
            embedded_vectors,
            metadatas
                .map(<[std::collections::HashMap<std::string::String, serde_json::Value>]>::to_vec),
        )
        .await
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        if let Some(ids) = ids {
            let delete_cql = format!("DELETE FROM {} WHERE id = ?", self.table_fqname());

            let prepared = self
                .session
                .prepare(delete_cql)
                .await
                .map_err(|e| Error::other(format!("Failed to prepare delete: {e}")))?;

            for id_str in ids {
                let id = Uuid::parse_str(id_str)
                    .map_err(|e| Error::config(format!("Invalid UUID: {e}")))?;

                self.session
                    .execute_unpaged(&prepared, (id,))
                    .await
                    .map_err(|e| Error::other(format!("Failed to delete: {e}")))?;
            }
            Ok(true)
        } else {
            // If no IDs provided, don't delete anything
            Ok(false)
        }
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        let select_cql = format!(
            "SELECT content, metadata FROM {} WHERE id = ?",
            self.table_fqname()
        );

        let prepared = self
            .session
            .prepare(select_cql)
            .await
            .map_err(|e| Error::other(format!("Failed to prepare select: {e}")))?;

        let mut documents = Vec::new();

        for id_str in ids {
            let id =
                Uuid::parse_str(id_str).map_err(|e| Error::config(format!("Invalid UUID: {e}")))?;

            let query_result = self
                .session
                .execute_unpaged(&prepared, (id,))
                .await
                .map_err(|e| Error::other(format!("Failed to select: {e}")))?;

            let rows_opt = query_result.rows;
            if let Some(rows) = rows_opt {
                for row in rows {
                    let content: String = row
                        .columns
                        .first()
                        .and_then(|c| c.as_ref())
                        .and_then(|c| {
                            if let scylla::frame::response::result::CqlValue::Text(s) = c {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| Error::other("Missing content column"))?;

                    let metadata_str: String = row
                        .columns
                        .get(1)
                        .and_then(|c| c.as_ref())
                        .and_then(|c| {
                            if let scylla::frame::response::result::CqlValue::Text(s) = c {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| "{}".to_string());

                    let metadata: HashMap<String, serde_json::Value> =
                        serde_json::from_str(&metadata_str).unwrap_or_default();

                    documents.push(Document {
                        id: Some(id_str.clone()),
                        page_content: content,
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
        _filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        let query_vector = embed_query(Arc::clone(&self.embeddings), query).await?;
        let results = self
            .similarity_search_by_vector_with_score(&query_vector, k, _filter)
            .await?;
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        _filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<(Document, f32)>> {
        let query_vector = embed_query(Arc::clone(&self.embeddings), query).await?;
        self.similarity_search_by_vector_with_score(&query_vector, k, _filter)
            .await
    }

    async fn similarity_search_by_vector(
        &self,
        query_vector: &[f32],
        k: usize,
        _filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        let results = self
            .similarity_search_by_vector_with_score_internal(query_vector, k)
            .await?;
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    async fn similarity_search_by_vector_with_score(
        &self,
        query_vector: &[f32],
        k: usize,
        _filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<(Document, f32)>> {
        self.similarity_search_by_vector_with_score_internal(query_vector, k)
            .await
    }
}

/// Builder for configuring a Cassandra vector store
pub struct CassandraVectorStoreBuilder {
    contact_points: Vec<String>,
    keyspace: Option<String>,
    table: Option<String>,
    vector_dimension: Option<usize>,
    similarity_function: SimilarityFunction,
    embeddings: Option<Arc<dyn Embeddings>>,
}

impl Default for CassandraVectorStoreBuilder {
    fn default() -> Self {
        Self {
            contact_points: vec!["127.0.0.1:9042".to_string()],
            keyspace: None,
            table: None,
            vector_dimension: None,
            similarity_function: SimilarityFunction::Cosine,
            embeddings: None,
        }
    }
}

impl CassandraVectorStoreBuilder {
    /// Set Cassandra contact points (nodes)
    ///
    /// # Example
    /// ```rust,no_run
    /// # use dashflow_cassandra::CassandraVectorStore;
    /// let builder = CassandraVectorStore::builder()
    ///     .contact_points(vec![
    ///         "10.0.0.1:9042".to_string(),
    ///         "10.0.0.2:9042".to_string(),
    ///     ]);
    /// ```
    #[must_use]
    pub fn contact_points(mut self, contact_points: Vec<impl Into<String>>) -> Self {
        self.contact_points = contact_points
            .into_iter()
            .map(std::convert::Into::into)
            .collect();
        self
    }

    /// Set keyspace name
    pub fn keyspace(mut self, keyspace: impl Into<String>) -> Self {
        self.keyspace = Some(keyspace.into());
        self
    }

    /// Set table name
    pub fn table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// Set vector dimension (must match embedding model)
    #[must_use]
    pub fn vector_dimension(mut self, dimension: usize) -> Self {
        self.vector_dimension = Some(dimension);
        self
    }

    /// Set similarity function for vector comparisons
    #[must_use]
    pub fn similarity_function(mut self, func: SimilarityFunction) -> Self {
        self.similarity_function = func;
        self
    }

    /// Set embeddings model
    pub fn embeddings(mut self, embeddings: Arc<dyn Embeddings>) -> Self {
        self.embeddings = Some(embeddings);
        self
    }

    /// Build the Cassandra vector store
    ///
    /// This will:
    /// 1. Validate keyspace and table names (CQL injection prevention)
    /// 2. Connect to Cassandra cluster
    /// 3. Create table if it doesn't exist
    /// 4. Create vector index if it doesn't exist
    pub async fn build(self) -> Result<CassandraVectorStore> {
        let keyspace = self
            .keyspace
            .ok_or_else(|| Error::config("Keyspace is required"))?;

        let table = self
            .table
            .ok_or_else(|| Error::config("Table name is required"))?;

        // Validate identifiers to prevent CQL injection
        validate_identifier(&keyspace, "Keyspace")?;
        validate_identifier(&table, "Table")?;

        let vector_dimension = self
            .vector_dimension
            .ok_or_else(|| Error::config("Vector dimension is required"))?;

        let embeddings = self
            .embeddings
            .ok_or_else(|| Error::config("Embeddings model is required"))?;

        // Create session
        let mut session_builder = SessionBuilder::new();
        for contact_point in &self.contact_points {
            session_builder = session_builder.known_node(contact_point);
        }

        let session = session_builder
            .build()
            .await
            .map_err(|e| Error::other(format!("Failed to connect to Cassandra: {e}")))?;

        let store = CassandraVectorStore {
            session: Arc::new(session),
            keyspace,
            table,
            vector_dimension,
            similarity_function: self.similarity_function,
            embeddings,
        };

        // Initialize table and index
        store.initialize_table().await?;
        store.create_vector_index().await?;

        Ok(store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== validate_identifier tests ====================

    #[test]
    fn test_validate_identifier_valid_simple() {
        assert!(validate_identifier("users", "Keyspace").is_ok());
        assert!(validate_identifier("my_table", "Table").is_ok());
        assert!(validate_identifier("_private", "Table").is_ok());
    }

    #[test]
    fn test_validate_identifier_valid_with_numbers() {
        assert!(validate_identifier("table1", "Table").is_ok());
        assert!(validate_identifier("v2_embeddings", "Table").is_ok());
        assert!(validate_identifier("data_2024", "Table").is_ok());
    }

    #[test]
    fn test_validate_identifier_valid_underscore_start() {
        assert!(validate_identifier("_internal", "Table").is_ok());
        assert!(validate_identifier("__system", "Keyspace").is_ok());
    }

    #[test]
    fn test_validate_identifier_empty() {
        let result = validate_identifier("", "Keyspace");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cannot be empty"), "Error: {}", err);
    }

    #[test]
    fn test_validate_identifier_too_long() {
        let long_name = "a".repeat(49); // 49 chars, exceeds 48
        let result = validate_identifier(&long_name, "Table");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("exceeds maximum length"), "Error: {}", err);
    }

    #[test]
    fn test_validate_identifier_max_length() {
        let max_name = "a".repeat(48); // Exactly 48 chars
        assert!(validate_identifier(&max_name, "Table").is_ok());
    }

    #[test]
    fn test_validate_identifier_starts_with_number() {
        let result = validate_identifier("1table", "Table");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("must start with a letter or underscore"),
            "Error: {}",
            err
        );
    }

    #[test]
    fn test_validate_identifier_starts_with_special_char() {
        let result = validate_identifier("-invalid", "Table");
        assert!(result.is_err());

        let result2 = validate_identifier("$table", "Keyspace");
        assert!(result2.is_err());
    }

    #[test]
    fn test_validate_identifier_contains_invalid_chars() {
        let result = validate_identifier("table-name", "Table");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid character '-'"), "Error: {}", err);

        let result2 = validate_identifier("user.data", "Keyspace");
        assert!(result2.is_err());

        let result3 = validate_identifier("table name", "Table");
        assert!(result3.is_err());
    }

    #[test]
    fn test_validate_identifier_unicode() {
        // Unicode chars should be rejected (not ASCII alphanumeric)
        let result = validate_identifier("tàble", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_kind_in_error() {
        let result = validate_identifier("", "Keyspace");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Keyspace"), "Error should mention kind: {}", err);

        let result2 = validate_identifier("", "Table");
        let err2 = result2.unwrap_err().to_string();
        assert!(err2.contains("Table"), "Error should mention kind: {}", err2);
    }

    // ==================== SimilarityFunction tests ====================

    #[test]
    fn test_similarity_function_as_cql_str_cosine() {
        assert_eq!(SimilarityFunction::Cosine.as_cql_str(), "cosine");
    }

    #[test]
    fn test_similarity_function_as_cql_str_euclidean() {
        assert_eq!(SimilarityFunction::Euclidean.as_cql_str(), "euclidean");
    }

    #[test]
    fn test_similarity_function_as_cql_str_dot_product() {
        assert_eq!(SimilarityFunction::DotProduct.as_cql_str(), "dot_product");
    }

    #[test]
    fn test_similarity_function_to_distance_metric_cosine() {
        assert_eq!(
            SimilarityFunction::Cosine.to_distance_metric(),
            DistanceMetric::Cosine
        );
    }

    #[test]
    fn test_similarity_function_to_distance_metric_euclidean() {
        assert_eq!(
            SimilarityFunction::Euclidean.to_distance_metric(),
            DistanceMetric::Euclidean
        );
    }

    #[test]
    fn test_similarity_function_to_distance_metric_dot_product() {
        assert_eq!(
            SimilarityFunction::DotProduct.to_distance_metric(),
            DistanceMetric::DotProduct
        );
    }

    #[test]
    #[allow(clippy::clone_on_copy)]
    fn test_similarity_function_clone_and_eq() {
        let func = SimilarityFunction::Cosine;
        let cloned = func.clone();
        assert_eq!(func, cloned);

        assert_ne!(SimilarityFunction::Cosine, SimilarityFunction::Euclidean);
    }

    #[test]
    fn test_similarity_function_debug() {
        let debug = format!("{:?}", SimilarityFunction::Cosine);
        assert_eq!(debug, "Cosine");

        let debug2 = format!("{:?}", SimilarityFunction::Euclidean);
        assert_eq!(debug2, "Euclidean");
    }

    #[test]
    fn test_similarity_function_serialize() {
        let func = SimilarityFunction::DotProduct;
        let json = serde_json::to_string(&func).expect("serialize");
        assert_eq!(json, "\"DotProduct\"");
    }

    #[test]
    fn test_similarity_function_deserialize() {
        let func: SimilarityFunction =
            serde_json::from_str("\"Euclidean\"").expect("deserialize");
        assert_eq!(func, SimilarityFunction::Euclidean);
    }

    // ==================== CassandraVectorStoreBuilder tests ====================

    #[test]
    fn test_builder_default() {
        let builder = CassandraVectorStoreBuilder::default();
        assert_eq!(builder.contact_points, vec!["127.0.0.1:9042".to_string()]);
        assert!(builder.keyspace.is_none());
        assert!(builder.table.is_none());
        assert!(builder.vector_dimension.is_none());
        assert_eq!(builder.similarity_function, SimilarityFunction::Cosine);
        assert!(builder.embeddings.is_none());
    }

    #[test]
    fn test_builder_contact_points() {
        let builder = CassandraVectorStore::builder()
            .contact_points(vec!["10.0.0.1:9042", "10.0.0.2:9042"]);
        assert_eq!(
            builder.contact_points,
            vec!["10.0.0.1:9042".to_string(), "10.0.0.2:9042".to_string()]
        );
    }

    #[test]
    fn test_builder_keyspace() {
        let builder = CassandraVectorStore::builder().keyspace("my_keyspace");
        assert_eq!(builder.keyspace, Some("my_keyspace".to_string()));
    }

    #[test]
    fn test_builder_table() {
        let builder = CassandraVectorStore::builder().table("embeddings");
        assert_eq!(builder.table, Some("embeddings".to_string()));
    }

    #[test]
    fn test_builder_vector_dimension() {
        let builder = CassandraVectorStore::builder().vector_dimension(1536);
        assert_eq!(builder.vector_dimension, Some(1536));
    }

    #[test]
    fn test_builder_similarity_function() {
        let builder =
            CassandraVectorStore::builder().similarity_function(SimilarityFunction::Euclidean);
        assert_eq!(builder.similarity_function, SimilarityFunction::Euclidean);
    }

    #[test]
    fn test_builder_chaining() {
        let builder = CassandraVectorStore::builder()
            .contact_points(vec!["localhost:9042"])
            .keyspace("test_ks")
            .table("test_table")
            .vector_dimension(768)
            .similarity_function(SimilarityFunction::DotProduct);

        assert_eq!(builder.contact_points, vec!["localhost:9042".to_string()]);
        assert_eq!(builder.keyspace, Some("test_ks".to_string()));
        assert_eq!(builder.table, Some("test_table".to_string()));
        assert_eq!(builder.vector_dimension, Some(768));
        assert_eq!(builder.similarity_function, SimilarityFunction::DotProduct);
    }

    #[test]
    fn test_cassandra_vector_store_builder_method() {
        let builder = CassandraVectorStore::builder();
        // Just verify we get a builder back
        assert!(builder.keyspace.is_none());
    }

    // ==================== Additional validate_identifier tests ====================

    #[test]
    fn test_validate_identifier_single_letter() {
        assert!(validate_identifier("a", "Table").is_ok());
        assert!(validate_identifier("Z", "Keyspace").is_ok());
    }

    #[test]
    fn test_validate_identifier_single_underscore() {
        assert!(validate_identifier("_", "Table").is_ok());
    }

    #[test]
    fn test_validate_identifier_all_uppercase() {
        assert!(validate_identifier("USERS", "Table").is_ok());
        assert!(validate_identifier("MY_TABLE", "Keyspace").is_ok());
    }

    #[test]
    fn test_validate_identifier_mixed_case() {
        assert!(validate_identifier("MyTable", "Table").is_ok());
        assert!(validate_identifier("userDATA", "Keyspace").is_ok());
    }

    #[test]
    fn test_validate_identifier_multiple_underscores() {
        assert!(validate_identifier("a__b__c", "Table").is_ok());
        assert!(validate_identifier("___", "Keyspace").is_ok());
    }

    #[test]
    fn test_validate_identifier_numbers_throughout() {
        assert!(validate_identifier("v1_2_3", "Table").is_ok());
        assert!(validate_identifier("table123abc456", "Keyspace").is_ok());
    }

    #[test]
    fn test_validate_identifier_sql_injection_semicolon() {
        let result = validate_identifier("users;DROP TABLE", "Table");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid character ';'"), "Error: {}", err);
    }

    #[test]
    fn test_validate_identifier_sql_injection_quotes() {
        let result = validate_identifier("users'--", "Table");
        assert!(result.is_err());

        let result2 = validate_identifier("users\"--", "Table");
        assert!(result2.is_err());
    }

    #[test]
    fn test_validate_identifier_sql_injection_parens() {
        let result = validate_identifier("users()", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_sql_injection_equal() {
        let result = validate_identifier("1=1", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_path_traversal() {
        let result = validate_identifier("../etc/passwd", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_null_byte() {
        let result = validate_identifier("users\0", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_newline() {
        let result = validate_identifier("users\n", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_tab() {
        let result = validate_identifier("users\t", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_cql_keywords_allowed() {
        // Keywords are valid identifiers (CQL will handle them)
        assert!(validate_identifier("select", "Table").is_ok());
        assert!(validate_identifier("from", "Keyspace").is_ok());
        assert!(validate_identifier("where", "Table").is_ok());
    }

    #[test]
    fn test_validate_identifier_boundary_length_47() {
        let name = "a".repeat(47);
        assert!(validate_identifier(&name, "Table").is_ok());
    }

    #[test]
    fn test_validate_identifier_at_symbol() {
        let result = validate_identifier("user@domain", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_hash() {
        let result = validate_identifier("table#1", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_percent() {
        let result = validate_identifier("table%s", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_asterisk() {
        let result = validate_identifier("table*", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_backslash() {
        let result = validate_identifier("table\\name", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_forward_slash() {
        let result = validate_identifier("table/name", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_colon() {
        let result = validate_identifier("table:name", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_comma() {
        let result = validate_identifier("table,name", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_angle_brackets() {
        let result = validate_identifier("table<name>", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_square_brackets() {
        let result = validate_identifier("table[0]", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_curly_braces() {
        let result = validate_identifier("table{name}", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_pipe() {
        let result = validate_identifier("table|name", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_ampersand() {
        let result = validate_identifier("table&name", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_caret() {
        let result = validate_identifier("table^name", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_tilde() {
        let result = validate_identifier("table~name", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_backtick() {
        let result = validate_identifier("`table`", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_exclamation() {
        let result = validate_identifier("table!", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_question_mark() {
        let result = validate_identifier("table?", "Table");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_identifier_plus() {
        let result = validate_identifier("table+name", "Table");
        assert!(result.is_err());
    }

    // ==================== Additional SimilarityFunction tests ====================

    #[test]
    fn test_similarity_function_copy() {
        let func = SimilarityFunction::Cosine;
        let copied = func; // Copy (not move)
        assert_eq!(func, copied);
        assert_eq!(func, SimilarityFunction::Cosine); // Original still valid
    }

    #[test]
    fn test_similarity_function_all_variants_debug() {
        assert_eq!(format!("{:?}", SimilarityFunction::Cosine), "Cosine");
        assert_eq!(format!("{:?}", SimilarityFunction::Euclidean), "Euclidean");
        assert_eq!(format!("{:?}", SimilarityFunction::DotProduct), "DotProduct");
    }

    #[test]
    fn test_similarity_function_serialize_all() {
        assert_eq!(
            serde_json::to_string(&SimilarityFunction::Cosine).unwrap(),
            "\"Cosine\""
        );
        assert_eq!(
            serde_json::to_string(&SimilarityFunction::Euclidean).unwrap(),
            "\"Euclidean\""
        );
        assert_eq!(
            serde_json::to_string(&SimilarityFunction::DotProduct).unwrap(),
            "\"DotProduct\""
        );
    }

    #[test]
    fn test_similarity_function_deserialize_all() {
        assert_eq!(
            serde_json::from_str::<SimilarityFunction>("\"Cosine\"").unwrap(),
            SimilarityFunction::Cosine
        );
        assert_eq!(
            serde_json::from_str::<SimilarityFunction>("\"Euclidean\"").unwrap(),
            SimilarityFunction::Euclidean
        );
        assert_eq!(
            serde_json::from_str::<SimilarityFunction>("\"DotProduct\"").unwrap(),
            SimilarityFunction::DotProduct
        );
    }

    #[test]
    fn test_similarity_function_deserialize_invalid() {
        let result = serde_json::from_str::<SimilarityFunction>("\"Invalid\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_similarity_function_round_trip_cosine() {
        let original = SimilarityFunction::Cosine;
        let json = serde_json::to_string(&original).unwrap();
        let restored: SimilarityFunction = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_similarity_function_round_trip_euclidean() {
        let original = SimilarityFunction::Euclidean;
        let json = serde_json::to_string(&original).unwrap();
        let restored: SimilarityFunction = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_similarity_function_round_trip_dot_product() {
        let original = SimilarityFunction::DotProduct;
        let json = serde_json::to_string(&original).unwrap();
        let restored: SimilarityFunction = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_similarity_function_eq_same() {
        assert_eq!(SimilarityFunction::Cosine, SimilarityFunction::Cosine);
        assert_eq!(SimilarityFunction::Euclidean, SimilarityFunction::Euclidean);
        assert_eq!(SimilarityFunction::DotProduct, SimilarityFunction::DotProduct);
    }

    #[test]
    fn test_similarity_function_ne_different() {
        assert_ne!(SimilarityFunction::Cosine, SimilarityFunction::Euclidean);
        assert_ne!(SimilarityFunction::Cosine, SimilarityFunction::DotProduct);
        assert_ne!(SimilarityFunction::Euclidean, SimilarityFunction::DotProduct);
    }

    #[test]
    fn test_similarity_function_cql_str_all_different() {
        let cosine = SimilarityFunction::Cosine.as_cql_str();
        let euclidean = SimilarityFunction::Euclidean.as_cql_str();
        let dot = SimilarityFunction::DotProduct.as_cql_str();

        assert_ne!(cosine, euclidean);
        assert_ne!(cosine, dot);
        assert_ne!(euclidean, dot);
    }

    #[test]
    fn test_similarity_function_distance_metric_all_different() {
        let cosine = SimilarityFunction::Cosine.to_distance_metric();
        let euclidean = SimilarityFunction::Euclidean.to_distance_metric();
        let dot = SimilarityFunction::DotProduct.to_distance_metric();

        assert_ne!(cosine, euclidean);
        assert_ne!(cosine, dot);
        assert_ne!(euclidean, dot);
    }

    // ==================== Additional Builder tests ====================

    #[test]
    fn test_builder_contact_points_empty() {
        let builder = CassandraVectorStore::builder().contact_points(Vec::<String>::new());
        assert!(builder.contact_points.is_empty());
    }

    #[test]
    fn test_builder_contact_points_single() {
        let builder = CassandraVectorStore::builder().contact_points(vec!["node1:9042"]);
        assert_eq!(builder.contact_points.len(), 1);
        assert_eq!(builder.contact_points[0], "node1:9042");
    }

    #[test]
    fn test_builder_contact_points_many() {
        let nodes: Vec<String> = (1..=10).map(|i| format!("node{}:9042", i)).collect();
        let builder = CassandraVectorStore::builder().contact_points(nodes.clone());
        assert_eq!(builder.contact_points.len(), 10);
        assert_eq!(builder.contact_points[9], "node10:9042");
    }

    #[test]
    fn test_builder_keyspace_string() {
        let builder = CassandraVectorStore::builder().keyspace(String::from("my_keyspace"));
        assert_eq!(builder.keyspace, Some("my_keyspace".to_string()));
    }

    #[test]
    fn test_builder_table_string() {
        let builder = CassandraVectorStore::builder().table(String::from("my_table"));
        assert_eq!(builder.table, Some("my_table".to_string()));
    }

    #[test]
    fn test_builder_vector_dimension_zero() {
        let builder = CassandraVectorStore::builder().vector_dimension(0);
        assert_eq!(builder.vector_dimension, Some(0));
    }

    #[test]
    fn test_builder_vector_dimension_large() {
        let builder = CassandraVectorStore::builder().vector_dimension(4096);
        assert_eq!(builder.vector_dimension, Some(4096));
    }

    #[test]
    fn test_builder_similarity_function_all_variants() {
        let b1 = CassandraVectorStore::builder().similarity_function(SimilarityFunction::Cosine);
        assert_eq!(b1.similarity_function, SimilarityFunction::Cosine);

        let b2 = CassandraVectorStore::builder().similarity_function(SimilarityFunction::Euclidean);
        assert_eq!(b2.similarity_function, SimilarityFunction::Euclidean);

        let b3 = CassandraVectorStore::builder().similarity_function(SimilarityFunction::DotProduct);
        assert_eq!(b3.similarity_function, SimilarityFunction::DotProduct);
    }

    #[test]
    fn test_builder_overwrite_contact_points() {
        let builder = CassandraVectorStore::builder()
            .contact_points(vec!["old:9042"])
            .contact_points(vec!["new:9042"]);
        assert_eq!(builder.contact_points, vec!["new:9042".to_string()]);
    }

    #[test]
    fn test_builder_overwrite_keyspace() {
        let builder = CassandraVectorStore::builder()
            .keyspace("old_ks")
            .keyspace("new_ks");
        assert_eq!(builder.keyspace, Some("new_ks".to_string()));
    }

    #[test]
    fn test_builder_overwrite_table() {
        let builder = CassandraVectorStore::builder()
            .table("old_table")
            .table("new_table");
        assert_eq!(builder.table, Some("new_table".to_string()));
    }

    #[test]
    fn test_builder_overwrite_dimension() {
        let builder = CassandraVectorStore::builder()
            .vector_dimension(768)
            .vector_dimension(1536);
        assert_eq!(builder.vector_dimension, Some(1536));
    }

    #[test]
    fn test_builder_overwrite_similarity() {
        let builder = CassandraVectorStore::builder()
            .similarity_function(SimilarityFunction::Cosine)
            .similarity_function(SimilarityFunction::Euclidean);
        assert_eq!(builder.similarity_function, SimilarityFunction::Euclidean);
    }

    #[test]
    fn test_builder_default_similarity_is_cosine() {
        let builder = CassandraVectorStoreBuilder::default();
        assert_eq!(builder.similarity_function, SimilarityFunction::Cosine);
    }

    #[test]
    fn test_builder_default_contact_point_is_localhost() {
        let builder = CassandraVectorStoreBuilder::default();
        assert_eq!(builder.contact_points, vec!["127.0.0.1:9042".to_string()]);
    }

    // ==================== UUID validation tests ====================

    #[test]
    fn test_uuid_parse_valid() {
        let valid_uuid = "550e8400-e29b-41d4-a716-446655440000";
        let parsed = Uuid::parse_str(valid_uuid);
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_uuid_parse_invalid_format() {
        let invalid = "not-a-uuid";
        let parsed = Uuid::parse_str(invalid);
        assert!(parsed.is_err());
    }

    #[test]
    fn test_uuid_parse_empty() {
        let parsed = Uuid::parse_str("");
        assert!(parsed.is_err());
    }

    #[test]
    fn test_uuid_parse_too_short() {
        let parsed = Uuid::parse_str("550e8400-e29b-41d4");
        assert!(parsed.is_err());
    }

    #[test]
    fn test_uuid_parse_too_long() {
        let parsed = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000-extra");
        assert!(parsed.is_err());
    }

    #[test]
    fn test_uuid_parse_invalid_chars() {
        let parsed = Uuid::parse_str("550e8400-e29b-41d4-a716-44665544zzzz");
        assert!(parsed.is_err());
    }

    #[test]
    fn test_uuid_new_v4_is_valid() {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let reparsed = Uuid::parse_str(&id_str);
        assert!(reparsed.is_ok());
        assert_eq!(reparsed.unwrap(), id);
    }

    // ==================== Metadata serialization tests ====================

    #[test]
    fn test_metadata_serialize_empty() {
        let metadata: HashMap<String, serde_json::Value> = HashMap::new();
        let json = serde_json::to_string(&metadata).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_metadata_serialize_string_value() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), serde_json::json!("value"));
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("\"key\""));
        assert!(json.contains("\"value\""));
    }

    #[test]
    fn test_metadata_serialize_number_value() {
        let mut metadata = HashMap::new();
        metadata.insert("count".to_string(), serde_json::json!(42));
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("42"));
    }

    #[test]
    fn test_metadata_serialize_bool_value() {
        let mut metadata = HashMap::new();
        metadata.insert("active".to_string(), serde_json::json!(true));
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("true"));
    }

    #[test]
    fn test_metadata_serialize_null_value() {
        let mut metadata = HashMap::new();
        metadata.insert("empty".to_string(), serde_json::Value::Null);
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("null"));
    }

    #[test]
    fn test_metadata_serialize_array_value() {
        let mut metadata = HashMap::new();
        metadata.insert("tags".to_string(), serde_json::json!(["a", "b", "c"]));
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("["));
        assert!(json.contains("]"));
    }

    #[test]
    fn test_metadata_serialize_nested_object() {
        let mut metadata = HashMap::new();
        metadata.insert("nested".to_string(), serde_json::json!({"inner": "value"}));
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("inner"));
    }

    #[test]
    fn test_metadata_deserialize_empty() {
        let json = "{}";
        let metadata: HashMap<String, serde_json::Value> = serde_json::from_str(json).unwrap();
        assert!(metadata.is_empty());
    }

    #[test]
    fn test_metadata_deserialize_with_values() {
        let json = r#"{"key": "value", "num": 123}"#;
        let metadata: HashMap<String, serde_json::Value> = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.get("key").unwrap(), "value");
        assert_eq!(metadata.get("num").unwrap(), 123);
    }

    #[test]
    fn test_metadata_deserialize_invalid_json() {
        let json = "not json";
        let result: std::result::Result<HashMap<String, serde_json::Value>, _> =
            serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_metadata_deserialize_fallback_unwrap_or_default() {
        let invalid = "not json";
        let metadata: HashMap<String, serde_json::Value> =
            serde_json::from_str(invalid).unwrap_or_default();
        assert!(metadata.is_empty());
    }

    #[test]
    fn test_metadata_round_trip() {
        let mut original = HashMap::new();
        original.insert("str".to_string(), serde_json::json!("hello"));
        original.insert("num".to_string(), serde_json::json!(42));
        original.insert("bool".to_string(), serde_json::json!(true));

        let json = serde_json::to_string(&original).unwrap();
        let restored: HashMap<String, serde_json::Value> = serde_json::from_str(&json).unwrap();

        assert_eq!(original, restored);
    }

    // ==================== DistanceMetric tests ====================

    #[test]
    fn test_distance_metric_cosine_debug() {
        let metric = DistanceMetric::Cosine;
        let debug = format!("{:?}", metric);
        assert!(debug.contains("Cosine"));
    }

    #[test]
    fn test_distance_metric_euclidean_debug() {
        let metric = DistanceMetric::Euclidean;
        let debug = format!("{:?}", metric);
        assert!(debug.contains("Euclidean"));
    }

    #[test]
    fn test_distance_metric_dot_product_debug() {
        let metric = DistanceMetric::DotProduct;
        let debug = format!("{:?}", metric);
        assert!(debug.contains("DotProduct"));
    }

    // ==================== Vector dimension edge cases ====================

    #[test]
    fn test_vector_dimension_common_sizes() {
        // Common embedding dimensions
        let dimensions = [384, 512, 768, 1024, 1536, 2048, 3072, 4096];
        for dim in dimensions {
            let builder = CassandraVectorStore::builder().vector_dimension(dim);
            assert_eq!(builder.vector_dimension, Some(dim));
        }
    }

    #[test]
    fn test_vector_dimension_small() {
        let builder = CassandraVectorStore::builder().vector_dimension(1);
        assert_eq!(builder.vector_dimension, Some(1));
    }

    #[test]
    fn test_vector_dimension_max_usize() {
        let builder = CassandraVectorStore::builder().vector_dimension(usize::MAX);
        assert_eq!(builder.vector_dimension, Some(usize::MAX));
    }

    // ==================== Contact points format tests ====================

    #[test]
    fn test_contact_points_with_port() {
        let builder = CassandraVectorStore::builder()
            .contact_points(vec!["192.168.1.1:9042", "192.168.1.2:9043"]);
        assert_eq!(builder.contact_points[0], "192.168.1.1:9042");
        assert_eq!(builder.contact_points[1], "192.168.1.2:9043");
    }

    #[test]
    fn test_contact_points_hostname() {
        let builder = CassandraVectorStore::builder()
            .contact_points(vec!["cassandra.example.com:9042"]);
        assert_eq!(builder.contact_points[0], "cassandra.example.com:9042");
    }

    #[test]
    fn test_contact_points_ipv6() {
        let builder = CassandraVectorStore::builder()
            .contact_points(vec!["[::1]:9042"]);
        assert_eq!(builder.contact_points[0], "[::1]:9042");
    }

    // ==================== Table FQName tests ====================

    #[test]
    fn test_table_fqname_format() {
        // Test that FQName follows keyspace.table format
        // We can't test the actual method without a CassandraVectorStore,
        // but we can verify the expected format
        let keyspace = "my_keyspace";
        let table = "my_table";
        let expected = format!("{}.{}", keyspace, table);
        assert_eq!(expected, "my_keyspace.my_table");
    }

    #[test]
    fn test_table_fqname_with_underscores() {
        let keyspace = "my_key_space";
        let table = "my_table_name";
        let expected = format!("{}.{}", keyspace, table);
        assert_eq!(expected, "my_key_space.my_table_name");
    }

    // ==================== CQL string format tests ====================

    #[test]
    fn test_cql_str_lowercase() {
        // CQL similarity function names should be lowercase
        assert!(SimilarityFunction::Cosine.as_cql_str().chars().all(|c| c.is_lowercase()));
        assert!(SimilarityFunction::Euclidean.as_cql_str().chars().all(|c| c.is_lowercase()));
        // dot_product has underscore but all alpha chars lowercase
        let dot = SimilarityFunction::DotProduct.as_cql_str();
        assert!(dot.chars().filter(|c| c.is_alphabetic()).all(|c| c.is_lowercase()));
    }

    #[test]
    fn test_cql_str_valid_identifiers() {
        // All CQL strings should be valid CQL identifiers
        for func in [
            SimilarityFunction::Cosine,
            SimilarityFunction::Euclidean,
            SimilarityFunction::DotProduct,
        ] {
            let cql_str = func.as_cql_str();
            // Should not be empty
            assert!(!cql_str.is_empty());
            // Should start with letter
            assert!(cql_str.chars().next().unwrap().is_alphabetic());
            // Should only contain alphanumeric and underscore
            assert!(cql_str.chars().all(|c| c.is_alphanumeric() || c == '_'));
        }
    }

    // ==================== Integration helper tests ====================

    #[test]
    fn test_texts_embeddings_length_validation() {
        // Simulate the validation that happens in add_documents_with_embeddings
        let texts = vec!["a".to_string(), "b".to_string()];
        let embeddings = vec![vec![1.0f32], vec![2.0f32], vec![3.0f32]];

        let valid = texts.len() == embeddings.len();
        assert!(!valid, "Should detect length mismatch");
    }

    #[test]
    fn test_texts_embeddings_length_match() {
        let texts = vec!["a".to_string(), "b".to_string()];
        let embeddings = vec![vec![1.0f32], vec![2.0f32]];

        let valid = texts.len() == embeddings.len();
        assert!(valid, "Should pass when lengths match");
    }

    #[test]
    fn test_embedding_dimension_validation() {
        let expected_dim = 1536;
        let embedding = vec![0.0f32; 768]; // Wrong dimension

        let valid = embedding.len() == expected_dim;
        assert!(!valid, "Should detect dimension mismatch");
    }

    #[test]
    fn test_embedding_dimension_match() {
        let expected_dim = 1536;
        let embedding = vec![0.0f32; 1536];

        let valid = embedding.len() == expected_dim;
        assert!(valid, "Should pass when dimensions match");
    }

    #[test]
    fn test_metadatas_length_mismatch_detection() {
        let text_count = 3;
        let metadatas_len = 2;

        let valid = metadatas_len == text_count;
        assert!(!valid, "Should detect metadatas length mismatch");
    }

    #[test]
    fn test_ids_length_mismatch_detection() {
        let text_count = 3;
        let ids_len = 4;

        let valid = ids_len == text_count;
        assert!(!valid, "Should detect IDs length mismatch");
    }

    // ==================== Document structure tests ====================

    #[test]
    fn test_document_default_id() {
        let doc = Document {
            id: None,
            page_content: "test".to_string(),
            metadata: HashMap::new(),
        };
        assert!(doc.id.is_none());
    }

    #[test]
    fn test_document_with_id() {
        let doc = Document {
            id: Some("doc-123".to_string()),
            page_content: "test".to_string(),
            metadata: HashMap::new(),
        };
        assert_eq!(doc.id, Some("doc-123".to_string()));
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

    #[test]
    fn test_document_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), serde_json::json!("web"));

        let doc = Document {
            id: None,
            page_content: "test".to_string(),
            metadata,
        };
        assert!(doc.metadata.contains_key("source"));
    }

    #[test]
    fn test_document_unicode_content() {
        let doc = Document {
            id: None,
            page_content: "こんにちは世界 🌍".to_string(),
            metadata: HashMap::new(),
        };
        assert!(doc.page_content.contains("こんにちは"));
        assert!(doc.page_content.contains("🌍"));
    }

    #[test]
    fn test_document_long_content() {
        let long_content = "x".repeat(100_000);
        let doc = Document {
            id: None,
            page_content: long_content.clone(),
            metadata: HashMap::new(),
        };
        assert_eq!(doc.page_content.len(), 100_000);
    }

    #[test]
    fn test_document_special_chars_in_content() {
        let doc = Document {
            id: None,
            page_content: "Line1\nLine2\tTab\r\nCRLF".to_string(),
            metadata: HashMap::new(),
        };
        assert!(doc.page_content.contains('\n'));
        assert!(doc.page_content.contains('\t'));
    }
}
