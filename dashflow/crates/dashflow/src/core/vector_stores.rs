//! Vector stores for storing and searching embedded data.
//!
//! Vector stores store embedded data (vectors) and perform vector search to find
//! the most similar vectors to a query. This is a fundamental component of RAG
//! (Retrieval-Augmented Generation) systems.
//!
//! # Core Concepts
//!
//! - **Vector Store**: Storage and retrieval of embeddings with metadata
//! - **Similarity Search**: Find k most similar documents to a query
//! - **Distance Metrics**: Measure similarity between vectors (cosine, euclidean, etc.)
//! - **Metadata Filtering**: Filter results based on document metadata
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::vector_stores::{VectorStore, DistanceMetric};
//!
//! // Add documents to vector store
//! let ids = store.add_texts(&["doc1", "doc2"], None, None).await?;
//!
//! // Search for similar documents
//! let results = store._similarity_search("query text", 5, None).await?;
//! ```

use crate::core::{
    documents::Document,
    embeddings::Embeddings,
    error::{Error, Result},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Distance metric used for vector similarity calculation.
///
/// Different metrics are appropriate for different embedding models:
/// - **Cosine**: Best for normalized embeddings (`OpenAI`, Cohere)
/// - **Euclidean**: Good for unnormalized embeddings
/// - **`DotProduct`**: Fast, works well with normalized embeddings
/// - **`MaxInnerProduct`**: Optimized for asymmetric similarity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    /// Cosine similarity: measures angle between vectors (0 = identical, 2 = opposite)
    /// Normalized to [0, 1] where 1 is most similar
    Cosine,

    /// Euclidean distance: L2 norm (0 = identical, ∞ = dissimilar)
    /// Normalized to [0, 1] where 1 is most similar
    Euclidean,

    /// Dot product: inner product of vectors
    /// Higher values indicate more similarity
    DotProduct,

    /// Maximum inner product: optimized for asymmetric similarity
    /// Used for learned embeddings where query and document spaces differ
    MaxInnerProduct,
}

impl DistanceMetric {
    /// Calculate distance between two vectors.
    ///
    /// Returns the raw distance value (interpretation depends on metric).
    pub fn calculate(&self, a: &[f32], b: &[f32]) -> Result<f32> {
        if a.len() != b.len() {
            return Err(Error::config(format!(
                "Vector dimension mismatch: {} vs {}",
                a.len(),
                b.len()
            )));
        }

        match self {
            DistanceMetric::Cosine => Self::cosine_distance(a, b),
            DistanceMetric::Euclidean => Self::euclidean_distance(a, b),
            DistanceMetric::DotProduct => Ok(Self::dot_product(a, b)),
            DistanceMetric::MaxInnerProduct => Ok(Self::dot_product(a, b)),
        }
    }

    /// Convert raw distance to normalized relevance score in [0, 1].
    ///
    /// 0 = dissimilar, 1 = most similar
    #[must_use]
    pub fn distance_to_relevance(&self, distance: f32) -> f32 {
        match self {
            DistanceMetric::Cosine => {
                // Cosine distance is [0, 2], convert to similarity [0, 1]
                1.0 - (distance / 2.0)
            }
            DistanceMetric::Euclidean => {
                // Euclidean distance for normalized embeddings is [0, sqrt(2)]
                // Convert to similarity [0, 1]
                1.0 - (distance / 2.0_f32.sqrt())
            }
            DistanceMetric::DotProduct => {
                // Dot product: higher is more similar
                // For normalized vectors, range is [-1, 1]
                // Convert to [0, 1]
                (distance + 1.0) / 2.0
            }
            DistanceMetric::MaxInnerProduct => {
                // Max inner product: higher is more similar
                if distance > 0.0 {
                    distance
                } else {
                    -distance
                }
            }
        }
    }

    /// Calculate cosine distance (1 - `cosine_similarity`)
    fn cosine_distance(a: &[f32], b: &[f32]) -> Result<f32> {
        let dot = Self::dot_product(a, b);
        let norm_a = Self::magnitude(a);
        let norm_b = Self::magnitude(b);

        if norm_a == 0.0 || norm_b == 0.0 {
            return Ok(1.0); // Maximum distance for zero vectors
        }

        let similarity = dot / (norm_a * norm_b);
        // Clamp to [-1, 1] to handle floating point errors
        let similarity = similarity.clamp(-1.0, 1.0);
        Ok(1.0 - similarity)
    }

    /// Calculate Euclidean distance (L2 norm)
    fn euclidean_distance(a: &[f32], b: &[f32]) -> Result<f32> {
        let sum: f32 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum();
        Ok(sum.sqrt())
    }

    /// Calculate dot product
    fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    /// Calculate vector magnitude (L2 norm)
    fn magnitude(v: &[f32]) -> f32 {
        v.iter().map(|x| x.powi(2)).sum::<f32>().sqrt()
    }
}

/// Search type for vector store queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchType {
    /// Standard similarity search (returns k most similar)
    Similarity,

    /// Similarity search with score threshold filtering
    SimilarityScoreThreshold,

    /// Maximum Marginal Relevance: balances similarity with diversity
    /// Avoids returning near-duplicate results
    Mmr,
}

/// Parameters for vector store searches.
#[derive(Debug, Clone)]
pub struct SearchParams {
    /// Number of results to return
    pub k: usize,

    /// Search type to use
    pub search_type: SearchType,

    /// Filter results by metadata (field -> value)
    pub filter: Option<HashMap<String, serde_json::Value>>,

    /// Minimum relevance score threshold (0.0 to 1.0)
    /// Only used with `SimilarityScoreThreshold` search type
    pub score_threshold: Option<f32>,

    /// Lambda parameter for MMR (0 = max diversity, 1 = max relevance)
    /// Only used with Mmr search type
    pub lambda: Option<f32>,

    /// Number of initial candidates to fetch for MMR
    /// Only used with Mmr search type
    pub fetch_k: Option<usize>,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            k: 4,
            search_type: SearchType::Similarity,
            filter: None,
            score_threshold: None,
            lambda: Some(0.5), // Balanced diversity/relevance
            fetch_k: Some(20), // Fetch 20 candidates for MMR by default
        }
    }
}

impl SearchParams {
    /// Create new search parameters with k results
    #[must_use]
    pub fn new(k: usize) -> Self {
        Self {
            k,
            ..Default::default()
        }
    }

    /// Set search type
    #[must_use]
    pub fn with_search_type(mut self, search_type: SearchType) -> Self {
        self.search_type = search_type;
        self
    }

    /// Set metadata filter
    #[must_use]
    pub fn with_filter(mut self, filter: HashMap<String, serde_json::Value>) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set score threshold for filtering
    #[must_use]
    pub fn with_score_threshold(mut self, threshold: f32) -> Self {
        self.score_threshold = Some(threshold);
        self.search_type = SearchType::SimilarityScoreThreshold;
        self
    }

    /// Set lambda for MMR (0 = max diversity, 1 = max relevance)
    #[must_use]
    pub fn with_lambda(mut self, lambda: f32) -> Self {
        self.lambda = Some(lambda);
        self.search_type = SearchType::Mmr;
        self
    }

    /// Set `fetch_k` for MMR (number of candidates to fetch)
    #[must_use]
    pub fn with_fetch_k(mut self, fetch_k: usize) -> Self {
        self.fetch_k = Some(fetch_k);
        self
    }
}

/// Calculate Maximal Marginal Relevance (MMR) for diverse retrieval.
///
/// MMR selects documents that balance relevance to the query with diversity
/// among the selected documents, avoiding near-duplicates in results.
///
/// The algorithm works by:
/// 1. Starting with the most relevant document to the query
/// 2. Iteratively adding documents that maximize:
///    `lambda * similarity_to_query - (1-lambda) * max_similarity_to_selected`
///
/// # Arguments
///
/// * `query_embedding` - Query vector
/// * `embedding_list` - List of candidate vectors
/// * `k` - Number of documents to return
/// * `lambda` - Diversity parameter (0 = max diversity, 1 = max relevance)
///
/// # Returns
///
/// List of indices into `embedding_list` ordered by MMR score
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::vector_stores::maximal_marginal_relevance;
///
/// let query_vec = vec![1.0, 0.0, 0.0];
/// let candidates = vec![
///     vec![0.9, 0.1, 0.0],  // Very similar to query
///     vec![0.85, 0.15, 0.0], // Also similar to query (near-duplicate)
///     vec![0.5, 0.5, 0.0],  // Less similar but diverse
/// ];
///
/// // With lambda=0.5, balances relevance and diversity
/// let selected = maximal_marginal_relevance(&query_vec, &candidates, 2, 0.5)?;
/// // Returns [0, 2] - most relevant + most diverse
/// ```
pub fn maximal_marginal_relevance(
    query_embedding: &[f32],
    embedding_list: &[Vec<f32>],
    k: usize,
    lambda: f32,
) -> Result<Vec<usize>> {
    let effective_k = k.min(embedding_list.len());
    if effective_k == 0 {
        return Ok(Vec::new());
    }

    // Calculate similarity to query for all candidates
    let mut similarities: Vec<f32> = Vec::new();
    for embedding in embedding_list {
        let distance = DistanceMetric::Cosine.calculate(query_embedding, embedding)?;
        let similarity = DistanceMetric::Cosine.distance_to_relevance(distance);
        similarities.push(similarity);
    }

    // Find most similar document
    let mut best_idx = 0;
    let mut best_similarity = similarities[0];
    for (idx, &sim) in similarities.iter().enumerate().skip(1) {
        if sim > best_similarity {
            best_similarity = sim;
            best_idx = idx;
        }
    }

    let mut selected_indices = vec![best_idx];
    let mut selected_embeddings = vec![embedding_list[best_idx].clone()];

    // Iteratively select diverse documents
    while selected_indices.len() < effective_k {
        let mut best_score = f32::NEG_INFINITY;
        let mut idx_to_add = 0;

        for (i, query_score) in similarities.iter().enumerate() {
            if selected_indices.contains(&i) {
                continue;
            }

            // Calculate max similarity to already selected documents
            let mut max_similarity = f32::NEG_INFINITY;
            for selected_emb in &selected_embeddings {
                let distance =
                    DistanceMetric::Cosine.calculate(&embedding_list[i], selected_emb)?;
                let similarity = DistanceMetric::Cosine.distance_to_relevance(distance);
                max_similarity = max_similarity.max(similarity);
            }

            // MMR score: lambda * similarity_to_query - (1 - lambda) * max_similarity_to_selected
            let mmr_score = lambda * query_score - (1.0 - lambda) * max_similarity;

            if mmr_score > best_score {
                best_score = mmr_score;
                idx_to_add = i;
            }
        }

        selected_indices.push(idx_to_add);
        selected_embeddings.push(embedding_list[idx_to_add].clone());
    }

    Ok(selected_indices)
}

/// Core vector store trait for storing and searching embeddings.
///
/// Vector stores provide persistent storage for embedded documents and enable
/// efficient similarity search. Implementations typically integrate with specialized
/// vector databases (Chroma, Qdrant, Pinecone, etc.).
///
/// # Required Methods
///
/// Implementations must provide:
/// - `add_texts`: Add documents to the store
/// - `similarity_search`: Find k most similar documents
///
/// # Optional Methods
///
/// Default implementations are provided for:
/// - `add_documents`: Adds documents (delegates to `add_texts`)
/// - `similarity_search_with_score`: Returns documents with scores
/// - `search`: Generic search with multiple strategies
/// - `delete`: Delete documents by ID
/// - `get_by_ids`: Retrieve documents by ID
///
/// # Example Implementation
///
/// ```rust,ignore
/// use dashflow::core::vector_stores::VectorStore;
///
/// struct MyVectorStore { /* ... */ }
///
/// #[async_trait]
/// impl VectorStore for MyVectorStore {
///     async fn add_texts(
///         &mut self,
///         texts: &[impl AsRef<str>],
///         metadatas: Option<&[HashMap<String, serde_json::Value>]>,
///         ids: Option<&[String]>,
///     ) -> Result<Vec<String>> {
///         // Embed texts and store vectors
///         todo!()
///     }
///
///     async fn _similarity_search(
///         &self,
///         query: &str,
///         k: usize,
///         filter: Option<&HashMap<String, serde_json::Value>>,
///     ) -> Result<Vec<Document>> {
///         // Embed query and search for similar vectors
///         todo!()
///     }
///
///     fn distance_metric(&self) -> DistanceMetric {
///         DistanceMetric::Cosine
///     }
/// }
/// ```
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Get the embeddings instance used by this vector store.
    ///
    /// Returns None if the vector store doesn't expose its embeddings.
    fn embeddings(&self) -> Option<Arc<dyn Embeddings>> {
        None
    }

    /// Get the distance metric used by this vector store.
    fn distance_metric(&self) -> DistanceMetric {
        DistanceMetric::Cosine
    }

    /// Add texts to the vector store.
    ///
    /// # Arguments
    ///
    /// * `texts` - Texts to embed and add to the store
    /// * `metadatas` - Optional metadata for each text (must match length of texts)
    /// * `ids` - Optional IDs for each text (if None, UUIDs will be generated)
    ///
    /// # Returns
    ///
    /// List of IDs for the added texts
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Metadatas length doesn't match texts length
    /// - IDs length doesn't match texts length
    /// - Embedding fails
    /// - Storage operation fails
    async fn add_texts(
        &mut self,
        texts: &[impl AsRef<str> + Send + Sync],
        metadatas: Option<&[HashMap<String, serde_json::Value>]>,
        ids: Option<&[String]>,
    ) -> Result<Vec<String>>;

    /// Add documents to the vector store.
    ///
    /// # Arguments
    ///
    /// * `documents` - Documents to add (`page_content` will be embedded)
    /// * `ids` - Optional IDs for each document (overrides document.id if present)
    ///
    /// # Returns
    ///
    /// List of IDs for the added documents
    async fn add_documents(
        &mut self,
        documents: &[Document],
        ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
        // Extract text references (no cloning - AsRef<str> works with &str)
        let texts: Vec<&str> = documents
            .iter()
            .map(|doc| doc.page_content.as_str())
            .collect();

        // Metadata must still be cloned (signature requires &[HashMap], not &[&HashMap])
        let metadatas: Vec<HashMap<String, serde_json::Value>> =
            documents.iter().map(|doc| doc.metadata.clone()).collect();

        // Generate IDs if not provided
        let generated_ids: Vec<String>;
        let ids_ref = if let Some(ids) = ids {
            ids
        } else {
            // Use document IDs if available, otherwise generate UUIDs
            generated_ids = documents
                .iter()
                .map(|doc| {
                    doc.id
                        .clone()
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
                })
                .collect();
            &generated_ids
        };

        self.add_texts(&texts, Some(&metadatas), Some(ids_ref))
            .await
    }

    /// Delete documents by ID.
    ///
    /// # Arguments
    ///
    /// * `ids` - IDs of documents to delete (if None, delete all)
    ///
    /// # Returns
    ///
    /// True if deletion was successful, False otherwise
    async fn delete(&mut self, _ids: Option<&[String]>) -> Result<bool> {
        Err(Error::NotImplemented(
            "delete not implemented for this vector store".to_string(),
        ))
    }

    /// Get documents by their IDs.
    ///
    /// # Arguments
    ///
    /// * `ids` - IDs of documents to retrieve
    ///
    /// # Returns
    ///
    /// List of documents (may be fewer than requested if some IDs not found)
    ///
    /// # Notes
    ///
    /// - Does not raise errors if some IDs are not found
    /// - Order of returned documents may not match order of input IDs
    /// - Users should check document.id field to match results
    async fn get_by_ids(&self, _ids: &[String]) -> Result<Vec<Document>> {
        Err(Error::NotImplemented(
            "get_by_ids not implemented for this vector store".to_string(),
        ))
    }

    /// Internal method - use `dashflow::vector_search()` instead.
    ///
    /// Application code should use the framework API which provides:
    /// - ExecutionTrace collection for optimizers
    /// - Streaming events for live progress
    /// - Introspection capabilities
    /// - Metrics collection (latency, result count)
    ///
    /// # Arguments
    ///
    /// * `query` - Query text to search for
    /// * `k` - Number of results to return
    /// * `filter` - Optional metadata filter (field -> value)
    ///
    /// # Returns
    ///
    /// List of most similar documents
    #[doc(hidden)]
    async fn _similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>>;

    /// Perform similarity search with relevance scores.
    ///
    /// # Arguments
    ///
    /// * `query` - Query text to search for
    /// * `k` - Number of results to return
    /// * `filter` - Optional metadata filter
    ///
    /// # Returns
    ///
    /// List of (document, score) tuples where score is in [0, 1]
    /// (0 = dissimilar, 1 = most similar)
    async fn similarity_search_with_score(
        &self,
        _query: &str,
        _k: usize,
        _filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<(Document, f32)>> {
        Err(Error::NotImplemented(
            "similarity_search_with_score not implemented for this vector store".to_string(),
        ))
    }

    /// Perform similarity search by vector.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Query embedding vector
    /// * `k` - Number of results to return
    /// * `filter` - Optional metadata filter
    ///
    /// # Returns
    ///
    /// List of most similar documents
    async fn similarity_search_by_vector(
        &self,
        _embedding: &[f32],
        _k: usize,
        _filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        Err(Error::NotImplemented(
            "similarity_search_by_vector not implemented for this vector store".to_string(),
        ))
    }

    /// Perform similarity search by vector with scores.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Query embedding vector
    /// * `k` - Number of results to return
    /// * `filter` - Optional metadata filter
    ///
    /// # Returns
    ///
    /// List of (document, score) tuples
    async fn similarity_search_by_vector_with_score(
        &self,
        _embedding: &[f32],
        _k: usize,
        _filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<(Document, f32)>> {
        Err(Error::NotImplemented(
            "similarity_search_by_vector_with_score not implemented for this vector store"
                .to_string(),
        ))
    }

    /// Maximum Marginal Relevance search.
    ///
    /// Returns documents that are similar to the query but diverse from each other.
    /// Useful for avoiding redundant results.
    ///
    /// # Arguments
    ///
    /// * `query` - Query text
    /// * `k` - Number of results to return
    /// * `fetch_k` - Number of candidates to fetch before MMR reranking
    /// * `lambda` - Diversity parameter (0 = max diversity, 1 = max relevance)
    /// * `filter` - Optional metadata filter
    ///
    /// # Returns
    ///
    /// List of diverse, relevant documents
    async fn max_marginal_relevance_search(
        &self,
        _query: &str,
        _k: usize,
        _fetch_k: usize,
        _lambda: f32,
        _filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        Err(Error::NotImplemented(
            "max_marginal_relevance_search not implemented for this vector store".to_string(),
        ))
    }

    /// Generic search method that dispatches to specific search types.
    ///
    /// # Arguments
    ///
    /// * `query` - Query text
    /// * `params` - Search parameters (k, search type, filters, etc.)
    ///
    /// # Returns
    ///
    /// List of documents matching the search criteria
    async fn search(&self, query: &str, params: &SearchParams) -> Result<Vec<Document>> {
        match params.search_type {
            SearchType::Similarity => {
                self._similarity_search(query, params.k, params.filter.as_ref())
                    .await
            }
            SearchType::SimilarityScoreThreshold => {
                let docs_and_scores = self
                    .similarity_search_with_score(query, params.k, params.filter.as_ref())
                    .await?;

                // Filter by score threshold if provided
                let docs: Vec<Document> = if let Some(threshold) = params.score_threshold {
                    docs_and_scores
                        .into_iter()
                        .filter(|(_, score)| *score >= threshold)
                        .map(|(doc, _)| doc)
                        .collect()
                } else {
                    docs_and_scores.into_iter().map(|(doc, _)| doc).collect()
                };

                Ok(docs)
            }
            SearchType::Mmr => {
                let fetch_k = params.fetch_k.unwrap_or(20);
                let lambda = params.lambda.unwrap_or(0.5);
                self.max_marginal_relevance_search(
                    query,
                    params.k,
                    fetch_k,
                    lambda,
                    params.filter.as_ref(),
                )
                .await
            }
        }
    }
}

/// In-memory vector store implementation.
///
/// This is a simple vector store that stores vectors in memory using a `HashMap`.
/// It's useful for testing, prototyping, and small datasets that fit in memory.
///
/// Uses cosine similarity for search by default (configurable via `distance_metric`).
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::vector_stores::{InMemoryVectorStore, VectorStore};
/// use dashflow::core::embeddings::Embeddings;
/// use std::sync::Arc;
///
/// let embeddings = Arc::new(MyEmbeddings::new());
/// let mut store = InMemoryVectorStore::new(embeddings);
///
/// // Add documents
/// let ids = store.add_texts(&["doc1", "doc2"], None, None).await?;
///
/// // Search
/// let results = store._similarity_search("query", 5, None).await?;
/// ```
#[derive(Clone)]
pub struct InMemoryVectorStore {
    /// Internal storage: ID -> `StoredDocument`
    store: HashMap<String, StoredDocument>,
    /// Embeddings function
    embedding: Arc<dyn Embeddings>,
    /// Distance metric to use for similarity calculation
    metric: DistanceMetric,
}

/// Internal document representation with embedded vector.
#[derive(Debug, Clone)]
struct StoredDocument {
    id: String,
    text: String,
    vector: Vec<f32>,
    metadata: HashMap<String, serde_json::Value>,
}

impl InMemoryVectorStore {
    /// Create a new in-memory vector store.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Embeddings function to use
    pub fn new(embedding: Arc<dyn Embeddings>) -> Self {
        Self {
            store: HashMap::new(),
            embedding,
            metric: DistanceMetric::Cosine,
        }
    }

    /// Create a new in-memory vector store with a specific distance metric.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Embeddings function to use
    /// * `metric` - Distance metric for similarity calculation
    #[must_use]
    pub fn with_metric(embedding: Arc<dyn Embeddings>, metric: DistanceMetric) -> Self {
        Self {
            store: HashMap::new(),
            embedding,
            metric,
        }
    }

    /// Check if a document matches the given metadata filter.
    ///
    /// Returns true if all filter key-value pairs match the document's metadata.
    fn matches_filter(
        doc: &StoredDocument,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> bool {
        match filter {
            None => true,
            Some(filter) => filter
                .iter()
                .all(|(key, value)| doc.metadata.get(key) == Some(value)),
        }
    }

    /// Internal similarity search by vector with scores and vectors returned.
    ///
    /// Used internally for MMR which needs access to the vectors.
    async fn similarity_search_by_vector_with_vectors(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<(Document, f32, Vec<f32>)>> {
        // Collect all documents
        let docs: Vec<&StoredDocument> = self.store.values().collect();

        // Filter if needed
        let filtered_docs: Vec<&StoredDocument> = docs
            .into_iter()
            .filter(|doc| Self::matches_filter(doc, filter))
            .collect();

        if filtered_docs.is_empty() {
            return Ok(Vec::new());
        }

        // Calculate distances for all filtered documents
        // Pre-allocate vector with exact capacity for better performance
        let mut doc_scores: Vec<(usize, f32)> = Vec::with_capacity(filtered_docs.len());
        for (idx, doc) in filtered_docs.iter().enumerate() {
            let distance = self.metric.calculate(embedding, &doc.vector)?;
            let score = self.metric.distance_to_relevance(distance);
            doc_scores.push((idx, score));
        }

        // Use unstable sort for better performance (order of equal elements doesn't matter)
        // Use partial_select to get top-k without fully sorting when k << n
        if k < doc_scores.len() {
            doc_scores.select_nth_unstable_by(k, |a, b| {
                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
            });
            // Truncate to k elements and sort them
            doc_scores.truncate(k);
        }
        doc_scores
            .sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Build result
        let results: Vec<(Document, f32, Vec<f32>)> = doc_scores
            .into_iter()
            .map(|(idx, score)| {
                let doc = filtered_docs[idx];
                (
                    Document {
                        id: Some(doc.id.clone()),
                        page_content: doc.text.clone(),
                        metadata: doc.metadata.clone(),
                    },
                    score,
                    doc.vector.clone(),
                )
            })
            .collect();

        Ok(results)
    }
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    fn embeddings(&self) -> Option<Arc<dyn Embeddings>> {
        Some(Arc::clone(&self.embedding))
    }

    fn distance_metric(&self) -> DistanceMetric {
        self.metric
    }

    async fn add_texts(
        &mut self,
        texts: &[impl AsRef<str> + Send + Sync],
        metadatas: Option<&[HashMap<String, serde_json::Value>]>,
        ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
        // Validate input lengths
        let text_count = texts.len();
        if let Some(metadatas) = metadatas {
            if metadatas.len() != text_count {
                return Err(Error::config(format!(
                    "Metadatas length ({}) must match texts length ({})",
                    metadatas.len(),
                    text_count
                )));
            }
        }
        if let Some(ids) = ids {
            if ids.len() != text_count {
                return Err(Error::config(format!(
                    "IDs length ({}) must match texts length ({})",
                    ids.len(),
                    text_count
                )));
            }
        }

        // Convert texts to strings and embed
        let text_strings: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();
        let vectors = self
            .embedding
            ._embed_documents(&text_strings)
            .await?;

        // Generate or use provided IDs
        let doc_ids: Vec<String> = if let Some(ids) = ids {
            ids.to_vec()
        } else {
            (0..text_count)
                .map(|_| uuid::Uuid::new_v4().to_string())
                .collect()
        };

        // Store documents
        let mut result_ids = Vec::new();
        for (idx, text) in text_strings.iter().enumerate() {
            let doc_id = doc_ids[idx].clone();
            let metadata = metadatas
                .and_then(|m| m.get(idx))
                .cloned()
                .unwrap_or_default();

            self.store.insert(
                doc_id.clone(),
                StoredDocument {
                    id: doc_id.clone(),
                    text: text.clone(),
                    vector: vectors[idx].clone(),
                    metadata,
                },
            );

            result_ids.push(doc_id);
        }

        Ok(result_ids)
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        if let Some(ids) = ids {
            for id in ids {
                self.store.remove(id);
            }
        } else {
            // Delete all
            self.store.clear();
        }
        Ok(true)
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        let mut documents = Vec::new();
        for id in ids {
            if let Some(doc) = self.store.get(id) {
                documents.push(Document {
                    id: Some(doc.id.clone()),
                    page_content: doc.text.clone(),
                    metadata: doc.metadata.clone(),
                });
            }
        }
        Ok(documents)
    }

    async fn _similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        let docs_and_scores = self.similarity_search_with_score(query, k, filter).await?;
        Ok(docs_and_scores.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<(Document, f32)>> {
        let embedding = self
            .embedding
            ._embed_query(query)
            .await?;
        self.similarity_search_by_vector_with_score(&embedding, k, filter)
            .await
    }

    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        let docs_and_scores = self
            .similarity_search_by_vector_with_score(embedding, k, filter)
            .await?;
        Ok(docs_and_scores.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<(Document, f32)>> {
        let results = self
            .similarity_search_by_vector_with_vectors(embedding, k, filter)
            .await?;
        Ok(results
            .into_iter()
            .map(|(doc, score, _)| (doc, score))
            .collect())
    }

    async fn max_marginal_relevance_search(
        &self,
        query: &str,
        k: usize,
        fetch_k: usize,
        lambda: f32,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        // Embed query
        let query_embedding = self
            .embedding
            ._embed_query(query)
            .await?;

        // Fetch candidates
        let candidates = self
            .similarity_search_by_vector_with_vectors(&query_embedding, fetch_k, filter)
            .await?;

        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // Extract vectors
        let vectors: Vec<Vec<f32>> = candidates.iter().map(|(_, _, vec)| vec.clone()).collect();

        // Run MMR
        let selected_indices = maximal_marginal_relevance(&query_embedding, &vectors, k, lambda)?;

        // Return selected documents
        let results: Vec<Document> = selected_indices
            .into_iter()
            .map(|idx| candidates[idx].0.clone())
            .collect();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_distance_metric_cosine() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let distance = DistanceMetric::Cosine.calculate(&a, &b).unwrap();
        assert!(
            (distance - 0.0).abs() < 1e-6,
            "Identical vectors should have 0 distance"
        );

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let distance = DistanceMetric::Cosine.calculate(&a, &b).unwrap();
        assert!(
            (distance - 1.0).abs() < 1e-6,
            "Orthogonal vectors should have distance 1.0"
        );

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let distance = DistanceMetric::Cosine.calculate(&a, &b).unwrap();
        assert!(
            (distance - 2.0).abs() < 1e-6,
            "Opposite vectors should have distance 2.0"
        );
    }

    #[test]
    fn test_distance_metric_euclidean() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![0.0, 0.0, 0.0];
        let distance = DistanceMetric::Euclidean.calculate(&a, &b).unwrap();
        assert!(
            (distance - 0.0).abs() < 1e-6,
            "Identical points should have 0 distance"
        );

        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let distance = DistanceMetric::Euclidean.calculate(&a, &b).unwrap();
        assert!((distance - 1.0).abs() < 1e-6, "Unit distance");

        let a = vec![0.0, 0.0, 0.0];
        let b = vec![3.0, 4.0, 0.0];
        let distance = DistanceMetric::Euclidean.calculate(&a, &b).unwrap();
        assert!((distance - 5.0).abs() < 1e-6, "3-4-5 triangle");
    }

    #[test]
    fn test_distance_metric_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let distance = DistanceMetric::DotProduct.calculate(&a, &b).unwrap();
        // 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32
        assert!((distance - 32.0).abs() < 1e-6);
    }

    #[test]
    fn test_distance_metric_dimension_mismatch() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let result = DistanceMetric::Cosine.calculate(&a, &b);
        assert!(result.is_err(), "Should error on dimension mismatch");
    }

    #[test]
    fn test_cosine_relevance_conversion() {
        // Distance 0 (identical) -> relevance 1.0
        let relevance = DistanceMetric::Cosine.distance_to_relevance(0.0);
        assert!((relevance - 1.0).abs() < 1e-6);

        // Distance 1.0 (orthogonal) -> relevance 0.5
        let relevance = DistanceMetric::Cosine.distance_to_relevance(1.0);
        assert!((relevance - 0.5).abs() < 1e-6);

        // Distance 2.0 (opposite) -> relevance 0.0
        let relevance = DistanceMetric::Cosine.distance_to_relevance(2.0);
        assert!((relevance - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_relevance_conversion() {
        // Distance 0 -> relevance 1.0
        let relevance = DistanceMetric::Euclidean.distance_to_relevance(0.0);
        assert!((relevance - 1.0).abs() < 1e-6);

        // Distance sqrt(2) (max for unit vectors) -> relevance 0.0
        let relevance = DistanceMetric::Euclidean.distance_to_relevance(2.0_f32.sqrt());
        assert!((relevance - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_search_params_builder() {
        let params = SearchParams::new(10)
            .with_search_type(SearchType::Mmr)
            .with_lambda(0.7)
            .with_fetch_k(50);

        assert_eq!(params.k, 10);
        assert_eq!(params.search_type, SearchType::Mmr);
        assert_eq!(params.lambda, Some(0.7));
        assert_eq!(params.fetch_k, Some(50));
    }

    #[test]
    fn test_search_params_score_threshold() {
        let params = SearchParams::new(5).with_score_threshold(0.8);

        assert_eq!(params.k, 5);
        assert_eq!(params.search_type, SearchType::SimilarityScoreThreshold);
        assert_eq!(params.score_threshold, Some(0.8));
    }

    #[test]
    fn test_search_params_default() {
        let params = SearchParams::default();

        assert_eq!(params.k, 4);
        assert_eq!(params.search_type, SearchType::Similarity);
        assert_eq!(params.lambda, Some(0.5));
        assert_eq!(params.fetch_k, Some(20));
    }

    #[test]
    fn test_search_params_with_filter() {
        let mut filter = std::collections::HashMap::new();
        filter.insert("source".to_string(), serde_json::json!("docs"));
        filter.insert("version".to_string(), serde_json::json!(1));

        let params = SearchParams::new(10).with_filter(filter);

        assert_eq!(params.k, 10);
        assert!(params.filter.is_some());
        let f = params.filter.unwrap();
        assert_eq!(f.get("source"), Some(&serde_json::json!("docs")));
        assert_eq!(f.get("version"), Some(&serde_json::json!(1)));
    }

    #[test]
    fn test_distance_metric_max_inner_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let distance = DistanceMetric::MaxInnerProduct.calculate(&a, &b).unwrap();
        // Same as dot product: 1*4 + 2*5 + 3*6 = 32
        assert!((distance - 32.0).abs() < 1e-6);
    }

    #[test]
    fn test_dot_product_relevance_conversion() {
        // For normalized vectors, dot product is in [-1, 1]
        // 1.0 (identical) -> relevance 1.0
        let relevance = DistanceMetric::DotProduct.distance_to_relevance(1.0);
        assert!((relevance - 1.0).abs() < 1e-6);

        // 0.0 (orthogonal) -> relevance 0.5
        let relevance = DistanceMetric::DotProduct.distance_to_relevance(0.0);
        assert!((relevance - 0.5).abs() < 1e-6);

        // -1.0 (opposite) -> relevance 0.0
        let relevance = DistanceMetric::DotProduct.distance_to_relevance(-1.0);
        assert!((relevance - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_max_inner_product_relevance_conversion() {
        // Positive values stay as is
        let relevance = DistanceMetric::MaxInnerProduct.distance_to_relevance(0.8);
        assert!((relevance - 0.8).abs() < 1e-6);

        // Negative values get negated (absolute value)
        let relevance = DistanceMetric::MaxInnerProduct.distance_to_relevance(-0.5);
        assert!((relevance - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_distance_zero_vectors() {
        // Zero vector should return max distance 1.0
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let distance = DistanceMetric::Cosine.calculate(&a, &b).unwrap();
        assert!(
            (distance - 1.0).abs() < 1e-6,
            "Zero vector should have max distance"
        );

        // Both zero vectors
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![0.0, 0.0, 0.0];
        let distance = DistanceMetric::Cosine.calculate(&a, &b).unwrap();
        assert!(
            (distance - 1.0).abs() < 1e-6,
            "Two zero vectors should have max distance"
        );
    }

    #[test]
    fn test_distance_metric_clone_and_debug() {
        let metric = DistanceMetric::Cosine;
        let cloned = metric;
        assert_eq!(metric, cloned);

        let debug_str = format!("{:?}", metric);
        assert!(debug_str.contains("Cosine"));
    }

    #[test]
    fn test_search_type_enum() {
        let similarity = SearchType::Similarity;
        let threshold = SearchType::SimilarityScoreThreshold;
        let mmr = SearchType::Mmr;

        // Each should be distinct
        assert_ne!(similarity, threshold);
        assert_ne!(threshold, mmr);
        assert_ne!(similarity, mmr);

        // Debug formatting
        assert!(format!("{:?}", similarity).contains("Similarity"));
        assert!(format!("{:?}", threshold).contains("Threshold"));
        assert!(format!("{:?}", mmr).contains("Mmr"));
    }

    #[test]
    fn test_search_params_chained_builders() {
        let mut filter = std::collections::HashMap::new();
        filter.insert("category".to_string(), serde_json::json!("tech"));

        let params = SearchParams::new(20)
            .with_search_type(SearchType::Similarity)
            .with_filter(filter)
            .with_fetch_k(100);

        assert_eq!(params.k, 20);
        assert_eq!(params.search_type, SearchType::Similarity);
        assert!(params.filter.is_some());
        assert_eq!(params.fetch_k, Some(100));
    }

    // InMemoryVectorStore tests

    /// Mock embeddings for testing - generates simple deterministic vectors
    struct MockEmbeddings;

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            // Generate simple deterministic vectors based on text content
            // Each text gets a 3D vector based on its first few characters
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

                    // Normalize to unit vector
                    let mag = (x * x + y * y + z * z).sqrt();
                    if mag > 0.0 {
                        vec![x / mag, y / mag, z / mag]
                    } else {
                        vec![0.0, 0.0, 0.0]
                    }
                })
                .collect())
        }

        async fn _embed_query(&self, text: &str) -> Result<Vec<f32>> {
            let result = self._embed_documents(&[text.to_string()]).await?;
            Ok(result.into_iter().next().unwrap())
        }
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_add_and_search() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        // Add documents
        let texts = vec!["apple", "banana", "cherry"];
        let ids = store.add_texts(&texts, None, None).await.unwrap();

        assert_eq!(ids.len(), 3);

        // Search
        let results = store._similarity_search("apple", 2, None).await.unwrap();
        assert_eq!(results.len(), 2);
        // First result should be "apple" (exact match)
        assert_eq!(results[0].page_content, "apple");
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_with_metadata() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        // Add documents with metadata
        let texts = vec!["doc1", "doc2", "doc3"];
        let mut metadata1 = HashMap::new();
        metadata1.insert("category".to_string(), serde_json::json!("fruit"));
        metadata1.insert("color".to_string(), serde_json::json!("red"));

        let mut metadata2 = HashMap::new();
        metadata2.insert("category".to_string(), serde_json::json!("fruit"));
        metadata2.insert("color".to_string(), serde_json::json!("yellow"));

        let mut metadata3 = HashMap::new();
        metadata3.insert("category".to_string(), serde_json::json!("vegetable"));
        metadata3.insert("color".to_string(), serde_json::json!("green"));

        let metadatas = vec![metadata1, metadata2, metadata3];
        let ids = store
            .add_texts(&texts, Some(&metadatas), None)
            .await
            .unwrap();

        assert_eq!(ids.len(), 3);

        // Verify metadata was stored
        let docs = store.get_by_ids(&ids).await.unwrap();
        assert_eq!(docs.len(), 3);
        assert_eq!(
            docs[0].metadata.get("category"),
            Some(&serde_json::json!("fruit"))
        );
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_metadata_filtering() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        // Add documents with metadata
        let texts = vec!["apple", "banana", "carrot"];
        let mut metadata1 = HashMap::new();
        metadata1.insert("type".to_string(), serde_json::json!("fruit"));

        let mut metadata2 = HashMap::new();
        metadata2.insert("type".to_string(), serde_json::json!("fruit"));

        let mut metadata3 = HashMap::new();
        metadata3.insert("type".to_string(), serde_json::json!("vegetable"));

        let metadatas = vec![metadata1, metadata2, metadata3];
        store
            .add_texts(&texts, Some(&metadatas), None)
            .await
            .unwrap();

        // Search with filter for fruits only
        let mut filter = HashMap::new();
        filter.insert("type".to_string(), serde_json::json!("fruit"));

        let results = store
            ._similarity_search("apple", 10, Some(&filter))
            .await
            .unwrap();

        // Should only get fruit documents
        assert_eq!(results.len(), 2);
        for doc in &results {
            assert_eq!(doc.metadata.get("type"), Some(&serde_json::json!("fruit")));
        }
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_with_scores() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        let texts = vec!["apple", "banana", "cherry"];
        store.add_texts(&texts, None, None).await.unwrap();

        let results = store
            .similarity_search_with_score("apple", 3, None)
            .await
            .unwrap();

        assert_eq!(results.len(), 3);

        // Scores should be in [0, 1] range
        for (_, score) in &results {
            assert!(*score >= 0.0 && *score <= 1.0);
        }

        // First result should have highest score
        assert!(results[0].1 >= results[1].1);
        assert!(results[1].1 >= results[2].1);
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_delete() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        let texts = vec!["doc1", "doc2", "doc3"];
        let ids = store.add_texts(&texts, None, None).await.unwrap();

        // Delete one document
        store.delete(Some(&[ids[1].clone()])).await.unwrap();

        // Verify it was deleted
        let docs = store.get_by_ids(&ids).await.unwrap();
        assert_eq!(docs.len(), 2); // Should only get 2 documents
        assert_eq!(docs[0].id, Some(ids[0].clone()));
        assert_eq!(docs[1].id, Some(ids[2].clone()));
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_delete_all() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        let texts = vec!["doc1", "doc2", "doc3"];
        let ids = store.add_texts(&texts, None, None).await.unwrap();

        // Delete all documents
        store.delete(None).await.unwrap();

        // Verify all deleted
        let docs = store.get_by_ids(&ids).await.unwrap();
        assert_eq!(docs.len(), 0);
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_get_by_ids() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        let texts = vec!["doc1", "doc2", "doc3"];
        let ids = store.add_texts(&texts, None, None).await.unwrap();

        // Get specific documents
        let docs = store
            .get_by_ids(&[ids[0].clone(), ids[2].clone()])
            .await
            .unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].page_content, "doc1");
        assert_eq!(docs[1].page_content, "doc3");
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_mmr() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        // Add similar documents
        let texts = vec!["apple", "apricot", "avocado", "banana", "blueberry"];
        store.add_texts(&texts, None, None).await.unwrap();

        // MMR search with high diversity (lambda = 0.3)
        let results = store
            .max_marginal_relevance_search("apple", 3, 5, 0.3, None)
            .await
            .unwrap();

        assert_eq!(results.len(), 3);
        // First result should be "apple" (most similar)
        assert_eq!(results[0].page_content, "apple");
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_custom_ids() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        let texts = vec!["doc1", "doc2"];
        let custom_ids = vec!["id-1".to_string(), "id-2".to_string()];

        let ids = store
            .add_texts(&texts, None, Some(&custom_ids))
            .await
            .unwrap();

        // Should return the custom IDs
        assert_eq!(ids, custom_ids);

        // Verify we can retrieve by custom IDs
        let docs = store.get_by_ids(&custom_ids).await.unwrap();
        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_add_documents() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), serde_json::json!("test"));

        let documents = vec![
            Document {
                id: Some("doc1".to_string()),
                page_content: "content1".to_string(),
                metadata: metadata.clone(),
            },
            Document {
                id: Some("doc2".to_string()),
                page_content: "content2".to_string(),
                metadata: metadata.clone(),
            },
        ];

        let ids = store.add_documents(&documents, None).await.unwrap();

        // Should use document IDs
        assert_eq!(ids[0], "doc1");
        assert_eq!(ids[1], "doc2");

        // Verify documents were stored correctly
        let retrieved = store.get_by_ids(&ids).await.unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].page_content, "content1");
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_search_by_vector() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);
        let embedding_clone = Arc::clone(&embeddings);
        let mut store = InMemoryVectorStore::new(embedding_clone);

        let texts = vec!["apple", "banana", "cherry"];
        store.add_texts(&texts, None, None).await.unwrap();

        // Get embedding for query
        let query_vec = embeddings
            ._embed_query("apple")
            .await
            .unwrap();

        // Search by vector
        let results = store
            .similarity_search_by_vector(&query_vec, 2, None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].page_content, "apple");
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_empty_search() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);

        // Search in empty store
        let results = store._similarity_search("query", 5, None).await.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_validation() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        let texts = vec!["doc1", "doc2"];

        // Mismatched metadata length
        let metadata = vec![HashMap::new()]; // Only 1 metadata for 2 texts
        let result = store.add_texts(&texts, Some(&metadata), None).await;
        assert!(result.is_err());

        // Mismatched IDs length
        let ids = vec!["id1".to_string()]; // Only 1 ID for 2 texts
        let result = store.add_texts(&texts, None, Some(&ids)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_in_memory_vector_store_with_euclidean() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::with_metric(embeddings, DistanceMetric::Euclidean);

        let texts = vec!["apple", "banana"];
        store.add_texts(&texts, None, None).await.unwrap();

        // Search should work with Euclidean distance
        let results = store._similarity_search("apple", 1, None).await.unwrap();
        assert_eq!(results.len(), 1);

        // Verify the metric is set correctly
        assert_eq!(store.distance_metric(), DistanceMetric::Euclidean);
    }

    #[test]
    fn test_mmr_algorithm() {
        // Test the MMR algorithm directly
        let query = vec![1.0, 0.0, 0.0];
        let embeddings = vec![
            vec![1.0, 0.0, 0.0], // Identical to query
            vec![0.9, 0.1, 0.0], // Very similar
            vec![0.0, 1.0, 0.0], // Orthogonal (diverse)
        ];

        let indices = maximal_marginal_relevance(&query, &embeddings, 2, 0.5).unwrap();

        // Should select the most similar first
        assert_eq!(indices[0], 0);
        // Second should balance relevance and diversity
        assert_eq!(indices.len(), 2);
    }

    #[test]
    fn test_mmr_empty() {
        let query = vec![1.0, 0.0, 0.0];
        let embeddings: Vec<Vec<f32>> = vec![];

        let indices = maximal_marginal_relevance(&query, &embeddings, 2, 0.5).unwrap();
        assert_eq!(indices.len(), 0);
    }

    #[test]
    fn test_mmr_high_relevance() {
        // With lambda=1.0 (all relevance, no diversity), should just pick top k by similarity
        let query = vec![1.0, 0.0, 0.0];
        let embeddings = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.9, 0.1, 0.0],
            vec![0.8, 0.2, 0.0],
        ];

        let indices = maximal_marginal_relevance(&query, &embeddings, 2, 1.0).unwrap();

        // Should pick the 2 most similar
        assert_eq!(indices[0], 0);
        assert_eq!(indices.len(), 2);
    }
}
