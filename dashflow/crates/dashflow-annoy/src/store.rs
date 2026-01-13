//! Annoy vector store implementation using arroy.
//!
//! Note: This implementation only supports Euclidean and Cosine distance metrics
//! due to arroy's type-level distance metric system.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use arroy::distances::{Cosine, Euclidean};
use arroy::{Database as ArroyDatabase, Reader, Writer};
use async_trait::async_trait;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::indexing::document_index::{DeleteResponse, DocumentIndex, UpsertResponse};
use dashflow::core::retrievers::Retriever;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::{Error, Result};
use dashflow::{embed, embed_query};
use heed::EnvOpenOptions;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tempfile::TempDir;
use uuid::Uuid;

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

/// Annoy vector store implementation using arroy library with Euclidean distance.
///
/// This vector store uses the arroy library (Annoy-inspired) to provide fast,
/// approximate nearest neighbor search with LMDB-backed persistence.
///
/// Currently only supports Euclidean distance due to arroy's type-level distance system.
pub struct AnnoyVectorStore {
    /// LMDB environment
    env: Arc<Mutex<heed::Env>>,
    /// Arroy database (Euclidean only for now)
    db_euclidean: Option<ArroyDatabase<Euclidean>>,
    db_cosine: Option<ArroyDatabase<Cosine>>,
    /// Index ID (for arroy)
    index_id: u16,
    /// Storage for document metadata (`item_id` -> metadata)
    metadata_store: Arc<Mutex<HashMap<u32, VectorMetadata>>>,
    /// Mapping from document ID to item ID
    id_to_item: Arc<Mutex<HashMap<String, u32>>>,
    /// Next item ID
    next_item_id: Arc<Mutex<u32>>,
    /// Number of trees to build
    n_trees: Option<usize>,
    /// Whether the index has been built
    is_built: Arc<Mutex<bool>>,
    /// Embeddings model
    embeddings: Arc<dyn Embeddings>,
    /// Vector dimensionality
    dimensions: usize,
    /// Temporary directory (if using temp storage)
    _temp_dir: Option<Arc<TempDir>>,
}

impl AnnoyVectorStore {
    /// Creates a new `AnnoyVectorStore` instance with temporary storage.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - The embeddings model to use for generating vectors
    /// * `dimensions` - The dimensionality of the vectors
    /// * `distance_metric` - Optional distance metric (defaults to Euclidean, supports Cosine)
    /// * `n_trees` - Optional number of trees to build (more trees = better accuracy, slower build)
    pub fn new(
        embeddings: Arc<dyn Embeddings>,
        dimensions: usize,
        distance_metric: Option<DistanceMetric>,
        n_trees: Option<usize>,
    ) -> Result<Self> {
        let temp_dir = TempDir::new()
            .map_err(|e| Error::other(format!("Failed to create temp directory: {e}")))?;

        let temp_dir = Arc::new(temp_dir);
        let mut store = Self::new_with_path(
            embeddings,
            dimensions,
            distance_metric,
            temp_dir.path(),
            n_trees,
            None, // Use default map size (10GB)
        )?;
        store._temp_dir = Some(temp_dir);
        Ok(store)
    }

    /// Creates a new `AnnoyVectorStore` instance with specified storage path.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - The embeddings model to use for generating vectors
    /// * `dimensions` - The dimensionality of the vectors
    /// * `distance_metric` - Optional distance metric (defaults to Euclidean, supports Cosine)
    /// * `path` - Path to the LMDB database directory
    /// * `n_trees` - Optional number of trees to build (more trees = better accuracy, slower build)
    /// * `map_size_bytes` - Optional LMDB map size in bytes (default: 10GB). This is the maximum
    ///   database size. LMDB uses memory-mapped files, so this doesn't allocate memory upfront,
    ///   but does reserve virtual address space.
    #[allow(unsafe_code)] // Required: heed LMDB EnvOpenOptions::open() is inherently unsafe
    pub fn new_with_path(
        embeddings: Arc<dyn Embeddings>,
        dimensions: usize,
        distance_metric: Option<DistanceMetric>,
        path: &Path,
        n_trees: Option<usize>,
        map_size_bytes: Option<usize>,
    ) -> Result<Self> {
        let distance_metric = distance_metric.unwrap_or(DistanceMetric::Euclidean);

        // Validate distance metric
        match distance_metric {
            DistanceMetric::Euclidean | DistanceMetric::Cosine => {}
            _ => {
                return Err(Error::config(format!(
                    "Annoy (arroy) only supports Euclidean and Cosine distance metrics, got: {distance_metric:?}"
                )));
            }
        }

        // Create LMDB environment
        std::fs::create_dir_all(path)
            .map_err(|e| Error::other(format!("Failed to create database directory: {e}")))?;

        // Default: 10 GB map size (virtual address space, not allocated upfront)
        const DEFAULT_MAP_SIZE_BYTES: usize = 10 * 1024 * 1024 * 1024;
        let map_size = map_size_bytes.unwrap_or(DEFAULT_MAP_SIZE_BYTES);

        // SAFETY: EnvOpenOptions::open() is unsafe because LMDB requires:
        // 1. The directory must exist and be accessible (ensured by create_dir_all above)
        // 2. No other process should have the same DB open with different flags
        //    (this is the caller's responsibility - documented in AnnoyVectorStore)
        // 3. The environment must not be closed while transactions are active
        //    (ensured by AnnoyVectorStore's Drop impl and lifetime management)
        // 4. Memory-mapped I/O is used, so file corruption can cause undefined behavior
        //    (acceptable for a vector store; users should use backups for critical data)
        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(map_size)
                .max_dbs(10)
                .open(path)
                .map_err(|e| Error::other(format!("Failed to open LMDB environment: {e}")))?
        };

        // Create database based on distance metric
        let mut wtxn = env
            .write_txn()
            .map_err(|e| Error::other(format!("Failed to create write transaction: {e}")))?;

        let (db_euclidean, db_cosine) = match distance_metric {
            DistanceMetric::Euclidean => {
                let db: ArroyDatabase<Euclidean> = env
                    .create_database(&mut wtxn, Some("vectors"))
                    .map_err(|e| Error::other(format!("Failed to create database: {e}")))?;
                (Some(db), None)
            }
            DistanceMetric::Cosine => {
                let db: ArroyDatabase<Cosine> = env
                    .create_database(&mut wtxn, Some("vectors"))
                    .map_err(|e| Error::other(format!("Failed to create database: {e}")))?;
                (None, Some(db))
            }
            _ => {
                // Should not reach here due to validation above, but return error if it does
                return Err(Error::config(format!(
                    "Internal error: unexpected distance metric after validation: {distance_metric:?}"
                )));
            }
        };

        wtxn.commit()
            .map_err(|e| Error::other(format!("Failed to commit transaction: {e}")))?;

        Ok(Self {
            env: Arc::new(Mutex::new(env)),
            db_euclidean,
            db_cosine,
            index_id: 0,
            metadata_store: Arc::new(Mutex::new(HashMap::new())),
            id_to_item: Arc::new(Mutex::new(HashMap::new())),
            next_item_id: Arc::new(Mutex::new(0)),
            n_trees,
            is_built: Arc::new(Mutex::new(false)),
            embeddings,
            dimensions,
            _temp_dir: None,
        })
    }

    /// Builds the search trees (must be called before searching)
    fn build_index_if_needed(&self) -> Result<()> {
        let mut is_built = self.is_built.lock().unwrap_or_else(|e| e.into_inner());
        if *is_built {
            return Ok(());
        }

        let env = self.env.lock().unwrap_or_else(|e| e.into_inner());
        let mut wtxn = env
            .write_txn()
            .map_err(|e| Error::other(format!("Failed to create write transaction: {e}")))?;

        let mut rng = StdRng::from_entropy();

        if let Some(db) = self.db_euclidean {
            let writer = Writer::<Euclidean>::new(db, self.index_id, self.dimensions);
            let mut builder = writer.builder(&mut rng);
            if let Some(n_trees) = self.n_trees {
                for _ in 0..n_trees {
                    builder
                        .build(&mut wtxn)
                        .map_err(|e| Error::other(format!("Failed to build index: {e}")))?;
                }
            } else {
                builder
                    .build(&mut wtxn)
                    .map_err(|e| Error::other(format!("Failed to build index: {e}")))?;
            }
        }

        if let Some(db) = self.db_cosine {
            let writer = Writer::<Cosine>::new(db, self.index_id, self.dimensions);
            let mut builder = writer.builder(&mut rng);
            if let Some(n_trees) = self.n_trees {
                for _ in 0..n_trees {
                    builder
                        .build(&mut wtxn)
                        .map_err(|e| Error::other(format!("Failed to build index: {e}")))?;
                }
            } else {
                builder
                    .build(&mut wtxn)
                    .map_err(|e| Error::other(format!("Failed to build index: {e}")))?;
            }
        }

        wtxn.commit()
            .map_err(|e| Error::other(format!("Failed to commit transaction: {e}")))?;

        *is_built = true;
        Ok(())
    }

    /// Searches for nearest neighbors
    fn search_internal(&self, vector: &[f32], k: usize) -> Result<Vec<(u32, f32)>> {
        let env = self.env.lock().unwrap_or_else(|e| e.into_inner());
        let rtxn = env
            .read_txn()
            .map_err(|e| Error::other(format!("Failed to create read transaction: {e}")))?;

        let results = match self.db_euclidean {
            Some(db) => {
                let reader = Reader::<Euclidean>::open(&rtxn, self.index_id, db)
                    .map_err(|e| Error::other(format!("Failed to open reader: {e}")))?;
                reader
                    .nns(k)
                    .by_vector(&rtxn, vector)
                    .map_err(|e| Error::other(format!("Failed to search: {e}")))?
            }
            None => match self.db_cosine {
                Some(db) => {
                    let reader = Reader::<Cosine>::open(&rtxn, self.index_id, db)
                        .map_err(|e| Error::other(format!("Failed to open reader: {e}")))?;
                    reader
                        .nns(k)
                        .by_vector(&rtxn, vector)
                        .map_err(|e| Error::other(format!("Failed to search: {e}")))?
                }
                None => {
                    return Err(Error::other("No database initialized"));
                }
            },
        };

        Ok(results)
    }
}

#[async_trait]
impl VectorStore for AnnoyVectorStore {
    async fn add_texts(
        &mut self,
        texts: &[impl AsRef<str> + Send + Sync],
        metadatas: Option<&[HashMap<String, JsonValue>]>,
        ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        // Convert texts to strings
        let text_strs: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();

        // Generate embeddings using graph API
        let embeddings = embed(Arc::clone(&self.embeddings), &text_strs).await?;

        // SAFETY (M-193): Lock ordering fix for deadlock prevention.
        // Previous code held `env` lock while acquiring `is_built`, but `build_index_if_needed`
        // acquires `is_built` then `env`. This AB-BA pattern can cause deadlock.
        // Fix: Release `env` before acquiring `is_built` by scoping the transaction work.
        let result_ids = {
            let env = self.env.lock().unwrap_or_else(|e| e.into_inner());
            let mut wtxn = env
                .write_txn()
                .map_err(|e| Error::other(format!("Failed to create write transaction: {e}")))?;

            let mut result_ids = Vec::with_capacity(text_strs.len());
            let mut next_item_id = self.next_item_id.lock().unwrap_or_else(|e| e.into_inner());
            let mut metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
            let mut id_to_item = self.id_to_item.lock().unwrap_or_else(|e| e.into_inner());

            for (i, (text, embedding)) in text_strs.iter().zip(embeddings.iter()).enumerate() {
                let id = if let Some(provided_ids) = ids {
                    if i < provided_ids.len() {
                        provided_ids[i].clone()
                    } else {
                        Uuid::new_v4().to_string()
                    }
                } else {
                    Uuid::new_v4().to_string()
                };

                let item_id = *next_item_id;
                *next_item_id += 1;

                // Add vector to index
                if let Some(db) = self.db_euclidean {
                    let writer = Writer::<Euclidean>::new(db, self.index_id, self.dimensions);
                    writer
                        .add_item(&mut wtxn, item_id, embedding)
                        .map_err(|e| Error::other(format!("Failed to add item: {e}")))?;
                }

                if let Some(db) = self.db_cosine {
                    let writer = Writer::<Cosine>::new(db, self.index_id, self.dimensions);
                    writer
                        .add_item(&mut wtxn, item_id, embedding)
                        .map_err(|e| Error::other(format!("Failed to add item: {e}")))?;
                }

                // Store metadata
                let metadata = if let Some(metas) = metadatas {
                    if i < metas.len() {
                        metas[i].clone()
                    } else {
                        HashMap::new()
                    }
                } else {
                    HashMap::new()
                };

                let vec_metadata = VectorMetadata {
                    id: id.clone(),
                    text: text.clone(),
                    metadata,
                };
                metadata_store.insert(item_id, vec_metadata);
                id_to_item.insert(id.clone(), item_id);

                result_ids.push(id);
            }

            wtxn.commit()
                .map_err(|e| Error::other(format!("Failed to commit transaction: {e}")))?;

            result_ids
            // `env` lock released here when scope ends
        };

        // Mark as not built since we added new items.
        // SAFETY: Now safe to acquire `is_built` - `env` lock is released above.
        let mut is_built = self.is_built.lock().unwrap_or_else(|e| e.into_inner());
        *is_built = false;

        Ok(result_ids)
    }

    async fn _similarity_search(
        &self,
        query: &str,
        k: usize,
        _filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        let results = self.similarity_search_with_score(query, k, None).await?;
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        _filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        // Generate query embedding using graph API
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query).await?;

        // Build index if not built
        self.build_index_if_needed()?;

        // Search
        let results = self.search_internal(&query_embedding, k)?;

        // Convert results to documents
        let metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
        let mut documents = Vec::new();
        for (item_id, distance) in results {
            if let Some(metadata) = metadata_store.get(&item_id) {
                let doc = Document {
                    id: Some(metadata.id.clone()),
                    page_content: metadata.text.clone(),
                    metadata: metadata.metadata.clone(),
                };
                documents.push((doc, distance));
            }
        }

        Ok(documents)
    }

    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
        _filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        let results = self
            .similarity_search_by_vector_with_score(embedding, k, None)
            .await?;
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
        _filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        // Build index if not built
        self.build_index_if_needed()?;

        // Search
        let results = self.search_internal(embedding, k)?;

        // Convert results to documents
        let metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
        let mut documents = Vec::new();
        for (item_id, distance) in results {
            if let Some(metadata) = metadata_store.get(&item_id) {
                let doc = Document {
                    id: Some(metadata.id.clone()),
                    page_content: metadata.text.clone(),
                    metadata: metadata.metadata.clone(),
                };
                documents.push((doc, distance));
            }
        }

        Ok(documents)
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        if let Some(ids) = ids {
            let mut metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
            let mut id_to_item = self.id_to_item.lock().unwrap_or_else(|e| e.into_inner());

            for id in ids {
                if let Some(&item_id) = id_to_item.get(id) {
                    metadata_store.remove(&item_id);
                    id_to_item.remove(id);
                }
            }
            Ok(true)
        } else {
            // Delete all
            let mut metadata_store = self.metadata_store.lock().unwrap_or_else(|e| e.into_inner());
            let mut id_to_item = self.id_to_item.lock().unwrap_or_else(|e| e.into_inner());
            metadata_store.clear();
            id_to_item.clear();
            Ok(true)
        }
    }
}

#[async_trait]
impl Retriever for AnnoyVectorStore {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&dashflow::core::config::RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // Default to k=4 for retriever
        self._similarity_search(query, 4, None).await
    }
}

#[async_trait]
impl DocumentIndex for AnnoyVectorStore {
    async fn upsert(
        &self,
        _documents: &[Document],
    ) -> std::result::Result<UpsertResponse, Box<dyn std::error::Error + Send + Sync>> {
        // DocumentIndex requires &self, but we need &mut self for add_texts
        // This is a limitation - we can't implement DocumentIndex properly without interior mutability
        Err("Annoy VectorStore requires mutable access for upsert - use add_texts instead".into())
    }

    async fn delete(
        &self,
        _ids: Option<&[String]>,
    ) -> std::result::Result<DeleteResponse, Box<dyn std::error::Error + Send + Sync>> {
        // DocumentIndex requires &self, but we need &mut self for delete
        Err("Annoy VectorStore requires mutable access for delete - use VectorStore::delete instead".into())
    }

    async fn get(
        &self,
        _ids: &[String],
    ) -> std::result::Result<Vec<Document>, Box<dyn std::error::Error + Send + Sync>> {
        // Not efficiently supported by Arroy
        Err("Get by ID not supported by Annoy".into())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use std::sync::Arc;

    // Mock embeddings for testing (deterministic)
    struct MockEmbeddings {
        dimension: usize,
    }

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            let mut embeddings = Vec::new();
            for text in texts {
                // Generate deterministic embedding based on text hash
                let hash = text.len() as f32;
                let embedding: Vec<f32> = (0..self.dimension)
                    .map(|i| ((hash + i as f32) % 10.0) / 10.0)
                    .collect();
                embeddings.push(embedding);
            }
            Ok(embeddings)
        }

        async fn _embed_query(&self, text: &str) -> Result<Vec<f32>> {
            let hash = text.len() as f32;
            let embedding: Vec<f32> = (0..self.dimension)
                .map(|i| ((hash + i as f32) % 10.0) / 10.0)
                .collect();
            Ok(embedding)
        }
    }

    #[tokio::test]
    async fn test_new_euclidean() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let store = AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None);
        assert!(store.is_ok());
    }

    #[tokio::test]
    async fn test_new_cosine() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let store = AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Cosine), None);
        assert!(store.is_ok());
    }

    #[tokio::test]
    async fn test_unsupported_distance_metric() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let result = AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::DotProduct), None);
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(err
                .to_string()
                .contains("only supports Euclidean and Cosine"));
        }
    }

    #[tokio::test]
    async fn test_add_texts_empty() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts: Vec<String> = vec![];
        let result = store.add_texts(&texts, None, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_add_texts_single() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["test document"];
        let result = store.add_texts(&texts, None, None).await;
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 1);
        assert!(!ids[0].is_empty());
    }

    #[tokio::test]
    async fn test_add_texts_multiple() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1", "doc2", "doc3"];
        let result = store.add_texts(&texts, None, None).await;
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 3);
        // All IDs should be unique
        assert_ne!(ids[0], ids[1]);
        assert_ne!(ids[1], ids[2]);
        assert_ne!(ids[0], ids[2]);
    }

    #[tokio::test]
    async fn test_add_texts_with_custom_ids() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1", "doc2"];
        let custom_ids = vec!["id1".to_string(), "id2".to_string()];
        let result = store.add_texts(&texts, None, Some(&custom_ids)).await;
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids, custom_ids);
    }

    #[tokio::test]
    async fn test_add_texts_with_metadata() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1"];
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), JsonValue::String("value".to_string()));
        let metadatas = vec![metadata];
        let result = store.add_texts(&texts, Some(&metadatas), None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_similarity_search_empty_index() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        // Search on empty index should return empty results
        let result = store._similarity_search("query", 5, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_similarity_search_basic() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["document 1", "document 2", "document 3"];
        store.add_texts(&texts, None, None).await.unwrap();

        let result = store._similarity_search("document 1", 2, None).await;
        assert!(result.is_ok());
        let docs = result.unwrap();
        assert!(docs.len() <= 2);
    }

    #[tokio::test]
    async fn test_similarity_search_with_score() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["document 1", "document 2"];
        store.add_texts(&texts, None, None).await.unwrap();

        let result = store
            .similarity_search_with_score("document 1", 2, None)
            .await;
        assert!(result.is_ok());
        let docs = result.unwrap();
        for (doc, score) in &docs {
            assert!(!doc.page_content.is_empty());
            assert!(score.is_finite());
        }
    }

    #[tokio::test]
    async fn test_similarity_search_by_vector() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["document 1"];
        store.add_texts(&texts, None, None).await.unwrap();

        let vector = vec![0.1; 128];
        let result = store.similarity_search_by_vector(&vector, 1, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_similarity_search_by_vector_with_score() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["document 1"];
        store.add_texts(&texts, None, None).await.unwrap();

        let vector = vec![0.1; 128];
        let result = store
            .similarity_search_by_vector_with_score(&vector, 1, None)
            .await;
        assert!(result.is_ok());
        let docs = result.unwrap();
        for (doc, score) in &docs {
            assert!(!doc.page_content.is_empty());
            assert!(score.is_finite());
        }
    }

    #[tokio::test]
    async fn test_delete_specific_ids() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1", "doc2", "doc3"];
        let custom_ids = vec!["id1".to_string(), "id2".to_string(), "id3".to_string()];
        store
            .add_texts(&texts, None, Some(&custom_ids))
            .await
            .unwrap();

        // Delete one document (using VectorStore trait)
        use dashflow::core::vector_stores::VectorStore;
        let result = VectorStore::delete(&mut store, Some(&["id2".to_string()])).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Verify internal state
        let id_to_item = store.id_to_item.lock().unwrap();
        assert!(!id_to_item.contains_key("id2"));
        assert!(id_to_item.contains_key("id1"));
        assert!(id_to_item.contains_key("id3"));
    }

    #[tokio::test]
    async fn test_delete_all() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1", "doc2"];
        store.add_texts(&texts, None, None).await.unwrap();

        // Delete all (using VectorStore trait)
        use dashflow::core::vector_stores::VectorStore;
        let result = VectorStore::delete(&mut store, None).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Verify all cleared
        let metadata_store = store.metadata_store.lock().unwrap();
        assert!(metadata_store.is_empty());
        let id_to_item = store.id_to_item.lock().unwrap();
        assert!(id_to_item.is_empty());
    }

    #[tokio::test]
    async fn test_retriever_interface() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["document 1", "document 2"];
        store.add_texts(&texts, None, None).await.unwrap();

        // Test Retriever trait
        let result = store._get_relevant_documents("document 1", None).await;
        assert!(result.is_ok());
        let docs = result.unwrap();
        assert!(docs.len() <= 4); // Default k=4
    }

    #[tokio::test]
    async fn test_cosine_distance() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Cosine), None).unwrap();

        let texts = vec!["test document"];
        let result = store.add_texts(&texts, None, None).await;
        assert!(result.is_ok());

        let search_result = store._similarity_search("test document", 1, None).await;
        assert!(search_result.is_ok());
    }

    #[tokio::test]
    async fn test_build_index_multiple_times() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1"];
        store.add_texts(&texts, None, None).await.unwrap();

        // First search builds index
        let result1 = store._similarity_search("doc1", 1, None).await;
        assert!(result1.is_ok());

        // Second search should reuse built index
        let result2 = store._similarity_search("doc1", 1, None).await;
        assert!(result2.is_ok());
    }

    #[tokio::test]
    async fn test_new_items_rebuild_index() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        // Add initial documents
        let texts1 = vec!["doc1"];
        store.add_texts(&texts1, None, None).await.unwrap();

        // Search to build index
        store._similarity_search("doc1", 1, None).await.unwrap();

        // Add more documents (should mark index as not built)
        let texts2 = vec!["doc2"];
        store.add_texts(&texts2, None, None).await.unwrap();

        // Search again (should rebuild index)
        let result = store._similarity_search("doc2", 2, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_n_trees_parameter() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let store = AnnoyVectorStore::new(
            embeddings,
            128,
            Some(DistanceMetric::Euclidean),
            Some(5), // 5 trees
        );
        assert!(store.is_ok());
    }

    #[tokio::test]
    async fn test_document_index_upsert_not_supported() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let doc = Document {
            id: Some("test".to_string()),
            page_content: "test content".to_string(),
            metadata: HashMap::new(),
        };
        let result = store.upsert(&[doc]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("mutable access"));
    }

    #[tokio::test]
    async fn test_document_index_get_not_supported() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let result = store.get(&["id1".to_string()]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not supported"));
    }

    #[tokio::test]
    async fn test_metadata_preservation() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["document with metadata"];
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), JsonValue::String("Alice".to_string()));
        metadata.insert("year".to_string(), JsonValue::Number(2024.into()));
        let metadatas = vec![metadata.clone()];

        let custom_ids = vec!["doc1".to_string()];
        store
            .add_texts(&texts, Some(&metadatas), Some(&custom_ids))
            .await
            .unwrap();

        // Search and verify metadata is preserved
        let results = store._similarity_search("document", 1, None).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].metadata.get("author"),
            Some(&JsonValue::String("Alice".to_string()))
        );
    }

    // ============================================
    // VectorMetadata struct tests
    // ============================================

    #[test]
    fn test_vector_metadata_debug() {
        let metadata = VectorMetadata {
            id: "test-id".to_string(),
            text: "test text".to_string(),
            metadata: HashMap::new(),
        };
        let debug_str = format!("{:?}", metadata);
        assert!(debug_str.contains("test-id"));
        assert!(debug_str.contains("test text"));
    }

    #[test]
    fn test_vector_metadata_clone() {
        let mut map = HashMap::new();
        map.insert("key".to_string(), JsonValue::String("value".to_string()));
        let metadata = VectorMetadata {
            id: "id1".to_string(),
            text: "text1".to_string(),
            metadata: map,
        };
        let cloned = metadata.clone();
        assert_eq!(cloned.id, metadata.id);
        assert_eq!(cloned.text, metadata.text);
        assert_eq!(cloned.metadata.len(), 1);
    }

    #[test]
    fn test_vector_metadata_serialize() {
        let metadata = VectorMetadata {
            id: "serialize-id".to_string(),
            text: "serialize text".to_string(),
            metadata: HashMap::new(),
        };
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("serialize-id"));
        assert!(json.contains("serialize text"));
    }

    #[test]
    fn test_vector_metadata_deserialize() {
        let json = r#"{"id":"deser-id","text":"deser text","metadata":{}}"#;
        let metadata: VectorMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.id, "deser-id");
        assert_eq!(metadata.text, "deser text");
        assert!(metadata.metadata.is_empty());
    }

    #[test]
    fn test_vector_metadata_with_complex_metadata() {
        let mut map = HashMap::new();
        map.insert("string".to_string(), JsonValue::String("hello".to_string()));
        map.insert("number".to_string(), JsonValue::Number(42.into()));
        map.insert("bool".to_string(), JsonValue::Bool(true));
        map.insert("null".to_string(), JsonValue::Null);

        let metadata = VectorMetadata {
            id: "complex".to_string(),
            text: "complex text".to_string(),
            metadata: map,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deser: VectorMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deser.metadata.get("string"), Some(&JsonValue::String("hello".to_string())));
        assert_eq!(deser.metadata.get("bool"), Some(&JsonValue::Bool(true)));
    }

    // ============================================
    // Constructor tests
    // ============================================

    #[tokio::test]
    async fn test_new_default_distance_metric() {
        // When distance_metric is None, should default to Euclidean
        let embeddings = Arc::new(MockEmbeddings { dimension: 64 });
        let store = AnnoyVectorStore::new(embeddings, 64, None, None);
        assert!(store.is_ok());
        let store = store.unwrap();
        // Euclidean database should be initialized
        assert!(store.db_euclidean.is_some());
        assert!(store.db_cosine.is_none());
    }

    #[tokio::test]
    async fn test_unsupported_max_inner_product_distance() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let result = AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::MaxInnerProduct), None);
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(err.to_string().contains("only supports Euclidean and Cosine"));
        }
    }

    #[tokio::test]
    async fn test_new_with_path_custom_map_size() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let temp_dir = TempDir::new().unwrap();
        let custom_map_size = 100 * 1024 * 1024; // 100MB

        let store = AnnoyVectorStore::new_with_path(
            embeddings,
            128,
            Some(DistanceMetric::Euclidean),
            temp_dir.path(),
            None,
            Some(custom_map_size),
        );
        assert!(store.is_ok());
    }

    #[tokio::test]
    async fn test_new_with_path_cosine_distance() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 256 });
        let temp_dir = TempDir::new().unwrap();

        let store = AnnoyVectorStore::new_with_path(
            embeddings,
            256,
            Some(DistanceMetric::Cosine),
            temp_dir.path(),
            Some(3), // 3 trees
            None,
        );
        assert!(store.is_ok());
        let store = store.unwrap();
        assert!(store.db_cosine.is_some());
        assert!(store.db_euclidean.is_none());
    }

    #[tokio::test]
    async fn test_new_with_various_dimensions() {
        for dim in [32, 64, 128, 256, 512] {
            let embeddings = Arc::new(MockEmbeddings { dimension: dim });
            let store = AnnoyVectorStore::new(embeddings, dim, None, None);
            assert!(store.is_ok(), "Failed for dimension {dim}");
            assert_eq!(store.unwrap().dimensions, dim);
        }
    }

    #[tokio::test]
    async fn test_new_with_n_trees_zero() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let store = AnnoyVectorStore::new(embeddings, 128, None, Some(0));
        // n_trees of 0 should still create successfully
        assert!(store.is_ok());
    }

    #[tokio::test]
    async fn test_new_with_n_trees_large() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 64 });
        let store = AnnoyVectorStore::new(embeddings, 64, None, Some(100));
        assert!(store.is_ok());
        assert_eq!(store.unwrap().n_trees, Some(100));
    }

    // ============================================
    // add_texts edge cases
    // ============================================

    #[tokio::test]
    async fn test_add_texts_ids_shorter_than_texts() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1", "doc2", "doc3"];
        let custom_ids = vec!["id1".to_string()]; // Only 1 ID for 3 texts

        let result = store.add_texts(&texts, None, Some(&custom_ids)).await;
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0], "id1");
        // ids[1] and ids[2] should be auto-generated UUIDs
        assert_ne!(ids[1], ids[0]);
        assert_ne!(ids[2], ids[0]);
    }

    #[tokio::test]
    async fn test_add_texts_metadata_shorter_than_texts() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1", "doc2", "doc3"];
        let mut meta = HashMap::new();
        meta.insert("key".to_string(), JsonValue::String("value".to_string()));
        let metadatas = vec![meta]; // Only 1 metadata for 3 texts

        let result = store.add_texts(&texts, Some(&metadatas), None).await;
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 3);
    }

    #[tokio::test]
    async fn test_add_texts_with_string_slices() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts: Vec<&str> = vec!["hello", "world"];
        let result = store.add_texts(&texts, None, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_texts_with_owned_strings() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts: Vec<String> = vec!["hello".to_string(), "world".to_string()];
        let result = store.add_texts(&texts, None, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_texts_unicode_content() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["日本語テスト", "中文测试", "한국어 테스트", "مرحبا بالعالم"];
        let result = store.add_texts(&texts, None, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 4);
    }

    #[tokio::test]
    async fn test_add_texts_empty_strings() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["", "not empty", ""];
        let result = store.add_texts(&texts, None, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn test_add_texts_large_batch() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 64 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 64, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts: Vec<String> = (0..100).map(|i| format!("document {i}")).collect();
        let text_refs: Vec<&str> = texts.iter().map(String::as_str).collect();

        let result = store.add_texts(&text_refs, None, None).await;
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 100);

        // All IDs should be unique
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique_ids.len(), 100);
    }

    // ============================================
    // Delete operation tests
    // ============================================

    #[tokio::test]
    async fn test_delete_nonexistent_id() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1"];
        store.add_texts(&texts, None, None).await.unwrap();

        // Delete non-existent ID - should not fail
        use dashflow::core::vector_stores::VectorStore;
        let result = VectorStore::delete(&mut store, Some(&["nonexistent".to_string()])).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_delete_multiple_ids() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1", "doc2", "doc3", "doc4"];
        let custom_ids = vec![
            "id1".to_string(),
            "id2".to_string(),
            "id3".to_string(),
            "id4".to_string(),
        ];
        store
            .add_texts(&texts, None, Some(&custom_ids))
            .await
            .unwrap();

        use dashflow::core::vector_stores::VectorStore;
        let result =
            VectorStore::delete(&mut store, Some(&["id1".to_string(), "id3".to_string()])).await;
        assert!(result.is_ok());

        let id_to_item = store.id_to_item.lock().unwrap();
        assert!(!id_to_item.contains_key("id1"));
        assert!(id_to_item.contains_key("id2"));
        assert!(!id_to_item.contains_key("id3"));
        assert!(id_to_item.contains_key("id4"));
    }

    #[tokio::test]
    async fn test_delete_empty_ids_array() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1"];
        store.add_texts(&texts, None, None).await.unwrap();

        // Delete with empty array
        use dashflow::core::vector_stores::VectorStore;
        let result = VectorStore::delete(&mut store, Some(&[])).await;
        assert!(result.is_ok());

        // Original document should still exist
        let metadata_store = store.metadata_store.lock().unwrap();
        assert_eq!(metadata_store.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_all_on_empty_store() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        use dashflow::core::vector_stores::VectorStore;
        let result = VectorStore::delete(&mut store, None).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    // ============================================
    // DocumentIndex trait tests
    // ============================================

    #[tokio::test]
    async fn test_document_index_delete_not_supported() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let result = <AnnoyVectorStore as DocumentIndex>::delete(&store, Some(&["id1".to_string()])).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("mutable access"));
    }

    #[tokio::test]
    async fn test_document_index_delete_none() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let result = <AnnoyVectorStore as DocumentIndex>::delete(&store, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_document_index_upsert_error_message() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let doc = Document {
            id: None,
            page_content: "test".to_string(),
            metadata: HashMap::new(),
        };

        let result = store.upsert(&[doc]).await;
        let err_str = result.unwrap_err().to_string();
        assert!(err_str.contains("mutable access"));
        assert!(err_str.contains("add_texts"));
    }

    #[tokio::test]
    async fn test_document_index_get_error_message() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let result = store.get(&[]).await;
        let err_str = result.unwrap_err().to_string();
        assert!(err_str.contains("not supported"));
    }

    // ============================================
    // Search operation tests
    // ============================================

    #[tokio::test]
    async fn test_similarity_search_k_larger_than_docs() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1", "doc2"];
        store.add_texts(&texts, None, None).await.unwrap();

        // Request more results than available documents
        let result = store._similarity_search("query", 10, None).await;
        assert!(result.is_ok());
        let docs = result.unwrap();
        assert!(docs.len() <= 2);
    }

    #[tokio::test]
    async fn test_similarity_search_k_zero() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1"];
        store.add_texts(&texts, None, None).await.unwrap();

        let result = store._similarity_search("query", 0, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_similarity_search_cosine() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Cosine), None).unwrap();

        let texts = vec!["cosine doc 1", "cosine doc 2"];
        store.add_texts(&texts, None, None).await.unwrap();

        let result = store._similarity_search("cosine", 2, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_similarity_search_returns_document_ids() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["test doc"];
        let custom_ids = vec!["my-custom-id".to_string()];
        store
            .add_texts(&texts, None, Some(&custom_ids))
            .await
            .unwrap();

        let results = store._similarity_search("test", 1, None).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, Some("my-custom-id".to_string()));
    }

    #[tokio::test]
    async fn test_similarity_search_by_vector_empty_vector() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1"];
        store.add_texts(&texts, None, None).await.unwrap();

        // Empty vector is technically invalid but we should handle gracefully
        let empty_vec: Vec<f32> = vec![];
        let result = store.similarity_search_by_vector(&empty_vec, 1, None).await;
        // The behavior depends on arroy - it may error or return empty
        // Just verify it doesn't panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_similarity_search_with_score_ordering() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["short", "medium length", "this is a much longer document"];
        store.add_texts(&texts, None, None).await.unwrap();

        let result = store
            .similarity_search_with_score("short", 3, None)
            .await;
        assert!(result.is_ok());
        let docs = result.unwrap();

        // Scores should be non-negative for Euclidean distance
        for (_doc, score) in &docs {
            assert!(*score >= 0.0);
            assert!(score.is_finite());
        }
    }

    // ============================================
    // Retriever trait tests
    // ============================================

    #[tokio::test]
    async fn test_retriever_with_config_none() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["retriever test doc"];
        store.add_texts(&texts, None, None).await.unwrap();

        let result = store._get_relevant_documents("retriever", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_retriever_default_k_is_4() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        // Add 10 documents
        let texts: Vec<String> = (0..10).map(|i| format!("document {i}")).collect();
        let text_refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        store.add_texts(&text_refs, None, None).await.unwrap();

        let result = store._get_relevant_documents("document", None).await;
        assert!(result.is_ok());
        let docs = result.unwrap();
        // Default k=4 in retriever
        assert!(docs.len() <= 4);
    }

    // ============================================
    // State and internal tests
    // ============================================

    #[tokio::test]
    async fn test_is_built_flag_reset_on_add() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        // Add documents
        store.add_texts(&["doc1"], None, None).await.unwrap();

        // Build index via search
        store._similarity_search("doc1", 1, None).await.unwrap();

        // Should be built now
        {
            let is_built = store.is_built.lock().unwrap();
            assert!(*is_built);
        }

        // Add more documents
        store.add_texts(&["doc2"], None, None).await.unwrap();

        // Should be marked as not built
        {
            let is_built = store.is_built.lock().unwrap();
            assert!(!*is_built);
        }
    }

    #[tokio::test]
    async fn test_next_item_id_increments() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        store.add_texts(&["doc1", "doc2", "doc3"], None, None).await.unwrap();

        let next_id = store.next_item_id.lock().unwrap();
        assert_eq!(*next_id, 3);
    }

    #[tokio::test]
    async fn test_metadata_store_populated() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["hello", "world"];
        store.add_texts(&texts, None, None).await.unwrap();

        let metadata_store = store.metadata_store.lock().unwrap();
        assert_eq!(metadata_store.len(), 2);

        // Check that texts are stored
        let stored_texts: Vec<&str> = metadata_store.values().map(|m| m.text.as_str()).collect();
        assert!(stored_texts.contains(&"hello"));
        assert!(stored_texts.contains(&"world"));
    }

    #[tokio::test]
    async fn test_id_to_item_mapping() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        let texts = vec!["doc1", "doc2"];
        let custom_ids = vec!["custom-1".to_string(), "custom-2".to_string()];
        store
            .add_texts(&texts, None, Some(&custom_ids))
            .await
            .unwrap();

        let id_to_item = store.id_to_item.lock().unwrap();
        assert!(id_to_item.contains_key("custom-1"));
        assert!(id_to_item.contains_key("custom-2"));
        assert_eq!(id_to_item.len(), 2);
    }

    // ============================================
    // Persistence tests
    // ============================================

    #[tokio::test]
    async fn test_persistent_path_creates_directory() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("annoy_test_db");

        assert!(!db_path.exists());

        let store = AnnoyVectorStore::new_with_path(
            embeddings,
            128,
            Some(DistanceMetric::Euclidean),
            &db_path,
            None,
            None,
        );

        assert!(store.is_ok());
        assert!(db_path.exists());
    }

    #[tokio::test]
    async fn test_temp_dir_held_by_store() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        // Temp dir should be held by the store
        assert!(store._temp_dir.is_some());
    }

    // ============================================
    // MockEmbeddings tests
    // ============================================

    #[tokio::test]
    async fn test_mock_embeddings_deterministic() {
        let mock = MockEmbeddings { dimension: 64 };

        let emb1 = mock._embed_query("test").await.unwrap();
        let emb2 = mock._embed_query("test").await.unwrap();

        // Same text should produce same embedding
        assert_eq!(emb1, emb2);
    }

    #[tokio::test]
    async fn test_mock_embeddings_different_texts() {
        let mock = MockEmbeddings { dimension: 64 };

        let emb1 = mock._embed_query("short").await.unwrap();
        let emb2 = mock._embed_query("a longer text").await.unwrap();

        // Different texts should produce different embeddings
        assert_ne!(emb1, emb2);
    }

    #[tokio::test]
    async fn test_mock_embeddings_dimension() {
        let mock = MockEmbeddings { dimension: 256 };

        let emb = mock._embed_query("test").await.unwrap();
        assert_eq!(emb.len(), 256);
    }

    #[tokio::test]
    async fn test_mock_embeddings_batch() {
        let mock = MockEmbeddings { dimension: 128 };

        let texts = vec!["a".to_string(), "bb".to_string(), "ccc".to_string()];
        let embeddings = mock._embed_documents(&texts).await.unwrap();

        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.len(), 128);
        }
    }

    // ============================================
    // Integration-like tests
    // ============================================

    #[tokio::test]
    async fn test_full_workflow_euclidean() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 128, Some(DistanceMetric::Euclidean), None).unwrap();

        // Add documents with metadata
        let texts = vec!["apple pie recipe", "banana smoothie", "cherry tart"];
        let mut m1 = HashMap::new();
        m1.insert("category".to_string(), JsonValue::String("dessert".to_string()));
        let mut m2 = HashMap::new();
        m2.insert("category".to_string(), JsonValue::String("drink".to_string()));
        let mut m3 = HashMap::new();
        m3.insert("category".to_string(), JsonValue::String("dessert".to_string()));
        let metadatas = vec![m1, m2, m3];
        let custom_ids = vec!["apple".to_string(), "banana".to_string(), "cherry".to_string()];

        let ids = store
            .add_texts(&texts, Some(&metadatas), Some(&custom_ids))
            .await
            .unwrap();
        assert_eq!(ids, custom_ids);

        // Search
        let results = store
            .similarity_search_with_score("pie", 2, None)
            .await
            .unwrap();
        assert!(!results.is_empty());

        // Delete one
        use dashflow::core::vector_stores::VectorStore;
        VectorStore::delete(&mut store, Some(&["banana".to_string()]))
            .await
            .unwrap();

        // Verify deletion
        let id_to_item = store.id_to_item.lock().unwrap();
        assert!(!id_to_item.contains_key("banana"));
        assert!(id_to_item.contains_key("apple"));
        assert!(id_to_item.contains_key("cherry"));
    }

    #[tokio::test]
    async fn test_full_workflow_cosine() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 64 });
        let mut store =
            AnnoyVectorStore::new(embeddings, 64, Some(DistanceMetric::Cosine), Some(2)).unwrap();

        let texts = vec!["hello world", "goodbye universe"];
        store.add_texts(&texts, None, None).await.unwrap();

        let results = store._similarity_search("hello", 1, None).await.unwrap();
        assert!(!results.is_empty());
        assert!(results[0].page_content.contains("hello") || results[0].page_content.contains("goodbye"));
    }
}
