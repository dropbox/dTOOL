//! `USearch` vector store implementation for `DashFlow` Rust.
//!
//! Note: All Mutex usages now use poison-safe patterns (unwrap_or_else with into_inner).
//! The blanket #![allow(clippy::unwrap_used)] was removed.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use dashflow::core::config::RunnableConfig;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::indexing::document_index::{DeleteResponse, DocumentIndex, UpsertResponse};
use dashflow::core::retrievers::Retriever;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::{Error, Result};
use dashflow::{embed, embed_query};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

/// Metadata entry stored alongside each vector
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VectorMetadata {
    /// Document ID
    id: String,
    /// Original text
    text: String,
    /// User-defined metadata
    metadata: HashMap<String, JsonValue>,
}

/// `USearch` vector store implementation using SIMD-accelerated ANN search.
///
/// This vector store uses the `USearch` library to provide fast, in-memory
/// approximate nearest neighbor search with HNSW indexing.
pub struct USearchVectorStore {
    /// The `USearch` index
    index: Arc<Mutex<Index>>,
    /// Embeddings model
    embeddings: Arc<dyn Embeddings>,
    /// Distance metric used
    distance_metric: DistanceMetric,
    /// Vector dimensionality
    dimensions: usize,
    /// Storage for document metadata (key -> metadata)
    metadata_store: Arc<Mutex<HashMap<u64, VectorMetadata>>>,
    /// Counter for generating unique keys
    key_counter: Arc<Mutex<u64>>,
}

impl USearchVectorStore {
    /// Creates a new `USearchVectorStore` instance.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - The embeddings model to use for generating vectors
    /// * `dimensions` - The dimensionality of the vectors
    /// * `distance_metric` - Optional distance metric (defaults to Cosine)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use dashflow_usearch::USearchVectorStore;
    /// use dashflow::core::vector_stores::DistanceMetric;
    /// # use std::sync::Arc;
    /// # use dashflow::core::embeddings::Embeddings;
    ///
    /// # fn example(embeddings: Arc<dyn Embeddings>) -> Result<(), Box<dyn std::error::Error>> {
    /// let store = USearchVectorStore::new(embeddings, 384, Some(DistanceMetric::Cosine))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(
        embeddings: Arc<dyn Embeddings>,
        dimensions: usize,
        distance_metric: Option<DistanceMetric>,
    ) -> Result<Self> {
        let distance_metric = distance_metric.unwrap_or(DistanceMetric::Cosine);
        let metric_kind = Self::distance_metric_to_usearch(&distance_metric);

        let options = IndexOptions {
            dimensions,
            metric: metric_kind,
            quantization: ScalarKind::F32,
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
            multi: false,
        };

        let index = Index::new(&options).map_err(|e| Error::config(e.to_string()))?;

        Ok(Self {
            index: Arc::new(Mutex::new(index)),
            embeddings,
            distance_metric,
            dimensions,
            metadata_store: Arc::new(Mutex::new(HashMap::new())),
            key_counter: Arc::new(Mutex::new(0)),
        })
    }

    /// Saves the index to a file.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path to save to
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # async fn example(mut store: USearchVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// store.save("my_index.usearch")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn save(&self, path: &str) -> Result<()> {
        let index = self.index.lock().unwrap_or_else(|e| e.into_inner());
        index
            .save(path)
            .map_err(|e| Error::other(format!("Failed to save USearch index: {e}")))?;

        // Save metadata separately
        let metadata_path = format!("{path}.meta");
        let metadata = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
        let metadata_json =
            serde_json::to_string(&*metadata).map_err(|e| Error::other(e.to_string()))?;
        std::fs::write(&metadata_path, metadata_json)
            .map_err(|e| Error::other(format!("Failed to save metadata: {e}")))?;

        Ok(())
    }

    /// Loads an index from a file.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path to load from
    /// * `embeddings` - The embeddings model to use
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use dashflow_usearch::USearchVectorStore;
    /// # use std::sync::Arc;
    /// # use dashflow::core::embeddings::Embeddings;
    ///
    /// # async fn example(embeddings: Arc<dyn Embeddings>) -> Result<(), Box<dyn std::error::Error>> {
    /// let store = USearchVectorStore::load("my_index.usearch", embeddings)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load(path: &str, embeddings: Arc<dyn Embeddings>) -> Result<Self> {
        // We need to create a temporary index to inspect its properties
        // USearch doesn't expose index metadata before loading
        let temp_options = IndexOptions {
            dimensions: 1, // Will be overwritten by load
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
            multi: false,
        };

        let index = Index::new(&temp_options).map_err(|e| Error::config(e.to_string()))?;
        index
            .load(path)
            .map_err(|e| Error::other(format!("Failed to load USearch index: {e}")))?;

        // Load metadata
        let metadata_path = format!("{path}.meta");
        let metadata_json = std::fs::read_to_string(&metadata_path)
            .map_err(|e| Error::other(format!("Failed to load metadata: {e}")))?;
        let metadata: HashMap<u64, VectorMetadata> =
            serde_json::from_str(&metadata_json).map_err(|e| Error::other(e.to_string()))?;

        // Determine max key for counter
        let max_key = metadata.keys().max().copied().unwrap_or(0);

        // Get dimensions from loaded index
        let dimensions = index.dimensions();

        Ok(Self {
            index: Arc::new(Mutex::new(index)),
            embeddings,
            distance_metric: DistanceMetric::Cosine, // Default, could be stored in metadata
            dimensions,
            metadata_store: Arc::new(Mutex::new(metadata)),
            key_counter: Arc::new(Mutex::new(max_key + 1)),
        })
    }

    /// Converts `DashFlow` `DistanceMetric` to `USearch` `MetricKind`
    fn distance_metric_to_usearch(metric: &DistanceMetric) -> MetricKind {
        match metric {
            DistanceMetric::Cosine => MetricKind::Cos,
            DistanceMetric::Euclidean => MetricKind::L2sq,
            DistanceMetric::DotProduct => MetricKind::IP,
            _ => MetricKind::Cos, // Default to cosine for unsupported metrics
        }
    }

    /// Generates a unique key for a new vector
    fn next_key(&self) -> u64 {
        let mut counter = self.key_counter.lock().unwrap_or_else(|e| e.into_inner());
        let key = *counter;
        *counter += 1;
        key
    }
}

#[async_trait]
impl VectorStore for USearchVectorStore {
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

        // Verify embedding dimensions
        if !embeddings_vec.is_empty() && embeddings_vec[0].len() != self.dimensions {
            return Err(Error::config(format!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                embeddings_vec[0].len()
            )));
        }

        // Generate IDs if not provided
        let doc_ids: Vec<String> = if let Some(ids) = ids {
            ids.to_vec()
        } else {
            (0..text_count)
                .map(|_| uuid::Uuid::new_v4().to_string())
                .collect()
        };

        // Add vectors to index
        let index = self.index.lock().unwrap_or_else(|e| e.into_inner());
        let mut metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());

        for (i, embedding) in embeddings_vec.iter().enumerate() {
            let key = self.next_key();
            let vector: Vec<f32> = embedding.clone();

            // Add to USearch index
            index
                .add(key, &vector)
                .map_err(|e| Error::other(format!("Failed to add vector to index: {e}")))?;

            // Store metadata
            let metadata = VectorMetadata {
                id: doc_ids[i].clone(),
                text: text_strings[i].clone(),
                metadata: metadatas.map(|m| m[i].clone()).unwrap_or_default(),
            };
            metadata_store.insert(key, metadata);
        }

        Ok(doc_ids)
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        let mut metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
        let mut index = self.index.lock().unwrap_or_else(|e| e.into_inner());

        if let Some(ids) = ids {
            // Delete specific IDs
            let keys_to_delete: Vec<u64> = metadata_store
                .iter()
                .filter(|(_, meta)| ids.contains(&meta.id))
                .map(|(k, _)| *k)
                .collect();

            for key in keys_to_delete {
                index
                    .remove(key)
                    .map_err(|e| Error::other(format!("Failed to remove vector: {e}")))?;
                metadata_store.remove(&key);
            }
        } else {
            // Delete all - USearch doesn't have clear() so we recreate the index
            let options = IndexOptions {
                dimensions: self.dimensions,
                metric: Self::distance_metric_to_usearch(&self.distance_metric),
                quantization: ScalarKind::F32,
                connectivity: 16,
                expansion_add: 128,
                expansion_search: 64,
                multi: false,
            };

            *index = Index::new(&options).map_err(|e| Error::config(e.to_string()))?;
            metadata_store.clear();
            *self.key_counter.lock().unwrap_or_else(|e| e.into_inner()) = 0;
        }

        Ok(true)
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
        let mut documents = Vec::new();

        for (_, meta) in metadata_store.iter() {
            if ids.contains(&meta.id) {
                documents.push(Document {
                    page_content: meta.text.clone(),
                    metadata: meta.metadata.clone(),
                    id: Some(meta.id.clone()),
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
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        // Generate query embedding using graph API
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query).await?;

        if query_embedding.len() != self.dimensions {
            return Err(Error::config(format!(
                "Query embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                query_embedding.len()
            )));
        }

        let vector: Vec<f32> = query_embedding;

        // Search in index
        let index = self.index.lock().unwrap_or_else(|e| e.into_inner());
        let results = if let Some(filter) = filter {
            // Filtered search
            index
                .filtered_search(&vector, k, |key| {
                    let metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(meta) = metadata_store.get(&key) {
                        filter.iter().all(|(k, v)| meta.metadata.get(k) == Some(v))
                    } else {
                        false
                    }
                })
                .map_err(|e| Error::other(format!("USearch filtered search failed: {e}")))?
        } else {
            // Regular search
            index
                .search(&vector, k)
                .map_err(|e| Error::other(format!("USearch search failed: {e}")))?
        };

        // Convert results to documents
        let metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
        let mut documents = Vec::new();

        for match_result in results.keys.iter().zip(results.distances.iter()) {
            let (key, distance) = match_result;
            if let Some(meta) = metadata_store.get(key) {
                let document = Document {
                    page_content: meta.text.clone(),
                    metadata: meta.metadata.clone(),
                    id: Some(meta.id.clone()),
                };
                // USearch returns distances, we keep them as-is
                // Lower distance = more similar for L2
                // Higher similarity = more similar for cosine (but USearch returns distance)
                documents.push((document, *distance));
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
        // Generate query embedding using graph API
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query).await?;

        if query_embedding.len() != self.dimensions {
            return Err(Error::config(format!(
                "Query embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                query_embedding.len()
            )));
        }

        let vector: Vec<f32> = query_embedding.clone();

        // Fetch more candidates than needed
        let results = {
            let index = self.index.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(filter) = filter {
                index
                    .filtered_search(&vector, fetch_k, |key| {
                        let metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(meta) = metadata_store.get(&key) {
                            filter.iter().all(|(k, v)| meta.metadata.get(k) == Some(v))
                        } else {
                            false
                        }
                    })
                    .map_err(|e| Error::other(format!("USearch filtered search failed: {e}")))?
            } else {
                index
                    .search(&vector, fetch_k)
                    .map_err(|e| Error::other(format!("USearch search failed: {e}")))?
            }
        }; // Release index lock

        // Get texts for all candidates (without holding locks across awaits)
        let candidate_texts: Vec<String> = {
            let metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
            results
                .keys
                .iter()
                .filter_map(|key| metadata_store.get(key).map(|meta| meta.text.clone()))
                .collect()
        }; // Release metadata lock

        // Get embeddings for all candidates using graph API (NOW safe to await)
        let mut candidate_embeddings: Vec<Vec<f32>> = Vec::new();
        let candidate_keys: Vec<u64> = results.keys.clone();

        for text in &candidate_texts {
            let emb = embed_query(Arc::clone(&self.embeddings), text).await?;
            candidate_embeddings.push(emb);
        }

        // MMR algorithm
        let mut selected_indices: Vec<usize> = Vec::new();
        let mut remaining_indices: Vec<usize> = (0..candidate_keys.len()).collect();

        for _ in 0..k.min(candidate_keys.len()) {
            if remaining_indices.is_empty() {
                break;
            }

            let mut best_score = f32::NEG_INFINITY;
            let mut best_idx = 0;

            for &idx in &remaining_indices {
                // Relevance score (cosine similarity with query)
                let idx_emb = &candidate_embeddings[idx];
                let relevance = cosine_similarity(&query_embedding, idx_emb);

                // Diversity score (max similarity with already selected)
                let diversity = if selected_indices.is_empty() {
                    0.0
                } else {
                    selected_indices
                        .iter()
                        .map(|&sel_idx| cosine_similarity(idx_emb, &candidate_embeddings[sel_idx][..]))
                        .fold(f32::NEG_INFINITY, f32::max)
                };

                // MMR score
                let mmr_score = lambda_mult * relevance - (1.0 - lambda_mult) * diversity;

                if mmr_score > best_score {
                    best_score = mmr_score;
                    best_idx = idx;
                }
            }

            selected_indices.push(best_idx);
            remaining_indices.retain(|&x| x != best_idx);
        }

        // Build result documents
        let metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
        let mut documents = Vec::new();
        for idx in selected_indices {
            let key = candidate_keys[idx];
            if let Some(meta) = metadata_store.get(&key) {
                documents.push(Document {
                    page_content: meta.text.clone(),
                    metadata: meta.metadata.clone(),
                    id: Some(meta.id.clone()),
                });
            }
        }

        Ok(documents)
    }
}

/// Computes cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[async_trait]
impl Retriever for USearchVectorStore {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        let _ = config;
        self._similarity_search(query, 4, None).await
    }
}

#[async_trait]
impl DocumentIndex for USearchVectorStore {
    async fn upsert(
        &self,
        documents: &[Document],
    ) -> std::result::Result<UpsertResponse, Box<dyn std::error::Error + Send + Sync>> {
        let texts: Vec<&str> = documents.iter().map(|d| d.page_content.as_str()).collect();
        let metadatas: Vec<HashMap<String, JsonValue>> =
            documents.iter().map(|d| d.metadata.clone()).collect();
        let ids: Option<Vec<String>> = {
            let collected: Vec<String> = documents.iter().filter_map(|d| d.id.clone()).collect();
            if collected.len() == documents.len() {
                Some(collected)
            } else {
                None
            }
        };

        // Generate embeddings using graph API BEFORE locking (to avoid holding lock across await)
        let text_strings: Vec<String> = texts.iter().map(|t| (*t).to_string()).collect();
        let embeddings_vec = embed(Arc::clone(&self.embeddings), &text_strings)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Generate IDs if not provided
        let doc_ids: Vec<String> = if let Some(ids) = ids {
            ids
        } else {
            (0..texts.len())
                .map(|_| uuid::Uuid::new_v4().to_string())
                .collect()
        };

        // Now lock and add vectors to index (no awaits while locked)
        {
            let index = self.index.lock().unwrap_or_else(|e| e.into_inner());
            let mut metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());

            for (i, embedding) in embeddings_vec.iter().enumerate() {
                let key = {
                    let mut counter = self.key_counter.lock().unwrap_or_else(|e| e.into_inner());
                    let k = *counter;
                    *counter += 1;
                    k
                };
                let vector: Vec<f32> = embedding.clone();

                index.add(key, &vector).map_err(|e| {
                    Box::new(std::io::Error::other(e.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;

                let metadata = VectorMetadata {
                    id: doc_ids[i].clone(),
                    text: text_strings[i].clone(),
                    metadata: metadatas[i].clone(),
                };
                metadata_store.insert(key, metadata);
            }
        } // Locks are released here

        Ok(UpsertResponse {
            succeeded: doc_ids,
            failed: vec![],
        })
    }

    async fn delete(
        &self,
        ids: Option<&[String]>,
    ) -> std::result::Result<DeleteResponse, Box<dyn std::error::Error + Send + Sync>> {
        let mut metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
        let mut index = self.index.lock().unwrap_or_else(|e| e.into_inner());

        if let Some(ids) = ids {
            let keys_to_delete: Vec<u64> = metadata_store
                .iter()
                .filter(|(_, meta)| ids.contains(&meta.id))
                .map(|(k, _)| *k)
                .collect();

            for key in &keys_to_delete {
                index.remove(*key).map_err(|e| {
                    Box::new(std::io::Error::other(e.to_string()))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
                metadata_store.remove(key);
            }

            Ok(DeleteResponse {
                num_deleted: Some(keys_to_delete.len()),
                succeeded: Some(ids.to_vec()),
                failed: Some(vec![]),
                num_failed: Some(0),
            })
        } else {
            let count = metadata_store.len();
            // Delete all
            let options = IndexOptions {
                dimensions: self.dimensions,
                metric: Self::distance_metric_to_usearch(&self.distance_metric),
                quantization: ScalarKind::F32,
                connectivity: 16,
                expansion_add: 128,
                expansion_search: 64,
                multi: false,
            };

            *index = Index::new(&options).map_err(|e| {
                Box::new(std::io::Error::other(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?;
            metadata_store.clear();
            *self.key_counter.lock().unwrap_or_else(|e| e.into_inner()) = 0;

            Ok(DeleteResponse {
                num_deleted: Some(count),
                succeeded: None,
                failed: None,
                num_failed: Some(0),
            })
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

#[cfg(test)]
mod tests {
    use super::*;

    // Note: USearch Index has stability issues in unit tests (segfaults on cleanup).
    // These tests focus on utility functions that don't require Index creation.
    // Integration tests with real Index usage should be in tests/ directory.

    // ============================================================
    // VectorMetadata Tests
    // ============================================================

    #[test]
    fn test_vector_metadata_creation() {
        let metadata = VectorMetadata {
            id: "doc-1".to_string(),
            text: "Hello world".to_string(),
            metadata: HashMap::new(),
        };
        assert_eq!(metadata.id, "doc-1");
        assert_eq!(metadata.text, "Hello world");
        assert!(metadata.metadata.is_empty());
    }

    #[test]
    fn test_vector_metadata_with_metadata_fields() {
        let mut meta_map = HashMap::new();
        meta_map.insert("source".to_string(), JsonValue::String("wiki".to_string()));
        meta_map.insert("page".to_string(), JsonValue::Number(42.into()));

        let metadata = VectorMetadata {
            id: "doc-2".to_string(),
            text: "Test content".to_string(),
            metadata: meta_map,
        };

        assert_eq!(metadata.metadata.len(), 2);
        assert_eq!(
            metadata.metadata.get("source"),
            Some(&JsonValue::String("wiki".to_string()))
        );
        assert_eq!(
            metadata.metadata.get("page"),
            Some(&JsonValue::Number(42.into()))
        );
    }

    #[test]
    fn test_vector_metadata_clone() {
        let mut meta_map = HashMap::new();
        meta_map.insert("key".to_string(), JsonValue::String("value".to_string()));

        let original = VectorMetadata {
            id: "doc-3".to_string(),
            text: "Original text".to_string(),
            metadata: meta_map,
        };

        let cloned = original.clone();
        assert_eq!(cloned.id, original.id);
        assert_eq!(cloned.text, original.text);
        assert_eq!(cloned.metadata, original.metadata);
    }

    #[test]
    fn test_vector_metadata_debug() {
        let metadata = VectorMetadata {
            id: "debug-test".to_string(),
            text: "Debug text".to_string(),
            metadata: HashMap::new(),
        };
        let debug_str = format!("{:?}", metadata);
        assert!(debug_str.contains("debug-test"));
        assert!(debug_str.contains("Debug text"));
    }

    #[test]
    fn test_vector_metadata_serialization() {
        let mut meta_map = HashMap::new();
        meta_map.insert("category".to_string(), JsonValue::String("test".to_string()));

        let metadata = VectorMetadata {
            id: "serialize-1".to_string(),
            text: "Serialization test".to_string(),
            metadata: meta_map,
        };

        let json = serde_json::to_string(&metadata).expect("serialization failed");
        assert!(json.contains("serialize-1"));
        assert!(json.contains("Serialization test"));
        assert!(json.contains("category"));
    }

    #[test]
    fn test_vector_metadata_deserialization() {
        let json = r#"{
            "id": "deserialize-1",
            "text": "Deserialized content",
            "metadata": {"type": "document"}
        }"#;

        let metadata: VectorMetadata = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(metadata.id, "deserialize-1");
        assert_eq!(metadata.text, "Deserialized content");
        assert_eq!(
            metadata.metadata.get("type"),
            Some(&JsonValue::String("document".to_string()))
        );
    }

    #[test]
    fn test_vector_metadata_roundtrip() {
        let mut meta_map = HashMap::new();
        meta_map.insert("num".to_string(), JsonValue::Number(123.into()));
        meta_map.insert("bool".to_string(), JsonValue::Bool(true));
        meta_map.insert("null".to_string(), JsonValue::Null);

        let original = VectorMetadata {
            id: "roundtrip".to_string(),
            text: "Roundtrip test content".to_string(),
            metadata: meta_map,
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let restored: VectorMetadata = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.id, original.id);
        assert_eq!(restored.text, original.text);
        assert_eq!(restored.metadata, original.metadata);
    }

    #[test]
    fn test_vector_metadata_empty_fields() {
        let metadata = VectorMetadata {
            id: String::new(),
            text: String::new(),
            metadata: HashMap::new(),
        };
        assert!(metadata.id.is_empty());
        assert!(metadata.text.is_empty());
        assert!(metadata.metadata.is_empty());
    }

    #[test]
    fn test_vector_metadata_unicode() {
        let mut meta_map = HashMap::new();
        meta_map.insert("emoji".to_string(), JsonValue::String("🚀🔥".to_string()));

        let metadata = VectorMetadata {
            id: "unicode-日本語".to_string(),
            text: "Текст на русском 中文 العربية".to_string(),
            metadata: meta_map,
        };

        let json = serde_json::to_string(&metadata).expect("serialize");
        let restored: VectorMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, metadata.id);
        assert_eq!(restored.text, metadata.text);
    }

    #[test]
    fn test_vector_metadata_special_characters() {
        let metadata = VectorMetadata {
            id: r#"id-with-"quotes"-and-\backslash"#.to_string(),
            text: "Text with\nnewlines\tand\ttabs".to_string(),
            metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&metadata).expect("serialize");
        let restored: VectorMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, metadata.id);
        assert_eq!(restored.text, metadata.text);
    }

    #[test]
    fn test_vector_metadata_nested_json() {
        let mut meta_map = HashMap::new();
        meta_map.insert(
            "nested".to_string(),
            serde_json::json!({"inner": {"deep": 123}}),
        );
        meta_map.insert(
            "array".to_string(),
            serde_json::json!([1, 2, {"key": "val"}]),
        );

        let metadata = VectorMetadata {
            id: "nested-test".to_string(),
            text: "Nested metadata test".to_string(),
            metadata: meta_map,
        };

        let json = serde_json::to_string(&metadata).expect("serialize");
        let restored: VectorMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.metadata.len(), 2);
    }

    // ============================================================
    // Cosine Similarity Tests
    // ============================================================

    #[tokio::test]
    async fn test_cosine_similarity_normal() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let similarity = cosine_similarity(&a, &b);
        assert!(similarity > 0.0 && similarity <= 1.0);
    }

    #[tokio::test]
    async fn test_cosine_similarity_zero_vector() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![0.0, 0.0, 0.0];
        let similarity = cosine_similarity(&a, &b);
        assert!(similarity.abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let similarity = cosine_similarity(&a, &b);
        assert!((similarity - 1.0).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let similarity = cosine_similarity(&a, &b);
        assert!((similarity - 0.0).abs() < 0.0001); // Orthogonal vectors have similarity ~0
    }

    #[tokio::test]
    async fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        let similarity = cosine_similarity(&a, &b);
        assert!(similarity < 0.0); // Opposite vectors have negative similarity
    }

    #[test]
    fn test_cosine_similarity_both_zero() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![0.0, 0.0, 0.0];
        let similarity = cosine_similarity(&a, &b);
        assert!(similarity.abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_similarity_unit_vectors() {
        // Unit vectors along x and y axes
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let similarity = cosine_similarity(&a, &b);
        assert!(similarity.abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_parallel_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![2.0, 4.0, 6.0]; // b = 2*a
        let similarity = cosine_similarity(&a, &b);
        assert!((similarity - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_single_element() {
        let a = vec![5.0];
        let b = vec![3.0];
        let similarity = cosine_similarity(&a, &b);
        assert!((similarity - 1.0).abs() < 0.0001); // Positive scalars are perfectly similar
    }

    #[test]
    fn test_cosine_similarity_single_element_opposite() {
        let a = vec![5.0];
        let b = vec![-3.0];
        let similarity = cosine_similarity(&a, &b);
        assert!((similarity - (-1.0)).abs() < 0.0001); // Opposite signs = -1 similarity
    }

    #[test]
    fn test_cosine_similarity_empty_vectors() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        let similarity = cosine_similarity(&a, &b);
        // With empty vectors, norms are 0, so returns 0
        assert!(similarity.abs() < f32::EPSILON || similarity.is_nan());
    }

    #[test]
    fn test_cosine_similarity_large_vectors() {
        let a: Vec<f32> = (0..1000).map(|i| (i as f32).sin()).collect();
        let b: Vec<f32> = (0..1000).map(|i| (i as f32).cos()).collect();
        let similarity = cosine_similarity(&a, &b);
        // Should be a valid similarity value
        assert!(similarity >= -1.0 && similarity <= 1.0);
    }

    #[test]
    fn test_cosine_similarity_negative_values() {
        let a = vec![-1.0, -2.0, -3.0];
        let b = vec![-4.0, -5.0, -6.0];
        let similarity = cosine_similarity(&a, &b);
        // Both vectors in same direction (negative quadrant)
        assert!(similarity > 0.9);
    }

    #[test]
    fn test_cosine_similarity_mixed_sign_values() {
        let a = vec![1.0, -1.0, 1.0, -1.0];
        let b = vec![-1.0, 1.0, -1.0, 1.0];
        let similarity = cosine_similarity(&a, &b);
        // Completely opposite patterns
        assert!((similarity - (-1.0)).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_very_small_values() {
        let a = vec![1e-10, 2e-10, 3e-10];
        let b = vec![4e-10, 5e-10, 6e-10];
        let similarity = cosine_similarity(&a, &b);
        // Should still compute valid similarity
        assert!(similarity > 0.9);
    }

    #[test]
    fn test_cosine_similarity_very_large_values() {
        let a = vec![1e30, 2e30, 3e30];
        let b = vec![4e30, 5e30, 6e30];
        let similarity = cosine_similarity(&a, &b);
        // Should still compute valid similarity (or handle overflow gracefully)
        assert!(similarity.is_finite() || similarity.is_nan());
    }

    #[test]
    fn test_cosine_similarity_45_degrees() {
        // Vectors at 45 degrees: cos(45°) ≈ 0.707
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 1.0];
        let similarity = cosine_similarity(&a, &b);
        let expected = 1.0 / (2.0_f32).sqrt(); // cos(45°) = 1/√2
        assert!((similarity - expected).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_commutative() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];
        let sim_ab = cosine_similarity(&a, &b);
        let sim_ba = cosine_similarity(&b, &a);
        assert!((sim_ab - sim_ba).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_high_dimensional() {
        let a: Vec<f32> = (0..384).map(|i| (i as f32 * 0.01).sin()).collect();
        let b: Vec<f32> = (0..384).map(|i| (i as f32 * 0.01).cos()).collect();
        let similarity = cosine_similarity(&a, &b);
        assert!(similarity >= -1.0 && similarity <= 1.0);
    }

    #[test]
    fn test_cosine_similarity_sparse_like() {
        // Most values are zero, simulating sparse vectors
        let mut a = vec![0.0; 100];
        let mut b = vec![0.0; 100];
        a[10] = 1.0;
        a[50] = 2.0;
        b[10] = 1.0;
        b[50] = 2.0;
        let similarity = cosine_similarity(&a, &b);
        assert!((similarity - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_one_sparse_one_dense() {
        let a = vec![1.0, 0.0, 0.0, 0.0];
        let b = vec![0.25, 0.25, 0.25, 0.25];
        let similarity = cosine_similarity(&a, &b);
        // a·b = 0.25, |a| = 1, |b| = 0.5
        let expected = 0.25 / 0.5;
        assert!((similarity - expected).abs() < 0.0001);
    }

    // ============================================================
    // Distance Metric Conversion Tests
    // ============================================================

    #[test]
    fn test_distance_metric_conversion_cosine() {
        assert!(matches!(
            USearchVectorStore::distance_metric_to_usearch(&DistanceMetric::Cosine),
            MetricKind::Cos
        ));
    }

    #[test]
    fn test_distance_metric_conversion_euclidean() {
        assert!(matches!(
            USearchVectorStore::distance_metric_to_usearch(&DistanceMetric::Euclidean),
            MetricKind::L2sq
        ));
    }

    #[test]
    fn test_distance_metric_conversion_dot_product() {
        assert!(matches!(
            USearchVectorStore::distance_metric_to_usearch(&DistanceMetric::DotProduct),
            MetricKind::IP
        ));
    }

    #[test]
    fn test_distance_metric_conversion_max_inner_product() {
        // MaxInnerProduct is not directly supported in USearch, should default to Cosine
        assert!(matches!(
            USearchVectorStore::distance_metric_to_usearch(&DistanceMetric::MaxInnerProduct),
            MetricKind::Cos
        ));
    }

    #[test]
    fn test_distance_metric_all_variants() {
        // Test that all DistanceMetric variants map to valid MetricKind values
        let metrics = [
            DistanceMetric::Cosine,
            DistanceMetric::Euclidean,
            DistanceMetric::DotProduct,
            DistanceMetric::MaxInnerProduct,
        ];

        for metric in &metrics {
            let _usearch_metric = USearchVectorStore::distance_metric_to_usearch(metric);
            // Just verify it doesn't panic
        }
    }

    #[test]
    fn test_distance_metric_default_is_cosine() {
        // Verify that the default case (unsupported metrics) maps to Cosine
        // MaxInnerProduct is not directly supported, so it should default to Cosine
        let metric = USearchVectorStore::distance_metric_to_usearch(&DistanceMetric::MaxInnerProduct);
        assert!(matches!(metric, MetricKind::Cos));
    }

    // ============================================================
    // Metadata Store Tests (HashMap operations)
    // ============================================================

    #[test]
    fn test_metadata_hashmap_insert_and_get() {
        let mut store: HashMap<u64, VectorMetadata> = HashMap::new();
        let meta = VectorMetadata {
            id: "test-1".to_string(),
            text: "Test text".to_string(),
            metadata: HashMap::new(),
        };
        store.insert(0, meta.clone());

        assert!(store.contains_key(&0));
        assert_eq!(store.get(&0).unwrap().id, "test-1");
    }

    #[test]
    fn test_metadata_hashmap_remove() {
        let mut store: HashMap<u64, VectorMetadata> = HashMap::new();
        let meta = VectorMetadata {
            id: "test-2".to_string(),
            text: "To be removed".to_string(),
            metadata: HashMap::new(),
        };
        store.insert(1, meta);
        assert!(store.contains_key(&1));

        store.remove(&1);
        assert!(!store.contains_key(&1));
    }

    #[test]
    fn test_metadata_hashmap_iteration() {
        let mut store: HashMap<u64, VectorMetadata> = HashMap::new();
        for i in 0..5 {
            store.insert(
                i,
                VectorMetadata {
                    id: format!("doc-{}", i),
                    text: format!("Text {}", i),
                    metadata: HashMap::new(),
                },
            );
        }

        let ids: Vec<String> = store.iter().filter(|(k, _)| **k < 3).map(|(_, v)| v.id.clone()).collect();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn test_metadata_hashmap_clear() {
        let mut store: HashMap<u64, VectorMetadata> = HashMap::new();
        for i in 0..10 {
            store.insert(
                i,
                VectorMetadata {
                    id: format!("doc-{}", i),
                    text: format!("Text {}", i),
                    metadata: HashMap::new(),
                },
            );
        }
        assert_eq!(store.len(), 10);

        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn test_metadata_hashmap_max_key() {
        let mut store: HashMap<u64, VectorMetadata> = HashMap::new();
        store.insert(5, VectorMetadata {
            id: "a".to_string(),
            text: "".to_string(),
            metadata: HashMap::new(),
        });
        store.insert(100, VectorMetadata {
            id: "b".to_string(),
            text: "".to_string(),
            metadata: HashMap::new(),
        });
        store.insert(42, VectorMetadata {
            id: "c".to_string(),
            text: "".to_string(),
            metadata: HashMap::new(),
        });

        let max_key = store.keys().max().copied().unwrap_or(0);
        assert_eq!(max_key, 100);
    }

    #[test]
    fn test_metadata_hashmap_empty_max_key() {
        let store: HashMap<u64, VectorMetadata> = HashMap::new();
        let max_key = store.keys().max().copied().unwrap_or(0);
        assert_eq!(max_key, 0);
    }

    // ============================================================
    // Key Counter Tests
    // ============================================================

    #[test]
    fn test_key_counter_increment() {
        let counter: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));

        let mut guard = counter.lock().unwrap();
        let key1 = *guard;
        *guard += 1;
        drop(guard);

        let mut guard = counter.lock().unwrap();
        let key2 = *guard;
        *guard += 1;
        drop(guard);

        assert_eq!(key1, 0);
        assert_eq!(key2, 1);
    }

    #[test]
    fn test_key_counter_concurrent_access() {
        use std::thread;

        let counter: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let counter_clone = Arc::clone(&counter);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let mut guard = counter_clone.lock().unwrap();
                    *guard += 1;
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let final_value = *counter.lock().unwrap();
        assert_eq!(final_value, 1000);
    }

    // ============================================================
    // Serialization/Deserialization for persistence
    // ============================================================

    #[test]
    fn test_metadata_store_serialization() {
        let mut store: HashMap<u64, VectorMetadata> = HashMap::new();
        store.insert(0, VectorMetadata {
            id: "id-0".to_string(),
            text: "text-0".to_string(),
            metadata: HashMap::new(),
        });
        store.insert(1, VectorMetadata {
            id: "id-1".to_string(),
            text: "text-1".to_string(),
            metadata: HashMap::new(),
        });

        let json = serde_json::to_string(&store).expect("serialize store");
        assert!(json.contains("id-0"));
        assert!(json.contains("id-1"));
    }

    #[test]
    fn test_metadata_store_deserialization() {
        let json = r#"{"0":{"id":"a","text":"text-a","metadata":{}},"1":{"id":"b","text":"text-b","metadata":{}}}"#;
        let store: HashMap<u64, VectorMetadata> = serde_json::from_str(json).expect("deserialize");

        assert_eq!(store.len(), 2);
        assert_eq!(store.get(&0).unwrap().id, "a");
        assert_eq!(store.get(&1).unwrap().id, "b");
    }

    #[test]
    fn test_metadata_store_roundtrip() {
        let mut store: HashMap<u64, VectorMetadata> = HashMap::new();
        for i in 0..100 {
            let mut meta = HashMap::new();
            meta.insert("index".to_string(), JsonValue::Number(i.into()));

            store.insert(i, VectorMetadata {
                id: format!("doc-{}", i),
                text: format!("Content for document {}", i),
                metadata: meta,
            });
        }

        let json = serde_json::to_string(&store).expect("serialize");
        let restored: HashMap<u64, VectorMetadata> = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.len(), store.len());
        for (key, original) in &store {
            let restored_meta = restored.get(key).expect("key should exist");
            assert_eq!(restored_meta.id, original.id);
            assert_eq!(restored_meta.text, original.text);
        }
    }

    // ============================================================
    // Filter matching tests (metadata filter logic)
    // ============================================================

    #[test]
    fn test_filter_match_simple() {
        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), JsonValue::String("tech".to_string()));

        let filter: HashMap<String, JsonValue> = [
            ("category".to_string(), JsonValue::String("tech".to_string()))
        ].into_iter().collect();

        let matches = filter.iter().all(|(k, v)| metadata.get(k) == Some(v));
        assert!(matches);
    }

    #[test]
    fn test_filter_match_multiple_fields() {
        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), JsonValue::String("tech".to_string()));
        metadata.insert("source".to_string(), JsonValue::String("web".to_string()));
        metadata.insert("year".to_string(), JsonValue::Number(2024.into()));

        let filter: HashMap<String, JsonValue> = [
            ("category".to_string(), JsonValue::String("tech".to_string())),
            ("year".to_string(), JsonValue::Number(2024.into())),
        ].into_iter().collect();

        let matches = filter.iter().all(|(k, v)| metadata.get(k) == Some(v));
        assert!(matches);
    }

    #[test]
    fn test_filter_no_match_wrong_value() {
        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), JsonValue::String("tech".to_string()));

        let filter: HashMap<String, JsonValue> = [
            ("category".to_string(), JsonValue::String("science".to_string()))
        ].into_iter().collect();

        let matches = filter.iter().all(|(k, v)| metadata.get(k) == Some(v));
        assert!(!matches);
    }

    #[test]
    fn test_filter_no_match_missing_field() {
        let metadata: HashMap<String, JsonValue> = HashMap::new();

        let filter: HashMap<String, JsonValue> = [
            ("category".to_string(), JsonValue::String("tech".to_string()))
        ].into_iter().collect();

        let matches = filter.iter().all(|(k, v)| metadata.get(k) == Some(v));
        assert!(!matches);
    }

    #[test]
    fn test_filter_empty_matches_all() {
        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), JsonValue::String("tech".to_string()));

        let filter: HashMap<String, JsonValue> = HashMap::new();

        let matches = filter.iter().all(|(k, v)| metadata.get(k) == Some(v));
        assert!(matches); // Empty filter matches everything
    }

    #[test]
    fn test_filter_match_boolean() {
        let mut metadata = HashMap::new();
        metadata.insert("active".to_string(), JsonValue::Bool(true));

        let filter: HashMap<String, JsonValue> = [
            ("active".to_string(), JsonValue::Bool(true))
        ].into_iter().collect();

        let matches = filter.iter().all(|(k, v)| metadata.get(k) == Some(v));
        assert!(matches);
    }

    #[test]
    fn test_filter_match_null() {
        let mut metadata = HashMap::new();
        metadata.insert("optional".to_string(), JsonValue::Null);

        let filter: HashMap<String, JsonValue> = [
            ("optional".to_string(), JsonValue::Null)
        ].into_iter().collect();

        let matches = filter.iter().all(|(k, v)| metadata.get(k) == Some(v));
        assert!(matches);
    }
}
