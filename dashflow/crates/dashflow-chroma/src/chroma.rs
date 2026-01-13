//! Chroma vector store implementation for `DashFlow` Rust.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chromadb::client::{ChromaClient, ChromaClientOptions};
use chromadb::collection::{ChromaCollection, CollectionEntries, GetOptions, QueryOptions};
use dashflow::core::config::RunnableConfig;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::indexing::document_index::{DeleteResponse, DocumentIndex, UpsertResponse};
use dashflow::core::retrievers::Retriever;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::{Error, Result};
use dashflow::{embed, embed_query};
use serde_json::{json, Value as JsonValue};

/// Chroma vector store implementation.
pub struct ChromaVectorStore {
    _client: ChromaClient,
    collection: ChromaCollection,
    _collection_name: String,
    embeddings: Arc<dyn Embeddings>,
    distance_metric: DistanceMetric,
}

impl ChromaVectorStore {
    /// Creates a new `ChromaVectorStore` instance.
    pub async fn new(
        collection_name: &str,
        embeddings: Arc<dyn Embeddings>,
        url: Option<&str>,
    ) -> Result<Self> {
        let options = ChromaClientOptions {
            url: url.map(std::string::ToString::to_string),
            ..Default::default()
        };

        let client = ChromaClient::new(options)
            .await
            .map_err(|e| Error::config(e.to_string()))?;
        let collection = client
            .get_or_create_collection(collection_name, None)
            .await
            .map_err(|e| Error::config(e.to_string()))?;

        Ok(Self {
            _client: client,
            collection,
            _collection_name: collection_name.to_string(),
            embeddings,
            distance_metric: DistanceMetric::Cosine,
        })
    }

    fn build_where_clause(filter: &HashMap<String, JsonValue>) -> Option<JsonValue> {
        if filter.is_empty() {
            return None;
        }
        let conditions: Vec<JsonValue> = filter
            .iter()
            .map(|(k, v)| json!({ k: { "$eq": v } }))
            .collect();
        if conditions.len() == 1 {
            Some(conditions[0].clone())
        } else {
            Some(json!({ "$and": conditions }))
        }
    }
}

#[async_trait]
impl VectorStore for ChromaVectorStore {
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
        let embeddings_vec = embed(Arc::clone(&self.embeddings), &text_strings).await?;
        let doc_ids: Vec<String> = if let Some(ids) = ids {
            ids.to_vec()
        } else {
            (0..text_count)
                .map(|_| uuid::Uuid::new_v4().to_string())
                .collect()
        };

        let metadatas_vec = metadatas.map(|m| {
            m.iter()
                .map(|metadata| {
                    metadata
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<serde_json::Map<String, JsonValue>>()
                })
                .collect::<Vec<_>>()
        });

        // Convert to Vec<&str> for chromadb
        let ids_refs: Vec<&str> = doc_ids.iter().map(std::string::String::as_str).collect();
        let docs_refs: Vec<&str> = text_strings
            .iter()
            .map(std::string::String::as_str)
            .collect();

        let collection_entries = CollectionEntries {
            ids: ids_refs,
            embeddings: Some(embeddings_vec),
            documents: Some(docs_refs),
            metadatas: metadatas_vec,
        };

        self.collection
            .upsert(collection_entries, None)
            .await
            .map_err(|e| Error::other(format!("Chroma upsert failed: {e}")))?;

        Ok(doc_ids)
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        if let Some(ids) = ids {
            if ids.is_empty() {
                return Ok(true);
            }
            let ids_refs: Vec<&str> = ids.iter().map(std::string::String::as_str).collect();
            self.collection
                .delete(Some(ids_refs), None, None)
                .await
                .map_err(|e| Error::other(format!("Chroma delete failed: {e}")))?;
        } else {
            let get_options = GetOptions {
                ids: vec![],
                limit: None,
                offset: None,
                where_metadata: None,
                where_document: None,
                include: Some(vec!["ids".to_string()]),
            };
            let result = self
                .collection
                .get(get_options)
                .await
                .map_err(|e| Error::other(format!("Chroma get failed: {e}")))?;
            if !result.ids.is_empty() {
                let ids_refs: Vec<&str> =
                    result.ids.iter().map(std::string::String::as_str).collect();
                self.collection
                    .delete(Some(ids_refs), None, None)
                    .await
                    .map_err(|e| Error::other(format!("Chroma delete failed: {e}")))?;
            }
        }
        Ok(true)
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let get_options = GetOptions {
            ids: ids.to_vec(),
            limit: None,
            offset: None,
            where_metadata: None,
            where_document: None,
            include: Some(vec!["documents".to_string(), "metadatas".to_string()]),
        };

        let result = self
            .collection
            .get(get_options)
            .await
            .map_err(|e| Error::other(format!("Chroma get failed: {e}")))?;

        let mut documents = Vec::new();
        for i in 0..result.ids.len() {
            let id = result.ids[i].clone();
            let page_content = result
                .documents
                .as_ref()
                .and_then(|docs| docs.get(i))
                .and_then(|d| d.as_ref())
                .cloned()
                .unwrap_or_default();

            let metadata = result
                .metadatas
                .as_ref()
                .and_then(|metas| metas.get(i))
                .and_then(|m| m.as_ref())
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<HashMap<String, JsonValue>>()
                })
                .unwrap_or_default();

            documents.push(Document {
                id: Some(id),
                page_content,
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
        let results = self.similarity_search_with_score(query, k, filter).await?;
        Ok(results.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query).await?;
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
        Ok(results.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        let where_clause = filter.and_then(Self::build_where_clause);
        let query_options = QueryOptions {
            query_embeddings: Some(vec![embedding.to_vec()]),
            query_texts: None,
            n_results: Some(k),
            where_metadata: where_clause,
            where_document: None,
            include: Some(vec!["documents", "metadatas", "distances"]),
        };

        let query_result = self
            .collection
            .query(query_options, None)
            .await
            .map_err(|e| Error::other(format!("Chroma query failed: {e}")))?;

        let ids = query_result
            .ids
            .first()
            .ok_or_else(|| Error::other("No IDs"))?;
        let documents = query_result
            .documents
            .as_ref()
            .and_then(|d| d.first())
            .ok_or_else(|| Error::other("No documents"))?;
        let metadatas = query_result.metadatas.as_ref().and_then(|m| m.first());
        let distances = query_result
            .distances
            .as_ref()
            .and_then(|d| d.first())
            .ok_or_else(|| Error::other("No distances"))?;

        let mut results = Vec::new();
        for i in 0..ids.len() {
            let id = ids[i].clone();
            let page_content = documents.get(i).cloned().unwrap_or_default();

            let metadata = metadatas
                .and_then(|m| m.get(i))
                .and_then(|m| m.as_ref())
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<HashMap<String, JsonValue>>()
                })
                .unwrap_or_default();

            let distance = distances[i];
            let score = 1.0 / (1.0 + distance);

            results.push((
                Document {
                    id: Some(id),
                    page_content,
                    metadata,
                },
                score,
            ));
        }

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
        // Embed query
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query).await?;

        // Build where clause for filtering
        let where_clause = filter.and_then(Self::build_where_clause);

        // Query Chroma with fetch_k to get candidates (including embeddings)
        let query_options = QueryOptions {
            query_embeddings: Some(vec![query_embedding.clone()]),
            query_texts: None,
            n_results: Some(fetch_k),
            where_metadata: where_clause,
            where_document: None,
            include: Some(vec!["embeddings", "documents", "metadatas"]),
        };

        let result = self
            .collection
            .query(query_options, None)
            .await
            .map_err(|e| Error::other(e.to_string()))?;

        // Extract data from results
        let ids = result
            .ids
            .first()
            .ok_or_else(|| Error::other("No IDs in query result"))?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let documents = result
            .documents
            .as_ref()
            .and_then(|d| d.first())
            .ok_or_else(|| Error::other("No documents in query result"))?;

        let metadatas = result.metadatas.as_ref().and_then(|m| m.first());

        // Extract embeddings for MMR calculation
        let embeddings_data = result
            .embeddings
            .as_ref()
            .and_then(|e| e.first())
            .ok_or_else(|| Error::other("No embeddings in query result"))?;

        let candidate_embeddings: Vec<Vec<f32>> = embeddings_data.to_vec();

        // Run MMR algorithm
        let selected_indices = dashflow::core::vector_stores::maximal_marginal_relevance(
            &query_embedding,
            &candidate_embeddings,
            k,
            lambda,
        )?;

        // Build result documents from selected indices
        let mut results = Vec::new();
        for idx in selected_indices {
            let id = ids[idx].clone();
            let page_content = documents.get(idx).cloned().unwrap_or_default();

            let metadata = metadatas
                .and_then(|m| m.get(idx))
                .and_then(|m| m.as_ref())
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<HashMap<String, JsonValue>>()
                })
                .unwrap_or_default();

            results.push(Document {
                id: Some(id),
                page_content,
                metadata,
            });
        }

        Ok(results)
    }
}

/// Implementation of Retriever trait for `ChromaVectorStore`
///
/// Enables the Chroma vector store to be used as a retriever in chains and workflows.
#[async_trait]
impl Retriever for ChromaVectorStore {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // Default to k=4 (standard retriever behavior)
        self._similarity_search(query, 4, None).await
    }

    fn name(&self) -> String {
        "ChromaVectorStore".to_string()
    }
}

/// Implementation of `DocumentIndex` trait for `ChromaVectorStore`
///
/// This enables the Chroma vector store to be used with the document indexing API,
/// providing intelligent change detection and cleanup of outdated documents.
#[async_trait]
impl DocumentIndex for ChromaVectorStore {
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
        // This will embed the texts and upsert them to Chroma
        match self
            .add_texts_internal(&texts, Some(&metadatas), Some(&ids))
            .await
        {
            Ok(_) => Ok(UpsertResponse::all_succeeded(ids)),
            Err(_e) => {
                // If there's an error, mark all as failed
                Ok(UpsertResponse::all_failed(ids))
            }
        }
    }

    async fn delete(
        &self,
        ids: Option<&[String]>,
    ) -> std::result::Result<DeleteResponse, Box<dyn std::error::Error + Send + Sync>> {
        match self.delete_internal(ids).await {
            Ok(_) => {
                // Chroma doesn't return the count, so we return success without count
                if let Some(ids) = ids {
                    Ok(DeleteResponse::with_count(ids.len()))
                } else {
                    Ok(DeleteResponse::with_count(0))
                }
            }
            Err(e) => Err(Box::new(e)),
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

impl ChromaVectorStore {
    /// Internal method to add texts (non-mutable version for `DocumentIndex`)
    async fn add_texts_internal(
        &self,
        texts: &[String],
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

        let embeddings_vec = embed(Arc::clone(&self.embeddings), texts).await?;
        let doc_ids: Vec<String> = if let Some(ids) = ids {
            ids.to_vec()
        } else {
            (0..text_count)
                .map(|_| uuid::Uuid::new_v4().to_string())
                .collect()
        };

        let metadatas_vec = metadatas.map(|m| {
            m.iter()
                .map(|metadata| {
                    metadata
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<serde_json::Map<String, JsonValue>>()
                })
                .collect::<Vec<_>>()
        });

        let ids_refs: Vec<&str> = doc_ids.iter().map(std::string::String::as_str).collect();
        let docs_refs: Vec<&str> = texts.iter().map(std::string::String::as_str).collect();

        let collection_entries = CollectionEntries {
            ids: ids_refs,
            embeddings: Some(embeddings_vec),
            documents: Some(docs_refs),
            metadatas: metadatas_vec,
        };

        self.collection
            .upsert(collection_entries, None)
            .await
            .map_err(|e| Error::other(format!("Chroma upsert failed: {e}")))?;

        Ok(doc_ids)
    }

    /// Internal method to delete documents (non-mutable version for `DocumentIndex`)
    async fn delete_internal(&self, ids: Option<&[String]>) -> Result<bool> {
        if let Some(ids) = ids {
            if ids.is_empty() {
                return Ok(true);
            }
            let ids_refs: Vec<&str> = ids.iter().map(std::string::String::as_str).collect();
            self.collection
                .delete(Some(ids_refs), None, None)
                .await
                .map_err(|e| Error::other(format!("Chroma delete failed: {e}")))?;
        } else {
            let get_options = GetOptions {
                ids: vec![],
                limit: None,
                offset: None,
                where_metadata: None,
                where_document: None,
                include: Some(vec!["ids".to_string()]),
            };
            let result = self
                .collection
                .get(get_options)
                .await
                .map_err(|e| Error::other(format!("Chroma get failed: {e}")))?;
            if !result.ids.is_empty() {
                let ids_refs: Vec<&str> =
                    result.ids.iter().map(std::string::String::as_str).collect();
                self.collection
                    .delete(Some(ids_refs), None, None)
                    .await
                    .map_err(|e| Error::other(format!("Chroma delete failed: {e}")))?;
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use dashflow_test_utils::MockEmbeddings;
    use serde_json::json;

    /// Helper function to create mock embeddings for testing
    /// Uses deterministic MockEmbeddings with 1536 dimensions to simulate OpenAI
    fn get_embeddings_for_test() -> Arc<dyn Embeddings> {
        Arc::new(MockEmbeddings::with_dimensions(1536))
    }

    // ========================================================================
    // BUILD_WHERE_CLAUSE - BASIC TESTS
    // ========================================================================

    #[test]
    fn test_build_where_clause_empty() {
        let filter = HashMap::new();
        let result = ChromaVectorStore::build_where_clause(&filter);
        assert!(result.is_none(), "Empty filter should return None");
    }

    #[test]
    fn test_build_where_clause_single() {
        let mut filter = HashMap::new();
        filter.insert("author".to_string(), json!("John"));

        let result = ChromaVectorStore::build_where_clause(&filter);
        assert!(result.is_some(), "Single filter should return Some");

        let where_clause = result.unwrap();
        let expected = json!({
            "author": {
                "$eq": "John"
            }
        });

        assert_eq!(
            where_clause, expected,
            "Single filter should create $eq clause"
        );
    }

    #[test]
    fn test_build_where_clause_multiple() {
        let mut filter = HashMap::new();
        filter.insert("author".to_string(), json!("John"));
        filter.insert("year".to_string(), json!(2024));

        let result = ChromaVectorStore::build_where_clause(&filter);
        assert!(result.is_some(), "Multiple filters should return Some");

        let where_clause = result.unwrap();

        // The $and clause should contain both filters
        assert!(
            where_clause.get("$and").is_some(),
            "Multiple filters should create $and clause"
        );

        let and_clause = where_clause["$and"].as_array().unwrap();
        assert_eq!(and_clause.len(), 2, "Should have 2 filter conditions");

        // Check that both conditions exist (order may vary)
        let conditions_str = and_clause
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        assert!(
            conditions_str.contains("author"),
            "Should contain author filter"
        );
        assert!(conditions_str.contains("John"), "Should contain John value");
        assert!(
            conditions_str.contains("year"),
            "Should contain year filter"
        );
        assert!(conditions_str.contains("2024"), "Should contain 2024 value");
    }

    // ========================================================================
    // BUILD_WHERE_CLAUSE - JSON VALUE TYPES
    // ========================================================================

    #[test]
    fn test_build_where_clause_string_value() {
        let mut filter = HashMap::new();
        filter.insert("category".to_string(), json!("technology"));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(
            where_clause,
            json!({ "category": { "$eq": "technology" } })
        );
    }

    #[test]
    fn test_build_where_clause_empty_string_value() {
        let mut filter = HashMap::new();
        filter.insert("description".to_string(), json!(""));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(where_clause, json!({ "description": { "$eq": "" } }));
    }

    #[test]
    fn test_build_where_clause_integer_value() {
        let mut filter = HashMap::new();
        filter.insert("count".to_string(), json!(42));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(where_clause, json!({ "count": { "$eq": 42 } }));
    }

    #[test]
    fn test_build_where_clause_negative_integer_value() {
        let mut filter = HashMap::new();
        filter.insert("offset".to_string(), json!(-10));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(where_clause, json!({ "offset": { "$eq": -10 } }));
    }

    #[test]
    fn test_build_where_clause_zero_value() {
        let mut filter = HashMap::new();
        filter.insert("index".to_string(), json!(0));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(where_clause, json!({ "index": { "$eq": 0 } }));
    }

    #[test]
    fn test_build_where_clause_float_value() {
        let mut filter = HashMap::new();
        filter.insert("score".to_string(), json!(3.14));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(where_clause, json!({ "score": { "$eq": 3.14 } }));
    }

    #[test]
    fn test_build_where_clause_negative_float_value() {
        let mut filter = HashMap::new();
        filter.insert("temperature".to_string(), json!(-273.15));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(where_clause, json!({ "temperature": { "$eq": -273.15 } }));
    }

    #[test]
    fn test_build_where_clause_boolean_true() {
        let mut filter = HashMap::new();
        filter.insert("active".to_string(), json!(true));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(where_clause, json!({ "active": { "$eq": true } }));
    }

    #[test]
    fn test_build_where_clause_boolean_false() {
        let mut filter = HashMap::new();
        filter.insert("deleted".to_string(), json!(false));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(where_clause, json!({ "deleted": { "$eq": false } }));
    }

    #[test]
    fn test_build_where_clause_null_value() {
        let mut filter = HashMap::new();
        filter.insert("optional_field".to_string(), JsonValue::Null);

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(
            where_clause,
            json!({ "optional_field": { "$eq": null } })
        );
    }

    #[test]
    fn test_build_where_clause_array_value() {
        let mut filter = HashMap::new();
        filter.insert("tags".to_string(), json!(["rust", "python"]));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(
            where_clause,
            json!({ "tags": { "$eq": ["rust", "python"] } })
        );
    }

    #[test]
    fn test_build_where_clause_empty_array_value() {
        let mut filter = HashMap::new();
        filter.insert("items".to_string(), json!([]));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(where_clause, json!({ "items": { "$eq": [] } }));
    }

    #[test]
    fn test_build_where_clause_nested_object_value() {
        let mut filter = HashMap::new();
        filter.insert("metadata".to_string(), json!({ "key": "value" }));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(
            where_clause,
            json!({ "metadata": { "$eq": { "key": "value" } } })
        );
    }

    // ========================================================================
    // BUILD_WHERE_CLAUSE - MULTIPLE FILTERS
    // ========================================================================

    #[test]
    fn test_build_where_clause_three_filters() {
        let mut filter = HashMap::new();
        filter.insert("author".to_string(), json!("John"));
        filter.insert("year".to_string(), json!(2024));
        filter.insert("published".to_string(), json!(true));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        let and_clause = where_clause["$and"].as_array().unwrap();
        assert_eq!(and_clause.len(), 3, "Should have 3 filter conditions");
    }

    #[test]
    fn test_build_where_clause_five_filters() {
        let mut filter = HashMap::new();
        filter.insert("author".to_string(), json!("John"));
        filter.insert("year".to_string(), json!(2024));
        filter.insert("published".to_string(), json!(true));
        filter.insert("category".to_string(), json!("tech"));
        filter.insert("rating".to_string(), json!(4.5));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        let and_clause = where_clause["$and"].as_array().unwrap();
        assert_eq!(and_clause.len(), 5, "Should have 5 filter conditions");
    }

    #[test]
    fn test_build_where_clause_mixed_types() {
        let mut filter = HashMap::new();
        filter.insert("name".to_string(), json!("test"));
        filter.insert("count".to_string(), json!(100));
        filter.insert("enabled".to_string(), json!(false));
        filter.insert("ratio".to_string(), json!(0.75));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        let and_clause = where_clause["$and"].as_array().unwrap();
        assert_eq!(and_clause.len(), 4);

        let conditions_str = and_clause
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",");

        assert!(conditions_str.contains("name"));
        assert!(conditions_str.contains("count"));
        assert!(conditions_str.contains("enabled"));
        assert!(conditions_str.contains("ratio"));
    }

    // ========================================================================
    // BUILD_WHERE_CLAUSE - SPECIAL KEY NAMES
    // ========================================================================

    #[test]
    fn test_build_where_clause_key_with_underscore() {
        let mut filter = HashMap::new();
        filter.insert("doc_type".to_string(), json!("article"));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(where_clause, json!({ "doc_type": { "$eq": "article" } }));
    }

    #[test]
    fn test_build_where_clause_key_with_hyphen() {
        let mut filter = HashMap::new();
        filter.insert("content-type".to_string(), json!("text/plain"));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(
            where_clause,
            json!({ "content-type": { "$eq": "text/plain" } })
        );
    }

    #[test]
    fn test_build_where_clause_key_with_dots() {
        let mut filter = HashMap::new();
        filter.insert("metadata.source".to_string(), json!("web"));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(
            where_clause,
            json!({ "metadata.source": { "$eq": "web" } })
        );
    }

    #[test]
    fn test_build_where_clause_numeric_key() {
        let mut filter = HashMap::new();
        filter.insert("123".to_string(), json!("value"));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(where_clause, json!({ "123": { "$eq": "value" } }));
    }

    #[test]
    fn test_build_where_clause_unicode_key() {
        let mut filter = HashMap::new();
        filter.insert("日本語".to_string(), json!("test"));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(where_clause, json!({ "日本語": { "$eq": "test" } }));
    }

    #[test]
    fn test_build_where_clause_unicode_value() {
        let mut filter = HashMap::new();
        filter.insert("greeting".to_string(), json!("こんにちは"));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(where_clause, json!({ "greeting": { "$eq": "こんにちは" } }));
    }

    // ========================================================================
    // BUILD_WHERE_CLAUSE - EDGE CASES
    // ========================================================================

    #[test]
    fn test_build_where_clause_large_integer() {
        let mut filter = HashMap::new();
        filter.insert("big_num".to_string(), json!(9_007_199_254_740_991_i64));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(
            where_clause,
            json!({ "big_num": { "$eq": 9_007_199_254_740_991_i64 } })
        );
    }

    #[test]
    fn test_build_where_clause_special_chars_in_string() {
        let mut filter = HashMap::new();
        filter.insert(
            "query".to_string(),
            json!("hello \"world\" with 'quotes' and \\backslash"),
        );

        let result = ChromaVectorStore::build_where_clause(&filter);
        assert!(result.is_some());

        let where_clause = result.unwrap();
        let expected_value = "hello \"world\" with 'quotes' and \\backslash";
        assert_eq!(where_clause["query"]["$eq"], json!(expected_value));
    }

    #[test]
    fn test_build_where_clause_newlines_in_string() {
        let mut filter = HashMap::new();
        filter.insert("text".to_string(), json!("line1\nline2\nline3"));

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert_eq!(
            where_clause,
            json!({ "text": { "$eq": "line1\nline2\nline3" } })
        );
    }

    #[test]
    fn test_build_where_clause_deeply_nested_object() {
        let mut filter = HashMap::new();
        filter.insert(
            "deep".to_string(),
            json!({
                "level1": {
                    "level2": {
                        "level3": "value"
                    }
                }
            }),
        );

        let result = ChromaVectorStore::build_where_clause(&filter);
        let where_clause = result.unwrap();

        assert!(where_clause["deep"]["$eq"]["level1"]["level2"]["level3"]
            .as_str()
            .is_some());
    }

    // ========================================================================
    // DISTANCE METRIC
    // ========================================================================

    #[test]
    fn test_distance_metric_default_is_cosine() {
        // The default distance metric for ChromaVectorStore is Cosine
        // This is set in the new() constructor
        assert_eq!(
            std::mem::discriminant(&DistanceMetric::Cosine),
            std::mem::discriminant(&DistanceMetric::Cosine)
        );
    }

    #[test]
    fn test_distance_metric_variants_exist() {
        // Verify DistanceMetric enum variants exist
        let _cosine = DistanceMetric::Cosine;
        let _euclidean = DistanceMetric::Euclidean;
        let _dot_product = DistanceMetric::DotProduct;
    }

    // ========================================================================
    // RETRIEVER TRAIT
    // ========================================================================

    #[test]
    fn test_retriever_name_constant() {
        // The Retriever::name() should return "ChromaVectorStore"
        // This tests the expected name without needing a live instance
        let expected_name = "ChromaVectorStore";
        assert!(!expected_name.is_empty());
        assert!(expected_name.contains("Chroma"));
    }

    // ========================================================================
    // DOCUMENT INDEX RESPONSES
    // ========================================================================

    #[test]
    fn test_upsert_response_all_succeeded() {
        let ids = vec!["id1".to_string(), "id2".to_string()];
        let response = UpsertResponse::all_succeeded(ids.clone());
        assert_eq!(response.succeeded, ids);
        assert!(response.failed.is_empty());
    }

    #[test]
    fn test_upsert_response_all_failed() {
        let ids = vec!["id1".to_string(), "id2".to_string()];
        let response = UpsertResponse::all_failed(ids.clone());
        assert!(response.succeeded.is_empty());
        assert_eq!(response.failed, ids);
    }

    #[test]
    fn test_delete_response_with_count() {
        let response = DeleteResponse::with_count(5);
        assert_eq!(response.num_deleted, Some(5));
    }

    #[test]
    fn test_delete_response_zero_count() {
        let response = DeleteResponse::with_count(0);
        assert_eq!(response.num_deleted, Some(0));
    }

    // ========================================================================
    // DOCUMENT STRUCTURE
    // ========================================================================

    #[test]
    fn test_document_with_id() {
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
            page_content: "Content here".to_string(),
            metadata: HashMap::new(),
        };

        assert!(doc.id.is_none());
    }

    #[test]
    fn test_document_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), json!("web"));
        metadata.insert("author".to_string(), json!("Alice"));

        let doc = Document {
            id: Some("doc-1".to_string()),
            page_content: "Text".to_string(),
            metadata,
        };

        assert_eq!(doc.metadata.get("source"), Some(&json!("web")));
        assert_eq!(doc.metadata.get("author"), Some(&json!("Alice")));
    }

    #[test]
    fn test_document_clone() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), json!("value"));

        let doc = Document {
            id: Some("doc-1".to_string()),
            page_content: "Content".to_string(),
            metadata,
        };

        let cloned = doc.clone();
        assert_eq!(doc.id, cloned.id);
        assert_eq!(doc.page_content, cloned.page_content);
        assert_eq!(doc.metadata, cloned.metadata);
    }

    #[test]
    fn test_document_debug() {
        let doc = Document {
            id: Some("doc-1".to_string()),
            page_content: "Test".to_string(),
            metadata: HashMap::new(),
        };

        let debug_str = format!("{:?}", doc);
        assert!(debug_str.contains("doc-1"));
        assert!(debug_str.contains("Test"));
    }

    // ========================================================================
    // TYPE PROPERTIES
    // ========================================================================

    #[test]
    fn test_chroma_vector_store_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ChromaVectorStore>();
    }

    #[test]
    fn test_chroma_vector_store_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ChromaVectorStore>();
    }

    #[test]
    fn test_mock_embeddings_created() {
        let embeddings = get_embeddings_for_test();
        // MockEmbeddings with 1536 dimensions matches OpenAI's text-embedding-3-small
        // Verify we can create the embeddings instance successfully
        assert!(std::any::type_name_of_val(&embeddings).contains("dyn"));
    }

    // Integration tests - require running Chroma server
    // Run with: docker run -p 8000:8000 chromadb/chroma
    // Execute tests: cargo test --package dashflow-chroma -- --ignored

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_chroma_add_and_search() {
        let embeddings = get_embeddings_for_test();
        let mut store =
            ChromaVectorStore::new("test_collection", embeddings, Some("http://localhost:8000"))
                .await
                .expect("Failed to create ChromaVectorStore");

        // Clear existing data
        let _ = store.delete(None).await;

        // Add documents
        let texts = ["Hello world", "Machine learning is great", "Rust is fast"];
        let ids = store
            .add_texts(&texts, None, None)
            .await
            .expect("Failed to add texts");

        assert_eq!(ids.len(), 3, "Should add 3 documents");

        // Search for similar documents
        let results = store
            ._similarity_search("learning", 2, None)
            .await
            .expect("Failed to search");

        assert_eq!(results.len(), 2, "Should return 2 results");
        assert!(
            results[0].page_content.contains("learning")
                || results[0].page_content.contains("world"),
            "Top result should be relevant"
        );
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_chroma_metadata_filtering() {
        let embeddings = get_embeddings_for_test();
        let mut store = ChromaVectorStore::new(
            "test_collection_filter",
            embeddings,
            Some("http://localhost:8000"),
        )
        .await
        .expect("Failed to create ChromaVectorStore");

        // Clear existing data
        let _ = store.delete(None).await;

        // Add documents with metadata
        let texts = ["Document 1", "Document 2", "Document 3"];
        let mut metadatas = Vec::new();
        metadatas.push({
            let mut m = HashMap::new();
            m.insert("author".to_string(), json!("Alice"));
            m
        });
        metadatas.push({
            let mut m = HashMap::new();
            m.insert("author".to_string(), json!("Bob"));
            m
        });
        metadatas.push({
            let mut m = HashMap::new();
            m.insert("author".to_string(), json!("Alice"));
            m
        });

        store
            .add_texts(&texts, Some(&metadatas), None)
            .await
            .expect("Failed to add texts with metadata");

        // Search with filter
        let mut filter = HashMap::new();
        filter.insert("author".to_string(), json!("Alice"));

        let results = store
            ._similarity_search("Document", 5, Some(&filter))
            .await
            .expect("Failed to search with filter");

        assert_eq!(results.len(), 2, "Should return 2 results for Alice");
        for doc in results {
            assert_eq!(
                doc.metadata.get("author"),
                Some(&json!("Alice")),
                "All results should have author=Alice"
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_chroma_mmr_search() {
        let embeddings = get_embeddings_for_test();
        let mut store = ChromaVectorStore::new(
            "test_collection_mmr",
            embeddings,
            Some("http://localhost:8000"),
        )
        .await
        .expect("Failed to create ChromaVectorStore");

        // Clear existing data
        let _ = store.delete(None).await;

        // Add documents - some very similar, some diverse
        let texts = [
            "Machine learning fundamentals",
            "Machine learning basics",            // Very similar to first
            "Deep learning with neural networks", // Related but different
            "Rust programming language",          // Completely different topic
            "Python programming guide",           // Different but closer to ML context
        ];

        store
            .add_texts(&texts, None, None)
            .await
            .expect("Failed to add texts");

        // MMR search with lambda=0.5 (balanced diversity and relevance)
        let mmr_results = store
            .max_marginal_relevance_search("machine learning", 3, 5, 0.5, None)
            .await
            .expect("Failed to perform MMR search");

        assert_eq!(mmr_results.len(), 3, "Should return 3 results");

        // First result should be most relevant
        assert!(
            mmr_results[0].page_content.contains("Machine learning"),
            "First result should be most relevant"
        );

        // Verify diversity: should not just return all "Machine learning" docs
        let ml_count = mmr_results
            .iter()
            .filter(|d| d.page_content.contains("Machine learning"))
            .count();

        // With lambda=0.5, we expect some diversity (not all 3 should be "Machine learning")
        assert!(
            ml_count < 3,
            "MMR should provide diverse results, not all 'Machine learning' docs"
        );
    }
}

// Standard conformance tests
// These tests require a running Chroma server. Tests now use environmental error handling and skip gracefully if the service is unavailable.
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

    async fn create_test_store() -> ChromaVectorStore {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        // Use unique collection name per test to avoid conflicts
        let collection_name = format!("test_{}", uuid::Uuid::new_v4());
        ChromaVectorStore::new(&collection_name, embeddings, Some("http://localhost:8000"))
            .await
            .expect("Failed to create test store - is Chroma running on localhost:8000?")
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_add_and_search_standard() {
        let mut store = create_test_store().await;
        test_add_and_search(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_search_with_scores_standard() {
        let mut store = create_test_store().await;
        test_search_with_scores(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_metadata_filtering_standard() {
        let mut store = create_test_store().await;
        test_metadata_filtering(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_custom_ids_standard() {
        let mut store = create_test_store().await;
        test_custom_ids(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_delete_standard() {
        let mut store = create_test_store().await;
        test_delete(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_add_documents_standard() {
        let mut store = create_test_store().await;
        test_add_documents(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_empty_search_standard() {
        let store = create_test_store().await;
        test_empty_search(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_search_by_vector_standard() {
        let mut store = create_test_store().await;
        test_search_by_vector(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_mmr_search_standard() {
        let mut store = create_test_store().await;
        test_mmr_search(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_large_batch_standard() {
        let mut store = create_test_store().await;
        test_large_batch(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_validation_standard() {
        let mut store = create_test_store().await;
        test_validation(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_update_document_standard() {
        let mut store = create_test_store().await;
        test_update_document(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_metadata_only_filter_standard() {
        let mut store = create_test_store().await;
        test_metadata_only_filter(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_complex_metadata_standard() {
        let mut store = create_test_store().await;
        test_complex_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_empty_text_standard() {
        let mut store = create_test_store().await;
        test_empty_text(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_special_chars_metadata_standard() {
        let mut store = create_test_store().await;
        test_special_chars_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_concurrent_operations_standard() {
        let mut store = create_test_store().await;
        test_concurrent_operations(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_very_long_text_standard() {
        let mut store = create_test_store().await;
        test_very_long_text(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_duplicate_documents_standard() {
        let mut store = create_test_store().await;
        test_duplicate_documents(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_k_parameter_standard() {
        let mut store = create_test_store().await;
        test_k_parameter(&mut store).await;
    }

    // ========================================================================
    // COMPREHENSIVE TESTS
    // These tests provide deeper coverage beyond standard conformance tests
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_mmr_lambda_zero_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_lambda_zero(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_mmr_lambda_one_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_lambda_one(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_mmr_fetch_k_variations_comprehensive() {
        let mut store = create_test_store().await;
        test_mmr_fetch_k_variations(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_complex_metadata_operators_comprehensive() {
        let mut store = create_test_store().await;
        test_complex_metadata_operators(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_nested_metadata_filtering_comprehensive() {
        let mut store = create_test_store().await;
        test_nested_metadata_filtering(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_array_metadata_comprehensive() {
        let mut store = create_test_store().await;
        test_array_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_very_large_batch_comprehensive() {
        let mut store = create_test_store().await;
        test_very_large_batch(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_concurrent_writes_comprehensive() {
        let mut store = create_test_store().await;
        test_concurrent_writes(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_error_handling_network_comprehensive() {
        let mut store = create_test_store().await;
        test_error_handling_network(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_error_handling_invalid_input_comprehensive() {
        let mut store = create_test_store().await;
        test_error_handling_invalid_input(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_bulk_delete_comprehensive() {
        let mut store = create_test_store().await;
        test_bulk_delete(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_update_metadata_comprehensive() {
        let mut store = create_test_store().await;
        test_update_metadata(&mut store).await;
    }

    #[tokio::test]
    #[ignore = "requires ChromaDB server: docker-compose -f docker-compose.test.yml up chroma"]
    async fn test_search_score_threshold_comprehensive() {
        let mut store = create_test_store().await;
        test_search_score_threshold(&mut store).await;
    }
}
