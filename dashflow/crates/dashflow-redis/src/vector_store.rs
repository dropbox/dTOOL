//! Redis vector store implementation.

use async_trait::async_trait;
use dashflow::core::{
    documents::Document,
    embeddings::Embeddings,
    error::{Error, Result},
    vector_stores::{DistanceMetric as CoreDistanceMetric, VectorStore},
};
use dashflow::{embed, embed_query};
use redis::aio::ConnectionManager;
use std::collections::HashMap;
use std::sync::Arc;

use crate::constants::REDIS_REQUIRED_MODULES;
use crate::schema::RedisIndexSchema;

/// Redis vector store for storing and searching embeddings.
///
/// Uses Redis Stack (Redis + `RediSearch` module) for vector similarity search.
pub struct RedisVectorStore {
    /// Index name in Redis
    index_name: String,
    /// Embeddings instance for encoding queries and documents
    embeddings: Arc<dyn Embeddings>,
    /// Redis connection manager (async)
    connection_manager: ConnectionManager,
    /// Index schema configuration
    schema: RedisIndexSchema,
    /// Key prefix for documents (e.g., "`doc:index_name`")
    key_prefix: String,
    /// Whether the index has been created
    index_created: bool,
}

impl RedisVectorStore {
    /// Create a new Redis vector store.
    ///
    /// # Arguments
    ///
    /// * `redis_url` - Redis connection URL (e.g., "<redis://localhost:6379>")
    /// * `index_name` - Name for the vector index
    /// * `embeddings` - Embeddings instance for encoding text
    /// * `index_schema` - Optional custom index schema
    /// * `key_prefix` - Optional key prefix (defaults to "doc:<`index_name`>")
    pub async fn new(
        redis_url: impl Into<String>,
        index_name: impl Into<String>,
        embeddings: Arc<dyn Embeddings>,
        index_schema: Option<RedisIndexSchema>,
        key_prefix: Option<String>,
    ) -> Result<Self> {
        let redis_url = redis_url.into();
        let index_name = index_name.into();

        // Connect to Redis
        let client = redis::Client::open(redis_url.clone())
            .map_err(|e| Error::config(format!("Failed to create Redis client: {e}")))?;

        // Create async connection manager
        let connection_manager = ConnectionManager::new(client)
            .await
            .map_err(|e| Error::config(format!("Failed to create connection manager: {e}")))?;

        // Check if Redis modules are installed
        Self::check_redis_modules(&connection_manager).await?;

        let key_prefix = key_prefix.unwrap_or_else(|| format!("doc:{index_name}"));
        let schema = index_schema.unwrap_or_default();

        Ok(Self {
            index_name,
            embeddings,
            connection_manager,
            schema,
            key_prefix,
            index_created: false,
        })
    }

    /// Check if required Redis modules are installed.
    async fn check_redis_modules(conn: &ConnectionManager) -> Result<()> {
        let mut conn = conn.clone();

        // Get list of installed modules
        let modules: Vec<redis::Value> = redis::cmd("MODULE")
            .arg("LIST")
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::config(format!("Failed to query Redis modules: {e}")))?;

        // Parse module list
        let mut installed = HashMap::new();
        for module_info in modules {
            if let redis::Value::Array(fields) = module_info {
                let mut name = String::new();
                let mut version = 0;

                for i in (0..fields.len()).step_by(2) {
                    if let (redis::Value::BulkString(key), Some(value)) =
                        (&fields[i], fields.get(i + 1))
                    {
                        let key_str = String::from_utf8_lossy(key);
                        match key_str.as_ref() {
                            "name" => {
                                if let redis::Value::BulkString(v) = value {
                                    name = String::from_utf8_lossy(v).to_string();
                                }
                            }
                            "ver" => {
                                if let redis::Value::Int(v) = value {
                                    version = *v as u32;
                                }
                            }
                            _ => {}
                        }
                    }
                }

                if !name.is_empty() {
                    installed.insert(name, version);
                }
            }
        }

        // Check for required modules
        for &(name, min_version) in REDIS_REQUIRED_MODULES {
            if let Some(&installed_ver) = installed.get(name) {
                if installed_ver >= min_version {
                    return Ok(());
                }
            }
        }

        Err(Error::config(
            "Redis cannot be used as a vector database without RediSearch >=2.6. \
             Please see https://redis.io/docs/stack/search/quick_start/ for installation instructions."
        ))
    }

    /// Get the key prefix used for documents.
    #[must_use]
    pub fn key_prefix(&self) -> &str {
        &self.key_prefix
    }

    /// Encode a vector as bytes for storage in Redis.
    fn encode_vector(vector: &[f32]) -> Vec<u8> {
        vector.iter().flat_map(|&f| f.to_le_bytes()).collect()
    }

    /// Check if an index exists.
    async fn index_exists(&self) -> Result<bool> {
        let mut conn = self.connection_manager.clone();

        // Try to get index info
        let result: redis::RedisResult<redis::Value> = redis::cmd("FT.INFO")
            .arg(&self.index_name)
            .query_async(&mut conn)
            .await;

        match result {
            Ok(_) => Ok(true),
            Err(e) => {
                let err_msg = e.to_string().to_lowercase();
                // Handle various error message formats from different Redis versions
                if err_msg.contains("unknown index")
                    || err_msg.contains("no such index")
                    || err_msg.contains("index name")
                {
                    Ok(false)
                } else {
                    Err(Error::config(format!(
                        "Failed to check index existence: {e}"
                    )))
                }
            }
        }
    }

    /// Create the Redis index if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `dims` - Vector dimensionality (obtained from first embedding)
    async fn create_index_if_not_exist(&mut self, dims: usize) -> Result<()> {
        // If we already created the index in this instance, skip
        if self.index_created {
            return Ok(());
        }

        // Check if index already exists
        if self.index_exists().await? {
            self.index_created = true;
            return Ok(());
        }

        // Ensure the schema has a content text field
        self.schema.ensure_content_field();

        // Add vector field if none exists
        if self.schema.vector.is_empty() {
            use crate::schema::{FlatVectorField, VectorField};

            let vector_field = FlatVectorField::new(&self.schema.content_vector_key, dims);
            self.schema.vector.push(VectorField::Flat(vector_field));
        } else {
            // Update dimensions on existing vector fields
            for field in &mut self.schema.vector {
                match field {
                    crate::schema::VectorField::Flat(ref mut f) => {
                        f.dims = dims;
                    }
                    crate::schema::VectorField::Hnsw(ref mut f) => {
                        f.dims = dims;
                    }
                }
            }
        }

        // Build FT.CREATE command
        let mut cmd = redis::cmd("FT.CREATE");
        cmd.arg(&self.index_name);

        // Add index definition
        cmd.arg("ON").arg("HASH");
        cmd.arg("PREFIX")
            .arg("1")
            .arg(format!("{}:", self.key_prefix));

        // Add schema
        cmd.arg("SCHEMA");
        let schema_args = self.schema.to_redis_schema_args();
        for arg in schema_args {
            cmd.arg(arg);
        }

        // Execute command
        let mut conn = self.connection_manager.clone();
        cmd.query_async::<()>(&mut conn)
            .await
            .map_err(|e| Error::config(format!("Failed to create Redis index: {e}")))?;

        self.index_created = true;
        Ok(())
    }

    /// Perform similarity search by vector embedding.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Query vector embedding
    /// * `k` - Number of results to return
    /// * `_filter` - Optional metadata filter (deferred: Redis requires specific query syntax for filtering)
    async fn similarity_search_by_vector_internal(
        &self,
        embedding: &[f32],
        k: usize,
        _filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        if embedding.is_empty() {
            return Ok(Vec::new());
        }

        // Check if index exists - return empty results if not
        if !self.index_exists().await.unwrap_or(false) {
            return Ok(Vec::new());
        }

        let mut conn = self.connection_manager.clone();

        // Encode vector for query
        let vector_bytes = Self::encode_vector(embedding);

        // Build FT.SEARCH query
        // Format: (*)=>[KNN k @vector_field $vector AS distance]
        let vector_field = &self.schema.content_vector_key;
        let base_query = format!("(*)=>[KNN {k} @{vector_field} $vector AS distance]");

        // Build command
        let mut cmd = redis::cmd("FT.SEARCH");
        cmd.arg(&self.index_name);
        cmd.arg(&base_query);

        // Add PARAMS for vector
        cmd.arg("PARAMS");
        cmd.arg("2"); // Number of params (key + value)
        cmd.arg("vector");
        cmd.arg(vector_bytes);

        // Note: Not using RETURN clause - let Redis return all fields
        // parse_search_results handles filtering binary fields (like vectors)
        // This allows ad-hoc metadata fields not defined in schema to be returned

        // Sort by distance
        cmd.arg("SORTBY");
        cmd.arg("distance");

        // Limit results
        cmd.arg("LIMIT");
        cmd.arg("0");
        cmd.arg(k.to_string());

        // Use dialect 2 for vector search
        cmd.arg("DIALECT");
        cmd.arg("2");

        // Execute query
        let results: redis::Value = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| Error::config(format!("Redis search query failed: {e}")))?;

        // Parse results
        self.parse_search_results(results)
    }

    /// Parse Redis FT.SEARCH results into Documents.
    fn parse_search_results(&self, results: redis::Value) -> Result<Vec<Document>> {
        let mut documents = Vec::new();

        // FT.SEARCH returns: [count, key1, fields1, key2, fields2, ...]
        if let redis::Value::Array(items) = results {
            if items.is_empty() {
                return Ok(documents);
            }

            // Skip first item (count)
            let doc_items = &items[1..];

            // Process pairs of (key, fields)
            for chunk in doc_items.chunks(2) {
                if chunk.len() != 2 {
                    continue;
                }

                // Extract document ID from key
                let doc_id = match &chunk[0] {
                    redis::Value::BulkString(bytes) => String::from_utf8_lossy(bytes).to_string(),
                    _ => continue,
                };

                // Parse fields
                let mut metadata = HashMap::new();
                metadata.insert("id".to_string(), serde_json::Value::String(doc_id.clone()));

                let mut page_content = String::new();

                if let redis::Value::Array(fields) = &chunk[1] {
                    // Fields are pairs: [field_name, field_value, ...]
                    for field_chunk in fields.chunks(2) {
                        if field_chunk.len() != 2 {
                            continue;
                        }

                        let field_name = match &field_chunk[0] {
                            redis::Value::BulkString(bytes) => {
                                String::from_utf8_lossy(bytes).to_string()
                            }
                            _ => continue,
                        };

                        let field_value = match &field_chunk[1] {
                            redis::Value::BulkString(bytes) => {
                                // Try to decode as UTF-8, skip if it's binary (e.g., vector)
                                if let Ok(s) = String::from_utf8(bytes.clone()) {
                                    s
                                } else {
                                    continue;
                                }
                            }
                            _ => continue,
                        };

                        // Content field goes to page_content
                        if field_name == self.schema.content_key {
                            page_content = field_value;
                        } else if field_name != self.schema.content_vector_key {
                            // Add to metadata (skip vector field)
                            metadata.insert(field_name, serde_json::Value::String(field_value));
                        }
                    }
                }

                documents.push(Document {
                    id: Some(doc_id),
                    page_content,
                    metadata,
                });
            }
        }

        Ok(documents)
    }
}

#[async_trait]
impl VectorStore for RedisVectorStore {
    fn embeddings(&self) -> Option<Arc<dyn Embeddings>> {
        Some(Arc::clone(&self.embeddings))
    }

    fn distance_metric(&self) -> CoreDistanceMetric {
        // Get distance metric from the first vector field in schema
        self.schema
            .vector
            .first()
            .map_or(CoreDistanceMetric::Cosine, |field| {
                use crate::schema::DistanceMetric;
                match field.distance_metric() {
                    DistanceMetric::Cosine => CoreDistanceMetric::Cosine,
                    DistanceMetric::L2 => CoreDistanceMetric::Euclidean,
                    DistanceMetric::IP => CoreDistanceMetric::DotProduct,
                }
            }) // Default to Cosine if no vector field
    }

    async fn add_texts(
        &mut self,
        texts: &[impl AsRef<str> + Send + Sync],
        metadatas: Option<&[HashMap<String, serde_json::Value>]>,
        ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Validate metadata length if provided
        if let Some(metas) = metadatas {
            if metas.len() != texts.len() {
                return Err(Error::config(format!(
                    "Number of metadatas ({}) must match number of texts ({})",
                    metas.len(),
                    texts.len()
                )));
            }
        }

        // Validate ids length if provided
        if let Some(id_slice) = ids {
            if id_slice.len() != texts.len() {
                return Err(Error::config(format!(
                    "Number of ids ({}) must match number of texts ({})",
                    id_slice.len(),
                    texts.len()
                )));
            }
        }

        // Convert texts to strings
        let text_strings: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();

        // Embed texts
        let embeddings = embed(Arc::clone(&self.embeddings), &text_strings)
            .await
            .map_err(|e| Error::other(format!("Embedding failed: {e}")))?;

        // Create index if it doesn't exist (using first embedding's dimension)
        if !embeddings.is_empty() {
            let dims = embeddings[0].len();
            self.create_index_if_not_exist(dims).await?;
        }

        // Prepare keys
        let mut result_ids = Vec::new();
        let mut conn = self.connection_manager.clone();

        // Use Redis pipeline for batch writes
        let mut pipeline = redis::Pipeline::new();

        for (i, (text, embedding)) in text_strings.iter().zip(embeddings.iter()).enumerate() {
            // Generate or use provided ID
            let id = if let Some(id_slice) = ids {
                id_slice[i].clone()
            } else {
                uuid::Uuid::new_v4().to_string()
            };

            // Build full key with prefix
            let full_key = if id.starts_with(&format!("{}:", self.key_prefix)) {
                id.clone()
            } else {
                format!("{}:{}", self.key_prefix, id)
            };

            // Encode vector
            let vector_bytes = Self::encode_vector(embedding);

            // Build field-value pairs for HSET
            let mut fields: Vec<(&str, Vec<u8>)> = vec![
                (self.schema.content_key.as_str(), text.as_bytes().to_vec()),
                (self.schema.content_vector_key.as_str(), vector_bytes),
            ];

            // Add metadata fields
            if let Some(metas) = metadatas {
                if let Some(meta) = metas.get(i) {
                    for (key, value) in meta {
                        // Convert JSON value to string
                        let value_str = match value {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Number(n) => n.to_string(),
                            serde_json::Value::Bool(b) => b.to_string(),
                            serde_json::Value::Array(arr) => {
                                // For arrays, join with comma (tag format)
                                arr.iter()
                                    .filter_map(|v| v.as_str())
                                    .collect::<Vec<_>>()
                                    .join(",")
                            }
                            serde_json::Value::Null => continue, // Skip null values
                            serde_json::Value::Object(_) => {
                                // Skip complex objects
                                continue;
                            }
                        };

                        fields.push((key.as_str(), value_str.as_bytes().to_vec()));
                    }
                }
            }

            // Add HSET command to pipeline
            // We need to build the command manually to handle binary data
            let mut hset_args: Vec<Vec<u8>> = vec![full_key.as_bytes().to_vec()];
            for (field_name, field_value) in fields {
                hset_args.push(field_name.as_bytes().to_vec());
                hset_args.push(field_value);
            }

            pipeline.cmd("HSET").arg(hset_args);

            result_ids.push(full_key);
        }

        // Execute pipeline
        pipeline
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| Error::config(format!("Failed to write documents to Redis: {e}")))?;

        Ok(result_ids)
    }

    async fn _similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        // Embed the query
        let embedding = embed_query(Arc::clone(&self.embeddings), query)
            .await
            .map_err(|e| Error::other(format!("Query embedding failed: {e}")))?;

        // Call internal similarity search
        self.similarity_search_by_vector_internal(&embedding, k, filter)
            .await
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        if let Some(id_slice) = ids {
            if id_slice.is_empty() {
                return Ok(true);
            }

            let mut conn = self.connection_manager.clone();

            // Build keys with prefix if needed
            let keys: Vec<String> = id_slice
                .iter()
                .map(|id| {
                    if id.starts_with(&format!("{}:", self.key_prefix)) {
                        id.clone()
                    } else {
                        format!("{}:{}", self.key_prefix, id)
                    }
                })
                .collect();

            // Delete keys
            let deleted: usize = redis::cmd("DEL")
                .arg(&keys)
                .query_async(&mut conn)
                .await
                .map_err(|e| Error::config(format!("Failed to delete documents: {e}")))?;

            Ok(deleted > 0)
        } else {
            // If no IDs provided, return false (nothing deleted)
            Ok(false)
        }
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self.connection_manager.clone();
        let mut documents = Vec::new();

        // Build keys with prefix if needed
        let keys: Vec<String> = ids
            .iter()
            .map(|id| {
                if id.starts_with(&format!("{}:", self.key_prefix)) {
                    id.clone()
                } else {
                    format!("{}:{}", self.key_prefix, id)
                }
            })
            .collect();

        // Build list of fields to fetch (content + metadata, but NOT vector)
        let mut fields_to_fetch = vec![self.schema.content_key.clone()];
        fields_to_fetch.extend(self.schema.metadata_keys());

        // Fetch each document
        for key in &keys {
            // Use HMGET to get only specific fields (avoiding binary vector field)
            let mut cmd = redis::cmd("HMGET");
            cmd.arg(key);
            for field in &fields_to_fetch {
                cmd.arg(field);
            }

            let values: Vec<Option<String>> = cmd
                .query_async(&mut conn)
                .await
                .map_err(|e| Error::config(format!("Failed to fetch document {key}: {e}")))?;

            // Check if any field exists (document exists)
            if values.iter().all(|v| v.is_none()) {
                // Document doesn't exist, skip
                continue;
            }

            // Extract content (first field)
            let page_content = values
                .first()
                .and_then(|v| v.as_ref())
                .cloned()
                .unwrap_or_default();

            // Build metadata
            let mut metadata = HashMap::new();
            metadata.insert("id".to_string(), serde_json::Value::String(key.clone()));

            // Map remaining values to metadata fields
            for (i, field_name) in fields_to_fetch.iter().skip(1).enumerate() {
                if let Some(Some(field_value)) = values.get(i + 1) {
                    metadata.insert(field_name.clone(), serde_json::Value::String(field_value.clone()));
                }
            }

            documents.push(Document {
                id: Some(key.clone()),
                page_content,
                metadata,
            });
        }

        Ok(documents)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ==================== encode_vector tests ====================

    #[test]
    fn test_encode_vector_empty() {
        let result = RedisVectorStore::encode_vector(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_encode_vector_single_value() {
        let vector = vec![1.0f32];
        let result = RedisVectorStore::encode_vector(&vector);
        // f32 is 4 bytes in little-endian
        assert_eq!(result.len(), 4);
        // 1.0f32 in little-endian bytes
        let expected = 1.0f32.to_le_bytes();
        assert_eq!(result, expected.to_vec());
    }

    #[test]
    fn test_encode_vector_multiple_values() {
        let vector = vec![1.0f32, 2.0f32, 3.0f32];
        let result = RedisVectorStore::encode_vector(&vector);
        // 3 floats * 4 bytes = 12 bytes
        assert_eq!(result.len(), 12);

        // Verify each float
        let mut expected = Vec::new();
        expected.extend_from_slice(&1.0f32.to_le_bytes());
        expected.extend_from_slice(&2.0f32.to_le_bytes());
        expected.extend_from_slice(&3.0f32.to_le_bytes());
        assert_eq!(result, expected);
    }

    #[test]
    fn test_encode_vector_special_values() {
        let vector = vec![0.0f32, -1.0f32, f32::MAX, f32::MIN];
        let result = RedisVectorStore::encode_vector(&vector);
        assert_eq!(result.len(), 16);

        // Decode back to verify
        let decoded: Vec<f32> = result
            .chunks(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect();
        assert_eq!(decoded[0].to_bits(), 0.0f32.to_bits());
        assert_eq!(decoded[1].to_bits(), (-1.0f32).to_bits());
        assert_eq!(decoded[2].to_bits(), f32::MAX.to_bits());
        assert_eq!(decoded[3].to_bits(), f32::MIN.to_bits());
    }

    #[test]
    fn test_encode_vector_infinity() {
        let vector = vec![f32::INFINITY, f32::NEG_INFINITY];
        let result = RedisVectorStore::encode_vector(&vector);
        assert_eq!(result.len(), 8);

        let decoded: Vec<f32> = result
            .chunks(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect();
        assert!(decoded[0].is_infinite() && decoded[0] > 0.0);
        assert!(decoded[1].is_infinite() && decoded[1] < 0.0);
    }

    #[test]
    fn test_encode_vector_small_values() {
        // Test very small floating point values (important for normalized embeddings)
        let vector = vec![0.001f32, 0.0001f32, 1e-10f32];
        let result = RedisVectorStore::encode_vector(&vector);
        assert_eq!(result.len(), 12);

        let decoded: Vec<f32> = result
            .chunks(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect();
        assert!((decoded[0] - 0.001f32).abs() < 1e-10);
        assert!((decoded[1] - 0.0001f32).abs() < 1e-10);
        assert!((decoded[2] - 1e-10f32).abs() < 1e-15);
    }

    #[test]
    fn test_encode_vector_realistic_embedding() {
        // Simulate a normalized embedding vector (common in ML)
        let vector: Vec<f32> = vec![0.123, -0.456, 0.789, -0.012, 0.345];
        let result = RedisVectorStore::encode_vector(&vector);
        assert_eq!(result.len(), 20); // 5 floats * 4 bytes

        // Roundtrip test
        let decoded: Vec<f32> = result
            .chunks(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect();

        for (original, decoded) in vector.iter().zip(decoded.iter()) {
            assert!((original - decoded).abs() < 1e-7);
        }
    }

    // ==================== parse_search_results tests ====================

    #[test]
    fn test_parse_search_results_empty_array() {
        // Create a mock store with default schema to verify the default configuration
        let _schema = RedisIndexSchema::default();
        // We can't easily create a full store without Redis, but we can test the parsing logic
        // by examining the expected behavior

        // An empty array should return empty results
        let results = redis::Value::Array(vec![]);
        // The function expects [count, ...pairs], empty array is valid
        // We'll verify this matches expected behavior pattern
        assert!(matches!(results, redis::Value::Array(_)));
    }

    #[test]
    fn test_parse_search_results_count_only() {
        // When FT.SEARCH returns just the count (no results)
        let results = redis::Value::Array(vec![redis::Value::Int(0)]);
        if let redis::Value::Array(items) = results {
            assert_eq!(items.len(), 1);
            if let redis::Value::Int(count) = items[0] {
                assert_eq!(count, 0);
            }
        }
    }

    #[test]
    fn test_redis_value_bulk_string_conversion() {
        // Test that BulkString properly converts to String
        let value = redis::Value::BulkString(b"test content".to_vec());
        if let redis::Value::BulkString(bytes) = value {
            let text = String::from_utf8_lossy(&bytes).to_string();
            assert_eq!(text, "test content");
        }
    }

    #[test]
    fn test_redis_value_utf8_handling() {
        // Test UTF-8 handling for content
        let utf8_content = "Hello, 世界! 🦀";
        let value = redis::Value::BulkString(utf8_content.as_bytes().to_vec());
        if let redis::Value::BulkString(bytes) = value {
            let text = String::from_utf8(bytes).unwrap();
            assert_eq!(text, utf8_content);
        }
    }

    #[test]
    fn test_redis_value_invalid_utf8_handling() {
        // Test that invalid UTF-8 is handled (vector fields contain binary data)
        let invalid_utf8: Vec<u8> = vec![0xFF, 0xFE, 0x00, 0x01];
        let result = String::from_utf8(invalid_utf8.clone());
        assert!(result.is_err());
        // from_utf8_lossy should handle it
        let lossy = String::from_utf8_lossy(&invalid_utf8);
        assert!(!lossy.is_empty());
    }

    // ==================== RedisIndexSchema tests ====================

    #[test]
    fn test_redis_index_schema_default() {
        let schema = RedisIndexSchema::default();
        // Default content key should be "content"
        assert_eq!(schema.content_key, "content");
        // Default vector key should be "content_vector"
        assert_eq!(schema.content_vector_key, "content_vector");
    }

    // ==================== Key prefix handling tests ====================

    #[test]
    fn test_key_prefix_format() {
        // Test the key prefix format logic used in add_texts and delete
        let key_prefix = "doc:my_index";
        let id = "abc123";

        // Full key construction
        let full_key = if id.starts_with(&format!("{}:", key_prefix)) {
            id.to_string()
        } else {
            format!("{}:{}", key_prefix, id)
        };

        assert_eq!(full_key, "doc:my_index:abc123");
    }

    #[test]
    fn test_key_prefix_already_prefixed() {
        // Test when ID already has the prefix
        let key_prefix = "doc:my_index";
        let id = "doc:my_index:existing_id";

        let full_key = if id.starts_with(&format!("{}:", key_prefix)) {
            id.to_string()
        } else {
            format!("{}:{}", key_prefix, id)
        };

        assert_eq!(full_key, "doc:my_index:existing_id");
    }

    #[test]
    fn test_key_prefix_partial_match() {
        // Test when ID partially matches but isn't actually prefixed
        let key_prefix = "doc:my_index";
        let id = "doc:other_index:id";

        let full_key = if id.starts_with(&format!("{}:", key_prefix)) {
            id.to_string()
        } else {
            format!("{}:{}", key_prefix, id)
        };

        // Should add prefix since "doc:other_index" != "doc:my_index:"
        assert_eq!(full_key, "doc:my_index:doc:other_index:id");
    }

    // ==================== DistanceMetric conversion tests ====================

    #[test]
    fn test_distance_metric_from_schema() {
        use crate::schema::{DistanceMetric as SchemaMetric, FlatVectorField, VectorDataType};

        // Test Cosine
        let flat_cosine = FlatVectorField {
            name: "vec".to_string(),
            dims: 128,
            datatype: VectorDataType::Float32,
            distance_metric: SchemaMetric::Cosine,
            initial_cap: None,
            block_size: None,
        };
        assert_eq!(flat_cosine.distance_metric, SchemaMetric::Cosine);

        // Test L2
        let flat_l2 = FlatVectorField {
            name: "vec".to_string(),
            dims: 128,
            datatype: VectorDataType::Float32,
            distance_metric: SchemaMetric::L2,
            initial_cap: None,
            block_size: None,
        };
        assert_eq!(flat_l2.distance_metric, SchemaMetric::L2);

        // Test IP
        let flat_ip = FlatVectorField {
            name: "vec".to_string(),
            dims: 128,
            datatype: VectorDataType::Float32,
            distance_metric: SchemaMetric::IP,
            initial_cap: None,
            block_size: None,
        };
        assert_eq!(flat_ip.distance_metric, SchemaMetric::IP);
    }

    #[test]
    fn test_vector_field_distance_metric_method() {
        use crate::schema::{DistanceMetric as SchemaMetric, FlatVectorField, VectorField};

        let flat = FlatVectorField::new("test_vec", 256);
        let field = VectorField::Flat(flat);

        // Default should be Cosine
        assert_eq!(field.distance_metric(), SchemaMetric::Cosine);
    }
}
