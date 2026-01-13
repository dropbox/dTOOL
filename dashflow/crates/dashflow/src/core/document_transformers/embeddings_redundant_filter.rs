//! Embeddings redundant filter transformer.
//!
//! This transformer filters out redundant documents by comparing their embeddings.
//! Documents with embeddings above a similarity threshold are considered redundant,
//! and only one from each redundant pair is kept.

use crate::core::document_transformers::DocumentTransformer;
use crate::core::documents::Document;
use crate::core::embeddings::Embeddings;
use crate::core::error::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// Filter that drops redundant documents by comparing their embeddings.
///
/// This transformer:
/// 1. Embeds all documents using the provided embeddings model
/// 2. Computes pairwise similarity between all document embeddings
/// 3. Identifies pairs with similarity above the threshold
/// 4. Removes one document from each redundant pair (keeps the first)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::document_transformers::{EmbeddingsRedundantFilter, DocumentTransformer};
/// use dashflow::core::documents::Document;
/// use dashflow_openai::OpenAIEmbeddings;
///
/// let embeddings = OpenAIEmbeddings::new();
/// let filter = EmbeddingsRedundantFilter::new(Arc::new(embeddings))
///     .with_similarity_threshold(0.95);
///
/// let docs = vec![
///     Document::new("The cat sat on the mat"),
///     Document::new("A cat was sitting on a mat"), // Similar to first
///     Document::new("The dog ran in the park"), // Different
/// ];
///
/// let filtered = filter.transform_documents(docs).await?;
/// // Result: 2 documents (duplicate removed)
/// ```
///
/// # Python Baseline
///
/// Python: `dashflow_community/document_transformers/embeddings_redundant_filter.py`
pub struct EmbeddingsRedundantFilter {
    /// Embeddings model to use for embedding document contents
    pub embeddings: Arc<dyn Embeddings>,
    /// Similarity threshold for determining redundancy (default: 0.95)
    ///
    /// Documents with similarity above this threshold are considered redundant.
    /// Range: 0.0 to 1.0 (0.0 = completely different, 1.0 = identical)
    pub similarity_threshold: f32,
}

impl EmbeddingsRedundantFilter {
    /// Create a new `EmbeddingsRedundantFilter`.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - The embeddings model to use
    pub fn new(embeddings: Arc<dyn Embeddings>) -> Self {
        Self {
            embeddings,
            similarity_threshold: 0.95,
        }
    }

    /// Set the similarity threshold.
    ///
    /// Documents with similarity above this threshold are considered redundant.
    #[must_use]
    pub fn with_similarity_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = threshold;
        self
    }

    /// Calculate cosine similarity between two embedding vectors.
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

    /// Compute similarity matrix (lower triangular only).
    ///
    /// Returns a vector of (i, j, similarity) tuples where i < j.
    fn compute_similarity_matrix(embeddings: &[Vec<f32>]) -> Vec<(usize, usize, f32)> {
        let mut similarities = Vec::new();

        for i in 0..embeddings.len() {
            for j in 0..i {
                // Only compute lower triangle (j < i)
                let sim = Self::cosine_similarity(&embeddings[i], &embeddings[j]);
                similarities.push((i, j, sim));
            }
        }

        similarities
    }

    /// Filter redundant embeddings based on similarity threshold.
    ///
    /// # Python Algorithm (from Python baseline)
    ///
    /// ```python
    /// def _filter_similar_embeddings(
    ///     embedded_documents: List[List[float]], similarity_fn: Callable, threshold: float
    /// ) -> List[int]:
    ///     similarity = np.tril(similarity_fn(embedded_documents, embedded_documents), k=-1)
    ///     redundant = np.where(similarity > threshold)
    ///     redundant_stacked = np.column_stack(redundant)
    ///     redundant_sorted = np.argsort(similarity[redundant])[::-1]
    ///     included_idxs = set(range(len(embedded_documents)))
    ///     for first_idx, second_idx in redundant_stacked[redundant_sorted]:
    ///         if first_idx in included_idxs and second_idx in included_idxs:
    ///             # Default to dropping the second document of any highly similar pair.
    ///             included_idxs.remove(second_idx)
    ///     return list(sorted(included_idxs))
    /// ```
    fn filter_similar_embeddings(embeddings: &[Vec<f32>], threshold: f32) -> Vec<usize> {
        // Compute similarity matrix (lower triangular)
        let similarities = Self::compute_similarity_matrix(embeddings);

        // Find redundant pairs (similarity > threshold)
        let mut redundant: Vec<(usize, usize, f32)> = similarities
            .into_iter()
            .filter(|(_, _, sim)| *sim > threshold)
            .collect();

        // Sort by similarity (highest first)
        redundant.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        // Keep track of which indices to include
        let mut included: std::collections::HashSet<usize> = (0..embeddings.len()).collect();

        // For each redundant pair, remove the document with larger index
        // (keep the first document that appeared)
        for (first_idx, second_idx, _) in redundant {
            if included.contains(&first_idx) && included.contains(&second_idx) {
                // first_idx > second_idx (from lower triangular matrix)
                // Remove first_idx to keep the document that appeared earlier
                included.remove(&first_idx);
            }
        }

        // Return sorted indices
        let mut result: Vec<usize> = included.into_iter().collect();
        result.sort_unstable();
        result
    }
}

#[async_trait]
impl DocumentTransformer for EmbeddingsRedundantFilter {
    fn transform_documents(&self, _documents: Vec<Document>) -> Result<Vec<Document>> {
        // Synchronous version not supported - embeddings are async
        Err(crate::core::error::Error::InvalidInput(
            "EmbeddingsRedundantFilter requires async operation. Use atransform_documents instead."
                .to_string(),
        ))
    }

    async fn atransform_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
        if documents.is_empty() {
            return Ok(documents);
        }

        // Extract text from all documents
        let texts: Vec<String> = documents.iter().map(|d| d.page_content.clone()).collect();

        // Embed all documents
        let embedded_documents = self
            .embeddings
            ._embed_documents(&texts)
            .await?;

        // Filter similar embeddings
        let included_idxs =
            Self::filter_similar_embeddings(&embedded_documents, self.similarity_threshold);

        // Return documents at included indices
        let result = included_idxs
            .into_iter()
            .map(|i| documents[i].clone())
            .collect();

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::embeddings::MockEmbeddings;
    use crate::test_prelude::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = EmbeddingsRedundantFilter::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001); // Identical vectors

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = EmbeddingsRedundantFilter::cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001); // Orthogonal vectors

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = EmbeddingsRedundantFilter::cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 0.001); // Opposite vectors
    }

    #[tokio::test]
    async fn test_embeddings_redundant_filter_basic() {
        // Create mock embeddings that return same embeddings for similar text
        let mock = MockEmbeddings::new(384);
        let filter = EmbeddingsRedundantFilter::new(Arc::new(mock)).with_similarity_threshold(0.95);

        let docs = vec![
            Document::new("text1"),
            Document::new("text2"),
            Document::new("text3"),
        ];

        let result = filter.atransform_documents(docs).await.unwrap();
        // With mock embeddings, all should pass through (no high similarity)
        assert_eq!(result.len(), 3);
    }

    #[tokio::test]
    async fn test_embeddings_redundant_filter_empty() {
        let mock = MockEmbeddings::new(384);
        let filter = EmbeddingsRedundantFilter::new(Arc::new(mock));

        let docs: Vec<Document> = vec![];
        let result = filter.atransform_documents(docs).await.unwrap();
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_embeddings_redundant_filter_single() {
        let mock = MockEmbeddings::new(384);
        let filter = EmbeddingsRedundantFilter::new(Arc::new(mock));

        let docs = vec![Document::new("only")];
        let result = filter.atransform_documents(docs).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_embeddings_redundant_filter_preserves_metadata() {
        let mock = MockEmbeddings::new(384);
        let filter = EmbeddingsRedundantFilter::new(Arc::new(mock));

        let docs = vec![
            Document::new("doc1").with_metadata("id", 1).with_id("1"),
            Document::new("doc2").with_metadata("id", 2).with_id("2"),
        ];

        let result = filter.atransform_documents(docs).await.unwrap();
        // Verify metadata is preserved
        for doc in result {
            assert!(doc.metadata.contains_key("id"));
            assert!(doc.id.is_some());
        }
    }

    #[test]
    fn test_filter_similar_embeddings() {
        // Create embeddings where [0] and [1] are identical (similarity=1.0), [2] is different
        let embeddings = vec![
            vec![1.0, 0.0, 0.0], // [0]
            vec![1.0, 0.0, 0.0], // [1] - identical to [0]
            vec![0.0, 0.0, 1.0], // [2] - orthogonal (different)
        ];

        let included = EmbeddingsRedundantFilter::filter_similar_embeddings(&embeddings, 0.95);

        // Should include [0] and [2], exclude [1] (duplicate of [0])
        assert_eq!(included.len(), 2);
        assert!(included.contains(&0));
        assert!(included.contains(&2));
        assert!(!included.contains(&1));
    }

    #[test]
    fn test_sync_not_supported() {
        let mock = MockEmbeddings::new(384);
        let filter = EmbeddingsRedundantFilter::new(Arc::new(mock));

        let docs = vec![Document::new("test")];
        let result = filter.transform_documents(docs);
        assert!(result.is_err());
    }
}
