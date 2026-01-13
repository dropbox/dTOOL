//! Embeddings-based document filter
//!
//! This compressor filters documents based on cosine similarity between
//! document embeddings and the query embedding.

use async_trait::async_trait;
use dashflow::core::{
    config::RunnableConfig,
    documents::{Document, DocumentCompressor},
    embeddings::Embeddings,
    error::{Error, Result},
};
use dashflow::{embed, embed_query};
use std::sync::Arc;

/// Calculate cosine similarity between a query embedding and multiple document embeddings
///
/// Returns a vector of similarity scores, one for each document embedding.
fn cosine_similarity_batch(query: &[f32], docs: &[Vec<f32>]) -> Vec<f32> {
    docs.iter()
        .map(|doc_embedding| cosine_similarity(query, doc_embedding))
        .collect()
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

/// Document compressor that filters by embedding similarity
///
/// This compressor embeds both the query and documents, then filters documents
/// based on cosine similarity to the query. You can specify either:
/// - `k`: Keep the top k most similar documents
/// - `similarity_threshold`: Keep all documents above this threshold
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_document_compressors::EmbeddingsFilter;
/// use dashflow_openai::OpenAIEmbeddings;
/// use dashflow::core::documents::Document;
///
/// let embeddings = OpenAIEmbeddings::new();
/// let filter = EmbeddingsFilter::new(embeddings).with_k(5);
///
/// let docs = vec![
///     Document::new("Rust is a systems programming language"),
///     Document::new("Python is a high-level language"),
/// ];
///
/// let filtered = filter.compress_documents(docs, "What is Rust?", None).await?;
/// // Returns top 5 most similar documents
/// ```
pub struct EmbeddingsFilter {
    /// Embeddings model to use
    embeddings: Arc<dyn Embeddings>,
    /// Number of top documents to keep (mutually exclusive with `similarity_threshold`)
    k: Option<usize>,
    /// Minimum similarity threshold (mutually exclusive with k)
    similarity_threshold: Option<f32>,
}

impl EmbeddingsFilter {
    /// Create a new filter with the given embeddings model
    ///
    /// By default, keeps top 20 documents. Use `with_k()` or `with_similarity_threshold()`
    /// to customize the filtering behavior.
    pub fn new(embeddings: Arc<dyn Embeddings>) -> Self {
        Self {
            embeddings,
            k: Some(20),
            similarity_threshold: None,
        }
    }

    /// Set the number of top documents to keep
    ///
    /// This will clear any `similarity_threshold` setting.
    #[must_use]
    pub fn with_k(mut self, k: usize) -> Self {
        self.k = Some(k);
        self.similarity_threshold = None;
        self
    }

    /// Set the minimum similarity threshold
    ///
    /// Only documents with similarity >= threshold will be kept.
    /// This will clear any k setting.
    #[must_use]
    pub fn with_similarity_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = Some(threshold);
        self.k = None;
        self
    }

    /// Validate that either k or `similarity_threshold` is set
    fn validate(&self) -> Result<()> {
        if self.k.is_none() && self.similarity_threshold.is_none() {
            return Err(Error::InvalidInput(
                "Must specify either k or similarity_threshold".to_string(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl DocumentCompressor for EmbeddingsFilter {
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        self.validate()?;

        if documents.is_empty() {
            return Ok(vec![]);
        }

        // Embed query using graph API
        let query_embedding = embed_query(Arc::clone(&self.embeddings), query).await?;

        // Embed documents using graph API
        let doc_texts: Vec<String> = documents.iter().map(|d| d.page_content.clone()).collect();
        let doc_embeddings = embed(Arc::clone(&self.embeddings), &doc_texts).await?;

        // Calculate similarities
        let similarities = cosine_similarity_batch(&query_embedding, &doc_embeddings);

        // Create indices with scores
        let mut indexed_docs: Vec<(usize, f32, &Document)> = documents
            .iter()
            .enumerate()
            .map(|(i, doc)| (i, similarities[i], doc))
            .collect();

        // Sort by similarity (descending)
        indexed_docs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Filter based on k or threshold
        let filtered_docs: Vec<Document> = if let Some(k) = self.k {
            // Keep top k
            indexed_docs
                .into_iter()
                .take(k)
                .map(|(_, score, doc)| {
                    let mut new_doc = doc.clone();
                    new_doc.metadata.insert(
                        "query_similarity_score".to_string(),
                        serde_json::json!(score),
                    );
                    new_doc
                })
                .collect()
        } else if let Some(threshold) = self.similarity_threshold {
            // Keep above threshold
            indexed_docs
                .into_iter()
                .filter(|(_, score, _)| *score >= threshold)
                .map(|(_, score, doc)| {
                    let mut new_doc = doc.clone();
                    new_doc.metadata.insert(
                        "query_similarity_score".to_string(),
                        serde_json::json!(score),
                    );
                    new_doc
                })
                .collect()
        } else {
            vec![]
        };

        Ok(filtered_docs)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use dashflow::core::documents::Document;
    use std::sync::Arc;

    // ============================================================
    // COSINE SIMILARITY TESTS
    // ============================================================

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let a = vec![1.0, 1.0];
        let b = vec![1.0, 1.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_partial() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 1.0];
        // Expected: 1.0 / sqrt(2) ≈ 0.707
        let result = cosine_similarity(&a, &b);
        assert!(result > 0.7 && result < 0.72);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0];
        // Should return 0.0 when vectors have different lengths
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        // Should return 0.0 when one vector is zero
        assert_eq!(cosine_similarity(&a, &b), 0.0);

        let a = vec![1.0, 2.0, 3.0];
        let b = vec![0.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);

        let a = vec![0.0, 0.0, 0.0];
        let b = vec![0.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_empty_vectors() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_single_element() {
        let a = vec![5.0];
        let b = vec![3.0];
        // Same direction, should be 1.0
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let a = vec![5.0];
        let b = vec![-3.0];
        // Opposite direction, should be -1.0
        assert!((cosine_similarity(&a, &b) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_large_values() {
        let a = vec![1e10, 1e10, 1e10];
        let b = vec![1e10, 1e10, 1e10];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_small_values() {
        let a = vec![1e-10, 1e-10, 1e-10];
        let b = vec![1e-10, 1e-10, 1e-10];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-5);
    }

    // ============================================================
    // COSINE SIMILARITY BATCH TESTS
    // ============================================================

    #[test]
    fn test_cosine_similarity_batch_basic() {
        let query = vec![1.0, 0.0, 0.0];
        let docs = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.5, 0.5, 0.0],
        ];

        let similarities = cosine_similarity_batch(&query, &docs);
        assert_eq!(similarities.len(), 3);
        assert!((similarities[0] - 1.0).abs() < 1e-6); // Identical
        assert!((similarities[1] - 0.0).abs() < 1e-6); // Orthogonal
        assert!(similarities[2] > 0.0 && similarities[2] < 1.0); // Partial similarity
    }

    #[test]
    fn test_cosine_similarity_batch_empty_docs() {
        let query = vec![1.0, 0.0, 0.0];
        let docs: Vec<Vec<f32>> = vec![];

        let similarities = cosine_similarity_batch(&query, &docs);
        assert!(similarities.is_empty());
    }

    #[test]
    fn test_cosine_similarity_batch_single_doc() {
        let query = vec![1.0, 1.0];
        let docs = vec![vec![1.0, 1.0]];

        let similarities = cosine_similarity_batch(&query, &docs);
        assert_eq!(similarities.len(), 1);
        assert!((similarities[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_batch_many_docs() {
        let query = vec![1.0, 0.0];
        let docs: Vec<Vec<f32>> = (0..100).map(|i| vec![1.0, i as f32]).collect();

        let similarities = cosine_similarity_batch(&query, &docs);
        assert_eq!(similarities.len(), 100);
        // First one should be highest (exactly [1, 0])
        assert!((similarities[0] - 1.0).abs() < 1e-6);
        // As i increases, similarity should decrease
        for i in 1..100 {
            assert!(similarities[i] < similarities[i - 1] || i == 1);
        }
    }

    // ============================================================
    // EMBEDDINGS FILTER BUILDER TESTS
    // ============================================================

    // Mock embeddings for testing
    struct MockEmbeddings {
        dimension: usize,
    }

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            // Generate deterministic embeddings based on text hash
            let mut embeddings = Vec::new();
            for text in texts {
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

    #[test]
    fn test_embeddings_filter_new() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings);

        // Default k should be 20
        assert_eq!(filter.k, Some(20));
        assert!(filter.similarity_threshold.is_none());
    }

    #[test]
    fn test_embeddings_filter_with_k() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings).with_k(5);

        assert_eq!(filter.k, Some(5));
        assert!(filter.similarity_threshold.is_none());
    }

    #[test]
    fn test_embeddings_filter_with_similarity_threshold() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings).with_similarity_threshold(0.8);

        assert!(filter.k.is_none());
        assert_eq!(filter.similarity_threshold, Some(0.8));
    }

    #[test]
    fn test_embeddings_filter_k_clears_threshold() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings)
            .with_similarity_threshold(0.8)
            .with_k(10);

        assert_eq!(filter.k, Some(10));
        assert!(filter.similarity_threshold.is_none());
    }

    #[test]
    fn test_embeddings_filter_threshold_clears_k() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings)
            .with_k(10)
            .with_similarity_threshold(0.5);

        assert!(filter.k.is_none());
        assert_eq!(filter.similarity_threshold, Some(0.5));
    }

    // ============================================================
    // EMBEDDINGS FILTER VALIDATION TESTS
    // ============================================================

    #[test]
    fn test_validate_with_k_passes() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings).with_k(5);
        assert!(filter.validate().is_ok());
    }

    #[test]
    fn test_validate_with_threshold_passes() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings).with_similarity_threshold(0.8);
        assert!(filter.validate().is_ok());
    }

    #[test]
    fn test_validate_with_neither_fails() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let mut filter = EmbeddingsFilter::new(embeddings);
        filter.k = None;
        filter.similarity_threshold = None;

        let result = filter.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Must specify"));
    }

    // ============================================================
    // DOCUMENT COMPRESSOR TESTS
    // ============================================================

    #[tokio::test]
    async fn test_compress_documents_empty() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings);

        let docs: Vec<Document> = vec![];
        let result = filter.compress_documents(docs, "test query", None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_compress_documents_single() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings).with_k(5);

        let docs = vec![Document::new("test document")];
        let result = filter.compress_documents(docs, "test query", None).await;

        assert!(result.is_ok());
        let filtered = result.unwrap();
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0]
            .metadata
            .contains_key("query_similarity_score"));
    }

    #[tokio::test]
    async fn test_compress_documents_with_k() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings).with_k(2);

        let docs = vec![
            Document::new("short"),
            Document::new("medium length"),
            Document::new("a longer document here"),
            Document::new("the longest document of them all"),
        ];

        let result = filter.compress_documents(docs, "test", None).await;
        assert!(result.is_ok());
        let filtered = result.unwrap();
        assert_eq!(filtered.len(), 2); // Only top 2
    }

    #[tokio::test]
    async fn test_compress_documents_k_greater_than_docs() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings).with_k(100);

        let docs = vec![
            Document::new("doc 1"),
            Document::new("doc 2"),
            Document::new("doc 3"),
        ];

        let result = filter.compress_documents(docs, "test", None).await;
        assert!(result.is_ok());
        let filtered = result.unwrap();
        assert_eq!(filtered.len(), 3); // All docs returned since k > len
    }

    #[tokio::test]
    async fn test_compress_documents_with_threshold_all_pass() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        // Very low threshold - all should pass
        let filter = EmbeddingsFilter::new(embeddings).with_similarity_threshold(0.0);

        let docs = vec![
            Document::new("doc 1"),
            Document::new("doc 2"),
            Document::new("doc 3"),
        ];

        let result = filter.compress_documents(docs, "test", None).await;
        assert!(result.is_ok());
        let filtered = result.unwrap();
        assert_eq!(filtered.len(), 3);
    }

    #[tokio::test]
    async fn test_compress_documents_with_threshold_none_pass() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        // Very high threshold - none should pass (mock embeddings won't produce 1.0)
        let filter = EmbeddingsFilter::new(embeddings).with_similarity_threshold(1.0);

        let docs = vec![
            Document::new("doc 1"),
            Document::new("doc 2"),
            Document::new("doc 3"),
        ];

        let result = filter.compress_documents(docs, "different query", None).await;
        assert!(result.is_ok());
        let filtered = result.unwrap();
        // With threshold 1.0, only identical embeddings would pass
        // Mock embeddings based on text length, so very few if any will match
        assert!(filtered.len() <= 3);
    }

    #[tokio::test]
    async fn test_compress_documents_adds_score_metadata() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings).with_k(5);

        let docs = vec![Document::new("test document")];
        let result = filter.compress_documents(docs, "test query", None).await;

        assert!(result.is_ok());
        let filtered = result.unwrap();
        assert_eq!(filtered.len(), 1);

        // Check that score was added
        let score = filtered[0].metadata.get("query_similarity_score");
        assert!(score.is_some());
        let score_val = score.unwrap().as_f64();
        assert!(score_val.is_some());
        assert!(score_val.unwrap() >= -1.0 && score_val.unwrap() <= 1.0);
    }

    #[tokio::test]
    async fn test_compress_documents_sorted_by_similarity() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings).with_k(10);

        // Create documents with different content lengths
        let docs = vec![
            Document::new("a"),
            Document::new("ab"),
            Document::new("abc"),
            Document::new("abcd"),
            Document::new("abcde"),
        ];

        let result = filter.compress_documents(docs, "abc", None).await;
        assert!(result.is_ok());
        let filtered = result.unwrap();

        // Results should be sorted by similarity (descending)
        for i in 1..filtered.len() {
            let prev_score = filtered[i - 1]
                .metadata
                .get("query_similarity_score")
                .unwrap()
                .as_f64()
                .unwrap();
            let curr_score = filtered[i]
                .metadata
                .get("query_similarity_score")
                .unwrap()
                .as_f64()
                .unwrap();
            assert!(prev_score >= curr_score);
        }
    }

    #[tokio::test]
    async fn test_compress_documents_preserves_original_metadata() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings).with_k(5);

        let mut doc = Document::new("test document");
        doc.metadata
            .insert("original_key".to_string(), serde_json::json!("original_value"));

        let docs = vec![doc];
        let result = filter.compress_documents(docs, "test", None).await;

        assert!(result.is_ok());
        let filtered = result.unwrap();
        assert_eq!(filtered.len(), 1);

        // Original metadata should still be there
        assert_eq!(
            filtered[0].metadata.get("original_key"),
            Some(&serde_json::json!("original_value"))
        );
        // New score should also be there
        assert!(filtered[0]
            .metadata
            .contains_key("query_similarity_score"));
    }

    #[tokio::test]
    async fn test_compress_documents_with_k_zero() {
        let embeddings = Arc::new(MockEmbeddings { dimension: 128 });
        let filter = EmbeddingsFilter::new(embeddings).with_k(0);

        let docs = vec![Document::new("doc 1"), Document::new("doc 2")];
        let result = filter.compress_documents(docs, "test", None).await;

        assert!(result.is_ok());
        let filtered = result.unwrap();
        assert!(filtered.is_empty()); // k=0 means take 0 documents
    }
}
