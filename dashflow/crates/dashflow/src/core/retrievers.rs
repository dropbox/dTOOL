// Allow clippy warnings for retrievers
// - needless_pass_by_value: query String used in async boundary requires ownership
// - expect_used: Lock acquisition - expect() on mutex for shared state
#![allow(clippy::needless_pass_by_value, clippy::expect_used)]

//! Retrievers for document retrieval systems.
//!
//! A retriever is a component that takes a text query and returns the most relevant documents.
//! Retrievers are more general than vector stores - they only need to return documents,
//! not necessarily store them.
//!
//! # Core Concepts
//!
//! - **Retriever**: Abstract interface for retrieving documents from a query
//! - **Search Types**: Different strategies for retrieval (similarity, MMR, score threshold)
//!
//! # Available Retrievers
//!
//! ## Core Retrievers
//! - **`VectorStoreRetriever`**: Wrapper around `VectorStore` for retrieval operations
//! - **`EnsembleRetriever`**: Combines multiple retrievers using weighted Reciprocal Rank Fusion
//! - **`MergerRetriever`**: Merges results from multiple retrievers using round-robin pattern
//!
//! ## LLM-Enhanced Retrievers
//! - **`MultiQueryRetriever`**: Generates multiple queries using an LLM to improve retrieval
//! - **`SelfQueryRetriever`**: Uses LLM to generate structured queries with filters
//! - **`RePhraseQueryRetriever`**: Uses LLM to rephrase queries before retrieval
//! - **`ContextualCompressionRetriever`**: Post-processes documents with a compressor
//!
//! ## Text-Based Retrievers
//! - **`BM25Retriever`**: BM25 ranking algorithm for keyword search
//! - **`TFIDFRetriever`**: TF-IDF ranking for document retrieval
//! - **`KNNRetriever`**: K-nearest neighbors retrieval using embeddings
//!
//! ## Parent/Multi-Vector Retrievers
//! - **`MultiVectorRetriever`**: Base class for retrievers with multiple embeddings per document
//! - **`ParentDocumentRetriever`**: Retrieves small chunks but returns parent documents
//!
//! ## Time-Aware Retrievers
//! - **`TimeWeightedVectorStoreRetriever`**: Combines recency with relevance using time decay
//!
//! ## Web Retrievers
//! - **`WebResearchRetriever`**: Generates search queries, fetches web content, and retrieves
//!
//! ## External Backend Retrievers
//! - **`ElasticSearchBM25Retriever`**: BM25 retrieval via Elasticsearch
//! - **`PineconeHybridSearchRetriever`**: Hybrid dense+sparse search via Pinecone
//! - **`WeaviateHybridSearchRetriever`**: Hybrid search via Weaviate
//!
#![allow(clippy::items_after_test_module)]
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::retrievers::{VectorStoreRetriever, SearchType};
//! use dashflow::core::runnable::Runnable;
//!
//! // Create retriever from vector store
//! let retriever = VectorStoreRetriever::new(
//!     vector_store,
//!     SearchType::Similarity,
//!     SearchConfig::default().with_k(5),
//! );
//!
//! // Use as runnable
//! let documents = retriever.invoke("What is DashFlow?", None).await?;
//! ```

use crate::core::{
    config::RunnableConfig,
    documents::{Document, DocumentCompressor},
    error::{Error, Result},
    runnable::Runnable,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error as ThisError;

/// Type alias for retriever input (query string).
pub type RetrieverInput = String;

/// Type alias for retriever output (list of documents).
pub type RetrieverOutput = Vec<Document>;

/// Search type for retrieval operations.
///
/// Different search types use different algorithms to find relevant documents:
/// - **Similarity**: Basic similarity search using vector distance
/// - **`SimilarityScoreThreshold`**: Similarity search with minimum score filtering
/// - **MMR**: Maximal Marginal Relevance for diverse results
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SearchType {
    /// Basic similarity search - returns k most similar documents.
    #[default]
    Similarity,

    /// Similarity search with score threshold filtering.
    /// Only returns documents with similarity score >= threshold.
    SimilarityScoreThreshold,

    /// Maximal Marginal Relevance search for diverse results.
    /// Balances relevance and diversity using lambda parameter.
    #[serde(rename = "mmr")]
    MMR,
}

/// Configuration for retrieval search operations.
///
/// Controls how many documents to retrieve and search-specific parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Number of documents to retrieve (default: 4).
    #[serde(default = "default_k")]
    pub k: usize,

    /// Score threshold for `similarity_score_threshold` search (0.0-1.0).
    /// Only documents with score >= threshold are returned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_threshold: Option<f32>,

    /// Lambda parameter for MMR search (0.0-1.0, default: 0.5).
    /// - 1.0 = maximum relevance, no diversity
    /// - 0.0 = maximum diversity, no relevance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lambda_mult: Option<f32>,

    /// Fetch multiplier for MMR search (default: 20).
    /// Number of candidates to fetch before running MMR = k * `fetch_k`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch_k: Option<usize>,

    /// Additional search parameters passed to the underlying vector store.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn default_k() -> usize {
    4
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            k: default_k(),
            score_threshold: None,
            lambda_mult: None,
            fetch_k: None,
            extra: HashMap::new(),
        }
    }
}

impl SearchConfig {
    /// Create a new search configuration with specified k.
    #[must_use]
    pub fn new(k: usize) -> Self {
        SearchConfig {
            k,
            ..Default::default()
        }
    }

    /// Set the number of documents to retrieve.
    #[must_use]
    pub fn with_k(mut self, k: usize) -> Self {
        self.k = k;
        self
    }

    /// Set the score threshold for filtering results.
    #[must_use]
    pub fn with_score_threshold(mut self, threshold: f32) -> Self {
        self.score_threshold = Some(threshold);
        self
    }

    /// Set the lambda parameter for MMR search.
    #[must_use]
    pub fn with_lambda_mult(mut self, lambda: f32) -> Self {
        self.lambda_mult = Some(lambda);
        self
    }

    /// Set the `fetch_k` multiplier for MMR search.
    #[must_use]
    pub fn with_fetch_k(mut self, fetch_k: usize) -> Self {
        self.fetch_k = Some(fetch_k);
        self
    }

    /// Add an extra parameter to the search configuration.
    #[must_use]
    pub fn with_extra(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.extra.insert(key.into(), value);
        self
    }
}

/// Abstract trait for document retrieval systems.
///
/// A retriever takes a text query and returns the most relevant documents.
/// Retrievers extend the Runnable trait, so they can be composed in chains.
///
/// # Implementation
///
/// Implement the `_get_relevant_documents` method to define retrieval logic:
///
/// ```rust,ignore
/// #[async_trait]
/// impl Retriever for MyRetriever {
///     async fn _get_relevant_documents(
///         &self,
///         query: &str,
///         config: Option<&RunnableConfig>,
///     ) -> Result<Vec<Document>> {
///         // Your retrieval logic here
///         Ok(vec![])
///     }
/// }
/// ```
///
/// **IMPORTANT:** Application code should use `dashflow::retrieve()` instead of
/// calling `_get_relevant_documents` directly to get full graph infrastructure benefits.
#[async_trait]
pub trait Retriever: Send + Sync {
    /// Internal method - use `dashflow::retrieve()` instead.
    ///
    /// Application code should use the framework API which provides:
    /// - ExecutionTrace collection for optimizers
    /// - Streaming events for live progress
    /// - Introspection capabilities
    /// - Metrics collection (latency, document count)
    ///
    /// ```rust,ignore
    /// use dashflow::retrieve;
    /// let docs = retrieve(retriever, "query").await?;
    /// ```
    #[doc(hidden)]
    async fn _get_relevant_documents(
        &self,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>>;

    /// Get the name of this retriever for tracing/logging.
    fn name(&self) -> String {
        std::any::type_name::<Self>()
            .split("::")
            .last()
            .unwrap_or("Retriever")
            .to_string()
    }
}

/// Retriever wrapper around a `VectorStore`.
///
/// Provides a Retriever interface for any `VectorStore` implementation.
/// Supports multiple search types: similarity, MMR, and score threshold filtering.
///
/// This is a generic implementation that works with any concrete `VectorStore` type.
/// Due to Rust's trait object limitations, we cannot use `Arc<dyn VectorStore>` directly.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::retrievers::{VectorStoreRetriever, SearchType, SearchConfig};
///
/// let retriever = VectorStoreRetriever::new(
///     vector_store,
///     SearchType::Similarity,
///     SearchConfig::default().with_k(5),
/// );
///
/// let docs = retriever._get_relevant_documents("query", None).await?;
/// ```
pub struct VectorStoreRetriever<VS> {
    /// The underlying vector store.
    vectorstore: VS,

    /// Search type to use for retrieval.
    search_type: SearchType,

    /// Search configuration (k, thresholds, etc.).
    search_config: SearchConfig,

    /// Optional tags for tracing.
    tags: Option<Vec<String>>,

    /// Optional metadata for tracing.
    metadata: Option<HashMap<String, serde_json::Value>>,
}

impl<VS> VectorStoreRetriever<VS> {
    /// Create a new `VectorStoreRetriever`.
    ///
    /// # Arguments
    ///
    /// * `vectorstore` - The vector store to retrieve from
    /// * `search_type` - Type of search to perform
    /// * `search_config` - Configuration for search operations
    pub fn new(vectorstore: VS, search_type: SearchType, search_config: SearchConfig) -> Self {
        VectorStoreRetriever {
            vectorstore,
            search_type,
            search_config,
            tags: None,
            metadata: None,
        }
    }

    /// Create a retriever with default similarity search.
    pub fn from_vectorstore(vectorstore: VS) -> Self {
        Self::new(vectorstore, SearchType::Similarity, SearchConfig::default())
    }

    /// Create a retriever with MMR search.
    #[must_use]
    pub fn with_mmr(vectorstore: VS, k: usize, lambda_mult: f32, fetch_k: usize) -> Self {
        let config = SearchConfig::default()
            .with_k(k)
            .with_lambda_mult(lambda_mult)
            .with_fetch_k(fetch_k);
        Self::new(vectorstore, SearchType::MMR, config)
    }

    /// Create a retriever with similarity score threshold.
    #[must_use]
    pub fn with_score_threshold(vectorstore: VS, k: usize, score_threshold: f32) -> Self {
        let config = SearchConfig::default()
            .with_k(k)
            .with_score_threshold(score_threshold);
        Self::new(vectorstore, SearchType::SimilarityScoreThreshold, config)
    }

    /// Set tags for tracing.
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    /// Set metadata for tracing.
    #[must_use]
    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Validate the search configuration for the selected search type.
    fn validate_config(&self) -> Result<()> {
        match self.search_type {
            SearchType::SimilarityScoreThreshold => {
                let threshold = self.search_config.score_threshold.ok_or_else(|| {
                    Error::config(
                        "score_threshold must be specified for SimilarityScoreThreshold search",
                    )
                })?;
                if !(0.0..=1.0).contains(&threshold) {
                    return Err(Error::config(format!(
                        "score_threshold must be in range [0.0, 1.0], got {threshold}"
                    )));
                }
            }
            SearchType::MMR => {
                if let Some(lambda) = self.search_config.lambda_mult {
                    if !(0.0..=1.0).contains(&lambda) {
                        return Err(Error::config(format!(
                            "lambda_mult must be in range [0.0, 1.0], got {lambda}"
                        )));
                    }
                }
            }
            SearchType::Similarity => {
                // No special validation needed
            }
        }
        Ok(())
    }
}

// Import VectorStore trait for implementing Retriever
use crate::core::vector_stores::VectorStore;

#[async_trait]
impl<VS> Retriever for VectorStoreRetriever<VS>
where
    VS: VectorStore + Send + Sync,
{
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // Validate configuration
        self.validate_config()?;

        match self.search_type {
            SearchType::Similarity => {
                self.vectorstore
                    ._similarity_search(query, self.search_config.k, None)
                    .await
            }
            SearchType::SimilarityScoreThreshold => {
                // Safe: validate_config() already verified threshold exists
                let threshold = self
                    .search_config
                    .score_threshold
                    .expect("score_threshold verified by validate_config");
                let docs_with_scores = self
                    .vectorstore
                    .similarity_search_with_score(query, self.search_config.k, None)
                    .await?;

                // Filter by score threshold
                Ok(docs_with_scores
                    .into_iter()
                    .filter(|(_, score)| *score >= threshold)
                    .map(|(doc, _)| doc)
                    .collect())
            }
            SearchType::MMR => {
                let lambda = self.search_config.lambda_mult.unwrap_or(0.5);
                let fetch_k = self.search_config.fetch_k.unwrap_or(20);
                self.vectorstore
                    .max_marginal_relevance_search(
                        query,
                        self.search_config.k,
                        fetch_k,
                        lambda,
                        None,
                    )
                    .await
            }
        }
    }

    fn name(&self) -> String {
        "VectorStoreRetriever".to_string()
    }
}

#[async_trait]
impl<VS> Runnable for VectorStoreRetriever<VS>
where
    VS: VectorStore + Send + Sync,
{
    type Input = RetrieverInput;
    type Output = RetrieverOutput;

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self._get_relevant_documents(&input, config.as_ref()).await
    }

    fn name(&self) -> String {
        "VectorStoreRetriever".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{unique_by_key, EnsembleRetrieverError, SearchType};
    use crate::core::embeddings::Embeddings;
    use crate::core::vector_stores::{InMemoryVectorStore, VectorStore};
    use crate::test_prelude::*;
    use std::sync::Arc;

    #[test]
    fn test_search_config_default() {
        let config = SearchConfig::default();
        assert_eq!(config.k, 4);
        assert!(config.score_threshold.is_none());
        assert!(config.lambda_mult.is_none());
        assert!(config.fetch_k.is_none());
    }

    #[test]
    fn test_search_config_builder() {
        let config = SearchConfig::default()
            .with_k(10)
            .with_score_threshold(0.7)
            .with_lambda_mult(0.5)
            .with_fetch_k(20);

        assert_eq!(config.k, 10);
        assert_eq!(config.score_threshold, Some(0.7));
        assert_eq!(config.lambda_mult, Some(0.5));
        assert_eq!(config.fetch_k, Some(20));
    }

    #[test]
    fn test_search_type_default() {
        let search_type = SearchType::default();
        assert_eq!(search_type, SearchType::Similarity);
    }

    #[test]
    fn test_search_config_serialization() {
        let config = SearchConfig::default().with_k(5).with_score_threshold(0.8);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SearchConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.k, 5);
        assert_eq!(deserialized.score_threshold, Some(0.8));
    }

    // Mock embeddings for testing
    struct MockEmbeddings;

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .enumerate()
                .map(|(i, _)| vec![i as f32, 0.5, 0.1])
                .collect())
        }

        async fn _embed_query(&self, text: &str) -> Result<Vec<f32>> {
            Ok(vec![text.len() as f32, 0.5, 0.1])
        }
    }

    #[tokio::test]
    async fn test_vector_store_retriever_similarity() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        // Add some documents
        let texts = vec!["apple", "banana", "cherry", "date"];
        store.add_texts(&texts, None, None).await.unwrap();

        // Create retriever
        let retriever = VectorStoreRetriever::new(
            store,
            SearchType::Similarity,
            SearchConfig::default().with_k(2),
        );

        // Retrieve documents
        let docs = retriever
            ._get_relevant_documents("apple", None)
            .await
            .unwrap();

        // Should return k=2 documents
        assert_eq!(docs.len(), 2);
        // All documents should be valid
        assert!(!docs[0].page_content.is_empty());
        assert!(!docs[1].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_vector_store_retriever_as_runnable() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        let texts = vec!["apple", "banana", "cherry"];
        store.add_texts(&texts, None, None).await.unwrap();

        let retriever = VectorStoreRetriever::from_vectorstore(store);

        // Use as Runnable
        let docs = retriever.invoke("banana".to_string(), None).await.unwrap();

        // Default k=4, but we only have 3 docs, so should return 3
        assert!(docs.len() <= 4);
        assert!(!docs.is_empty());
    }

    #[tokio::test]
    async fn test_vector_store_retriever_mmr() {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        let texts = vec!["doc1", "doc2", "doc3", "doc4", "doc5"];
        store.add_texts(&texts, None, None).await.unwrap();

        let retriever = VectorStoreRetriever::with_mmr(
            store, 3,   // k=3
            0.5, // lambda
            10,  // fetch_k
        );

        let docs = retriever
            ._get_relevant_documents("query", None)
            .await
            .unwrap();

        assert_eq!(docs.len(), 3);
    }

    #[test]
    fn test_retriever_validation_score_threshold() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);

        // Missing score_threshold should fail validation
        let retriever = VectorStoreRetriever::new(
            store,
            SearchType::SimilarityScoreThreshold,
            SearchConfig::default().with_k(5),
        );

        let result = retriever.validate_config();
        assert!(result.is_err());
    }

    #[test]
    fn test_retriever_validation_score_threshold_range() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);

        // Score threshold out of range should fail
        let retriever = VectorStoreRetriever::new(
            store,
            SearchType::SimilarityScoreThreshold,
            SearchConfig::default().with_k(5).with_score_threshold(1.5),
        );

        let result = retriever.validate_config();
        assert!(result.is_err());
    }

    #[test]
    fn test_retriever_validation_lambda_range() {
        let embeddings = Arc::new(MockEmbeddings);
        let store = InMemoryVectorStore::new(embeddings);

        // Lambda out of range should fail
        let retriever = VectorStoreRetriever::new(
            store,
            SearchType::MMR,
            SearchConfig::default().with_k(5).with_lambda_mult(2.0),
        );

        let result = retriever.validate_config();
        assert!(result.is_err());
    }

    // ============================================================================
    // EnsembleRetriever Tests
    // ============================================================================

    /// Mock retriever that returns a fixed list of documents
    struct MockRetriever2 {
        docs: Vec<Document>,
    }

    #[async_trait]
    impl Retriever for MockRetriever2 {
        async fn _get_relevant_documents(
            &self,
            _query: &str,
            _config: Option<&RunnableConfig>,
        ) -> Result<Vec<Document>> {
            Ok(self.docs.clone())
        }
    }

    #[tokio::test]
    async fn test_ensemble_basic_merging() {
        // Test case from Python: documents with duplicate content are merged
        let documents1 = vec![
            Document::new("a").with_metadata("id", 1),
            Document::new("b").with_metadata("id", 2),
            Document::new("c").with_metadata("id", 3),
        ];
        let documents2 = vec![Document::new("b")];

        let retriever1 = Arc::new(MockRetriever2 { docs: documents1 }) as Arc<dyn Retriever>;
        let retriever2 = Arc::new(MockRetriever2 { docs: documents2 }) as Arc<dyn Retriever>;

        let ensemble = EnsembleRetriever::new(
            vec![retriever1, retriever2],
            vec![0.5, 0.5],
            60,
            None, // Use page_content for deduplication
        );

        let ranked_documents = ensemble._get_relevant_documents("_", None).await.unwrap();

        // The document with page_content "b" appears in both retrievers,
        // so it gets merged and ranked 1st due to higher RRF score
        assert_eq!(ranked_documents.len(), 3);
        assert_eq!(ranked_documents[0].page_content, "b");
    }

    #[tokio::test]
    async fn test_ensemble_no_duplicates() {
        // Test case from Python: all unique documents, rank order determined by position
        let documents1 = vec![
            Document::new("a").with_metadata("id", 1),
            Document::new("b").with_metadata("id", 2),
            Document::new("c").with_metadata("id", 3),
        ];
        let documents2 = vec![Document::new("d")];

        let retriever1 = Arc::new(MockRetriever2 { docs: documents1 }) as Arc<dyn Retriever>;
        let retriever2 = Arc::new(MockRetriever2 { docs: documents2 }) as Arc<dyn Retriever>;

        let ensemble =
            EnsembleRetriever::new(vec![retriever1, retriever2], vec![0.5, 0.5], 60, None);

        let ranked_documents = ensemble._get_relevant_documents("_", None).await.unwrap();

        // No duplicates, so we get 4 documents
        // "a" and "d" have the same RRF score (both rank 1 in their respective retrievers)
        // "a" appears first because retriever1 comes first
        assert_eq!(ranked_documents.len(), 4);
        assert_eq!(ranked_documents[0].page_content, "a");
    }

    #[tokio::test]
    async fn test_ensemble_with_id_key() {
        // Test case from Python: deduplication by metadata field
        let documents1 = vec![
            Document::new("a").with_metadata("id", 1),
            Document::new("b").with_metadata("id", 2),
            Document::new("c").with_metadata("id", 3),
        ];
        let documents2 = vec![Document::new("d").with_metadata("id", 2)];

        let retriever1 = Arc::new(MockRetriever2 { docs: documents1 }) as Arc<dyn Retriever>;
        let retriever2 = Arc::new(MockRetriever2 { docs: documents2 }) as Arc<dyn Retriever>;

        let ensemble = EnsembleRetriever::new(
            vec![retriever1, retriever2],
            vec![0.5, 0.5],
            60,
            Some("id".to_string()), // Use "id" metadata field for deduplication
        );

        let ranked_documents = ensemble._get_relevant_documents("_", None).await.unwrap();

        // Documents with id=2 are merged (even though content differs: "b" vs "d")
        // So we get 3 unique documents
        // "b" is ranked 1st because it has id=2 which appears in both retrievers
        assert_eq!(ranked_documents.len(), 3);
        assert_eq!(ranked_documents[0].page_content, "b");
    }

    #[tokio::test]
    async fn test_ensemble_equal_weights_constructor() {
        // Test the with_equal_weights convenience constructor
        let documents1 = vec![Document::new("a"), Document::new("b")];
        let documents2 = vec![Document::new("c"), Document::new("d")];
        let documents3 = vec![Document::new("e"), Document::new("f")];

        let retriever1 = Arc::new(MockRetriever2 { docs: documents1 }) as Arc<dyn Retriever>;
        let retriever2 = Arc::new(MockRetriever2 { docs: documents2 }) as Arc<dyn Retriever>;
        let retriever3 = Arc::new(MockRetriever2 { docs: documents3 }) as Arc<dyn Retriever>;

        let ensemble = EnsembleRetriever::with_equal_weights(
            vec![retriever1, retriever2, retriever3],
            60,
            None,
        );

        assert_eq!(ensemble.weights.len(), 3);
        // Each weight should be 1/3 ≈ 0.333...
        for weight in &ensemble.weights {
            assert!((weight - 1.0 / 3.0).abs() < 0.001);
        }

        let ranked_documents = ensemble._get_relevant_documents("_", None).await.unwrap();
        assert_eq!(ranked_documents.len(), 6); // All unique
    }

    #[tokio::test]
    async fn test_ensemble_weighted_preference() {
        // Test that weights affect ranking
        let documents1 = vec![Document::new("a"), Document::new("b")];
        let documents2 = vec![Document::new("c"), Document::new("d")];

        let retriever1 = Arc::new(MockRetriever2 { docs: documents1 }) as Arc<dyn Retriever>;
        let retriever2 = Arc::new(MockRetriever2 { docs: documents2 }) as Arc<dyn Retriever>;

        // Give retriever2 much higher weight
        let ensemble = EnsembleRetriever::new(
            vec![retriever1, retriever2],
            vec![0.1, 0.9], // 90% weight on retriever2
            60,
            None,
        );

        let ranked_documents = ensemble._get_relevant_documents("_", None).await.unwrap();

        // Documents from retriever2 should be ranked higher due to weight
        assert_eq!(ranked_documents.len(), 4);
        assert_eq!(ranked_documents[0].page_content, "c"); // retriever2's first doc
    }

    #[tokio::test]
    async fn test_ensemble_empty_retriever() {
        // Test with one empty retriever
        let documents1 = vec![Document::new("a"), Document::new("b")];
        let documents2 = vec![]; // Empty

        let retriever1 = Arc::new(MockRetriever2 { docs: documents1 }) as Arc<dyn Retriever>;
        let retriever2 = Arc::new(MockRetriever2 { docs: documents2 }) as Arc<dyn Retriever>;

        let ensemble =
            EnsembleRetriever::new(vec![retriever1, retriever2], vec![0.5, 0.5], 60, None);

        let ranked_documents = ensemble._get_relevant_documents("_", None).await.unwrap();

        // Should return documents from retriever1 only
        assert_eq!(ranked_documents.len(), 2);
        assert_eq!(ranked_documents[0].page_content, "a");
        assert_eq!(ranked_documents[1].page_content, "b");
    }

    #[test]
    fn test_ensemble_try_new_valid() {
        let documents = vec![Document::new("a")];
        let retriever = Arc::new(MockRetriever2 { docs: documents }) as Arc<dyn Retriever>;

        let result = EnsembleRetriever::try_new(vec![retriever], vec![1.0], 60, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ensemble_try_new_mismatched_weights() {
        // Should return error if retrievers and weights have different lengths
        let documents = vec![Document::new("a")];
        let retriever = Arc::new(MockRetriever2 { docs: documents }) as Arc<dyn Retriever>;

        let result = EnsembleRetriever::try_new(
            vec![retriever],
            vec![0.5, 0.5], // 2 weights but only 1 retriever
            60,
            None,
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            EnsembleRetrieverError::MismatchedLengths {
                retrievers: 1,
                weights: 2
            }
        ));
    }

    #[test]
    fn test_unique_by_key_helper() {
        // Test the unique_by_key helper function
        let docs = vec![
            Document::new("a"),
            Document::new("b"),
            Document::new("a"), // duplicate
            Document::new("c"),
            Document::new("b"), // duplicate
        ];

        let unique: Vec<Document> =
            unique_by_key(docs.into_iter(), |doc| doc.page_content.clone()).collect();

        assert_eq!(unique.len(), 3);
        assert_eq!(unique[0].page_content, "a");
        assert_eq!(unique[1].page_content, "b");
        assert_eq!(unique[2].page_content, "c");
    }
}

/// Standard conformance tests for VectorStoreRetriever
///
/// These tests verify that VectorStoreRetriever behaves consistently
/// and follows retriever best practices.
#[cfg(test)]
mod vectorstore_retriever_standard_tests {
    use super::SearchType;
    use crate::core::embeddings::Embeddings;
    use crate::core::vector_stores::{InMemoryVectorStore, VectorStore};
    use crate::test_prelude::*;
    use std::sync::Arc;

    // Reuse MockEmbeddings from parent tests module
    struct MockEmbeddings;

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .enumerate()
                .map(|(i, _)| vec![i as f32, 0.5, 0.1])
                .collect())
        }

        async fn _embed_query(&self, text: &str) -> Result<Vec<f32>> {
            Ok(vec![text.len() as f32, 0.5, 0.1])
        }
    }

    async fn create_test_retriever() -> VectorStoreRetriever<InMemoryVectorStore> {
        let embeddings = Arc::new(MockEmbeddings);
        let mut store = InMemoryVectorStore::new(embeddings);

        // Populate with test documents
        let texts = vec![
            "The quick brown fox jumps over the lazy dog",
            "A journey of a thousand miles begins with a single step",
            "To be or not to be, that is the question",
            "Machine learning is a subset of artificial intelligence",
            "Deep learning uses neural networks",
            "Rust is a systems programming language",
        ];
        store.add_texts(&texts, None, None).await.unwrap();

        VectorStoreRetriever::new(
            store,
            SearchType::Similarity,
            SearchConfig::default().with_k(3),
        )
    }

    /// Test 1: Basic retrieval with content verification
    /// Criteria met: 1 (Real functionality), 3 (Edge case - k limit), 4 (State verification), 7 (Comparison)
    /// Score: 4/7
    #[tokio::test]
    async fn test_basic_retrieval_standard() {
        let retriever = create_test_retriever().await;
        let result = retriever
            ._get_relevant_documents("machine learning neural", None)
            .await;

        assert!(result.is_ok(), "Basic retrieval should succeed");
        let documents = result.unwrap();

        // Real functionality: Actual retrieval from vector store
        assert!(!documents.is_empty(), "Should return at least one document");
        assert!(
            !documents[0].page_content.is_empty(),
            "Document should have content"
        );

        // Edge case: Verify k limit is respected (configured to k=3)
        assert!(
            documents.len() <= 3,
            "Should respect k=3 limit, got {}",
            documents.len()
        );

        // State verification: All documents should be from our populated set
        let expected_texts = [
            "The quick brown fox jumps over the lazy dog",
            "A journey of a thousand miles begins with a single step",
            "To be or not to be, that is the question",
            "Machine learning is a subset of artificial intelligence",
            "Deep learning uses neural networks",
            "Rust is a systems programming language",
        ];
        for doc in &documents {
            assert!(
                expected_texts.contains(&doc.page_content.as_str()),
                "Document content should be from populated set: {}",
                doc.page_content
            );
        }

        // Comparison: Query about "machine learning neural" should return relevant docs
        let contents: Vec<&str> = documents.iter().map(|d| d.page_content.as_str()).collect();
        assert!(
            contents.contains(&"Machine learning is a subset of artificial intelligence")
                || contents.contains(&"Deep learning uses neural networks"),
            "Should return machine learning or neural network related docs"
        );
    }

    /// Test 2: Query consistency (determinism)
    /// Criteria met: 1 (Real functionality), 3 (Edge case - repeated queries), 4 (State verification), 7 (Comparison)
    /// Score: 4/7
    #[tokio::test]
    async fn test_query_consistency_standard() {
        let retriever = create_test_retriever().await;
        let query = "consistent query test";

        // Real functionality: Multiple queries to same retriever
        let results1 = retriever._get_relevant_documents(query, None).await.unwrap();
        let results2 = retriever._get_relevant_documents(query, None).await.unwrap();
        let results3 = retriever._get_relevant_documents(query, None).await.unwrap();

        // Edge case: Test determinism across multiple runs
        assert_eq!(
            results1.len(),
            results2.len(),
            "Same query should return same number of documents (run 1 vs 2)"
        );
        assert_eq!(
            results1.len(),
            results3.len(),
            "Same query should return same number of documents (run 1 vs 3)"
        );

        // State verification: Order and content must be identical (retrievers should be deterministic)
        for (doc1, doc2) in results1.iter().zip(results2.iter()) {
            assert_eq!(
                doc1.page_content, doc2.page_content,
                "Document content should be identical"
            );
            assert_eq!(
                doc1.metadata, doc2.metadata,
                "Document metadata should be identical"
            );
        }

        // Comparison: Verify with third run
        for (doc1, doc3) in results1.iter().zip(results3.iter()) {
            assert_eq!(
                doc1.page_content, doc3.page_content,
                "Document content should be identical (run 3)"
            );
        }
    }

    /// Test 3: Different query types with result validation
    /// Criteria met: 1 (Real functionality), 3 (Edge case - varied inputs), 4 (State verification), 7 (Comparison)
    /// Score: 4/7
    #[tokio::test]
    async fn test_different_query_types_standard() {
        let retriever = create_test_retriever().await;

        // Edge case: Various query formats
        let test_cases = vec![
            ("What is artificial intelligence?", "question format"),
            ("Machine learning algorithms", "keyword format"),
            ("The cat sits on the mat", "statement format"),
            ("AI", "single word"),
            ("programming language systems software", "multiple keywords"),
        ];

        // Real functionality: Each query type should work
        for (query, query_type) in test_cases {
            let result = retriever._get_relevant_documents(query, None).await;
            assert!(result.is_ok(), "Should handle {}: {}", query_type, query);

            let documents = result.unwrap();

            // State verification: All results should be non-empty and valid
            assert!(
                !documents.is_empty(),
                "Query '{}' ({}) should return documents",
                query,
                query_type
            );
            for doc in &documents {
                assert!(
                    !doc.page_content.is_empty(),
                    "Document should have content for query: {}",
                    query
                );
            }

            // Comparison: Results should be within configured k limit
            assert!(
                documents.len() <= 3,
                "Should respect k=3 for query '{}': got {}",
                query,
                documents.len()
            );
        }
    }

    /// Test 4: Document structure validation
    /// Criteria met: 1 (Real functionality), 3 (Edge case - structure), 4 (State verification), 7 (Comparison)
    /// Score: 4/7
    #[tokio::test]
    async fn test_document_structure_standard() {
        let retriever = create_test_retriever().await;
        let documents = retriever
            ._get_relevant_documents("test query structure", None)
            .await
            .unwrap();

        // Real functionality: Verify all document fields are properly populated
        assert!(!documents.is_empty(), "Should return documents");

        for (i, doc) in documents.iter().enumerate() {
            // State verification: Check all document fields are valid
            assert!(
                !doc.page_content.is_empty(),
                "Document {} should have non-empty content",
                i
            );

            // Edge case: Verify ID field handling
            if let Some(id) = &doc.id {
                assert!(
                    !id.is_empty(),
                    "Document {} ID should not be empty string",
                    i
                );
            }

            // Comparison: Metadata should be valid HashMap (can be empty but must be valid)
            for (key, value) in &doc.metadata {
                assert!(!key.is_empty(), "Metadata keys should not be empty");
                assert!(
                    value.is_object()
                        || value.is_string()
                        || value.is_number()
                        || value.is_boolean()
                        || value.is_array()
                        || value.is_null(),
                    "Metadata values should be valid JSON"
                );
            }
        }

        // State verification: Documents should be from our populated set
        let expected_texts = [
            "The quick brown fox jumps over the lazy dog",
            "A journey of a thousand miles begins with a single step",
            "To be or not to be, that is the question",
            "Machine learning is a subset of artificial intelligence",
            "Deep learning uses neural networks",
            "Rust is a systems programming language",
        ];
        for doc in &documents {
            assert!(
                expected_texts.contains(&doc.page_content.as_str()),
                "Document should be from populated set: {}",
                doc.page_content
            );
        }
    }

    /// Test 5: Retriever name and type information
    /// Criteria met: 1 (Real functionality), 4 (State verification), 7 (Comparison)
    /// Score: 3/7 (simple test, but necessary for interface compliance)
    #[tokio::test]
    async fn test_retriever_name_standard() {
        let retriever = create_test_retriever().await;

        // Real functionality: Retriever should implement name() method
        let name = Retriever::name(&retriever);

        // Comparison: Name should be non-empty and descriptive
        assert!(!name.is_empty(), "Retriever name should not be empty");
        assert!(
            name.len() > 3,
            "Retriever name should be descriptive: {}",
            name
        );

        // State verification: Name should identify retriever type
        assert!(
            name.contains("VectorStore") || name.contains("Retriever"),
            "Name should indicate retriever type: {}",
            name
        );
    }

    /// Test 6: Whitespace and special characters in query
    /// Criteria met: 1 (Real functionality), 3 (Edge case - whitespace), 4 (State verification), 7 (Comparison)
    /// Score: 4/7
    #[tokio::test]
    async fn test_whitespace_in_query_standard() {
        let retriever = create_test_retriever().await;

        // Edge case: Various whitespace scenarios
        let test_cases = vec![
            ("  leading spaces", "leading whitespace"),
            ("trailing spaces  ", "trailing whitespace"),
            ("  both  ", "both sides"),
            ("multiple   spaces   between", "multiple internal spaces"),
            ("\ttabs\tand\tnewlines\n", "tabs and newlines"),
        ];

        // Real functionality: All should be handled gracefully
        for (query, description) in test_cases {
            let result = retriever._get_relevant_documents(query, None).await;
            assert!(result.is_ok(), "Should handle {}: {:?}", description, query);

            let documents = result.unwrap();

            // State verification: Should return valid documents despite whitespace
            assert!(
                !documents.is_empty(),
                "Should return documents for {}",
                description
            );
            for doc in &documents {
                assert!(!doc.page_content.is_empty(), "Document should have content");
            }

            // Comparison: Verify k limit
            assert!(documents.len() <= 3, "Should respect k limit");
        }
    }

    /// Test 7: Empty query handling (error case)
    /// Criteria met: 1 (Real functionality), 2 (Error testing), 3 (Edge case), 7 (Comparison)
    /// Score: 4/7
    #[tokio::test]
    async fn test_empty_query_standard() {
        let retriever = create_test_retriever().await;

        // Error case: Empty string query
        let result = retriever._get_relevant_documents("", None).await;

        // Implementation should handle this gracefully (either succeed or error, but no panic)
        match result {
            Ok(docs) => {
                // If accepting empty queries, should still return valid results
                for doc in docs {
                    assert!(!doc.page_content.is_empty(), "Documents should be valid");
                }
            }
            Err(_) => {
                // Rejecting empty queries is also acceptable behavior
                // Test passes - error was returned gracefully
            }
        }

        // Edge case: Whitespace-only query
        let whitespace_result = retriever._get_relevant_documents("   ", None).await;
        match whitespace_result {
            Ok(docs) => {
                for doc in docs {
                    assert!(!doc.page_content.is_empty());
                }
            }
            Err(_) => {
                // Also acceptable
            }
        }
    }

    /// Test 8: Unicode and special characters
    /// Criteria met: 1 (Real functionality), 3 (Edge case - unicode), 4 (State verification), 7 (Comparison)
    /// Score: 4/7
    #[tokio::test]
    async fn test_unicode_and_special_chars_standard() {
        let retriever = create_test_retriever().await;

        // Edge case: Various special character scenarios
        let queries = vec![
            "你好世界",                        // Chinese
            "Привет мир",                      // Russian
            "مرحبا بالعالم",                   // Arabic
            "Hello 🌍 World 🚀",               // Emojis
            "test@example.com",                // Email
            "fn main() { println!(\"hi\"); }", // Code
            "C++, Rust, Go",                   // Punctuation
            "$100 €50 £30",                    // Currency symbols
        ];

        // Real functionality: All character types should be handled
        for query in queries {
            let result = retriever._get_relevant_documents(query, None).await;
            assert!(result.is_ok(), "Should handle special chars in: {}", query);

            let documents = result.unwrap();

            // State verification: Valid results returned
            assert!(
                !documents.is_empty(),
                "Should return documents for query: {}",
                query
            );
            assert!(documents.len() <= 3, "Should respect k limit");
        }
    }

    /// Test 9: Very long query handling (scalability)
    /// Criteria met: 1 (Real functionality), 3 (Edge case - long input), 6 (Performance), 7 (Comparison)
    /// Score: 4/7
    #[tokio::test]
    async fn test_long_query_standard() {
        let retriever = create_test_retriever().await;

        // Edge case: Very long query (500+ words)
        let long_query =
            "artificial intelligence machine learning deep learning neural networks ".repeat(100);
        assert!(long_query.len() > 5000, "Query should be very long");

        // Performance: Should complete without timeout
        let start = std::time::Instant::now();
        let result = retriever._get_relevant_documents(&long_query, None).await;
        let duration = start.elapsed();

        // Real functionality: Should handle long queries
        assert!(result.is_ok(), "Should handle very long query");
        let documents = result.unwrap();
        assert!(
            !documents.is_empty(),
            "Should return results for long query"
        );

        // Performance: Should complete in reasonable time (< 5 seconds)
        assert!(
            duration.as_secs() < 5,
            "Long query should complete quickly: {:?}",
            duration
        );

        // Comparison: Verify result structure
        assert!(documents.len() <= 3, "Should still respect k limit");
        for doc in documents {
            assert!(!doc.page_content.is_empty());
        }
    }

    /// Test 10: Concurrent retrievals (thread safety)
    /// Criteria met: 1 (Real functionality), 3 (Edge case - concurrency), 5 (Integration), 6 (Performance)
    /// Score: 4/7
    #[tokio::test]
    async fn test_concurrent_retrievals_standard() {
        use futures::future::join_all;

        let retriever = create_test_retriever().await;

        // Edge case: Multiple concurrent queries
        let queries = [
            "concurrent query 1 machine",
            "concurrent query 2 learning",
            "concurrent query 3 rust",
            "concurrent query 4 neural",
            "concurrent query 5 programming",
        ];

        // Performance: Execute concurrently
        let start = std::time::Instant::now();
        let tasks: Vec<_> = queries
            .iter()
            .map(|query| retriever._get_relevant_documents(query, None))
            .collect();

        // Integration: All tasks should complete successfully
        let results = join_all(tasks).await;
        let duration = start.elapsed();

        // Real functionality: All concurrent requests should succeed
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok(), "Concurrent retrieval {} should succeed", i);
            let docs = result.as_ref().unwrap();
            assert!(
                !docs.is_empty(),
                "Concurrent query {} should return documents",
                i
            );
            assert!(docs.len() <= 3, "Should respect k limit");
        }

        // Performance: Concurrent execution should be faster than sequential
        // (or at least complete in reasonable time)
        assert!(
            duration.as_secs() < 10,
            "Concurrent queries should complete quickly: {:?}",
            duration
        );
    }

    /// Test 11: Numeric queries
    /// Criteria met: 1 (Real functionality), 3 (Edge case - numeric), 4 (State verification), 7 (Comparison)
    /// Score: 4/7
    #[tokio::test]
    async fn test_numeric_queries_standard() {
        let retriever = create_test_retriever().await;

        // Edge case: Various numeric formats
        let queries = vec![
            "42",
            "3.14159",
            "2024-01-01",
            "v1.2.3",
            "0xDEADBEEF",
            "1000000",
        ];

        // Real functionality: All numeric queries should work
        for query in queries {
            let result = retriever._get_relevant_documents(query, None).await;
            assert!(result.is_ok(), "Should handle numeric query: {}", query);

            let documents = result.unwrap();

            // State verification: Valid results
            assert!(
                !documents.is_empty(),
                "Numeric query '{}' should return documents",
                query
            );
            for doc in &documents {
                assert!(!doc.page_content.is_empty());
            }

            // Comparison: Verify k limit
            assert!(documents.len() <= 3, "Should respect k limit");
        }
    }

    /// Test 12: Large batch performance (scalability)
    /// Criteria met: 1 (Real functionality), 3 (Edge case - scale), 6 (Performance), 7 (Comparison)
    /// Score: 4/7
    #[tokio::test]
    async fn test_large_batch_performance_standard() {
        let retriever = create_test_retriever().await;

        // Performance: Test with many queries
        let num_queries = 50;
        let queries: Vec<String> = (0..num_queries)
            .map(|i| format!("test query number {}", i))
            .collect();

        // Edge case: Sequential processing of many queries
        let start = std::time::Instant::now();
        let mut results = Vec::new();
        for query in &queries {
            let result = retriever._get_relevant_documents(query, None).await;
            assert!(result.is_ok(), "Batch query should succeed");
            results.push(result.unwrap());
        }
        let duration = start.elapsed();

        // Real functionality: All queries should complete
        assert_eq!(results.len(), num_queries, "All queries should complete");

        // Comparison: All results should be valid
        for (i, docs) in results.iter().enumerate() {
            assert!(!docs.is_empty(), "Query {} should return documents", i);
            assert!(docs.len() <= 3, "Should respect k limit");
        }

        // Performance: Should complete in reasonable time (< 10 seconds for 50 queries)
        assert!(
            duration.as_secs() < 10,
            "Batch processing should complete quickly: {:?}",
            duration
        );
    }
}

/// Helper function to deduplicate documents by comparing all fields.
///
/// Returns unique documents in the order they first appear.
fn unique_documents(documents: Vec<Document>) -> Vec<Document> {
    let mut seen = Vec::new();
    let mut unique = Vec::new();

    for doc in documents {
        if !seen.contains(&doc) {
            seen.push(doc.clone());
            unique.push(doc);
        }
    }

    unique
}

/// Multi-query retriever that generates multiple search queries from a single user query.
///
/// Uses an LLM to generate multiple variations of the user's question, retrieves documents
/// for each variation, and returns the unique union of all retrieved documents. This helps
/// overcome limitations of distance-based similarity search by exploring multiple perspectives
/// on the same question.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::retrievers::MultiQueryRetriever;
/// use dashflow::core::prompts::PromptTemplate;
/// use dashflow::core::output_parsers::LineListOutputParser;
///
/// // Create a prompt that generates alternative questions
/// let prompt = PromptTemplate::from_template(
///     "Generate 3 different versions of this question: {question}"
/// ).unwrap();
///
/// // Create an LLM chain: prompt | llm | parser
/// let llm_chain = prompt.pipe(llm).pipe(LineListOutputParser);
///
/// // Create multi-query retriever
/// let retriever = MultiQueryRetriever::new(
///     base_retriever,
///     llm_chain,
///     true,  // include_original
/// );
///
/// // Retrieve documents - will generate multiple queries and merge results
/// let docs = retriever.invoke("What is DashFlow?", None).await?;
/// ```
pub struct MultiQueryRetriever<R, C>
where
    R: Retriever,
    C: Runnable<Input = HashMap<String, String>, Output = Vec<String>>,
{
    /// Base retriever to query for each generated question
    pub retriever: R,

    /// LLM chain that generates alternative queries from the input question.
    /// Should be a chain like: `PromptTemplate` | `ChatModel` | `LineListOutputParser`
    pub llm_chain: C,

    /// Whether to include the original query in addition to generated queries
    pub include_original: bool,

    /// Whether to log generated queries (for debugging)
    pub verbose: bool,
}

impl<R, C> MultiQueryRetriever<R, C>
where
    R: Retriever,
    C: Runnable<Input = HashMap<String, String>, Output = Vec<String>>,
{
    /// Create a new multi-query retriever.
    ///
    /// # Arguments
    ///
    /// * `retriever` - Base retriever to use for document retrieval
    /// * `llm_chain` - Chain that generates alternative queries (`PromptTemplate` | LLM | Parser)
    /// * `include_original` - Whether to include the original query in the search
    pub fn new(retriever: R, llm_chain: C, include_original: bool) -> Self {
        Self {
            retriever,
            llm_chain,
            include_original,
            verbose: true,
        }
    }

    /// Set verbosity for logging generated queries.
    #[must_use]
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Generate alternative queries from the user's question.
    async fn generate_queries(
        &self,
        question: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<String>> {
        let mut vars = HashMap::new();
        vars.insert("question".to_string(), question.to_string());

        let queries = self.llm_chain.invoke(vars, config.cloned()).await?;

        if self.verbose {
            tracing::debug!("Generated queries: {queries:?}");
        }

        Ok(queries)
    }

    /// Retrieve documents for all queries.
    async fn retrieve_documents(
        &self,
        queries: Vec<String>,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        let mut all_docs = Vec::new();

        // Retrieve documents for each query
        for query in queries {
            let docs = self
                .retriever
                ._get_relevant_documents(&query, config)
                .await?;
            all_docs.extend(docs);
        }

        Ok(all_docs)
    }
}

#[async_trait]
impl<R, C> Retriever for MultiQueryRetriever<R, C>
where
    R: Retriever + Send + Sync,
    C: Runnable<Input = HashMap<String, String>, Output = Vec<String>> + Send + Sync,
{
    async fn _get_relevant_documents(
        &self,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // Generate alternative queries
        let mut queries = self.generate_queries(query, config).await?;

        // Optionally include the original query
        if self.include_original {
            queries.push(query.to_string());
        }

        // Retrieve documents for all queries
        let documents = self.retrieve_documents(queries, config).await?;

        // Return unique union of documents
        Ok(unique_documents(documents))
    }

    fn name(&self) -> String {
        "MultiQueryRetriever".to_string()
    }
}

#[async_trait]
impl<R, C> Runnable for MultiQueryRetriever<R, C>
where
    R: Retriever + Send + Sync,
    C: Runnable<Input = HashMap<String, String>, Output = Vec<String>> + Send + Sync,
{
    type Input = String;
    type Output = Vec<Document>;

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self._get_relevant_documents(&input, config.as_ref()).await
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        let mut results = Vec::new();
        for input in inputs {
            results.push(self.invoke(input, config.clone()).await?);
        }
        Ok(results)
    }

    async fn stream(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.invoke(input, config).await?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

/// Contextual compression retriever that post-processes retrieved documents.
///
/// Wraps a base retriever and applies document compression to the results. This is useful
/// for filtering, extracting relevant passages, re-ranking, or any other post-processing
/// that improves result quality.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::retrievers::ContextualCompressionRetriever;
/// use dashflow::core::documents::DocumentCompressor;
///
/// // Create a compressor (e.g., LLM-based filter)
/// let compressor = MyDocumentCompressor::new();
///
/// // Wrap base retriever with compression
/// let retriever = ContextualCompressionRetriever::new(
///     base_retriever,
///     Arc::new(compressor),
/// );
///
/// // Retrieve and compress documents
/// let docs = retriever.invoke("What is DashFlow?", None).await?;
/// // Documents are automatically compressed/filtered
/// ```
pub struct ContextualCompressionRetriever<R>
where
    R: Retriever,
{
    /// Base retriever to get initial documents
    pub base_retriever: R,

    /// Document compressor to post-process results
    pub base_compressor: Arc<dyn DocumentCompressor>,
}

impl<R> ContextualCompressionRetriever<R>
where
    R: Retriever,
{
    /// Create a new contextual compression retriever.
    ///
    /// # Arguments
    ///
    /// * `base_retriever` - Retriever to use for initial document retrieval
    /// * `base_compressor` - Compressor to post-process the retrieved documents
    pub fn new(base_retriever: R, base_compressor: Arc<dyn DocumentCompressor>) -> Self {
        Self {
            base_retriever,
            base_compressor,
        }
    }
}

#[async_trait]
impl<R> Retriever for ContextualCompressionRetriever<R>
where
    R: Retriever + Send + Sync,
{
    async fn _get_relevant_documents(
        &self,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // Retrieve documents from base retriever
        let docs = self
            .base_retriever
            ._get_relevant_documents(query, config)
            .await?;

        // Compress documents if any were retrieved
        if docs.is_empty() {
            Ok(vec![])
        } else {
            let compressed = self
                .base_compressor
                .compress_documents(docs, query, config)
                .await?;
            Ok(compressed)
        }
    }

    fn name(&self) -> String {
        "ContextualCompressionRetriever".to_string()
    }
}

#[async_trait]
impl<R> Runnable for ContextualCompressionRetriever<R>
where
    R: Retriever + Send + Sync,
{
    type Input = String;
    type Output = Vec<Document>;

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output> {
        self._get_relevant_documents(&input, config.as_ref()).await
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Option<RunnableConfig>,
    ) -> Result<Vec<Self::Output>> {
        let mut results = Vec::new();
        for input in inputs {
            results.push(self.invoke(input, config.clone()).await?);
        }
        Ok(results)
    }

    async fn stream(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>> {
        let result = self.invoke(input, config).await?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

// ================================================================================================
// EnsembleRetriever - Combines multiple retrievers using weighted Reciprocal Rank Fusion
// ================================================================================================

/// Helper function to yield unique elements based on a key function.
///
/// Filters out duplicates by tracking seen keys. Useful for deduplicating documents
/// by content or metadata fields.
///
/// # Arguments
/// * `iterable` - Iterator of items to filter
/// * `key_fn` - Function that extracts a hashable key from each item
///
/// # Returns
/// Iterator yielding unique items based on the key function
fn unique_by_key<T, K, F>(
    iterable: impl Iterator<Item = T>,
    mut key_fn: F,
) -> impl Iterator<Item = T>
where
    K: std::hash::Hash + Eq,
    F: FnMut(&T) -> K,
{
    let mut seen = std::collections::HashSet::new();
    iterable.filter(move |item| seen.insert(key_fn(item)))
}

/// Error type for EnsembleRetriever configuration validation.
#[derive(Debug, Clone, PartialEq, ThisError)]
#[non_exhaustive]
pub enum EnsembleRetrieverError {
    /// Number of retrievers must equal number of weights.
    #[error("Number of retrievers ({retrievers}) must equal number of weights ({weights})")]
    MismatchedLengths {
        /// The number of retrievers provided.
        retrievers: usize,
        /// The number of weights provided.
        weights: usize,
    },
}

/// Retriever that ensembles multiple retrievers using weighted Reciprocal Rank Fusion (RRF).
///
/// `EnsembleRetriever` combines results from multiple retrievers (e.g., dense vector search,
/// sparse keyword search, different embedding models) into a single ranked list. It uses
/// Reciprocal Rank Fusion with configurable weights to balance the contributions of each retriever.
///
/// # Reciprocal Rank Fusion (RRF)
///
/// RRF is a rank aggregation method that doesn't require score normalization. For each document,
/// it computes a score based on its rank in each retriever's results:
///
/// ```text
/// RRF_score = Σ(weight_i / (rank_i + c))
/// ```
///
/// Where:
/// - `weight_i`: Weight for retriever i
/// - `rank_i`: Document's rank in retriever i's results (1-indexed)
/// - `c`: Constant (default 60) controlling balance between high and low ranks
///
/// Documents appearing in multiple retrievers get higher scores. Final results are
/// deduplicated and sorted by RRF score.
///
/// # Use Cases
///
/// - **Hybrid Search**: Combine dense (vector) and sparse (keyword) retrievers
/// - **Multi-Model**: Combine results from different embedding models
/// - **Ensemble**: Combine multiple retrieval strategies for robustness
///
/// # Example
///
/// ```rust
/// use dashflow::core::retrievers::{EnsembleRetriever, Retriever};
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # use dashflow::core::retrievers::VectorStoreRetriever;
/// # let vector_retriever: Arc<dyn Retriever> = todo!();
/// # let keyword_retriever: Arc<dyn Retriever> = todo!();
///
/// // Combine dense vector search with sparse keyword search
/// let ensemble = EnsembleRetriever::new(
///     vec![vector_retriever, keyword_retriever],
///     vec![0.7, 0.3], // 70% weight on vector, 30% on keyword
///     60,             // RRF constant
///     None,           // Use page_content for deduplication
/// );
///
/// let docs = ensemble._get_relevant_documents("What is Rust?", None).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Deduplication
///
/// Documents are deduplicated based on:
/// - `page_content` (default): Exact content match
/// - `id_key`: Metadata field specified (e.g., "`doc_id`", "url")
///
/// This prevents the same document from appearing multiple times if retrieved
/// by different retrievers.
pub struct EnsembleRetriever {
    /// List of retrievers to ensemble
    retrievers: Vec<Arc<dyn Retriever>>,

    /// Weights for each retriever (must sum to ~1.0 for normalized results)
    weights: Vec<f64>,

    /// Constant added to rank (default 60, from RRF paper)
    /// Controls balance between high-ranked and low-ranked items
    c: usize,

    /// Optional metadata key for document deduplication
    /// If None, uses `page_content` for deduplication
    id_key: Option<String>,
}

impl std::fmt::Debug for EnsembleRetriever {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnsembleRetriever")
            .field(
                "retrievers",
                &format!("[{} retrievers]", self.retrievers.len()),
            )
            .field("weights", &self.weights)
            .field("c", &self.c)
            .field("id_key", &self.id_key)
            .finish()
    }
}

impl EnsembleRetriever {
    /// Create a new `EnsembleRetriever`.
    ///
    /// # Arguments
    /// * `retrievers` - List of retrievers to combine
    /// * `weights` - Weights for each retriever (should sum to ~1.0)
    /// * `c` - RRF constant (default 60 from paper)
    /// * `id_key` - Optional metadata field for deduplication
    ///
    /// # Panics
    /// Panics if retrievers and weights have different lengths
    #[must_use]
    pub fn new(
        retrievers: Vec<Arc<dyn Retriever>>,
        weights: Vec<f64>,
        c: usize,
        id_key: Option<String>,
    ) -> Self {
        Self::try_new(retrievers, weights, c, id_key)
            .expect("Number of retrievers must equal number of weights")
    }

    /// Create a new `EnsembleRetriever`, returning an error if configuration is invalid.
    ///
    /// # Arguments
    /// * `retrievers` - List of retrievers to combine
    /// * `weights` - Weights for each retriever (should sum to ~1.0)
    /// * `c` - RRF constant (default 60 from paper)
    /// * `id_key` - Optional metadata field for deduplication
    ///
    /// # Errors
    /// Returns `EnsembleRetrieverError::MismatchedLengths` if retrievers and weights have different lengths.
    pub fn try_new(
        retrievers: Vec<Arc<dyn Retriever>>,
        weights: Vec<f64>,
        c: usize,
        id_key: Option<String>,
    ) -> std::result::Result<Self, EnsembleRetrieverError> {
        if retrievers.len() != weights.len() {
            return Err(EnsembleRetrieverError::MismatchedLengths {
                retrievers: retrievers.len(),
                weights: weights.len(),
            });
        }
        Ok(Self {
            retrievers,
            weights,
            c,
            id_key,
        })
    }

    /// Create an `EnsembleRetriever` with equal weights.
    ///
    /// Convenience constructor that assigns equal weight (1/n) to each retriever.
    ///
    /// # Arguments
    /// * `retrievers` - List of retrievers to combine
    /// * `c` - RRF constant (default 60)
    /// * `id_key` - Optional metadata field for deduplication
    #[must_use]
    pub fn with_equal_weights(
        retrievers: Vec<Arc<dyn Retriever>>,
        c: usize,
        id_key: Option<String>,
    ) -> Self {
        let n = retrievers.len();
        let equal_weight = 1.0 / n as f64;
        let weights = vec![equal_weight; n];
        Self::new(retrievers, weights, c, id_key)
    }

    /// Perform weighted Reciprocal Rank Fusion on multiple rank lists.
    ///
    /// Implements the RRF algorithm from:
    /// <https://plg.uwaterloo.ca/~gvcormac/cormacksigir09-rrf.pdf>
    ///
    /// # Arguments
    /// * `doc_lists` - Results from each retriever (one list per retriever)
    ///
    /// # Returns
    /// Deduplicated and sorted list of documents by RRF score
    ///
    /// # Algorithm
    /// 1. For each document in each list, compute: weight / (rank + c)
    /// 2. Sum scores for documents appearing in multiple lists
    /// 3. Deduplicate by `page_content` or `id_key`
    /// 4. Sort by total RRF score (descending)
    fn weighted_reciprocal_rank(&self, doc_lists: Vec<Vec<Document>>) -> Vec<Document> {
        // Build RRF scores for each unique document
        let mut rrf_scores: HashMap<String, f64> = HashMap::new();

        for (doc_list, weight) in doc_lists.iter().zip(&self.weights) {
            for (rank, doc) in doc_list.iter().enumerate() {
                // rank is 0-indexed, but RRF uses 1-indexed ranks
                let rank_1indexed = rank + 1;

                // Get document key for deduplication
                let doc_key = if let Some(ref id_key) = self.id_key {
                    // Use metadata field if specified
                    doc.metadata
                        .get(id_key)
                        .map(|v| {
                            // Convert JSON value to string for comparison
                            match v {
                                serde_json::Value::String(s) => s.clone(),
                                _ => v.to_string(),
                            }
                        })
                        .unwrap_or_else(|| doc.page_content.clone())
                } else {
                    // Default to page_content
                    doc.page_content.clone()
                };

                // Compute RRF score contribution from this retriever
                let score = weight / (rank_1indexed + self.c) as f64;
                *rrf_scores.entry(doc_key).or_insert(0.0) += score;
            }
        }

        // Flatten all documents and deduplicate
        let all_docs: Vec<Document> = doc_lists.into_iter().flatten().collect();

        let unique_docs: Vec<Document> = unique_by_key(all_docs.into_iter(), |doc| {
            if let Some(ref id_key) = self.id_key {
                doc.metadata
                    .get(id_key)
                    .map(|v| match v {
                        serde_json::Value::String(s) => s.clone(),
                        _ => v.to_string(),
                    })
                    .unwrap_or_else(|| doc.page_content.clone())
            } else {
                doc.page_content.clone()
            }
        })
        .collect();

        // Sort by RRF score (descending)
        let mut scored_docs: Vec<(Document, f64)> = unique_docs
            .into_iter()
            .map(|doc| {
                let doc_key = if let Some(ref id_key) = self.id_key {
                    doc.metadata
                        .get(id_key)
                        .map(|v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            _ => v.to_string(),
                        })
                        .unwrap_or_else(|| doc.page_content.clone())
                } else {
                    doc.page_content.clone()
                };
                let score = *rrf_scores.get(&doc_key).unwrap_or(&0.0);
                (doc, score)
            })
            .collect();

        scored_docs.sort_by(|(_, score_a), (_, score_b)| {
            score_b
                .partial_cmp(score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored_docs.into_iter().map(|(doc, _)| doc).collect()
    }
}

#[async_trait]
impl Retriever for EnsembleRetriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // Retrieve from all retrievers in parallel
        let mut handles = Vec::new();

        // Clone config once for all tasks (if present)
        let config_owned = config.cloned();

        for retriever in &self.retrievers {
            let retriever = Arc::clone(retriever);
            let query = query.to_string();
            let task_config = config_owned.clone();
            let handle = tokio::spawn(async move {
                retriever
                    ._get_relevant_documents(&query, task_config.as_ref())
                    .await
            });
            handles.push(handle);
        }

        // Collect results
        let mut doc_lists = Vec::new();
        for handle in handles {
            let result = handle
                .await
                .map_err(|e| Error::Other(format!("Failed to join retriever task: {e}")))?;
            doc_lists.push(result?);
        }

        // Apply weighted reciprocal rank fusion
        Ok(self.weighted_reciprocal_rank(doc_lists))
    }
}

// ================================================================================================
// MultiQueryRetriever Tests
// ================================================================================================

#[cfg(test)]
mod multi_query_retriever_tests {
    use super::unique_documents;
    use crate::test_prelude::*;

    /// Mock retriever that returns fixed documents
    struct MockRetriever3 {
        docs: Vec<Document>,
    }

    #[async_trait]
    impl Retriever for MockRetriever3 {
        async fn _get_relevant_documents(
            &self,
            query: &str,
            _config: Option<&RunnableConfig>,
        ) -> Result<Vec<Document>> {
            // Return documents with query embedded for verification
            Ok(self
                .docs
                .iter()
                .map(|d| {
                    let mut doc = d.clone();
                    doc.metadata
                        .insert("query".to_string(), serde_json::json!(query));
                    doc
                })
                .collect())
        }
    }

    /// Mock LLM chain that generates predefined queries
    struct MockQueryGenerator {
        queries: Vec<String>,
    }

    #[async_trait]
    impl Runnable for MockQueryGenerator {
        type Input = HashMap<String, String>;
        type Output = Vec<String>;

        async fn invoke(
            &self,
            _input: Self::Input,
            _config: Option<RunnableConfig>,
        ) -> Result<Self::Output> {
            Ok(self.queries.clone())
        }

        async fn batch(
            &self,
            inputs: Vec<Self::Input>,
            _config: Option<RunnableConfig>,
        ) -> Result<Vec<Self::Output>> {
            Ok(vec![self.queries.clone(); inputs.len()])
        }

        async fn stream(
            &self,
            input: Self::Input,
            config: Option<RunnableConfig>,
        ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>>
        {
            let result = self.invoke(input, config).await?;
            Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
        }
    }

    #[tokio::test]
    async fn test_multi_query_basic_retrieval() {
        // Test that multi-query retriever generates queries and retrieves documents
        let base_docs = vec![
            Document::new("doc1").with_metadata("id", 1),
            Document::new("doc2").with_metadata("id", 2),
        ];

        let base_retriever = MockRetriever3 {
            docs: base_docs.clone(),
        };

        let query_generator = MockQueryGenerator {
            queries: vec![
                "What is AI?".to_string(),
                "Explain artificial intelligence".to_string(),
            ],
        };

        let retriever =
            MultiQueryRetriever::new(base_retriever, query_generator, false).with_verbose(false);

        let results = retriever._get_relevant_documents("AI", None).await.unwrap();

        // Should retrieve documents (may have duplicates removed)
        assert!(!results.is_empty(), "Should return documents");

        // Verify documents came from base retriever
        for doc in &results {
            assert!(
                doc.page_content == "doc1" || doc.page_content == "doc2",
                "Document should be from base retriever"
            );
        }
    }

    #[tokio::test]
    async fn test_multi_query_include_original() {
        // Test that include_original adds the original query
        let base_docs = vec![Document::new("result")];

        let base_retriever = MockRetriever3 {
            docs: base_docs.clone(),
        };

        let query_generator = MockQueryGenerator {
            queries: vec!["generated query 1".to_string()],
        };

        let retriever =
            MultiQueryRetriever::new(base_retriever, query_generator, true).with_verbose(false);

        let results = retriever
            ._get_relevant_documents("original query", None)
            .await
            .unwrap();

        // Should have retrieved documents for both generated + original query
        // (2 queries × 1 doc each = 2 docs before deduplication)
        assert!(!results.is_empty());

        // Check that original query was used (via metadata)
        let queries_used: Vec<String> = results
            .iter()
            .filter_map(|d| d.metadata.get("query"))
            .filter_map(|v| v.as_str())
            .map(|s| s.to_string())
            .collect();

        assert!(
            queries_used.contains(&"original query".to_string())
                || queries_used.contains(&"generated query 1".to_string()),
            "Should have used original or generated query"
        );
    }

    #[tokio::test]
    async fn test_multi_query_deduplication() {
        // Test that duplicate documents are removed
        // Note: Documents with same content but different metadata are NOT duplicates
        let base_docs = vec![Document::new("duplicate"), Document::new("unique")];

        let base_retriever = MockRetriever3 {
            docs: base_docs.clone(),
        };

        let query_generator = MockQueryGenerator {
            queries: vec![
                "query1".to_string(),
                "query2".to_string(),
                "query3".to_string(),
            ],
        };

        let retriever =
            MultiQueryRetriever::new(base_retriever, query_generator, false).with_verbose(false);

        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        // With 3 queries and 2 docs each, we get 6 docs
        // MockRetriever3 adds query metadata, making each doc unique even with same content
        // So deduplication doesn't reduce count (metadata differs)
        assert_eq!(
            results.len(),
            6,
            "Each doc has unique metadata from different queries"
        );

        // But page_content should only have 2 unique values
        let unique_contents: std::collections::HashSet<_> =
            results.iter().map(|d| d.page_content.as_str()).collect();
        assert_eq!(unique_contents.len(), 2, "Only 2 unique content values");
        assert!(unique_contents.contains("duplicate"));
        assert!(unique_contents.contains("unique"));
    }

    #[tokio::test]
    async fn test_multi_query_empty_generated_queries() {
        // Test behavior when query generator returns empty list
        let base_docs = vec![Document::new("doc")];

        let base_retriever = MockRetriever3 {
            docs: base_docs.clone(),
        };

        let query_generator = MockQueryGenerator {
            queries: vec![], // Empty
        };

        let retriever =
            MultiQueryRetriever::new(base_retriever, query_generator, false).with_verbose(false);

        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        // With no generated queries and include_original=false, should return empty
        assert_eq!(results.len(), 0, "Should return empty with no queries");
    }

    #[tokio::test]
    async fn test_multi_query_empty_with_original() {
        // Test that include_original still works when no queries generated
        let base_docs = vec![Document::new("doc")];

        let base_retriever = MockRetriever3 {
            docs: base_docs.clone(),
        };

        let query_generator = MockQueryGenerator {
            queries: vec![], // Empty
        };

        let retriever =
            MultiQueryRetriever::new(base_retriever, query_generator, true).with_verbose(false);

        let results = retriever
            ._get_relevant_documents("original", None)
            .await
            .unwrap();

        // Should use original query even though no queries were generated
        assert_eq!(results.len(), 1, "Should return docs from original query");
        assert_eq!(results[0].page_content, "doc");
    }

    #[tokio::test]
    async fn test_multi_query_runnable_interface() {
        // Test that MultiQueryRetriever implements Runnable correctly
        let base_docs = vec![Document::new("result")];

        let base_retriever = MockRetriever3 {
            docs: base_docs.clone(),
        };

        let query_generator = MockQueryGenerator {
            queries: vec!["generated".to_string()],
        };

        let retriever =
            MultiQueryRetriever::new(base_retriever, query_generator, false).with_verbose(false);

        // Test invoke
        let results = retriever
            .invoke("test query".to_string(), None)
            .await
            .unwrap();
        assert!(!results.is_empty());

        // Test batch
        let batch_results = retriever
            .batch(vec!["query1".to_string(), "query2".to_string()], None)
            .await
            .unwrap();
        assert_eq!(batch_results.len(), 2);
    }

    #[tokio::test]
    async fn test_multi_query_name() {
        let base_docs = vec![Document::new("doc")];
        let base_retriever = MockRetriever3 { docs: base_docs };
        let query_generator = MockQueryGenerator {
            queries: vec!["q".to_string()],
        };

        let retriever =
            MultiQueryRetriever::new(base_retriever, query_generator, false).with_verbose(false);

        assert_eq!(
            Retriever::name(&retriever),
            "MultiQueryRetriever",
            "Name should be MultiQueryRetriever"
        );
    }

    #[tokio::test]
    async fn test_unique_documents_helper() {
        // Test the unique_documents helper function
        let docs = vec![
            Document::new("a"),
            Document::new("b"),
            Document::new("a"), // duplicate
            Document::new("c"),
            Document::new("b"), // duplicate
        ];

        let unique = unique_documents(docs);

        assert_eq!(unique.len(), 3);
        assert_eq!(unique[0].page_content, "a");
        assert_eq!(unique[1].page_content, "b");
        assert_eq!(unique[2].page_content, "c");
    }

    #[tokio::test]
    async fn test_unique_documents_with_metadata() {
        // Test that metadata is considered in deduplication
        let docs = vec![
            Document::new("same").with_metadata("id", 1),
            Document::new("same").with_metadata("id", 2), // Different metadata
            Document::new("same").with_metadata("id", 1), // True duplicate
        ];

        let unique = unique_documents(docs);

        // Should have 2 unique (same content but different metadata counts as different)
        assert_eq!(unique.len(), 2);
    }

    #[tokio::test]
    async fn test_multi_query_large_number_of_queries() {
        // Test with many generated queries
        let base_docs = vec![Document::new("doc1"), Document::new("doc2")];

        let base_retriever = MockRetriever3 {
            docs: base_docs.clone(),
        };

        let queries: Vec<String> = (0..20).map(|i| format!("query {}", i)).collect();

        let query_generator = MockQueryGenerator { queries };

        let retriever =
            MultiQueryRetriever::new(base_retriever, query_generator, false).with_verbose(false);

        let start = std::time::Instant::now();
        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();
        let duration = start.elapsed();

        // Should handle many queries efficiently
        // 20 queries × 2 docs = 40 docs (each with unique query metadata)
        assert_eq!(
            results.len(),
            40,
            "Should return all docs (metadata makes them unique)"
        );

        // But only 2 unique content values
        let unique_contents: std::collections::HashSet<_> =
            results.iter().map(|d| d.page_content.as_str()).collect();
        assert_eq!(unique_contents.len(), 2, "Only 2 unique content values");

        assert!(
            duration.as_secs() < 5,
            "Should complete in reasonable time: {:?}",
            duration
        );
    }

    /// Mock query generator that requires config to be passed.
    /// Used to verify RunnableConfig propagation through MultiQueryRetriever.
    struct ConfigRequiredQueryGenerator;

    #[async_trait]
    impl Runnable for ConfigRequiredQueryGenerator {
        type Input = HashMap<String, String>;
        type Output = Vec<String>;

        async fn invoke(
            &self,
            _input: Self::Input,
            config: Option<RunnableConfig>,
        ) -> Result<Self::Output> {
            if config.is_none() {
                return Err(Error::other(
                    "Expected RunnableConfig to be propagated to query generator",
                ));
            }
            Ok(vec!["generated query".to_string()])
        }

        async fn batch(
            &self,
            inputs: Vec<Self::Input>,
            config: Option<RunnableConfig>,
        ) -> Result<Vec<Self::Output>> {
            for input in inputs {
                self.invoke(input, config.clone()).await?;
            }
            Ok(vec![vec!["generated query".to_string()]])
        }

        async fn stream(
            &self,
            input: Self::Input,
            config: Option<RunnableConfig>,
        ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<Self::Output>> + Send>>>
        {
            let result = self.invoke(input, config).await?;
            Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
        }
    }

    #[tokio::test]
    async fn test_multi_query_retriever_propagates_config_to_llm_chain() {
        // Regression test: Verify RunnableConfig is propagated to the llm_chain.
        // This prevents regressions like M-1079 (EnsembleRetriever) and M-1081 (SelfQueryRetriever).
        let base_docs = vec![Document::new("test doc")];
        let base_retriever = MockRetriever3 {
            docs: base_docs.clone(),
        };

        let query_generator = ConfigRequiredQueryGenerator;
        let retriever =
            MultiQueryRetriever::new(base_retriever, query_generator, false).with_verbose(false);

        // Without config, should fail
        let result_without_config = retriever._get_relevant_documents("test", None).await;
        assert!(
            result_without_config.is_err(),
            "Should fail without config because ConfigRequiredQueryGenerator requires it"
        );

        // With config, should succeed
        let config = RunnableConfig::default();
        let result_with_config = retriever
            ._get_relevant_documents("test", Some(&config))
            .await;
        assert!(
            result_with_config.is_ok(),
            "Should succeed with config: {:?}",
            result_with_config.err()
        );
    }
}

// ================================================================================================
// ContextualCompressionRetriever Tests
// ================================================================================================

#[cfg(test)]
mod contextual_compression_retriever_tests {
    use crate::test_prelude::*;
    use std::sync::Arc;

    /// Mock retriever that returns fixed documents
    struct MockRetriever4 {
        docs: Vec<Document>,
    }

    #[async_trait]
    impl Retriever for MockRetriever4 {
        async fn _get_relevant_documents(
            &self,
            _query: &str,
            _config: Option<&RunnableConfig>,
        ) -> Result<Vec<Document>> {
            Ok(self.docs.clone())
        }
    }

    /// Mock compressor that filters documents based on content length
    struct MockCompressor {
        min_length: usize,
    }

    #[async_trait]
    impl DocumentCompressor for MockCompressor {
        async fn compress_documents(
            &self,
            documents: Vec<Document>,
            _query: &str,
            _config: Option<&RunnableConfig>,
        ) -> Result<Vec<Document>> {
            // Filter documents by minimum length
            Ok(documents
                .into_iter()
                .filter(|d| d.page_content.len() >= self.min_length)
                .collect())
        }
    }

    /// Mock compressor that adds metadata
    struct MetadataAddingCompressor;

    #[async_trait]
    impl DocumentCompressor for MetadataAddingCompressor {
        async fn compress_documents(
            &self,
            documents: Vec<Document>,
            query: &str,
            _config: Option<&RunnableConfig>,
        ) -> Result<Vec<Document>> {
            // Add compression metadata to each document
            Ok(documents
                .into_iter()
                .map(|mut d| {
                    d.metadata
                        .insert("compressed".to_string(), serde_json::json!(true));
                    d.metadata
                        .insert("query_used".to_string(), serde_json::json!(query));
                    d
                })
                .collect())
        }
    }

    #[tokio::test]
    async fn test_contextual_compression_basic() {
        // Test basic compression filtering
        let base_docs = vec![
            Document::new("short"),
            Document::new("this is a longer document"),
            Document::new("mid"),
        ];

        let base_retriever = MockRetriever4 {
            docs: base_docs.clone(),
        };

        let compressor = Arc::new(MockCompressor { min_length: 10 });

        let retriever = ContextualCompressionRetriever::new(base_retriever, compressor);

        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        // Should only return documents with length >= 10
        assert_eq!(results.len(), 1, "Should filter to 1 long document");
        assert_eq!(results[0].page_content, "this is a longer document");
    }

    #[tokio::test]
    async fn test_contextual_compression_empty_results() {
        // Test when base retriever returns empty
        let base_retriever = MockRetriever4 { docs: vec![] };

        let compressor = Arc::new(MockCompressor { min_length: 5 });

        let retriever = ContextualCompressionRetriever::new(base_retriever, compressor);

        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 0, "Should return empty when no docs");
    }

    #[tokio::test]
    async fn test_contextual_compression_all_filtered() {
        // Test when compressor filters out all documents
        let base_docs = vec![Document::new("a"), Document::new("b"), Document::new("c")];

        let base_retriever = MockRetriever4 {
            docs: base_docs.clone(),
        };

        let compressor = Arc::new(MockCompressor { min_length: 100 }); // Very high threshold

        let retriever = ContextualCompressionRetriever::new(base_retriever, compressor);

        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        assert_eq!(
            results.len(),
            0,
            "Should return empty when all docs filtered"
        );
    }

    #[tokio::test]
    async fn test_contextual_compression_metadata_preservation() {
        // Test that original metadata is preserved and new metadata is added
        let base_docs = vec![
            Document::new("doc1").with_metadata("original", 1),
            Document::new("doc2").with_metadata("original", 2),
        ];

        let base_retriever = MockRetriever4 {
            docs: base_docs.clone(),
        };

        let compressor = Arc::new(MetadataAddingCompressor);

        let retriever = ContextualCompressionRetriever::new(base_retriever, compressor);

        let results = retriever
            ._get_relevant_documents("test query", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);

        for doc in &results {
            // Original metadata preserved
            assert!(doc.metadata.contains_key("original"));

            // Compressor added metadata
            assert_eq!(
                doc.metadata.get("compressed"),
                Some(&serde_json::json!(true))
            );
            assert_eq!(
                doc.metadata.get("query_used"),
                Some(&serde_json::json!("test query"))
            );
        }
    }

    #[tokio::test]
    async fn test_contextual_compression_runnable_interface() {
        // Test that ContextualCompressionRetriever implements Runnable correctly
        let base_docs = vec![Document::new("document")];

        let base_retriever = MockRetriever4 {
            docs: base_docs.clone(),
        };

        let compressor = Arc::new(MockCompressor { min_length: 1 });

        let retriever = ContextualCompressionRetriever::new(base_retriever, compressor);

        // Test invoke
        let results = retriever
            .invoke("test query".to_string(), None)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // Test batch
        let batch_results = retriever
            .batch(vec!["query1".to_string(), "query2".to_string()], None)
            .await
            .unwrap();
        assert_eq!(batch_results.len(), 2);
    }

    #[tokio::test]
    async fn test_contextual_compression_name() {
        let base_retriever = MockRetriever4 { docs: vec![] };
        let compressor = Arc::new(MockCompressor { min_length: 1 });
        let retriever = ContextualCompressionRetriever::new(base_retriever, compressor);

        assert_eq!(
            Retriever::name(&retriever),
            "ContextualCompressionRetriever",
            "Name should be ContextualCompressionRetriever"
        );
    }

    #[tokio::test]
    async fn test_contextual_compression_query_passed_to_compressor() {
        // Test that query is correctly passed to compressor
        let base_docs = vec![Document::new("test doc")];

        let base_retriever = MockRetriever4 {
            docs: base_docs.clone(),
        };

        let compressor = Arc::new(MetadataAddingCompressor);

        let retriever = ContextualCompressionRetriever::new(base_retriever, compressor);

        let results = retriever
            ._get_relevant_documents("specific query", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].metadata.get("query_used"),
            Some(&serde_json::json!("specific query")),
            "Query should be passed to compressor"
        );
    }

    #[tokio::test]
    async fn test_contextual_compression_order_preservation() {
        // Test that compressor preserves document order
        let base_docs = vec![
            Document::new("first document content"),
            Document::new("second document content"),
            Document::new("third document content"),
        ];

        let base_retriever = MockRetriever4 {
            docs: base_docs.clone(),
        };

        let compressor = Arc::new(MockCompressor { min_length: 10 });

        let retriever = ContextualCompressionRetriever::new(base_retriever, compressor);

        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        // All should pass filter, order should be preserved
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].page_content, "first document content");
        assert_eq!(results[1].page_content, "second document content");
        assert_eq!(results[2].page_content, "third document content");
    }

    #[tokio::test]
    async fn test_contextual_compression_partial_filtering() {
        // Test compressor that filters some but not all documents
        let base_docs = vec![
            Document::new("a"),      // too short
            Document::new("medium"), // passes
            Document::new("bb"),     // too short
            Document::new("longer"), // passes
        ];

        let base_retriever = MockRetriever4 {
            docs: base_docs.clone(),
        };

        let compressor = Arc::new(MockCompressor { min_length: 5 });

        let retriever = ContextualCompressionRetriever::new(base_retriever, compressor);

        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        assert_eq!(
            results.len(),
            2,
            "Should return 2 documents that passed filter"
        );
        assert_eq!(results[0].page_content, "medium");
        assert_eq!(results[1].page_content, "longer");
    }

    #[tokio::test]
    async fn test_contextual_compression_performance() {
        // Test performance with many documents
        let base_docs: Vec<Document> = (0..100)
            .map(|i| Document::new(format!("document number {}", i)))
            .collect();

        let base_retriever = MockRetriever4 {
            docs: base_docs.clone(),
        };

        let compressor = Arc::new(MockCompressor { min_length: 15 });

        let retriever = ContextualCompressionRetriever::new(base_retriever, compressor);

        let start = std::time::Instant::now();
        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();
        let duration = start.elapsed();

        // Should handle many documents efficiently
        assert!(!results.is_empty(), "Should return filtered documents");
        assert!(
            duration.as_millis() < 500,
            "Should complete quickly: {:?}",
            duration
        );
    }

    /// Mock compressor that requires config to be passed.
    /// Used to verify RunnableConfig propagation through ContextualCompressionRetriever.
    struct ConfigRequiredCompressor;

    #[async_trait]
    impl DocumentCompressor for ConfigRequiredCompressor {
        async fn compress_documents(
            &self,
            documents: Vec<Document>,
            _query: &str,
            config: Option<&RunnableConfig>,
        ) -> Result<Vec<Document>> {
            if config.is_none() {
                return Err(Error::other(
                    "Expected RunnableConfig to be propagated to compressor",
                ));
            }
            Ok(documents)
        }
    }

    /// Mock retriever that requires config to be passed.
    /// Used to verify RunnableConfig propagation through ContextualCompressionRetriever.
    struct ConfigRequiredRetriever {
        docs: Vec<Document>,
    }

    #[async_trait]
    impl Retriever for ConfigRequiredRetriever {
        async fn _get_relevant_documents(
            &self,
            _query: &str,
            config: Option<&RunnableConfig>,
        ) -> Result<Vec<Document>> {
            if config.is_none() {
                return Err(Error::other(
                    "Expected RunnableConfig to be propagated to base retriever",
                ));
            }
            Ok(self.docs.clone())
        }
    }

    #[tokio::test]
    async fn test_contextual_compression_retriever_propagates_config_to_compressor() {
        // Regression test: Verify RunnableConfig is propagated to the compressor.
        // This prevents regressions similar to M-1079 (EnsembleRetriever).
        let base_docs = vec![Document::new("test doc")];
        let base_retriever = MockRetriever4 {
            docs: base_docs.clone(),
        };

        let compressor = Arc::new(ConfigRequiredCompressor);
        let retriever = ContextualCompressionRetriever::new(base_retriever, compressor);

        // Without config, should fail (compressor requires config)
        let result_without_config = retriever._get_relevant_documents("test", None).await;
        assert!(
            result_without_config.is_err(),
            "Should fail without config because ConfigRequiredCompressor requires it"
        );

        // With config, should succeed
        let config = RunnableConfig::default();
        let result_with_config = retriever
            ._get_relevant_documents("test", Some(&config))
            .await;
        assert!(
            result_with_config.is_ok(),
            "Should succeed with config: {:?}",
            result_with_config.err()
        );
    }

    #[tokio::test]
    async fn test_contextual_compression_retriever_propagates_config_to_retriever() {
        // Regression test: Verify RunnableConfig is propagated to the base retriever.
        let base_docs = vec![Document::new("test doc")];
        let base_retriever = ConfigRequiredRetriever {
            docs: base_docs.clone(),
        };

        let compressor = Arc::new(MockCompressor { min_length: 1 });
        let retriever = ContextualCompressionRetriever::new(base_retriever, compressor);

        // Without config, should fail (base retriever requires config)
        let result_without_config = retriever._get_relevant_documents("test", None).await;
        assert!(
            result_without_config.is_err(),
            "Should fail without config because ConfigRequiredRetriever requires it"
        );

        // With config, should succeed
        let config = RunnableConfig::default();
        let result_with_config = retriever
            ._get_relevant_documents("test", Some(&config))
            .await;
        assert!(
            result_with_config.is_ok(),
            "Should succeed with config: {:?}",
            result_with_config.err()
        );
    }
}

// ================================================================================================
// Parent Document Retriever - Separate module for advanced retrieval
// ================================================================================================

pub mod parent_document_retriever;
pub use parent_document_retriever::{MultiVectorRetriever, ParentDocumentRetriever};

// ================================================================================================
// Time Weighted Retriever - Combines recency with relevance
// ================================================================================================

pub mod time_weighted_retriever;
pub use time_weighted_retriever::TimeWeightedVectorStoreRetriever;

// ================================================================================================
// BM25 Retriever - BM25 ranking without Elasticsearch
// ================================================================================================

pub mod bm25_retriever;
pub use bm25_retriever::{default_preprocessing_func, BM25Retriever};

// ================================================================================================
// TF-IDF Retriever - TF-IDF scoring for document retrieval
// ================================================================================================

pub mod tfidf_retriever;
pub use tfidf_retriever::TFIDFRetriever;

// ================================================================================================
// KNN Retriever - K-Nearest Neighbors with embeddings
// ================================================================================================

pub mod knn_retriever;
pub use knn_retriever::KNNRetriever;

// ================================================================================================
// Deprecated Stub Retrievers - Feature-gated for backward compatibility
// ================================================================================================
//
// These are configuration-only stubs that return errors when called. They exist only for
// backward compatibility with code that references these types. For actual implementations:
// - ElasticSearchBM25Retriever → use dashflow-elasticsearch crate
// - PineconeHybridSearchRetriever → use dashflow-pinecone crate (vector search only, hybrid not yet available)
// - WeaviateHybridSearchRetriever → use dashflow-weaviate crate
//
// Enable with: features = ["stub-retrievers"]

#[cfg(feature = "stub-retrievers")]
pub mod elasticsearch_bm25_retriever;
#[cfg(feature = "stub-retrievers")]
#[allow(deprecated)]
pub use elasticsearch_bm25_retriever::ElasticSearchBM25Retriever;

#[cfg(feature = "stub-retrievers")]
pub mod pinecone_hybrid_search_retriever;
#[cfg(feature = "stub-retrievers")]
#[allow(deprecated)]
pub use pinecone_hybrid_search_retriever::{PineconeHybridConfig, PineconeHybridSearchRetriever};

#[cfg(feature = "stub-retrievers")]
pub mod weaviate_hybrid_search_retriever;
#[cfg(feature = "stub-retrievers")]
#[allow(deprecated)]
pub use weaviate_hybrid_search_retriever::{WeaviateHybridConfig, WeaviateHybridSearchRetriever};

// ================================================================================================
// Merger Retriever - Merges results from multiple retrievers
// ================================================================================================

/// Merges results from multiple retrievers into a single result set.
///
/// This module provides [`MergerRetriever`] which combines documents from
/// multiple underlying retrievers, deduplicating and optionally re-ranking
/// the results. Useful for ensemble retrieval strategies.
pub mod merger_retriever;

/// Self-query retriever that generates structured queries from natural language
pub mod self_query;
pub use merger_retriever::MergerRetriever;
pub use self_query::{QueryConstructor, SelfQueryRetriever};

// ================================================================================================
// RePhraseQuery Retriever - Rephrases queries using an LLM before retrieval
// ================================================================================================

pub mod rephrase_query_retriever;
pub use rephrase_query_retriever::RePhraseQueryRetriever;

// ================================================================================================
// Web Research Retriever - Generate search queries and retrieve web content
// ================================================================================================

pub mod web_research_retriever;
pub use web_research_retriever::{WebResearchRetriever, WebSearchTool, DEFAULT_SEARCH_PROMPT};
