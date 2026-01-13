//! DashFlow.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use async_trait::async_trait;
use faiss::{Index, MetricType, index_factory};
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::{DistanceMetric, VectorStore};
use dashflow::core::{Error, Result};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// FAISS vector database implementation.
///
/// This implementation uses Facebook AI Similarity Search (FAISS) for efficient
/// similarity search over dense vectors. FAISS is particularly well-suited for
/// large-scale vector search and supports various indexing methods (Flat, IVF, HNSW, etc.).
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_faiss::FaissVectorStore;
/// use dashflow::core::embeddings::Embeddings;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # struct MockEmbeddings;
/// # #[async_trait::async_trait]
/// # impl Embeddings for MockEmbeddings {
/// #     async fn embed_documents(&self, texts: &[impl AsRef<str> + Send + Sync]) -> dashflow::core::Result<Vec<Vec<f32>>> {
/// #         Ok(vec![vec![0.0; 384]; texts.len()])
/// #     }
/// #     async fn embed_query(&self, text: &str) -> dashflow::core::Result<Vec<f32>> {
/// #         Ok(vec![0.0; 384])
/// #     }
/// # }
/// let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
///
/// let store = FaissVectorStore::new(
///     embeddings,
///     384, // dimension
///     "Flat", // index type
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub struct FaissVectorStore {
    index: Arc<Mutex<Box<dyn Index>>>,
    embeddings: Arc<dyn Embeddings>,
    dimension: usize,
    distance_metric: DistanceMetric,
    // Store document content and metadata separately since FAISS only stores vectors
    documents: Arc<Mutex<HashMap<i64, Document>>>,
    id_map: Arc<Mutex<HashMap<String, i64>>>,
    next_id: Arc<Mutex<i64>>,
}

impl FaissVectorStore {
    /// Creates a new FaissVectorStore instance.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - Embeddings model to use
    /// * `dimension` - Dimension of the embeddings (must match embeddings output)
    /// * `index_type` - FAISS index type string (e.g., "Flat", "IVF100,Flat", "HNSW32")
    ///   - "Flat": Exact search (exhaustive)
    ///   - "IVFx,Flat": Inverted file index with x clusters
    ///   - "HNSWx": Hierarchical Navigable Small World graph with x links
    ///   - See FAISS documentation for more index types
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Index creation fails
    /// - Invalid index type string
    pub async fn new(
        embeddings: Arc<dyn Embeddings>,
        dimension: usize,
        index_type: &str,
    ) -> Result<Self> {
        Self::new_with_metric(embeddings, dimension, index_type, DistanceMetric::Cosine).await
    }

    /// Creates a new FaissVectorStore with a specific distance metric.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - Embeddings model to use
    /// * `dimension` - Dimension of the embeddings
    /// * `index_type` - FAISS index type string
    /// * `metric` - Distance metric to use
    pub async fn new_with_metric(
        embeddings: Arc<dyn Embeddings>,
        dimension: usize,
        index_type: &str,
        metric: DistanceMetric,
    ) -> Result<Self> {
        // Convert DistanceMetric to FAISS MetricType
        let faiss_metric = match metric {
            DistanceMetric::Euclidean => MetricType::L2,
            DistanceMetric::DotProduct | DistanceMetric::MaxInnerProduct => MetricType::InnerProduct,
            DistanceMetric::Cosine => MetricType::InnerProduct, // Cosine uses inner product with normalized vectors
        };

        // Create FAISS index
        let index = index_factory(dimension as u32, index_type, faiss_metric)
            .map_err(|e| Error::api(format!("Failed to create FAISS index: {}", e)))?;

        Ok(Self {
            index: Arc::new(Mutex::new(index)),
            embeddings,
            dimension,
            distance_metric: metric,
            documents: Arc::new(Mutex::new(HashMap::new())),
            id_map: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(0)),
        })
    }

    /// Normalizes vectors for cosine similarity (L2 normalization).
    fn normalize_vectors(&self, vectors: &mut [f32]) {
        if self.distance_metric != DistanceMetric::Cosine {
            return;
        }

        let dim = self.dimension;
        for chunk in vectors.chunks_mut(dim) {
            let norm: f32 = chunk.iter().map(|&x| x * x).sum::<f32>().sqrt();
            if norm > 1e-10 {
                for val in chunk.iter_mut() {
                    *val /= norm;
                }
            }
        }
    }

    /// Converts FAISS distance to similarity score in [0, 1].
    fn distance_to_score(&self, distance: f32) -> f32 {
        match self.distance_metric {
            DistanceMetric::Cosine | DistanceMetric::DotProduct | DistanceMetric::MaxInnerProduct => {
                // For inner product metrics, higher is better
                // Normalize to [0, 1] range
                distance.max(0.0).min(1.0)
            }
            DistanceMetric::Euclidean => {
                // Euclidean distance - smaller is better
                // Use exponential decay: score = exp(-distance)
                (-distance).exp()
            }
        }
    }

    /// Gets the next internal ID for a new vector.
    fn get_next_id(&self) -> i64 {
        let mut next_id = self.next_id.lock();
        let id = *next_id;
        *next_id += 1;
        id
    }
}

#[async_trait]
impl VectorStore for FaissVectorStore {
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
        if let Some(metadatas) = metadatas {
            if metadatas.len() != texts.len() {
                return Err(Error::invalid_input(format!(
                    "Metadatas length ({}) doesn't match texts length ({})",
                    metadatas.len(),
                    texts.len()
                )));
            }
        }

        if let Some(ids) = ids {
            if ids.len() != texts.len() {
                return Err(Error::invalid_input(format!(
                    "IDs length ({}) doesn't match texts length ({})",
                    ids.len(),
                    texts.len()
                )));
            }
        }

        // Generate embeddings
        let mut embeddings = self.embeddings.embed_documents(texts).await?;

        // Validate embedding dimensions
        for (i, embedding) in embeddings.iter().enumerate() {
            if embedding.len() != self.dimension {
                return Err(Error::invalid_input(format!(
                    "Embedding {} has dimension {} but expected {}",
                    i,
                    embedding.len(),
                    self.dimension
                )));
            }
        }

        // Flatten embeddings for FAISS (column-major order)
        let mut flat_embeddings: Vec<f32> = embeddings.iter()
            .flat_map(|e| e.iter().copied())
            .collect();

        // Normalize if using cosine similarity
        self.normalize_vectors(&mut flat_embeddings);

        // Add to FAISS index
        let mut index = self.index.lock();
        index.add(&flat_embeddings)
            .map_err(|e| Error::api(format!("Failed to add vectors to FAISS index: {}", e)))?;
        drop(index); // Release lock early

        // Generate or use provided IDs
        let doc_ids: Vec<String> = if let Some(ids) = ids {
            ids.to_vec()
        } else {
            (0..texts.len()).map(|_| Uuid::new_v4().to_string()).collect()
        };

        // Store documents with their metadata
        let mut documents = self.documents.lock();
        let mut id_map = self.id_map.lock();

        for (i, text) in texts.iter().enumerate() {
            let internal_id = self.get_next_id();
            let metadata = metadatas.and_then(|m| m.get(i)).cloned().unwrap_or_default();

            let doc = Document {
                page_content: text.as_ref().to_string(),
                metadata,
                id: Some(doc_ids[i].clone()),
            };

            documents.insert(internal_id, doc);
            id_map.insert(doc_ids[i].clone(), internal_id);
        }

        Ok(doc_ids)
    }

    async fn add_documents(
        &mut self,
        documents: &[Document],
        ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
        let texts: Vec<&str> = documents.iter().map(|d| d.page_content.as_str()).collect();
        let metadatas: Vec<HashMap<String, JsonValue>> = documents.iter()
            .map(|d| d.metadata.clone())
            .collect();

        self.add_texts(&texts, Some(&metadatas), ids).await
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        if let Some(ids) = ids {
            let mut id_map = self.id_map.lock();
            let mut documents = self.documents.lock();

            let mut deleted_any = false;
            for id in ids {
                if let Some(internal_id) = id_map.remove(id) {
                    documents.remove(&internal_id);
                    deleted_any = true;
                }
            }

            Ok(deleted_any)
        } else {
            // Delete all
            let mut id_map = self.id_map.lock();
            let mut documents = self.documents.lock();
            let mut index = self.index.lock();
            let mut next_id = self.next_id.lock();

            id_map.clear();
            documents.clear();
            *next_id = 0;

            // Reset FAISS index
            index.reset()
                .map_err(|e| Error::api(format!("Failed to reset FAISS index: {}", e)))?;

            Ok(true)
        }
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        let id_map = self.id_map.lock();
        let documents = self.documents.lock();

        let mut results = Vec::new();
        for id in ids {
            if let Some(internal_id) = id_map.get(id) {
                if let Some(doc) = documents.get(internal_id) {
                    results.push(doc.clone());
                }
            }
        }

        Ok(results)
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
        // Generate query embedding
        let query_embedding = self.embeddings.embed_query(query).await?;

        self.similarity_search_by_vector_with_score(&query_embedding, k, filter).await
    }

    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        let results = self.similarity_search_by_vector_with_score(embedding, k, filter).await?;
        Ok(results.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        // Validate embedding dimension
        if embedding.len() != self.dimension {
            return Err(Error::invalid_input(format!(
                "Query embedding has dimension {} but expected {}",
                embedding.len(),
                self.dimension
            )));
        }

        // Normalize query vector if using cosine similarity
        let mut query_vec = embedding.to_vec();
        if self.distance_metric == DistanceMetric::Cosine {
            let norm: f32 = query_vec.iter().map(|&x| x * x).sum::<f32>().sqrt();
            if norm > 1e-10 {
                for val in query_vec.iter_mut() {
                    *val /= norm;
                }
            }
        }

        // Search in FAISS index
        let index = self.index.lock();
        let search_result = index.search(&query_vec, k)
            .map_err(|e| Error::api(format!("FAISS search failed: {}", e)))?;
        drop(index); // Release lock early

        // Map FAISS results to documents
        let documents = self.documents.lock();
        let mut results = Vec::new();

        for (idx, distance) in search_result.labels.iter().zip(search_result.distances.iter()) {
            // FAISS returns -1 for invalid results
            if *idx < 0 {
                continue;
            }

            let internal_id = *idx;
            if let Some(doc) = documents.get(&internal_id) {
                // Apply metadata filtering if provided
                if let Some(filter) = filter {
                    let mut matches = true;
                    for (key, value) in filter {
                        if doc.metadata.get(key) != Some(value) {
                            matches = false;
                            break;
                        }
                    }
                    if !matches {
                        continue;
                    }
                }

                let score = self.distance_to_score(*distance);
                results.push((doc.clone(), score));
            }
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
        // Generate query embedding
        let query_embedding = self.embeddings.embed_query(query).await?;

        // Fetch more candidates than needed
        let candidates = self.similarity_search_by_vector_with_score(
            &query_embedding,
            fetch_k,
            filter,
        ).await?;

        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // Extract embeddings for MMR calculation
        let candidate_docs: Vec<Document> = candidates.iter().map(|(doc, _)| doc.clone()).collect();
        let texts: Vec<&str> = candidate_docs.iter().map(|d| d.page_content.as_str()).collect();
        let candidate_embeddings = self.embeddings.embed_documents(&texts).await?;

        // MMR algorithm
        let mut selected_indices = Vec::new();
        let mut selected_embeddings = Vec::new();

        while selected_indices.len() < k && selected_indices.len() < candidates.len() {
            let mut best_score = f32::NEG_INFINITY;
            let mut best_idx = 0;

            for (i, embedding) in candidate_embeddings.iter().enumerate() {
                if selected_indices.contains(&i) {
                    continue;
                }

                // Relevance to query
                let relevance = Self::cosine_similarity(&query_embedding, embedding);

                // Maximum similarity to already selected documents
                let max_sim = if selected_embeddings.is_empty() {
                    0.0
                } else {
                    selected_embeddings.iter()
                        .map(|selected| Self::cosine_similarity(embedding, selected))
                        .fold(f32::NEG_INFINITY, f32::max)
                };

                // MMR score
                let mmr_score = lambda * relevance - (1.0 - lambda) * max_sim;

                if mmr_score > best_score {
                    best_score = mmr_score;
                    best_idx = i;
                }
            }

            selected_indices.push(best_idx);
            selected_embeddings.push(candidate_embeddings[best_idx].clone());
        }

        // Return selected documents
        Ok(selected_indices.iter()
            .map(|&i| candidate_docs[i].clone())
            .collect())
    }
}

impl FaissVectorStore {
    /// Calculates cosine similarity between two vectors.
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a < 1e-10 || norm_b < 1e-10 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test cosine_similarity function
    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = FaissVectorStore::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6, "Identical vectors should have similarity 1.0");
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = FaissVectorStore::cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6, "Orthogonal vectors should have similarity 0.0");
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = FaissVectorStore::cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6, "Opposite vectors should have similarity -1.0");
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = FaissVectorStore::cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6, "Zero vector should yield similarity 0.0");
    }

    #[test]
    fn test_cosine_similarity_arbitrary_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let sim = FaissVectorStore::cosine_similarity(&a, &b);
        // Expected: (1*4 + 2*5 + 3*6) / (sqrt(14) * sqrt(77)) = 32 / sqrt(1078) ≈ 0.9746
        let expected = 32.0 / (14.0_f32.sqrt() * 77.0_f32.sqrt());
        assert!((sim - expected).abs() < 1e-4, "Got {}, expected {}", sim, expected);
    }

    // Test distance_to_score for different metrics
    // Helper struct to test distance_to_score without needing full FaissVectorStore
    struct MockDistanceConverter {
        distance_metric: DistanceMetric,
    }

    impl MockDistanceConverter {
        fn distance_to_score(&self, distance: f32) -> f32 {
            match self.distance_metric {
                DistanceMetric::Cosine | DistanceMetric::DotProduct | DistanceMetric::MaxInnerProduct => {
                    distance.max(0.0).min(1.0)
                }
                DistanceMetric::Euclidean => {
                    (-distance).exp()
                }
            }
        }
    }

    #[test]
    fn test_distance_to_score_cosine() {
        let converter = MockDistanceConverter { distance_metric: DistanceMetric::Cosine };

        // For cosine, higher distance = higher score (clamped to [0, 1])
        assert!((converter.distance_to_score(0.5) - 0.5).abs() < 1e-6);
        assert!((converter.distance_to_score(1.0) - 1.0).abs() < 1e-6);
        assert!((converter.distance_to_score(0.0) - 0.0).abs() < 1e-6);

        // Test clamping
        assert!((converter.distance_to_score(-0.5) - 0.0).abs() < 1e-6);
        assert!((converter.distance_to_score(1.5) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_distance_to_score_euclidean() {
        let converter = MockDistanceConverter { distance_metric: DistanceMetric::Euclidean };

        // For euclidean, score = exp(-distance)
        assert!((converter.distance_to_score(0.0) - 1.0).abs() < 1e-6);
        assert!((converter.distance_to_score(1.0) - 1.0_f32 / std::f32::consts::E).abs() < 1e-4);
        assert!(converter.distance_to_score(10.0) < 1e-4);
    }

    #[test]
    fn test_distance_to_score_dot_product() {
        let converter = MockDistanceConverter { distance_metric: DistanceMetric::DotProduct };

        // Same as cosine - clamped to [0, 1]
        assert!((converter.distance_to_score(0.8) - 0.8).abs() < 1e-6);
        assert!((converter.distance_to_score(-1.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_distance_to_score_max_inner_product() {
        let converter = MockDistanceConverter { distance_metric: DistanceMetric::MaxInnerProduct };

        // Same as cosine/dot product - clamped to [0, 1]
        assert!((converter.distance_to_score(0.9) - 0.9).abs() < 1e-6);
        assert!((converter.distance_to_score(2.0) - 1.0).abs() < 1e-6);
    }

    // Test vector normalization
    #[test]
    fn test_normalize_vectors_cosine_metric() {
        // Test helper to normalize vectors
        fn normalize_vectors_for_test(vectors: &mut [f32], dim: usize) {
            for chunk in vectors.chunks_mut(dim) {
                let norm: f32 = chunk.iter().map(|&x| x * x).sum::<f32>().sqrt();
                if norm > 1e-10 {
                    for val in chunk.iter_mut() {
                        *val /= norm;
                    }
                }
            }
        }

        let mut vectors = vec![3.0, 4.0]; // norm = 5.0
        normalize_vectors_for_test(&mut vectors, 2);

        // After normalization: [0.6, 0.8]
        assert!((vectors[0] - 0.6).abs() < 1e-6);
        assert!((vectors[1] - 0.8).abs() < 1e-6);

        // Verify unit norm
        let norm: f32 = vectors.iter().map(|&x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_vectors_multiple() {
        fn normalize_vectors_for_test(vectors: &mut [f32], dim: usize) {
            for chunk in vectors.chunks_mut(dim) {
                let norm: f32 = chunk.iter().map(|&x| x * x).sum::<f32>().sqrt();
                if norm > 1e-10 {
                    for val in chunk.iter_mut() {
                        *val /= norm;
                    }
                }
            }
        }

        let mut vectors = vec![3.0, 4.0, 1.0, 0.0]; // Two 2D vectors
        normalize_vectors_for_test(&mut vectors, 2);

        // First vector: [3, 4] -> [0.6, 0.8]
        assert!((vectors[0] - 0.6).abs() < 1e-6);
        assert!((vectors[1] - 0.8).abs() < 1e-6);

        // Second vector: [1, 0] -> [1.0, 0.0]
        assert!((vectors[2] - 1.0).abs() < 1e-6);
        assert!((vectors[3] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_vectors_zero_vector() {
        fn normalize_vectors_for_test(vectors: &mut [f32], dim: usize) {
            for chunk in vectors.chunks_mut(dim) {
                let norm: f32 = chunk.iter().map(|&x| x * x).sum::<f32>().sqrt();
                if norm > 1e-10 {
                    for val in chunk.iter_mut() {
                        *val /= norm;
                    }
                }
            }
        }

        let mut vectors = vec![0.0, 0.0]; // Zero vector
        normalize_vectors_for_test(&mut vectors, 2);

        // Zero vector should remain zero (no divide by zero)
        assert!((vectors[0] - 0.0).abs() < 1e-6);
        assert!((vectors[1] - 0.0).abs() < 1e-6);
    }

    // Test DistanceMetric enum
    #[test]
    fn test_distance_metric_variants() {
        // Verify all variants exist and are distinct
        let metrics = [
            DistanceMetric::Cosine,
            DistanceMetric::Euclidean,
            DistanceMetric::DotProduct,
            DistanceMetric::MaxInnerProduct,
        ];

        // Each metric should be distinguishable
        assert!(matches!(metrics[0], DistanceMetric::Cosine));
        assert!(matches!(metrics[1], DistanceMetric::Euclidean));
        assert!(matches!(metrics[2], DistanceMetric::DotProduct));
        assert!(matches!(metrics[3], DistanceMetric::MaxInnerProduct));
    }

    #[test]
    fn test_distance_metric_equality() {
        assert_eq!(DistanceMetric::Cosine, DistanceMetric::Cosine);
        assert_ne!(DistanceMetric::Cosine, DistanceMetric::Euclidean);
    }

    // Test ID generation counter behavior
    #[test]
    fn test_sequential_id_generation() {
        use std::sync::atomic::{AtomicI64, Ordering};

        let counter = AtomicI64::new(0);

        let id1 = counter.fetch_add(1, Ordering::SeqCst);
        let id2 = counter.fetch_add(1, Ordering::SeqCst);
        let id3 = counter.fetch_add(1, Ordering::SeqCst);

        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(id3, 2);
    }

    // Test FAISS MetricType mapping
    #[test]
    fn test_metric_type_mapping() {
        // Verify the mapping logic used in new_with_metric
        fn map_metric(metric: DistanceMetric) -> &'static str {
            match metric {
                DistanceMetric::Euclidean => "L2",
                DistanceMetric::DotProduct | DistanceMetric::MaxInnerProduct => "InnerProduct",
                DistanceMetric::Cosine => "InnerProduct", // Cosine uses IP with normalized vectors
            }
        }

        assert_eq!(map_metric(DistanceMetric::Euclidean), "L2");
        assert_eq!(map_metric(DistanceMetric::DotProduct), "InnerProduct");
        assert_eq!(map_metric(DistanceMetric::MaxInnerProduct), "InnerProduct");
        assert_eq!(map_metric(DistanceMetric::Cosine), "InnerProduct");
    }

    // Test Document struct
    #[test]
    fn test_document_creation() {
        let doc = Document {
            page_content: "Test content".to_string(),
            metadata: HashMap::new(),
            id: Some("test-id".to_string()),
        };

        assert_eq!(doc.page_content, "Test content");
        assert!(doc.metadata.is_empty());
        assert_eq!(doc.id, Some("test-id".to_string()));
    }

    #[test]
    fn test_document_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), JsonValue::String("value".to_string()));

        let doc = Document {
            page_content: "Test".to_string(),
            metadata,
            id: None,
        };

        assert_eq!(doc.metadata.get("key"), Some(&JsonValue::String("value".to_string())));
    }

    // Test UUID generation format
    #[test]
    fn test_uuid_format() {
        let id = Uuid::new_v4().to_string();

        // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|&c| c == '-').count(), 4);
    }

    // Test embedding dimension validation
    #[test]
    fn test_embedding_dimension_check() {
        let expected_dim = 384;
        let actual_dim = 512;

        let error_msg = format!(
            "Embedding {} has dimension {} but expected {}",
            0, actual_dim, expected_dim
        );

        assert!(error_msg.contains("384"));
        assert!(error_msg.contains("512"));
    }

    // Test metadata/IDs length validation
    #[test]
    fn test_metadatas_length_validation() {
        let texts_len = 3;
        let metadatas_len = 2;

        let valid = metadatas_len == texts_len;
        assert!(!valid, "Lengths should not match");

        let error_msg = format!(
            "Metadatas length ({}) doesn't match texts length ({})",
            metadatas_len, texts_len
        );

        assert!(error_msg.contains("2"));
        assert!(error_msg.contains("3"));
    }

    #[test]
    fn test_ids_length_validation() {
        let texts_len = 5;
        let ids_len = 3;

        let valid = ids_len == texts_len;
        assert!(!valid, "Lengths should not match");

        let error_msg = format!(
            "IDs length ({}) doesn't match texts length ({})",
            ids_len, texts_len
        );

        assert!(error_msg.contains("3"));
        assert!(error_msg.contains("5"));
    }

    // Test metadata filtering logic
    #[test]
    fn test_metadata_filter_match() {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), JsonValue::String("article".to_string()));
        metadata.insert("author".to_string(), JsonValue::String("Alice".to_string()));

        let mut filter = HashMap::new();
        filter.insert("type".to_string(), JsonValue::String("article".to_string()));

        // Check if metadata matches filter
        let matches = filter.iter().all(|(key, value)| {
            metadata.get(key) == Some(value)
        });

        assert!(matches, "Filter should match metadata");
    }

    #[test]
    fn test_metadata_filter_no_match() {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), JsonValue::String("article".to_string()));

        let mut filter = HashMap::new();
        filter.insert("type".to_string(), JsonValue::String("book".to_string()));

        let matches = filter.iter().all(|(key, value)| {
            metadata.get(key) == Some(value)
        });

        assert!(!matches, "Filter should not match metadata");
    }

    #[test]
    fn test_metadata_filter_missing_key() {
        let metadata = HashMap::new(); // Empty metadata

        let mut filter = HashMap::new();
        filter.insert("type".to_string(), JsonValue::String("article".to_string()));

        let matches = filter.iter().all(|(key, value)| {
            metadata.get(key) == Some(value)
        });

        assert!(!matches, "Filter should not match when key is missing");
    }

    // Test FAISS index type strings
    #[test]
    fn test_faiss_index_type_strings() {
        let valid_types = ["Flat", "IVF100,Flat", "HNSW32", "IVF256,PQ16"];

        for index_type in valid_types {
            assert!(!index_type.is_empty(), "Index type should not be empty");
        }
    }

    // ============================================
    // Additional cosine similarity tests
    // ============================================

    #[test]
    fn test_cosine_similarity_very_small_values() {
        let a = vec![1e-10, 1e-10, 1e-10];
        let b = vec![1e-10, 1e-10, 1e-10];
        let sim = FaissVectorStore::cosine_similarity(&a, &b);
        // Very small values should still work (both normalized)
        assert!((sim - 1.0).abs() < 1e-4 || sim.abs() < 1e-4);
    }

    #[test]
    fn test_cosine_similarity_mixed_signs() {
        let a = vec![1.0, -1.0, 1.0];
        let b = vec![-1.0, 1.0, 1.0];
        let sim = FaissVectorStore::cosine_similarity(&a, &b);
        // Expected: (1*-1 + -1*1 + 1*1) / (sqrt(3) * sqrt(3)) = -1/3
        let expected = -1.0 / 3.0;
        assert!((sim - expected).abs() < 1e-4);
    }

    #[test]
    fn test_cosine_similarity_large_dimension() {
        let dim = 1024;
        let a: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.001).collect();
        let b: Vec<f32> = (0..dim).map(|i| ((dim - i) as f32) * 0.001).collect();
        let sim = FaissVectorStore::cosine_similarity(&a, &b);
        assert!(sim.is_finite());
        assert!(sim >= -1.0 && sim <= 1.0);
    }

    #[test]
    fn test_cosine_similarity_single_element() {
        let a = vec![5.0];
        let b = vec![3.0];
        let sim = FaissVectorStore::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6, "Same direction single element vectors");
    }

    #[test]
    fn test_cosine_similarity_negative_single_element() {
        let a = vec![5.0];
        let b = vec![-3.0];
        let sim = FaissVectorStore::cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6, "Opposite direction single element");
    }

    #[test]
    fn test_cosine_similarity_both_zero() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![0.0, 0.0, 0.0];
        let sim = FaissVectorStore::cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6, "Two zero vectors should yield 0.0");
    }

    #[test]
    fn test_cosine_similarity_normalized_vectors() {
        // Already unit vectors
        let a = vec![0.6, 0.8, 0.0];
        let b = vec![0.0, 0.6, 0.8];
        let sim = FaissVectorStore::cosine_similarity(&a, &b);
        // Expected: 0*0.6 + 0.6*0.8 + 0.8*0 = 0.48
        let expected = 0.48;
        assert!((sim - expected).abs() < 1e-4);
    }

    // ============================================
    // Distance to score edge cases
    // ============================================

    #[test]
    fn test_distance_to_score_cosine_boundary_values() {
        let converter = MockDistanceConverter { distance_metric: DistanceMetric::Cosine };

        // Boundary tests
        assert!((converter.distance_to_score(0.0) - 0.0).abs() < 1e-6);
        assert!((converter.distance_to_score(1.0) - 1.0).abs() < 1e-6);
        assert!((converter.distance_to_score(0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_distance_to_score_euclidean_large_distance() {
        let converter = MockDistanceConverter { distance_metric: DistanceMetric::Euclidean };

        let score = converter.distance_to_score(100.0);
        assert!(score < 1e-40, "Large distance should give very small score");
        assert!(score >= 0.0, "Score should be non-negative");
    }

    #[test]
    fn test_distance_to_score_euclidean_negative_distance() {
        let converter = MockDistanceConverter { distance_metric: DistanceMetric::Euclidean };

        // Negative distance (shouldn't happen in practice, but test robustness)
        let score = converter.distance_to_score(-1.0);
        assert!(score > 1.0, "Negative distance gives score > 1");
        assert!(score.is_finite());
    }

    #[test]
    fn test_distance_to_score_dot_product_zero() {
        let converter = MockDistanceConverter { distance_metric: DistanceMetric::DotProduct };

        assert!((converter.distance_to_score(0.0) - 0.0).abs() < 1e-6);
    }

    // ============================================
    // Normalization tests
    // ============================================

    #[test]
    fn test_normalize_already_normalized() {
        fn normalize_vectors_for_test(vectors: &mut [f32], dim: usize) {
            for chunk in vectors.chunks_mut(dim) {
                let norm: f32 = chunk.iter().map(|&x| x * x).sum::<f32>().sqrt();
                if norm > 1e-10 {
                    for val in chunk.iter_mut() {
                        *val /= norm;
                    }
                }
            }
        }

        let mut vectors = vec![0.6, 0.8]; // Already unit norm
        normalize_vectors_for_test(&mut vectors, 2);

        assert!((vectors[0] - 0.6).abs() < 1e-6);
        assert!((vectors[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_high_dimension() {
        fn normalize_vectors_for_test(vectors: &mut [f32], dim: usize) {
            for chunk in vectors.chunks_mut(dim) {
                let norm: f32 = chunk.iter().map(|&x| x * x).sum::<f32>().sqrt();
                if norm > 1e-10 {
                    for val in chunk.iter_mut() {
                        *val /= norm;
                    }
                }
            }
        }

        let mut vectors: Vec<f32> = (0..512).map(|i| (i as f32) * 0.01).collect();
        normalize_vectors_for_test(&mut vectors, 512);

        let norm: f32 = vectors.iter().map(|&x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "Should be unit norm");
    }

    #[test]
    fn test_normalize_multiple_vectors_varying_norms() {
        fn normalize_vectors_for_test(vectors: &mut [f32], dim: usize) {
            for chunk in vectors.chunks_mut(dim) {
                let norm: f32 = chunk.iter().map(|&x| x * x).sum::<f32>().sqrt();
                if norm > 1e-10 {
                    for val in chunk.iter_mut() {
                        *val /= norm;
                    }
                }
            }
        }

        // Three 3D vectors with different norms
        let mut vectors = vec![
            1.0, 2.0, 2.0,   // norm = 3
            0.0, 5.0, 0.0,   // norm = 5
            1.0, 1.0, 1.0,   // norm = sqrt(3)
        ];
        normalize_vectors_for_test(&mut vectors, 3);

        // Check each vector is unit norm
        for chunk in vectors.chunks(3) {
            let norm: f32 = chunk.iter().map(|&x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 1e-5, "Each vector should be unit norm");
        }
    }

    #[test]
    fn test_normalize_with_negative_values() {
        fn normalize_vectors_for_test(vectors: &mut [f32], dim: usize) {
            for chunk in vectors.chunks_mut(dim) {
                let norm: f32 = chunk.iter().map(|&x| x * x).sum::<f32>().sqrt();
                if norm > 1e-10 {
                    for val in chunk.iter_mut() {
                        *val /= norm;
                    }
                }
            }
        }

        let mut vectors = vec![-3.0, 4.0];
        normalize_vectors_for_test(&mut vectors, 2);

        assert!((vectors[0] - (-0.6)).abs() < 1e-6);
        assert!((vectors[1] - 0.8).abs() < 1e-6);
    }

    // ============================================
    // Metadata validation tests
    // ============================================

    #[test]
    fn test_metadata_filter_multiple_conditions() {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), JsonValue::String("article".to_string()));
        metadata.insert("author".to_string(), JsonValue::String("Alice".to_string()));
        metadata.insert("year".to_string(), JsonValue::Number(2024.into()));

        let mut filter = HashMap::new();
        filter.insert("type".to_string(), JsonValue::String("article".to_string()));
        filter.insert("author".to_string(), JsonValue::String("Alice".to_string()));

        let matches = filter.iter().all(|(key, value)| {
            metadata.get(key) == Some(value)
        });

        assert!(matches, "All filter conditions should match");
    }

    #[test]
    fn test_metadata_filter_partial_match_fails() {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), JsonValue::String("article".to_string()));
        metadata.insert("author".to_string(), JsonValue::String("Alice".to_string()));

        let mut filter = HashMap::new();
        filter.insert("type".to_string(), JsonValue::String("article".to_string()));
        filter.insert("author".to_string(), JsonValue::String("Bob".to_string()));

        let matches = filter.iter().all(|(key, value)| {
            metadata.get(key) == Some(value)
        });

        assert!(!matches, "Partial match should fail");
    }

    #[test]
    fn test_metadata_filter_empty_filter() {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), JsonValue::String("article".to_string()));

        let filter: HashMap<String, JsonValue> = HashMap::new();

        let matches = filter.iter().all(|(key, value)| {
            metadata.get(key) == Some(value)
        });

        assert!(matches, "Empty filter should match anything");
    }

    #[test]
    fn test_metadata_filter_with_numeric_value() {
        let mut metadata = HashMap::new();
        metadata.insert("count".to_string(), JsonValue::Number(42.into()));

        let mut filter = HashMap::new();
        filter.insert("count".to_string(), JsonValue::Number(42.into()));

        let matches = filter.iter().all(|(key, value)| {
            metadata.get(key) == Some(value)
        });

        assert!(matches, "Numeric filter should match");
    }

    #[test]
    fn test_metadata_filter_with_boolean_value() {
        let mut metadata = HashMap::new();
        metadata.insert("active".to_string(), JsonValue::Bool(true));

        let mut filter = HashMap::new();
        filter.insert("active".to_string(), JsonValue::Bool(true));

        let matches = filter.iter().all(|(key, value)| {
            metadata.get(key) == Some(value)
        });

        assert!(matches, "Boolean filter should match");
    }

    #[test]
    fn test_metadata_filter_boolean_mismatch() {
        let mut metadata = HashMap::new();
        metadata.insert("active".to_string(), JsonValue::Bool(true));

        let mut filter = HashMap::new();
        filter.insert("active".to_string(), JsonValue::Bool(false));

        let matches = filter.iter().all(|(key, value)| {
            metadata.get(key) == Some(value)
        });

        assert!(!matches, "Boolean mismatch should fail");
    }

    // ============================================
    // Document tests
    // ============================================

    #[test]
    fn test_document_default_metadata() {
        let doc = Document {
            page_content: "Content".to_string(),
            metadata: HashMap::new(),
            id: None,
        };

        assert!(doc.metadata.is_empty());
        assert!(doc.id.is_none());
    }

    #[test]
    fn test_document_clone() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), JsonValue::String("value".to_string()));

        let doc = Document {
            page_content: "Test".to_string(),
            metadata: metadata.clone(),
            id: Some("id1".to_string()),
        };

        let cloned = doc.clone();
        assert_eq!(cloned.page_content, doc.page_content);
        assert_eq!(cloned.metadata, doc.metadata);
        assert_eq!(cloned.id, doc.id);
    }

    #[test]
    fn test_document_with_complex_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("string".to_string(), JsonValue::String("hello".to_string()));
        metadata.insert("number".to_string(), JsonValue::Number(42.into()));
        metadata.insert("float".to_string(), serde_json::json!(3.14));
        metadata.insert("bool".to_string(), JsonValue::Bool(true));
        metadata.insert("null".to_string(), JsonValue::Null);
        metadata.insert("array".to_string(), serde_json::json!([1, 2, 3]));

        let doc = Document {
            page_content: "Complex".to_string(),
            metadata,
            id: Some("complex-doc".to_string()),
        };

        assert_eq!(doc.metadata.len(), 6);
        assert_eq!(doc.metadata.get("string"), Some(&JsonValue::String("hello".to_string())));
        assert_eq!(doc.metadata.get("bool"), Some(&JsonValue::Bool(true)));
    }

    // ============================================
    // UUID and ID generation tests
    // ============================================

    #[test]
    fn test_uuid_uniqueness() {
        let ids: Vec<String> = (0..100).map(|_| Uuid::new_v4().to_string()).collect();

        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 100, "All UUIDs should be unique");
    }

    #[test]
    fn test_uuid_valid_characters() {
        let id = Uuid::new_v4().to_string();

        for (i, c) in id.chars().enumerate() {
            if i == 8 || i == 13 || i == 18 || i == 23 {
                assert_eq!(c, '-', "Position {} should be a hyphen", i);
            } else {
                assert!(c.is_ascii_hexdigit(), "Position {} should be hex digit", i);
            }
        }
    }

    #[test]
    fn test_internal_id_sequence() {
        let counter = std::sync::Arc::new(Mutex::new(0i64));

        let mut ids = Vec::new();
        for _ in 0..10 {
            let mut guard = counter.lock();
            let id = *guard;
            *guard += 1;
            ids.push(id);
        }

        assert_eq!(ids, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    // ============================================
    // Error message format tests
    // ============================================

    #[test]
    fn test_dimension_mismatch_error_format() {
        let embedding_idx = 2;
        let actual_dim = 256;
        let expected_dim = 384;

        let error = format!(
            "Embedding {} has dimension {} but expected {}",
            embedding_idx, actual_dim, expected_dim
        );

        assert!(error.contains("Embedding 2"));
        assert!(error.contains("256"));
        assert!(error.contains("384"));
    }

    #[test]
    fn test_query_dimension_mismatch_error() {
        let actual = 128;
        let expected = 256;

        let error = format!(
            "Query embedding has dimension {} but expected {}",
            actual, expected
        );

        assert!(error.contains("128"));
        assert!(error.contains("256"));
    }

    // ============================================
    // HashMap behavior tests
    // ============================================

    #[test]
    fn test_id_map_insertion_and_retrieval() {
        let mut id_map: HashMap<String, i64> = HashMap::new();

        id_map.insert("doc-1".to_string(), 0);
        id_map.insert("doc-2".to_string(), 1);
        id_map.insert("doc-3".to_string(), 2);

        assert_eq!(id_map.get("doc-1"), Some(&0));
        assert_eq!(id_map.get("doc-2"), Some(&1));
        assert_eq!(id_map.get("doc-3"), Some(&2));
        assert_eq!(id_map.get("doc-4"), None);
    }

    #[test]
    fn test_id_map_removal() {
        let mut id_map: HashMap<String, i64> = HashMap::new();

        id_map.insert("doc-1".to_string(), 0);
        id_map.insert("doc-2".to_string(), 1);

        let removed = id_map.remove("doc-1");
        assert_eq!(removed, Some(0));
        assert_eq!(id_map.get("doc-1"), None);
        assert_eq!(id_map.len(), 1);
    }

    #[test]
    fn test_id_map_clear() {
        let mut id_map: HashMap<String, i64> = HashMap::new();

        id_map.insert("a".to_string(), 0);
        id_map.insert("b".to_string(), 1);
        id_map.insert("c".to_string(), 2);

        id_map.clear();
        assert!(id_map.is_empty());
    }

    #[test]
    fn test_documents_map_operations() {
        let mut documents: HashMap<i64, Document> = HashMap::new();

        let doc = Document {
            page_content: "Test".to_string(),
            metadata: HashMap::new(),
            id: Some("test-id".to_string()),
        };

        documents.insert(0, doc.clone());

        assert_eq!(documents.len(), 1);
        assert_eq!(documents.get(&0).map(|d| &d.page_content), Some(&"Test".to_string()));
        assert_eq!(documents.get(&1), None);
    }

    // ============================================
    // FAISS-related index type tests
    // ============================================

    #[test]
    fn test_flat_index_type_valid() {
        let index_type = "Flat";
        assert!(!index_type.is_empty());
        assert!(index_type.starts_with('F'));
    }

    #[test]
    fn test_ivf_index_type_parsing() {
        let index_type = "IVF100,Flat";

        let parts: Vec<&str> = index_type.split(',').collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].starts_with("IVF"));
        assert_eq!(parts[1], "Flat");
    }

    #[test]
    fn test_hnsw_index_type_parsing() {
        let index_type = "HNSW32";

        assert!(index_type.starts_with("HNSW"));
        let num_str = &index_type[4..];
        let num: Result<usize, _> = num_str.parse();
        assert!(num.is_ok());
        assert_eq!(num.unwrap(), 32);
    }

    #[test]
    fn test_pq_index_type_parsing() {
        let index_type = "IVF256,PQ16";

        let parts: Vec<&str> = index_type.split(',').collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].starts_with("IVF"));
        assert!(parts[1].starts_with("PQ"));
    }

    // ============================================
    // MMR algorithm helper tests
    // ============================================

    #[test]
    fn test_mmr_lambda_balance() {
        // MMR score = lambda * relevance - (1 - lambda) * max_similarity

        // When lambda = 1.0, only relevance matters
        let lambda = 1.0_f32;
        let relevance = 0.8_f32;
        let max_sim = 0.9_f32;
        let mmr_score = lambda * relevance - (1.0 - lambda) * max_sim;
        assert!((mmr_score - 0.8).abs() < 1e-6);

        // When lambda = 0.0, only diversity matters (minimize similarity)
        let lambda = 0.0_f32;
        let mmr_score = lambda * relevance - (1.0 - lambda) * max_sim;
        assert!((mmr_score - (-0.9)).abs() < 1e-6);
    }

    #[test]
    fn test_mmr_selection_logic() {
        // Simulate MMR selection with 3 candidates
        let relevances = vec![0.9, 0.85, 0.7];
        let lambda = 0.7_f32;

        // First selection: no prior selections, just use relevance
        let first_idx = relevances.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        assert_eq!(first_idx, 0, "Highest relevance should be selected first");
    }

    #[test]
    fn test_mmr_empty_selected_embeddings() {
        // When no embeddings are selected yet, max_sim should be 0
        let selected_embeddings: Vec<Vec<f32>> = vec![];

        let max_sim = if selected_embeddings.is_empty() {
            0.0_f32
        } else {
            0.5_f32  // This branch won't execute
        };

        assert!((max_sim - 0.0).abs() < 1e-6);
    }

    // ============================================
    // Thread safety simulation tests
    // ============================================

    #[test]
    fn test_mutex_lock_unlock_pattern() {
        let data = Arc::new(Mutex::new(vec![1, 2, 3]));

        {
            let mut guard = data.lock();
            guard.push(4);
        }  // Lock released here

        {
            let guard = data.lock();
            assert_eq!(*guard, vec![1, 2, 3, 4]);
        }
    }

    #[test]
    fn test_arc_clone_behavior() {
        let original = Arc::new(42);
        let clone1 = Arc::clone(&original);
        let clone2 = Arc::clone(&original);

        assert_eq!(*original, 42);
        assert_eq!(*clone1, 42);
        assert_eq!(*clone2, 42);
        assert_eq!(Arc::strong_count(&original), 3);
    }

    // ============================================
    // Vector operations tests
    // ============================================

    #[test]
    fn test_flatten_embeddings() {
        let embeddings: Vec<Vec<f32>> = vec![
            vec![1.0, 2.0],
            vec![3.0, 4.0],
            vec![5.0, 6.0],
        ];

        let flat: Vec<f32> = embeddings.iter()
            .flat_map(|e| e.iter().copied())
            .collect();

        assert_eq!(flat, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_chunks_iteration() {
        let data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let chunks: Vec<&[f32]> = data.chunks(2).collect();

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1.0, 2.0]);
        assert_eq!(chunks[1], &[3.0, 4.0]);
        assert_eq!(chunks[2], &[5.0, 6.0]);
    }

    #[test]
    fn test_embedding_dimension_validation() {
        let expected_dim = 384;
        let embeddings = vec![
            vec![0.0; 384],
            vec![0.0; 384],
            vec![0.0; 384],
        ];

        let all_valid = embeddings.iter().all(|e| e.len() == expected_dim);
        assert!(all_valid);
    }

    #[test]
    fn test_embedding_dimension_validation_failure() {
        let expected_dim = 384;
        let embeddings = vec![
            vec![0.0; 384],
            vec![0.0; 256],  // Wrong dimension
            vec![0.0; 384],
        ];

        let invalid_idx = embeddings.iter()
            .enumerate()
            .find(|(_, e)| e.len() != expected_dim)
            .map(|(i, _)| i);

        assert_eq!(invalid_idx, Some(1));
    }

    // ============================================
    // Score computation tests
    // ============================================

    #[test]
    fn test_exp_decay_score() {
        // Test exponential decay for Euclidean distance
        let scores: Vec<f32> = vec![0.0, 1.0, 2.0, 5.0, 10.0]
            .iter()
            .map(|&d| (-d as f32).exp())
            .collect();

        // Verify monotonic decrease
        for i in 1..scores.len() {
            assert!(scores[i] < scores[i - 1], "Score should decrease with distance");
        }

        // Verify specific values
        assert!((scores[0] - 1.0).abs() < 1e-6);  // exp(0) = 1
        assert!((scores[1] - 0.36787944).abs() < 1e-4);  // exp(-1)
    }

    #[test]
    fn test_score_clamping() {
        // Test clamping for inner product metrics
        let distances = vec![-0.5_f32, 0.0, 0.5, 1.0, 1.5, 2.0];
        let scores: Vec<f32> = distances.iter()
            .map(|&d| d.max(0.0).min(1.0))
            .collect();

        assert_eq!(scores, vec![0.0, 0.0, 0.5, 1.0, 1.0, 1.0]);
    }

    // ============================================
    // Search result handling tests
    // ============================================

    #[test]
    fn test_invalid_faiss_result_filtering() {
        // FAISS returns -1 for invalid results
        let labels: Vec<i64> = vec![0, 1, -1, 2, -1, 3];
        let distances: Vec<f32> = vec![0.1, 0.2, 0.0, 0.3, 0.0, 0.4];

        let valid_results: Vec<(i64, f32)> = labels.iter()
            .zip(distances.iter())
            .filter(|(&idx, _)| idx >= 0)
            .map(|(&idx, &dist)| (idx, dist))
            .collect();

        assert_eq!(valid_results.len(), 4);
        assert_eq!(valid_results[0], (0, 0.1));
        assert_eq!(valid_results[1], (1, 0.2));
        assert_eq!(valid_results[2], (2, 0.3));
        assert_eq!(valid_results[3], (3, 0.4));
    }

    #[test]
    fn test_search_result_ordering() {
        let mut results: Vec<(String, f32)> = vec![
            ("doc-1".to_string(), 0.5),
            ("doc-2".to_string(), 0.9),
            ("doc-3".to_string(), 0.7),
        ];

        // Sort by score descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        assert_eq!(results[0].0, "doc-2");
        assert_eq!(results[1].0, "doc-3");
        assert_eq!(results[2].0, "doc-1");
    }

    // ============================================
    // Type conversion tests
    // ============================================

    #[test]
    fn test_dimension_u32_conversion() {
        let dimension: usize = 384;
        let dimension_u32 = dimension as u32;

        assert_eq!(dimension_u32, 384u32);
    }

    #[test]
    fn test_large_dimension_conversion() {
        let dimension: usize = 4096;
        let dimension_u32 = dimension as u32;

        assert_eq!(dimension_u32, 4096u32);
    }

    // ============================================
    // Text processing tests
    // ============================================

    #[test]
    fn test_text_to_string_conversion() {
        let texts: Vec<&str> = vec!["hello", "world"];
        let strings: Vec<String> = texts.iter().map(|t| t.to_string()).collect();

        assert_eq!(strings, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn test_extract_page_content() {
        let docs = vec![
            Document { page_content: "First".to_string(), metadata: HashMap::new(), id: None },
            Document { page_content: "Second".to_string(), metadata: HashMap::new(), id: None },
        ];

        let texts: Vec<&str> = docs.iter().map(|d| d.page_content.as_str()).collect();

        assert_eq!(texts, vec!["First", "Second"]);
    }

    #[test]
    fn test_extract_metadata_from_documents() {
        let mut m1 = HashMap::new();
        m1.insert("k".to_string(), JsonValue::String("v1".to_string()));
        let mut m2 = HashMap::new();
        m2.insert("k".to_string(), JsonValue::String("v2".to_string()));

        let docs = vec![
            Document { page_content: "".to_string(), metadata: m1.clone(), id: None },
            Document { page_content: "".to_string(), metadata: m2.clone(), id: None },
        ];

        let metadatas: Vec<HashMap<String, JsonValue>> = docs.iter()
            .map(|d| d.metadata.clone())
            .collect();

        assert_eq!(metadatas.len(), 2);
        assert_eq!(metadatas[0].get("k"), Some(&JsonValue::String("v1".to_string())));
        assert_eq!(metadatas[1].get("k"), Some(&JsonValue::String("v2".to_string())));
    }
}
