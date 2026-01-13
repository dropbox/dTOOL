//! Pinecone vector store implementation for `DashFlow` Rust.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use async_trait::async_trait;
use dashflow::core::config::RunnableConfig;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::indexing::document_index::{DeleteResponse, DocumentIndex, UpsertResponse};
use dashflow::core::retrievers::Retriever;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::{Error, Result};
use dashflow::{embed, embed_query};
use pinecone_sdk::models::{Kind, Metadata, Value as PineconeValue, Vector};
use pinecone_sdk::pinecone::{PineconeClient, PineconeClientConfig};
use serde_json::Value as JsonValue;

/// Pinecone vector store implementation.
///
/// Pinecone is a managed vector database offering:
/// - Serverless and pod-based deployment options
/// - Sub-second query latency at scale
/// - Automatic index optimization and management
/// - Built-in metadata filtering
/// - Namespace support for data isolation
pub struct PineconeVectorStore {
    client: PineconeClient,
    index_host: String,
    embeddings: Arc<dyn Embeddings>,
    namespace: Option<String>,
    distance_metric: DistanceMetric,
}

impl PineconeVectorStore {
    /// Creates a new `PineconeVectorStore` instance.
    ///
    /// # Arguments
    ///
    /// * `index_host` - The host URL for your Pinecone index (e.g., "my-index-abc123.svc.aped-0000-1111.pinecone.io")
    /// * `embeddings` - Embeddings implementation for converting text to vectors
    /// * `api_key` - Optional API key (if None, uses `PINECONE_API_KEY` environment variable)
    /// * `namespace` - Optional namespace for data isolation (defaults to empty string)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let store = PineconeVectorStore::new(
    ///     "my-index-abc123.svc.aped-0000-1111.pinecone.io",
    ///     embeddings,
    ///     None, // Use PINECONE_API_KEY env var
    ///     Some("my-namespace"),
    /// ).await?;
    /// ```
    pub async fn new(
        index_host: &str,
        embeddings: Arc<dyn Embeddings>,
        api_key: Option<&str>,
        namespace: Option<&str>,
    ) -> Result<Self> {
        let config = PineconeClientConfig {
            api_key: api_key.map(std::string::ToString::to_string),
            ..Default::default()
        };

        let client = config
            .client()
            .map_err(|e| Error::config(format!("Failed to create Pinecone client: {e}")))?;

        Ok(Self {
            client,
            index_host: index_host.to_string(),
            embeddings,
            namespace: namespace.map(std::string::ToString::to_string),
            distance_metric: DistanceMetric::Cosine,
        })
    }

    /// Set the namespace for this vector store.
    #[must_use]
    pub fn with_namespace(mut self, namespace: &str) -> Self {
        self.namespace = Some(namespace.to_string());
        self
    }

    /// Set the distance metric for this vector store.
    #[must_use]
    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Convert `serde_json::Value` to `pinecone_sdk::models::Value`.
    fn json_to_pinecone_value(json: &JsonValue) -> PineconeValue {
        let kind = match json {
            JsonValue::Null => Some(Kind::NullValue(0)),
            JsonValue::Bool(b) => Some(Kind::BoolValue(*b)),
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Some(Kind::NumberValue(i as f64))
                } else if let Some(f) = n.as_f64() {
                    Some(Kind::NumberValue(f))
                } else {
                    Some(Kind::NullValue(0))
                }
            }
            JsonValue::String(s) => Some(Kind::StringValue(s.clone())),
            JsonValue::Array(arr) => {
                // Note: pinecone-sdk 0.1.2 has ListValue but it might not be exported
                // For now, we'll store arrays as strings or skip them
                // This is a limitation of the current SDK version
                let json_str = serde_json::to_string(arr).unwrap_or_default();
                Some(Kind::StringValue(json_str))
            }
            JsonValue::Object(obj) => {
                let fields: BTreeMap<String, PineconeValue> = obj
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::json_to_pinecone_value(v)))
                    .collect();
                Some(Kind::StructValue(Metadata { fields }))
            }
        };

        PineconeValue { kind }
    }

    /// Convert `pinecone_sdk::models::Value` to `serde_json::Value`.
    fn pinecone_value_to_json(value: &PineconeValue) -> JsonValue {
        match &value.kind {
            None | Some(Kind::NullValue(_)) => JsonValue::Null,
            Some(Kind::BoolValue(b)) => JsonValue::Bool(*b),
            Some(Kind::NumberValue(n)) => {
                // Try to preserve integer types
                if n.fract() == 0.0 && n.is_finite() {
                    JsonValue::Number((*n as i64).into())
                } else {
                    serde_json::Number::from_f64(*n).map_or(JsonValue::Null, JsonValue::Number)
                }
            }
            Some(Kind::StringValue(s)) => JsonValue::String(s.clone()),
            Some(Kind::ListValue(_list)) => {
                // Note: pinecone-sdk 0.1.2 ListValue might not be fully accessible
                // Return empty array for now
                JsonValue::Array(vec![])
            }
            Some(Kind::StructValue(metadata)) => {
                let map: serde_json::Map<String, JsonValue> = metadata
                    .fields
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::pinecone_value_to_json(v)))
                    .collect();
                JsonValue::Object(map)
            }
        }
    }

    /// Convert `HashMap` metadata to Pinecone Metadata.
    fn metadata_to_pinecone(metadata: &HashMap<String, JsonValue>) -> Metadata {
        let fields: BTreeMap<String, PineconeValue> = metadata
            .iter()
            .map(|(k, v)| (k.clone(), Self::json_to_pinecone_value(v)))
            .collect();
        Metadata { fields }
    }

    /// Convert Pinecone Metadata to `HashMap`.
    fn pinecone_to_metadata(metadata: &Metadata) -> HashMap<String, JsonValue> {
        metadata
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), Self::pinecone_value_to_json(v)))
            .collect()
    }

    /// Build a metadata filter for Pinecone queries.
    fn build_filter(filter: &HashMap<String, JsonValue>) -> Option<Metadata> {
        if filter.is_empty() {
            return None;
        }
        Some(Self::metadata_to_pinecone(filter))
    }
}

#[async_trait]
impl VectorStore for PineconeVectorStore {
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

        let text_strings: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();
        // Generate embeddings using graph API
        let embeddings_vec = embed(Arc::clone(&self.embeddings), &text_strings).await?;
        let doc_ids: Vec<String> = if let Some(ids) = ids {
            ids.to_vec()
        } else {
            (0..text_count)
                .map(|_| uuid::Uuid::new_v4().to_string())
                .collect()
        };

        // Build Pinecone vectors
        let vectors: Vec<Vector> = doc_ids
            .iter()
            .zip(embeddings_vec.iter())
            .enumerate()
            .map(|(i, (id, values))| {
                let metadata = metadatas.map(|m| Self::metadata_to_pinecone(&m[i]));
                Vector {
                    id: id.clone(),
                    values: values.clone(),
                    sparse_values: None,
                    metadata,
                }
            })
            .collect();

        // Get index and upsert
        let mut index = self
            .client
            .index(&self.index_host)
            .await
            .map_err(|e| Error::other(format!("Failed to get Pinecone index: {e}")))?;

        let namespace = self.namespace.as_deref().unwrap_or("");
        index
            .upsert(&vectors, &namespace.into())
            .await
            .map_err(|e| Error::other(format!("Pinecone upsert failed: {e}")))?;

        Ok(doc_ids)
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        if let Some(ids) = ids {
            if ids.is_empty() {
                return Ok(true);
            }

            let mut index = self
                .client
                .index(&self.index_host)
                .await
                .map_err(|e| Error::other(format!("Failed to get Pinecone index: {e}")))?;

            let namespace = self.namespace.as_deref().unwrap_or("");
            let ids_refs: Vec<&str> = ids.iter().map(std::string::String::as_str).collect();

            index
                .delete_by_id(&ids_refs, &namespace.into())
                .await
                .map_err(|e| Error::other(format!("Pinecone delete failed: {e}")))?;

            Ok(true)
        } else {
            // Delete all vectors in namespace
            let mut index = self
                .client
                .index(&self.index_host)
                .await
                .map_err(|e| Error::other(format!("Failed to get Pinecone index: {e}")))?;

            let namespace = self.namespace.as_deref().unwrap_or("");
            index
                .delete_all(&namespace.into())
                .await
                .map_err(|e| Error::other(format!("Pinecone delete_all failed: {e}")))?;

            Ok(true)
        }
    }

    async fn _similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        // Embed the query using graph API
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query).await?;

        // Get index
        let mut index = self
            .client
            .index(&self.index_host)
            .await
            .map_err(|e| Error::other(format!("Failed to get Pinecone index: {e}")))?;

        let namespace = self.namespace.as_deref().unwrap_or("");
        let filter_metadata = filter.and_then(Self::build_filter);

        // Query Pinecone
        // Signature: query_by_value(vector, sparse_vector, top_k, namespace, filter, include_values, include_metadata)
        let response = index
            .query_by_value(
                query_embedding,
                None, // sparse_vector
                k as u32,
                &namespace.into(),
                filter_metadata, // filter
                Some(true),      // include_values
                Some(true),      // include_metadata
            )
            .await
            .map_err(|e| Error::other(format!("Pinecone query failed: {e}")))?;

        // Convert matches to documents
        let documents: Vec<Document> = response
            .matches
            .iter()
            .map(|m| {
                let mut metadata = if let Some(ref meta) = m.metadata {
                    Self::pinecone_to_metadata(meta)
                } else {
                    HashMap::new()
                };

                // Add score to metadata
                metadata.insert("score".to_string(), JsonValue::from(m.score));

                // Use the ID as page_content if no text is stored
                // (In production, you'd typically store the text in metadata)
                let page_content = metadata
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&m.id)
                    .to_string();

                Document {
                    id: Some(m.id.clone()),
                    page_content,
                    metadata,
                }
            })
            .collect();

        Ok(documents)
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        // Embed the query using graph API
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query).await?;
        self.similarity_search_by_vector_with_score(&query_embedding, k, filter)
            .await
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        // Get index
        let mut index = self
            .client
            .index(&self.index_host)
            .await
            .map_err(|e| Error::other(format!("Failed to get Pinecone index: {e}")))?;

        let namespace = self.namespace.as_deref().unwrap_or("");
        let ids_refs: Vec<&str> = ids.iter().map(std::string::String::as_str).collect();

        // Fetch vectors by ID
        let response = index
            .fetch(&ids_refs, &namespace.into())
            .await
            .map_err(|e| Error::other(format!("Pinecone fetch failed: {e}")))?;

        // Convert response to documents
        let documents: Vec<Document> = response
            .vectors
            .iter()
            .map(|(id, vector)| {
                let metadata = if let Some(ref meta) = vector.metadata {
                    Self::pinecone_to_metadata(meta)
                } else {
                    HashMap::new()
                };

                // Extract text from metadata or use ID as fallback
                let page_content = metadata
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or(id)
                    .to_string();

                Document {
                    id: Some(id.clone()),
                    page_content,
                    metadata,
                }
            })
            .collect();

        Ok(documents)
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
        Ok(results.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        // Get index
        let mut index = self
            .client
            .index(&self.index_host)
            .await
            .map_err(|e| Error::other(format!("Failed to get Pinecone index: {e}")))?;

        let namespace = self.namespace.as_deref().unwrap_or("");
        let filter_metadata = filter.and_then(Self::build_filter);

        // Query Pinecone
        // Signature: query_by_value(vector, sparse_vector, top_k, namespace, filter, include_values, include_metadata)
        let response = index
            .query_by_value(
                embedding.to_vec(),
                None, // sparse_vector
                k as u32,
                &namespace.into(),
                filter_metadata, // filter
                Some(true),      // include_values
                Some(true),      // include_metadata
            )
            .await
            .map_err(|e| Error::other(format!("Pinecone query failed: {e}")))?;

        // Convert matches to documents with scores
        let results: Vec<(Document, f32)> = response
            .matches
            .iter()
            .map(|m| {
                let metadata = if let Some(ref meta) = m.metadata {
                    Self::pinecone_to_metadata(meta)
                } else {
                    HashMap::new()
                };

                // Don't add score to metadata here (it's returned separately)
                let page_content = metadata
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&m.id)
                    .to_string();

                let doc = Document {
                    id: Some(m.id.clone()),
                    page_content,
                    metadata,
                };
                (doc, m.score)
            })
            .collect();

        Ok(results)
    }

    async fn max_marginal_relevance_search(
        &self,
        query: &str,
        k: usize,
        fetch_k: usize,
        lambda: f32,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        // Embed the query using graph API
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query).await?;

        // Get index
        let mut index = self
            .client
            .index(&self.index_host)
            .await
            .map_err(|e| Error::other(format!("Failed to get Pinecone index: {e}")))?;

        let namespace = self.namespace.as_deref().unwrap_or("");
        let filter_metadata = filter.and_then(Self::build_filter);

        // Query Pinecone with fetch_k candidates (need include_values=true to get embeddings)
        let response = index
            .query_by_value(
                query_embedding.clone(),
                None, // sparse_vector
                fetch_k as u32,
                &namespace.into(),
                filter_metadata, // filter
                Some(true),      // include_values (get embeddings back)
                Some(true),      // include_metadata
            )
            .await
            .map_err(|e| Error::other(format!("Pinecone query failed: {e}")))?;

        if response.matches.is_empty() {
            return Ok(Vec::new());
        }

        // Extract embeddings for MMR calculation
        // Note: Pinecone returns values directly (not as Option) when include_values=true
        let candidate_embeddings: Vec<Vec<f32>> = response
            .matches
            .iter()
            .map(|m| m.values.clone())
            .collect();

        if candidate_embeddings.is_empty() {
            // Fallback: if no embeddings returned, just return top k by score
            return Ok(response
                .matches
                .iter()
                .take(k)
                .map(|m| {
                    let metadata = if let Some(ref meta) = m.metadata {
                        Self::pinecone_to_metadata(meta)
                    } else {
                        HashMap::new()
                    };
                    let page_content = metadata
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&m.id)
                        .to_string();

                    Document {
                        id: Some(m.id.clone()),
                        page_content,
                        metadata,
                    }
                })
                .collect());
        }

        // Run MMR algorithm
        let selected_indices = dashflow::core::vector_stores::maximal_marginal_relevance(
            &query_embedding,
            &candidate_embeddings,
            k,
            lambda,
        )?;

        // Build result documents from selected indices
        let results: Vec<Document> = selected_indices
            .into_iter()
            .filter_map(|idx| {
                response.matches.get(idx).map(|m| {
                    let metadata = if let Some(ref meta) = m.metadata {
                        Self::pinecone_to_metadata(meta)
                    } else {
                        HashMap::new()
                    };
                    let page_content = metadata
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&m.id)
                        .to_string();

                    Document {
                        id: Some(m.id.clone()),
                        page_content,
                        metadata,
                    }
                })
            })
            .collect();

        Ok(results)
    }
}

#[async_trait]
impl Retriever for PineconeVectorStore {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        self._similarity_search(query, 4, None).await
    }
}

#[async_trait]
impl DocumentIndex for PineconeVectorStore {
    async fn upsert(
        &self,
        _documents: &[Document],
    ) -> std::result::Result<UpsertResponse, Box<dyn std::error::Error + Send + Sync>> {
        // Note: DocumentIndex requires &self but VectorStore::add_documents requires &mut self
        // This is a design limitation - for now we'll return an error
        // In production, you'd need interior mutability (Arc<Mutex<...>>)
        Err("DocumentIndex::upsert not supported - requires interior mutability".into())
    }

    async fn delete(
        &self,
        _ids: Option<&[String]>,
    ) -> std::result::Result<DeleteResponse, Box<dyn std::error::Error + Send + Sync>> {
        // Note: Same issue as upsert - requires &mut self
        Err("DocumentIndex::delete not supported - requires interior mutability".into())
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

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    // ========================================================================
    // JSON TO PINECONE VALUE CONVERSION TESTS
    // ========================================================================

    #[test]
    fn test_json_to_pinecone_value_null() {
        let json = JsonValue::Null;
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        assert!(matches!(pinecone.kind, Some(Kind::NullValue(_))));
    }

    #[test]
    fn test_json_to_pinecone_value_bool_true() {
        let json = JsonValue::Bool(true);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        assert!(matches!(pinecone.kind, Some(Kind::BoolValue(true))));
    }

    #[test]
    fn test_json_to_pinecone_value_bool_false() {
        let json = JsonValue::Bool(false);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        assert!(matches!(pinecone.kind, Some(Kind::BoolValue(false))));
    }

    #[test]
    fn test_json_to_pinecone_value_float() {
        let json = serde_json::json!(42.5);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::NumberValue(n)) = pinecone.kind {
            assert!((n - 42.5).abs() < 1e-9);
        } else {
            panic!("Expected NumberValue");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_integer() {
        let json = serde_json::json!(42);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::NumberValue(n)) = pinecone.kind {
            assert!((n - 42.0).abs() < 1e-9);
        } else {
            panic!("Expected NumberValue");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_negative_integer() {
        let json = serde_json::json!(-100);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::NumberValue(n)) = pinecone.kind {
            assert!((n - (-100.0)).abs() < 1e-9);
        } else {
            panic!("Expected NumberValue");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_zero() {
        let json = serde_json::json!(0);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::NumberValue(n)) = pinecone.kind {
            assert!((n - 0.0).abs() < 1e-9);
        } else {
            panic!("Expected NumberValue");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_large_integer() {
        let json = serde_json::json!(i64::MAX);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::NumberValue(n)) = pinecone.kind {
            assert!((n - i64::MAX as f64).abs() < 1e6); // Allow some rounding
        } else {
            panic!("Expected NumberValue");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_string() {
        let json = JsonValue::String("hello".to_string());
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StringValue(s)) = pinecone.kind {
            assert_eq!(s, "hello");
        } else {
            panic!("Expected StringValue");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_empty_string() {
        let json = JsonValue::String(String::new());
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StringValue(s)) = pinecone.kind {
            assert_eq!(s, "");
        } else {
            panic!("Expected StringValue");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_string_with_unicode() {
        let json = JsonValue::String("こんにちは 🎉".to_string());
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StringValue(s)) = pinecone.kind {
            assert_eq!(s, "こんにちは 🎉");
        } else {
            panic!("Expected StringValue");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_string_with_special_chars() {
        let json = JsonValue::String("line1\nline2\ttabbed".to_string());
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StringValue(s)) = pinecone.kind {
            assert_eq!(s, "line1\nline2\ttabbed");
        } else {
            panic!("Expected StringValue");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_array() {
        let json = serde_json::json!([1, 2, 3]);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        // Arrays are serialized to JSON strings
        if let Some(Kind::StringValue(s)) = pinecone.kind {
            assert_eq!(s, "[1,2,3]");
        } else {
            panic!("Expected StringValue for array");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_empty_array() {
        let json = serde_json::json!([]);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StringValue(s)) = pinecone.kind {
            assert_eq!(s, "[]");
        } else {
            panic!("Expected StringValue for empty array");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_object() {
        let json = serde_json::json!({"key": "value"});
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StructValue(metadata)) = pinecone.kind {
            assert!(metadata.fields.contains_key("key"));
            let value = metadata.fields.get("key").unwrap();
            assert!(matches!(
                value.kind,
                Some(Kind::StringValue(ref s)) if s == "value"
            ));
        } else {
            panic!("Expected StructValue for object");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_empty_object() {
        let json = serde_json::json!({});
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StructValue(metadata)) = pinecone.kind {
            assert!(metadata.fields.is_empty());
        } else {
            panic!("Expected StructValue for empty object");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_nested_object() {
        let json = serde_json::json!({
            "outer": {
                "inner": "value"
            }
        });
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StructValue(metadata)) = pinecone.kind {
            let outer = metadata.fields.get("outer").unwrap();
            if let Some(Kind::StructValue(inner_meta)) = &outer.kind {
                let inner = inner_meta.fields.get("inner").unwrap();
                assert!(matches!(
                    inner.kind,
                    Some(Kind::StringValue(ref s)) if s == "value"
                ));
            } else {
                panic!("Expected nested StructValue");
            }
        } else {
            panic!("Expected StructValue");
        }
    }

    #[test]
    fn test_json_to_pinecone_value_mixed_object() {
        let json = serde_json::json!({
            "string": "text",
            "number": 123,
            "bool": true,
            "null": null
        });
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StructValue(metadata)) = pinecone.kind {
            assert_eq!(metadata.fields.len(), 4);

            // Check string
            let string_val = metadata.fields.get("string").unwrap();
            assert!(matches!(
                string_val.kind,
                Some(Kind::StringValue(ref s)) if s == "text"
            ));

            // Check number
            let number_val = metadata.fields.get("number").unwrap();
            if let Some(Kind::NumberValue(n)) = number_val.kind {
                assert!((n - 123.0).abs() < 1e-9);
            } else {
                panic!("Expected NumberValue");
            }

            // Check bool
            let bool_val = metadata.fields.get("bool").unwrap();
            assert!(matches!(bool_val.kind, Some(Kind::BoolValue(true))));

            // Check null
            let null_val = metadata.fields.get("null").unwrap();
            assert!(matches!(null_val.kind, Some(Kind::NullValue(_))));
        } else {
            panic!("Expected StructValue");
        }
    }

    // ========================================================================
    // PINECONE VALUE TO JSON CONVERSION TESTS
    // ========================================================================

    #[test]
    fn test_pinecone_value_to_json_null() {
        let pinecone = PineconeValue {
            kind: Some(Kind::NullValue(0)),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert!(json.is_null());
    }

    #[test]
    fn test_pinecone_value_to_json_none() {
        let pinecone = PineconeValue { kind: None };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert!(json.is_null());
    }

    #[test]
    fn test_pinecone_value_to_json_bool_true() {
        let pinecone = PineconeValue {
            kind: Some(Kind::BoolValue(true)),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert_eq!(json, JsonValue::Bool(true));
    }

    #[test]
    fn test_pinecone_value_to_json_bool_false() {
        let pinecone = PineconeValue {
            kind: Some(Kind::BoolValue(false)),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert_eq!(json, JsonValue::Bool(false));
    }

    #[test]
    fn test_pinecone_value_to_json_integer_number() {
        let pinecone = PineconeValue {
            kind: Some(Kind::NumberValue(42.0)),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert_eq!(json, serde_json::json!(42));
    }

    #[test]
    fn test_pinecone_value_to_json_float_number() {
        let pinecone = PineconeValue {
            kind: Some(Kind::NumberValue(42.5)),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        if let JsonValue::Number(n) = json {
            assert!((n.as_f64().unwrap() - 42.5).abs() < 1e-9);
        } else {
            panic!("Expected Number");
        }
    }

    #[test]
    fn test_pinecone_value_to_json_negative_number() {
        let pinecone = PineconeValue {
            kind: Some(Kind::NumberValue(-99.5)),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        if let JsonValue::Number(n) = json {
            assert!((n.as_f64().unwrap() - (-99.5)).abs() < 1e-9);
        } else {
            panic!("Expected Number");
        }
    }

    #[test]
    fn test_pinecone_value_to_json_zero() {
        let pinecone = PineconeValue {
            kind: Some(Kind::NumberValue(0.0)),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert_eq!(json, serde_json::json!(0));
    }

    #[test]
    fn test_pinecone_value_to_json_string() {
        let pinecone = PineconeValue {
            kind: Some(Kind::StringValue("world".to_string())),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert_eq!(json, JsonValue::String("world".to_string()));
    }

    #[test]
    fn test_pinecone_value_to_json_empty_string() {
        let pinecone = PineconeValue {
            kind: Some(Kind::StringValue(String::new())),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert_eq!(json, JsonValue::String(String::new()));
    }

    #[test]
    fn test_pinecone_value_to_json_struct() {
        let mut fields = BTreeMap::new();
        fields.insert(
            "key".to_string(),
            PineconeValue {
                kind: Some(Kind::StringValue("value".to_string())),
            },
        );
        let pinecone = PineconeValue {
            kind: Some(Kind::StructValue(Metadata { fields })),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert_eq!(json, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn test_pinecone_value_to_json_empty_struct() {
        let pinecone = PineconeValue {
            kind: Some(Kind::StructValue(Metadata {
                fields: BTreeMap::new(),
            })),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert_eq!(json, serde_json::json!({}));
    }

    #[test]
    fn test_pinecone_value_to_json_nested_struct() {
        let mut inner_fields = BTreeMap::new();
        inner_fields.insert(
            "inner_key".to_string(),
            PineconeValue {
                kind: Some(Kind::NumberValue(42.0)),
            },
        );
        let mut outer_fields = BTreeMap::new();
        outer_fields.insert(
            "outer".to_string(),
            PineconeValue {
                kind: Some(Kind::StructValue(Metadata {
                    fields: inner_fields,
                })),
            },
        );
        let pinecone = PineconeValue {
            kind: Some(Kind::StructValue(Metadata {
                fields: outer_fields,
            })),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert_eq!(json, serde_json::json!({"outer": {"inner_key": 42}}));
    }

    // ========================================================================
    // METADATA CONVERSION TESTS
    // ========================================================================

    #[test]
    fn test_metadata_to_pinecone_empty() {
        let metadata: HashMap<String, JsonValue> = HashMap::new();
        let pinecone_meta = PineconeVectorStore::metadata_to_pinecone(&metadata);
        assert!(pinecone_meta.fields.is_empty());
    }

    #[test]
    fn test_metadata_to_pinecone_single_string() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), JsonValue::String("value".to_string()));
        let pinecone_meta = PineconeVectorStore::metadata_to_pinecone(&metadata);
        assert_eq!(pinecone_meta.fields.len(), 1);
        let val = pinecone_meta.fields.get("key").unwrap();
        assert!(matches!(
            val.kind,
            Some(Kind::StringValue(ref s)) if s == "value"
        ));
    }

    #[test]
    fn test_metadata_to_pinecone_multiple_types() {
        let mut metadata = HashMap::new();
        metadata.insert("string".to_string(), JsonValue::String("text".to_string()));
        metadata.insert("number".to_string(), serde_json::json!(42));
        metadata.insert("bool".to_string(), JsonValue::Bool(true));
        let pinecone_meta = PineconeVectorStore::metadata_to_pinecone(&metadata);
        assert_eq!(pinecone_meta.fields.len(), 3);
    }

    #[test]
    fn test_pinecone_to_metadata_empty() {
        let pinecone_meta = Metadata {
            fields: BTreeMap::new(),
        };
        let metadata = PineconeVectorStore::pinecone_to_metadata(&pinecone_meta);
        assert!(metadata.is_empty());
    }

    #[test]
    fn test_pinecone_to_metadata_single_string() {
        let mut fields = BTreeMap::new();
        fields.insert(
            "key".to_string(),
            PineconeValue {
                kind: Some(Kind::StringValue("value".to_string())),
            },
        );
        let pinecone_meta = Metadata { fields };
        let metadata = PineconeVectorStore::pinecone_to_metadata(&pinecone_meta);
        assert_eq!(metadata.len(), 1);
        assert_eq!(
            metadata.get("key").and_then(|v| v.as_str()),
            Some("value")
        );
    }

    #[test]
    fn test_metadata_round_trip_empty() {
        let metadata: HashMap<String, JsonValue> = HashMap::new();
        let pinecone_meta = PineconeVectorStore::metadata_to_pinecone(&metadata);
        let recovered = PineconeVectorStore::pinecone_to_metadata(&pinecone_meta);
        assert!(recovered.is_empty());
    }

    #[test]
    fn test_metadata_round_trip_string() {
        let mut metadata = HashMap::new();
        metadata.insert("name".to_string(), JsonValue::String("test".to_string()));
        let pinecone_meta = PineconeVectorStore::metadata_to_pinecone(&metadata);
        let recovered = PineconeVectorStore::pinecone_to_metadata(&pinecone_meta);
        assert_eq!(recovered.get("name").and_then(|v| v.as_str()), Some("test"));
    }

    #[test]
    fn test_metadata_round_trip_integer() {
        let mut metadata = HashMap::new();
        metadata.insert("count".to_string(), serde_json::json!(42));
        let pinecone_meta = PineconeVectorStore::metadata_to_pinecone(&metadata);
        let recovered = PineconeVectorStore::pinecone_to_metadata(&pinecone_meta);
        assert_eq!(recovered.get("count").and_then(|v| v.as_i64()), Some(42));
    }

    #[test]
    fn test_metadata_round_trip_bool() {
        let mut metadata = HashMap::new();
        metadata.insert("active".to_string(), JsonValue::Bool(true));
        let pinecone_meta = PineconeVectorStore::metadata_to_pinecone(&metadata);
        let recovered = PineconeVectorStore::pinecone_to_metadata(&pinecone_meta);
        assert_eq!(
            recovered.get("active").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_metadata_round_trip_null() {
        let mut metadata = HashMap::new();
        metadata.insert("empty".to_string(), JsonValue::Null);
        let pinecone_meta = PineconeVectorStore::metadata_to_pinecone(&metadata);
        let recovered = PineconeVectorStore::pinecone_to_metadata(&pinecone_meta);
        assert!(recovered.get("empty").unwrap().is_null());
    }

    #[test]
    fn test_metadata_round_trip_multiple() {
        let mut metadata = HashMap::new();
        metadata.insert("name".to_string(), JsonValue::String("test".to_string()));
        metadata.insert("count".to_string(), serde_json::json!(42));
        metadata.insert("active".to_string(), JsonValue::Bool(true));
        let pinecone_meta = PineconeVectorStore::metadata_to_pinecone(&metadata);
        let recovered = PineconeVectorStore::pinecone_to_metadata(&pinecone_meta);
        assert_eq!(recovered.get("name").and_then(|v| v.as_str()), Some("test"));
        assert_eq!(recovered.get("count").and_then(|v| v.as_i64()), Some(42));
        assert_eq!(
            recovered.get("active").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_metadata_round_trip_nested_object() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "nested".to_string(),
            serde_json::json!({"inner": "value"}),
        );
        let pinecone_meta = PineconeVectorStore::metadata_to_pinecone(&metadata);
        let recovered = PineconeVectorStore::pinecone_to_metadata(&pinecone_meta);
        let nested = recovered.get("nested").unwrap();
        assert!(nested.is_object());
        assert_eq!(nested.get("inner").and_then(|v| v.as_str()), Some("value"));
    }

    #[test]
    fn test_metadata_round_trip_special_characters() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "text".to_string(),
            JsonValue::String("line1\nline2\ttab".to_string()),
        );
        let pinecone_meta = PineconeVectorStore::metadata_to_pinecone(&metadata);
        let recovered = PineconeVectorStore::pinecone_to_metadata(&pinecone_meta);
        assert_eq!(
            recovered.get("text").and_then(|v| v.as_str()),
            Some("line1\nline2\ttab")
        );
    }

    #[test]
    fn test_metadata_round_trip_unicode() {
        let mut metadata = HashMap::new();
        metadata.insert(
            "text".to_string(),
            JsonValue::String("日本語 🎉 émoji".to_string()),
        );
        let pinecone_meta = PineconeVectorStore::metadata_to_pinecone(&metadata);
        let recovered = PineconeVectorStore::pinecone_to_metadata(&pinecone_meta);
        assert_eq!(
            recovered.get("text").and_then(|v| v.as_str()),
            Some("日本語 🎉 émoji")
        );
    }

    // ========================================================================
    // BUILD FILTER TESTS
    // ========================================================================

    #[test]
    fn test_build_filter_empty() {
        let filter: HashMap<String, JsonValue> = HashMap::new();
        let result = PineconeVectorStore::build_filter(&filter);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_filter_single_field() {
        let mut filter = HashMap::new();
        filter.insert("category".to_string(), JsonValue::String("tech".to_string()));
        let result = PineconeVectorStore::build_filter(&filter);
        assert!(result.is_some());
        let metadata = result.unwrap();
        assert_eq!(metadata.fields.len(), 1);
    }

    #[test]
    fn test_build_filter_multiple_fields() {
        let mut filter = HashMap::new();
        filter.insert("category".to_string(), JsonValue::String("tech".to_string()));
        filter.insert("year".to_string(), serde_json::json!(2024));
        let result = PineconeVectorStore::build_filter(&filter);
        assert!(result.is_some());
        let metadata = result.unwrap();
        assert_eq!(metadata.fields.len(), 2);
    }

    #[test]
    fn test_build_filter_with_null() {
        let mut filter = HashMap::new();
        filter.insert("empty".to_string(), JsonValue::Null);
        let result = PineconeVectorStore::build_filter(&filter);
        assert!(result.is_some());
    }

    #[test]
    fn test_build_filter_with_bool() {
        let mut filter = HashMap::new();
        filter.insert("active".to_string(), JsonValue::Bool(true));
        let result = PineconeVectorStore::build_filter(&filter);
        assert!(result.is_some());
        let metadata = result.unwrap();
        let val = metadata.fields.get("active").unwrap();
        assert!(matches!(val.kind, Some(Kind::BoolValue(true))));
    }

    // ========================================================================
    // DISTANCE METRIC TESTS
    // ========================================================================

    #[test]
    fn test_distance_metric_default() {
        // Can't test directly without async, but we can verify the enum exists
        let metric = DistanceMetric::Cosine;
        assert!(matches!(metric, DistanceMetric::Cosine));
    }

    #[test]
    fn test_distance_metric_variants() {
        let cosine = DistanceMetric::Cosine;
        let euclidean = DistanceMetric::Euclidean;
        let dot = DistanceMetric::DotProduct;

        assert!(matches!(cosine, DistanceMetric::Cosine));
        assert!(matches!(euclidean, DistanceMetric::Euclidean));
        assert!(matches!(dot, DistanceMetric::DotProduct));
    }

    // ========================================================================
    // NUMERIC EDGE CASE TESTS
    // ========================================================================

    #[test]
    fn test_json_to_pinecone_very_small_number() {
        let json = serde_json::json!(0.000001);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::NumberValue(n)) = pinecone.kind {
            assert!((n - 0.000001).abs() < 1e-12);
        } else {
            panic!("Expected NumberValue");
        }
    }

    #[test]
    fn test_json_to_pinecone_very_large_number() {
        let json = serde_json::json!(1e308);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::NumberValue(n)) = pinecone.kind {
            assert!(n > 1e307);
        } else {
            panic!("Expected NumberValue");
        }
    }

    #[test]
    fn test_pinecone_to_json_preserves_integers() {
        let pinecone = PineconeValue {
            kind: Some(Kind::NumberValue(100.0)),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        // Should be represented as integer since fract() == 0
        assert_eq!(json, serde_json::json!(100));
    }

    #[test]
    fn test_pinecone_to_json_preserves_floats() {
        let pinecone = PineconeValue {
            kind: Some(Kind::NumberValue(100.5)),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        if let JsonValue::Number(n) = json {
            assert!((n.as_f64().unwrap() - 100.5).abs() < 1e-9);
        } else {
            panic!("Expected Number");
        }
    }

    // ========================================================================
    // STRING EDGE CASE TESTS
    // ========================================================================

    #[test]
    fn test_json_to_pinecone_long_string() {
        let long_string = "a".repeat(10000);
        let json = JsonValue::String(long_string.clone());
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StringValue(s)) = pinecone.kind {
            assert_eq!(s.len(), 10000);
            assert_eq!(s, long_string);
        } else {
            panic!("Expected StringValue");
        }
    }

    #[test]
    fn test_json_to_pinecone_string_with_quotes() {
        let json = JsonValue::String(r#"He said "Hello""#.to_string());
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StringValue(s)) = pinecone.kind {
            assert_eq!(s, r#"He said "Hello""#);
        } else {
            panic!("Expected StringValue");
        }
    }

    #[test]
    fn test_json_to_pinecone_string_with_backslashes() {
        let json = JsonValue::String(r"path\to\file".to_string());
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StringValue(s)) = pinecone.kind {
            assert_eq!(s, r"path\to\file");
        } else {
            panic!("Expected StringValue");
        }
    }

    // ========================================================================
    // ARRAY EDGE CASE TESTS
    // ========================================================================

    #[test]
    fn test_json_to_pinecone_nested_array() {
        let json = serde_json::json!([[1, 2], [3, 4]]);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StringValue(s)) = pinecone.kind {
            assert_eq!(s, "[[1,2],[3,4]]");
        } else {
            panic!("Expected StringValue for nested array");
        }
    }

    #[test]
    fn test_json_to_pinecone_array_with_mixed_types() {
        let json = serde_json::json!([1, "two", true, null]);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StringValue(s)) = pinecone.kind {
            assert_eq!(s, r#"[1,"two",true,null]"#);
        } else {
            panic!("Expected StringValue for mixed array");
        }
    }

    // ========================================================================
    // OBJECT EDGE CASE TESTS
    // ========================================================================

    #[test]
    fn test_json_to_pinecone_deeply_nested_object() {
        let json = serde_json::json!({
            "level1": {
                "level2": {
                    "level3": {
                        "value": 42
                    }
                }
            }
        });
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StructValue(l1)) = pinecone.kind {
            if let Some(Kind::StructValue(l2)) = l1.fields.get("level1").unwrap().kind.as_ref() {
                if let Some(Kind::StructValue(l3)) = l2.fields.get("level2").unwrap().kind.as_ref()
                {
                    if let Some(Kind::StructValue(l4)) =
                        l3.fields.get("level3").unwrap().kind.as_ref()
                    {
                        if let Some(Kind::NumberValue(n)) =
                            l4.fields.get("value").unwrap().kind.as_ref()
                        {
                            assert!((n - 42.0).abs() < 1e-9);
                        } else {
                            panic!("Expected NumberValue at deepest level");
                        }
                    } else {
                        panic!("Expected StructValue at level3");
                    }
                } else {
                    panic!("Expected StructValue at level2");
                }
            } else {
                panic!("Expected StructValue at level1");
            }
        } else {
            panic!("Expected StructValue at root");
        }
    }

    #[test]
    fn test_json_to_pinecone_object_with_special_key_names() {
        let json = serde_json::json!({
            "key with spaces": "value1",
            "key-with-dashes": "value2",
            "key.with.dots": "value3"
        });
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StructValue(metadata)) = pinecone.kind {
            assert!(metadata.fields.contains_key("key with spaces"));
            assert!(metadata.fields.contains_key("key-with-dashes"));
            assert!(metadata.fields.contains_key("key.with.dots"));
        } else {
            panic!("Expected StructValue");
        }
    }

    // ========================================================================
    // LEGACY TESTS (original test methods preserved for compatibility)
    // ========================================================================

    #[test]
    fn test_json_to_pinecone_value_conversion() {
        // Test null
        let json = JsonValue::Null;
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        assert!(matches!(pinecone.kind, Some(Kind::NullValue(_))));

        // Test bool
        let json = JsonValue::Bool(true);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        assert!(matches!(pinecone.kind, Some(Kind::BoolValue(true))));

        // Test number
        let json = serde_json::json!(42.5);
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::NumberValue(n)) = pinecone.kind {
            assert!((n - 42.5).abs() < 1e-9);
        } else {
            panic!("Expected NumberValue");
        }

        // Test string
        let json = JsonValue::String("hello".to_string());
        let pinecone = PineconeVectorStore::json_to_pinecone_value(&json);
        if let Some(Kind::StringValue(s)) = pinecone.kind {
            assert_eq!(s, "hello");
        } else {
            panic!("Expected StringValue");
        }
    }

    #[test]
    fn test_pinecone_value_to_json_conversion() {
        // Test null
        let pinecone = PineconeValue {
            kind: Some(Kind::NullValue(0)),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert!(json.is_null());

        // Test bool
        let pinecone = PineconeValue {
            kind: Some(Kind::BoolValue(false)),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert_eq!(json, JsonValue::Bool(false));

        // Test string
        let pinecone = PineconeValue {
            kind: Some(Kind::StringValue("world".to_string())),
        };
        let json = PineconeVectorStore::pinecone_value_to_json(&pinecone);
        assert_eq!(json, JsonValue::String("world".to_string()));
    }

    #[test]
    fn test_metadata_round_trip_conversion() {
        let mut metadata = HashMap::new();
        metadata.insert("name".to_string(), JsonValue::String("test".to_string()));
        metadata.insert("count".to_string(), serde_json::json!(42));
        metadata.insert("active".to_string(), JsonValue::Bool(true));

        // Convert to Pinecone and back
        let pinecone_meta = PineconeVectorStore::metadata_to_pinecone(&metadata);
        let recovered = PineconeVectorStore::pinecone_to_metadata(&pinecone_meta);

        assert_eq!(recovered.get("name").and_then(|v| v.as_str()), Some("test"));
        assert_eq!(recovered.get("count").and_then(|v| v.as_i64()), Some(42));
        assert_eq!(
            recovered.get("active").and_then(|v| v.as_bool()),
            Some(true)
        );
    }
}

// Standard conformance tests
// These tests require Pinecone API key and index setup. Tests now use environmental error handling and skip gracefully if credentials are unavailable.
#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
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

    async fn create_test_store() -> PineconeVectorStore {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let api_key = std::env::var("PINECONE_API_KEY")
            .expect("PINECONE_API_KEY environment variable required");
        let index_host = std::env::var("PINECONE_INDEX_HOST")
            .expect("PINECONE_INDEX_HOST environment variable required");

        PineconeVectorStore::new(&index_host, embeddings, Some(&api_key), None)
            .await
            .expect("Failed to create test store - check PINECONE_API_KEY and PINECONE_INDEX_HOST")
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_add_and_search_standard() {
        let mut store = create_test_store().await;
        test_add_and_search(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_search_with_scores_standard() {
        let mut store = create_test_store().await;
        test_search_with_scores(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_metadata_filtering_standard() {
        let mut store = create_test_store().await;
        test_metadata_filtering(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_custom_ids_standard() {
        let mut store = create_test_store().await;
        test_custom_ids(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_delete_standard() {
        let mut store = create_test_store().await;
        test_delete(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_add_documents_standard() {
        let mut store = create_test_store().await;
        test_add_documents(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_empty_search_standard() {
        let store = create_test_store().await;
        test_empty_search(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_search_by_vector_standard() {
        let mut store = create_test_store().await;
        test_search_by_vector(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_mmr_search_standard() {
        let mut store = create_test_store().await;
        test_mmr_search(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_large_batch_standard() {
        let mut store = create_test_store().await;
        test_large_batch(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_validation_standard() {
        let mut store = create_test_store().await;
        test_validation(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_update_document_standard() {
        let mut store = create_test_store().await;
        test_update_document(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_metadata_only_filter_standard() {
        let mut store = create_test_store().await;
        test_metadata_only_filter(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_complex_metadata_standard() {
        let mut store = create_test_store().await;
        test_complex_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_empty_text_standard() {
        let mut store = create_test_store().await;
        test_empty_text(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_special_chars_metadata_standard() {
        let mut store = create_test_store().await;
        test_special_chars_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_concurrent_operations_standard() {
        let mut store = create_test_store().await;
        test_concurrent_operations(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_very_long_text_standard() {
        let mut store = create_test_store().await;
        test_very_long_text(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_duplicate_documents_standard() {
        let mut store = create_test_store().await;
        test_duplicate_documents(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_k_parameter_standard() {
        let mut store = create_test_store().await;
        test_k_parameter(&mut store).await;
    }

    // ========================================================================
    // COMPREHENSIVE TESTS
    // These tests provide deeper coverage beyond standard conformance tests
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_mmr_lambda_zero_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_lambda_zero(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_mmr_lambda_one_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_lambda_one(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_mmr_fetch_k_variations_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_fetch_k_variations(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_complex_metadata_operators_comprehensive() {
        let mut store = create_test_store().await;
        test_complex_metadata_operators(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_nested_metadata_filtering_comprehensive() {
        let mut store = create_test_store().await;
        test_nested_metadata_filtering(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_array_metadata_comprehensive() {
        let mut store = create_test_store().await;
        test_array_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_very_large_batch_comprehensive() {
        let mut store = create_test_store().await;
        test_very_large_batch(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_concurrent_writes_comprehensive() {
        let mut store = create_test_store().await;
        test_concurrent_writes(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_error_handling_network_comprehensive() {
        let mut store = create_test_store().await;
        test_error_handling_network(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_error_handling_invalid_input_comprehensive() {
        let mut store = create_test_store().await;
        test_error_handling_invalid_input(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_bulk_delete_comprehensive() {
        let mut store = create_test_store().await;
        test_bulk_delete(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_update_metadata_comprehensive() {
        let mut store = create_test_store().await;
        test_update_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires PINECONE_API_KEY"]
    async fn test_search_score_threshold_comprehensive() {
        let mut store = create_test_store().await;
        test_search_score_threshold(&mut store).await;
    }
}
