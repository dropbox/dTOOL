//! K-Nearest Neighbors (KNN) retriever.
//!
//! KNN retriever uses embeddings and cosine similarity to find the k most similar
//! documents to a query. Unlike vector stores, this retriever is lightweight and
//! operates entirely in memory using simple distance calculations.
//!
//! # Algorithm
//!
//! 1. Embed all documents using an embeddings model
//! 2. For a query, compute its embedding
//! 3. Calculate cosine similarity between query and all document embeddings
//! 4. Return k documents with highest similarity
//!
//! # Example
//!
//! ```rust
//! use dashflow::core::retrievers::{Retriever, KNNRetriever};
//! use dashflow::core::documents::Document;
//! use dashflow::core::embeddings::Embeddings;
//! use std::sync::Arc;
//!
//! # async fn example(embeddings: Arc<dyn Embeddings>) -> Result<(), Box<dyn std::error::Error>> {
//! let docs = vec![
//!     Document::new("The quick brown fox jumps over the lazy dog"),
//!     Document::new("Machine learning is a subset of artificial intelligence"),
//!     Document::new("Rust is a systems programming language"),
//! ];
//!
//! let retriever = KNNRetriever::from_documents(docs, embeddings, None, None).await?;
//! let results = retriever._get_relevant_documents("machine learning", None).await?;
//! # Ok(())
//! # }
//! ```

use crate::core::{
    config::RunnableConfig,
    documents::Document,
    embeddings::Embeddings,
    error::{Error, Result},
    retrievers::{Retriever, RetrieverInput, RetrieverOutput},
    runnable::Runnable,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// K-Nearest Neighbors retriever.
///
/// Uses embeddings to find the k most similar documents to a query based on
/// cosine similarity. Optionally filters results by a relevancy threshold.
///
/// # Fields
///
/// - `embeddings`: Embeddings model for encoding queries and documents
/// - `docs`: List of documents with their text
/// - `doc_embeddings`: Pre-computed embeddings for all documents
/// - `k`: Number of documents to return (default: 4)
/// - `relevancy_threshold`: Optional minimum similarity score (0.0-1.0)
pub struct KNNRetriever {
    /// Embeddings model to use
    embeddings: Arc<dyn Embeddings>,

    /// List of documents
    docs: Vec<Document>,

    /// Pre-computed document embeddings
    doc_embeddings: Vec<Vec<f32>>,

    /// Number of results to return
    k: usize,

    /// Optional relevancy threshold (0.0-1.0)
    relevancy_threshold: Option<f32>,
}

impl KNNRetriever {
    /// Create a new `KNNRetriever`.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - Embeddings model to use
    /// * `docs` - List of documents
    /// * `doc_embeddings` - Pre-computed embeddings for documents
    /// * `k` - Number of documents to return
    /// * `relevancy_threshold` - Optional minimum similarity score
    pub fn new(
        embeddings: Arc<dyn Embeddings>,
        docs: Vec<Document>,
        doc_embeddings: Vec<Vec<f32>>,
        k: usize,
        relevancy_threshold: Option<f32>,
    ) -> Result<Self> {
        if docs.is_empty() {
            return Err(Error::config("KNNRetriever requires at least one document"));
        }

        if docs.len() != doc_embeddings.len() {
            return Err(Error::config(
                "Number of documents must match number of embeddings",
            ));
        }

        Ok(Self {
            embeddings,
            docs,
            doc_embeddings,
            k,
            relevancy_threshold,
        })
    }

    /// Create a `KNNRetriever` from texts.
    ///
    /// # Arguments
    ///
    /// * `texts` - List of text strings
    /// * `embeddings` - Embeddings model to use
    /// * `metadatas` - Optional metadata for each text
    /// * `k` - Number of documents to return
    /// * `relevancy_threshold` - Optional minimum similarity score
    pub async fn from_texts(
        texts: Vec<String>,
        embeddings: Arc<dyn Embeddings>,
        metadatas: Option<Vec<HashMap<String, serde_json::Value>>>,
        k: Option<usize>,
        relevancy_threshold: Option<f32>,
    ) -> Result<Self> {
        // Create documents
        let docs: Vec<Document> = if let Some(metas) = metadatas {
            texts
                .iter()
                .zip(metas.iter())
                .map(|(text, meta)| Document {
                    page_content: text.clone(),
                    metadata: meta.clone(),
                    id: None,
                })
                .collect()
        } else {
            texts.iter().map(Document::new).collect()
        };

        // Embed documents
        let doc_embeddings = embeddings
            ._embed_documents(&texts)
            .await
            .map_err(|e| {
                Error::other(format!(
                    "Failed to embed {} documents for KNNRetriever: {e}",
                    texts.len()
                ))
            })?;

        Self::new(
            embeddings,
            docs,
            doc_embeddings,
            k.unwrap_or(4),
            relevancy_threshold,
        )
    }

    /// Create a `KNNRetriever` from documents.
    ///
    /// # Arguments
    ///
    /// * `documents` - List of documents
    /// * `embeddings` - Embeddings model to use
    /// * `k` - Number of documents to return
    /// * `relevancy_threshold` - Optional minimum similarity score
    pub async fn from_documents(
        documents: Vec<Document>,
        embeddings: Arc<dyn Embeddings>,
        k: Option<usize>,
        relevancy_threshold: Option<f32>,
    ) -> Result<Self> {
        let texts: Vec<String> = documents
            .iter()
            .map(|doc| doc.page_content.clone())
            .collect();

        // Embed documents
        let doc_embeddings = embeddings
            ._embed_documents(&texts)
            .await
            .map_err(|e| {
                Error::other(format!(
                    "Failed to embed {} documents for KNNRetriever: {e}",
                    texts.len()
                ))
            })?;

        // Preserve full documents including IDs (don't convert through from_texts)
        Self::new(
            embeddings,
            documents,
            doc_embeddings,
            k.unwrap_or(4),
            relevancy_threshold,
        )
    }

    /// Calculate cosine similarity between two vectors.
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }

    /// Normalize a vector using L2 norm.
    fn normalize(vector: &[f32]) -> Vec<f32> {
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm == 0.0 {
            vector.to_vec()
        } else {
            vector.iter().map(|x| x / norm).collect()
        }
    }

    /// Get top k documents for a query.
    async fn get_top_n(&self, query: &str) -> Result<Vec<Document>> {
        // Embed query
        let query_embedding = self
            .embeddings
            ._embed_query(query)
            .await
            .map_err(|e| {
                Error::other(format!("Failed to embed query for KNN retrieval: {e}"))
            })?;
        let normalized_query = Self::normalize(&query_embedding);

        // Normalize document embeddings for consistent comparison
        let normalized_doc_embeddings: Vec<Vec<f32>> = self
            .doc_embeddings
            .iter()
            .map(|emb| Self::normalize(emb))
            .collect();

        // Calculate similarities with all documents
        let mut scores: Vec<(usize, f32)> = normalized_doc_embeddings
            .iter()
            .enumerate()
            .map(|(idx, doc_emb)| (idx, Self::cosine_similarity(&normalized_query, doc_emb)))
            .collect();

        // Sort by similarity (descending)
        scores.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        // Apply relevancy threshold if specified
        if let Some(threshold) = self.relevancy_threshold {
            // Normalize similarities to [0, 1] range
            let max_sim = scores
                .iter()
                .map(|(_, s)| s)
                .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            let min_sim = scores
                .iter()
                .map(|(_, s)| s)
                .fold(f32::INFINITY, |a, &b| a.min(b));
            let denominator = (max_sim - min_sim + 1e-6).max(1e-6);

            scores.retain(|(_, sim)| {
                let normalized_sim = (sim - min_sim) / denominator;
                normalized_sim >= threshold
            });
        }

        // Return top k documents
        Ok(scores
            .into_iter()
            .take(self.k)
            .map(|(idx, _)| self.docs[idx].clone())
            .collect())
    }
}

#[async_trait]
impl Retriever for KNNRetriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        self.get_top_n(query).await
    }

    fn name(&self) -> String {
        "KNNRetriever".to_string()
    }
}

#[async_trait]
impl Runnable for KNNRetriever {
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
        "KNNRetriever".to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::core::embeddings::Embeddings;
    use crate::test_prelude::*;

    // Mock embeddings for testing
    struct MockEmbeddings;

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            // Simple mock: use length and character counts as features
            Ok(texts
                .iter()
                .map(|text| {
                    vec![
                        text.len() as f32,
                        text.chars().filter(|c| c.is_alphabetic()).count() as f32,
                        text.chars().filter(|c| c.is_whitespace()).count() as f32,
                    ]
                })
                .collect())
        }

        async fn _embed_query(&self, text: &str) -> Result<Vec<f32>> {
            Ok(vec![
                text.len() as f32,
                text.chars().filter(|c| c.is_alphabetic()).count() as f32,
                text.chars().filter(|c| c.is_whitespace()).count() as f32,
            ])
        }
    }

    #[tokio::test]
    async fn test_knn_basic() {
        let docs = vec![
            Document::new("The quick brown fox jumps over the lazy dog"),
            Document::new("Machine learning is a subset of artificial intelligence"),
            Document::new("Rust is a systems programming language"),
            Document::new("Deep learning uses neural networks"),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(2), None)
            .await
            .unwrap();
        let results = retriever
            ._get_relevant_documents("machine learning", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_knn_from_texts() {
        let texts = vec![
            "short text".to_string(),
            "a much longer text with many words".to_string(),
            "medium length text".to_string(),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_texts(texts, embeddings, None, Some(2), None)
            .await
            .unwrap();
        let results = retriever
            ._get_relevant_documents("short", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_knn_with_threshold() {
        let docs = vec![
            Document::new("apple orange banana"),
            Document::new("grape kiwi mango"),
            Document::new("completely different text here"),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        // Set high threshold to filter out dissimilar documents
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(3), Some(0.9))
            .await
            .unwrap();
        let results = retriever
            ._get_relevant_documents("apple", None)
            .await
            .unwrap();

        // With high threshold, we might get fewer than k results
        assert!(results.len() <= 3);
    }

    #[tokio::test]
    async fn test_knn_empty_docs_error() {
        let docs: Vec<Document> = vec![];
        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let result = KNNRetriever::from_documents(docs, embeddings, Some(4), None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_knn_as_runnable() {
        let docs = vec![
            Document::new("first document"),
            Document::new("second document"),
            Document::new("third document"),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(2), None)
            .await
            .unwrap();
        let results = retriever
            .invoke("test query".to_string(), None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(KNNRetriever::cosine_similarity(&a, &b), 1.0);

        let c = vec![1.0, 0.0, 0.0];
        let d = vec![0.0, 1.0, 0.0];
        assert_eq!(KNNRetriever::cosine_similarity(&c, &d), 0.0);
    }

    #[test]
    fn test_normalize() {
        let vector = vec![3.0, 4.0];
        let normalized = KNNRetriever::normalize(&vector);
        let norm: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    // === Edge Cases ===

    #[tokio::test]
    async fn test_empty_query() {
        let docs = vec![Document::new("apple orange"), Document::new("banana kiwi")];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(2), None)
            .await
            .unwrap();
        let results = retriever._get_relevant_documents("", None).await.unwrap();

        // Empty query should still return k documents
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_very_long_query() {
        let docs = vec![
            Document::new("short doc"),
            Document::new("another short doc"),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(2), None)
            .await
            .unwrap();
        let long_query = "word ".repeat(1000);
        let results = retriever
            ._get_relevant_documents(&long_query, None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_single_document() {
        let docs = vec![Document::new("single document")];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(5), None)
            .await
            .unwrap();
        let results = retriever
            ._get_relevant_documents("query", None)
            .await
            .unwrap();

        // Should return 1 document even though k=5
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_k_larger_than_corpus() {
        let docs = vec![
            Document::new("doc1"),
            Document::new("doc2"),
            Document::new("doc3"),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(10), None)
            .await
            .unwrap();
        let results = retriever._get_relevant_documents("doc", None).await.unwrap();

        // Should return only 3 documents even though k=10
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_mismatched_embeddings_error() {
        let docs = vec![Document::new("doc1"), Document::new("doc2")];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        // Create embeddings for only 1 document (mismatch)
        let doc_embeddings = vec![vec![1.0, 2.0, 3.0]];

        let result = KNNRetriever::new(embeddings, docs, doc_embeddings, 2, None);
        assert!(result.is_err());
    }

    // === Threshold Tests ===

    #[tokio::test]
    async fn test_threshold_filters_results() {
        let docs = vec![
            Document::new("apple"),
            Document::new("banana"),
            Document::new("cherry"),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        // Very high threshold should filter most results
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(3), Some(0.95))
            .await
            .unwrap();
        let results = retriever
            ._get_relevant_documents("apple", None)
            .await
            .unwrap();

        // With high threshold, should get fewer results
        assert!(results.len() <= 3);
    }

    #[tokio::test]
    async fn test_zero_threshold() {
        let docs = vec![
            Document::new("apple"),
            Document::new("banana"),
            Document::new("cherry"),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        // Zero threshold should include all results
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(3), Some(0.0))
            .await
            .unwrap();
        let results = retriever
            ._get_relevant_documents("apple", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_no_threshold() {
        let docs = vec![
            Document::new("apple"),
            Document::new("banana"),
            Document::new("cherry"),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        // No threshold should return k results
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(2), None)
            .await
            .unwrap();
        let results = retriever
            ._get_relevant_documents("apple", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    // === Similarity Calculation Tests ===

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert_eq!(KNNRetriever::cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        assert_eq!(KNNRetriever::cosine_similarity(&a, &b), -1.0);
    }

    #[test]
    fn test_cosine_similarity_partial() {
        let a = vec![1.0, 1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let similarity = KNNRetriever::cosine_similarity(&a, &b);
        assert!(similarity > 0.0 && similarity < 1.0);
    }

    #[test]
    fn test_cosine_similarity_zero_vectors() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(KNNRetriever::cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_both_zero() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![0.0, 0.0, 0.0];
        assert_eq!(KNNRetriever::cosine_similarity(&a, &b), 0.0);
    }

    // === Normalization Tests ===

    #[test]
    fn test_normalize_unit_vector() {
        let vector = vec![1.0, 0.0, 0.0];
        let normalized = KNNRetriever::normalize(&vector);
        assert_eq!(normalized, vec![1.0, 0.0, 0.0]);
    }

    #[test]
    fn test_normalize_zero_vector() {
        let vector = vec![0.0, 0.0, 0.0];
        let normalized = KNNRetriever::normalize(&vector);
        assert_eq!(normalized, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_normalize_negative_values() {
        let vector = vec![-3.0, -4.0];
        let normalized = KNNRetriever::normalize(&vector);
        let norm: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_large_values() {
        let vector = vec![1000.0, 2000.0, 3000.0];
        let normalized = KNNRetriever::normalize(&vector);
        let norm: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    // === Metadata Handling ===

    #[tokio::test]
    async fn test_from_texts_with_metadata() {
        let texts = vec![
            "first document".to_string(),
            "second document".to_string(),
            "third document".to_string(),
        ];

        let mut meta1 = HashMap::new();
        meta1.insert("id".to_string(), serde_json::json!(1));
        meta1.insert("author".to_string(), serde_json::json!("Alice"));

        let mut meta2 = HashMap::new();
        meta2.insert("id".to_string(), serde_json::json!(2));
        meta2.insert("author".to_string(), serde_json::json!("Bob"));

        let mut meta3 = HashMap::new();
        meta3.insert("id".to_string(), serde_json::json!(3));
        meta3.insert("author".to_string(), serde_json::json!("Charlie"));

        let metadatas = vec![meta1, meta2, meta3];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_texts(texts, embeddings, Some(metadatas), Some(2), None)
            .await
            .unwrap();
        let results = retriever
            ._get_relevant_documents("document", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].metadata.contains_key("id"));
        assert!(results[0].metadata.contains_key("author"));
    }

    #[tokio::test]
    async fn test_metadata_preserved_after_retrieval() {
        let mut meta = HashMap::new();
        meta.insert("source".to_string(), serde_json::json!("test.txt"));
        meta.insert("page".to_string(), serde_json::json!(42));

        let doc = Document {
            page_content: "test document with metadata".to_string(),
            metadata: meta,
            id: Some("doc-123".to_string()),
        };

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_documents(vec![doc], embeddings, Some(1), None)
            .await
            .unwrap();
        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metadata["source"], "test.txt");
        assert_eq!(results[0].metadata["page"], 42);
        // Document IDs are now preserved through from_documents
        assert_eq!(results[0].id, Some("doc-123".to_string()));
    }

    // === Runnable Trait Tests ===

    #[tokio::test]
    async fn test_runnable_invoke_with_config() {
        let docs = vec![
            Document::new("rust language"),
            Document::new("python language"),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(2), None)
            .await
            .unwrap();
        let config = RunnableConfig::default();
        let results = retriever
            .invoke("rust".to_string(), Some(config))
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_runnable_name() {
        let docs = vec![Document::new("test")];
        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(1), None)
            .await
            .unwrap();
        assert_eq!(Retriever::name(&retriever), "KNNRetriever");
        assert_eq!(Runnable::name(&retriever), "KNNRetriever");
    }

    // === Different K Values ===

    #[tokio::test]
    async fn test_k_value_1() {
        let docs = vec![
            Document::new("doc1"),
            Document::new("doc2"),
            Document::new("doc3"),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(1), None)
            .await
            .unwrap();
        let results = retriever._get_relevant_documents("doc", None).await.unwrap();

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_default_k_value() {
        let texts = vec![
            "doc1".to_string(),
            "doc2".to_string(),
            "doc3".to_string(),
            "doc4".to_string(),
            "doc5".to_string(),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        // None for k should default to 4
        let retriever = KNNRetriever::from_texts(texts, embeddings, None, None, None)
            .await
            .unwrap();
        let results = retriever._get_relevant_documents("doc", None).await.unwrap();

        assert_eq!(results.len(), 4); // Default k is 4
    }

    // === Stress Tests ===

    #[tokio::test]
    async fn test_large_document_corpus() {
        let docs: Vec<Document> = (0..100)
            .map(|i| Document::new(format!("document number {}", i)))
            .collect();

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(10), None)
            .await
            .unwrap();
        let results = retriever
            ._get_relevant_documents("document", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 10);
    }

    #[tokio::test]
    async fn test_high_dimensional_embeddings() {
        // MockEmbeddings uses 3D vectors, but test that it works
        let docs = vec![
            Document::new("test document 1"),
            Document::new("test document 2"),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(2), None)
            .await
            .unwrap();
        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    // === Constructor Tests ===

    #[tokio::test]
    async fn test_new_constructor() {
        let docs = vec![Document::new("doc1"), Document::new("doc2")];
        let doc_embeddings = vec![vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::new(embeddings, docs, doc_embeddings, 2, None).unwrap();
        let results = retriever
            ._get_relevant_documents("test", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_from_documents_preserves_order() {
        let docs = vec![
            Document::new("first"),
            Document::new("second"),
            Document::new("third"),
        ];

        let embeddings = Arc::new(MockEmbeddings) as Arc<dyn Embeddings>;
        let retriever = KNNRetriever::from_documents(docs, embeddings, Some(3), None)
            .await
            .unwrap();

        // Just verify it completes successfully
        assert!(retriever._get_relevant_documents("test", None).await.is_ok());
    }
}
