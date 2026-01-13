// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! K-nearest neighbors retrieval for DashOptimize.
//!
//! This module provides the `KNN` struct for retrieving the k most similar examples
//! from a training set based on embedding similarity.
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::optimize::knn::KNN;
//! use dashflow::optimize::Example;
//! use dashflow_openai::embeddings::OpenAIEmbeddings;
//! use dashflow::core::embeddings::Embeddings;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create training dataset
//! let trainset = vec![
//!     Example::from([("question", "What is 2+2?"), ("answer", "4")]),
//!     Example::from([("question", "What is the capital of France?"), ("answer", "Paris")]),
//!     Example::from([("question", "What is 10-3?"), ("answer", "7")]),
//! ];
//!
//! // Initialize KNN with embedding model
//! let embedder = Arc::new(OpenAIEmbeddings::new().with_model("text-embedding-3-small"));
//!
//! let knn = KNN::new(2, trainset, embedder).await?;
//!
//! // Find similar examples
//! let query = Example::from([("question", "What is 5+5?")]);
//! let similar = knn.retrieve(&query).await?;
//!
//! // similar[0] will be the math question (highest similarity)
//! assert_eq!(similar.len(), 2);
//! # Ok(())
//! # }
//! ```

use crate::core::embeddings::Embeddings;
use crate::core::Error;
use crate::optimize::Example;
use std::sync::Arc;

/// A k-nearest neighbors retriever that finds similar examples from a training set.
///
/// KNN uses embedding-based cosine similarity to find the most relevant examples
/// for few-shot prompting. This is a fundamental component for retrieval-augmented
/// few-shot learning in DashOptimize.
///
/// # Architecture
///
/// The retriever pre-computes embeddings for all training examples during initialization.
/// At query time, it:
/// 1. Embeds the query inputs (concatenated as "key: value | key: value...")
/// 2. Computes cosine similarity between query and all training embeddings
/// 3. Returns the k examples with highest similarity scores
#[derive(Clone)]
pub struct KNN<E: Embeddings> {
    /// Number of nearest neighbors to retrieve
    k: usize,

    /// Training examples to search through
    trainset: Vec<Example>,

    /// Embedding client for vectorization
    embedder: Arc<E>,

    /// Pre-computed embeddings for training set (each row is one example's embedding)
    trainset_embeddings: Vec<Vec<f32>>,
}

impl<E: Embeddings> KNN<E> {
    /// Create a new KNN retriever.
    ///
    /// This will pre-compute embeddings for all training examples, which requires
    /// an API call to the embedding provider.
    ///
    /// # Arguments
    ///
    /// * `k` - Number of nearest neighbors to retrieve (must be > 0)
    /// * `trainset` - List of training examples to search through
    /// * `embedder` - The embedding client to use for vectorization
    ///
    /// # Returns
    ///
    /// A KNN retriever ready to find similar examples, or an error if:
    /// - `k == 0` (semantically meaningless)
    /// - Embedding fails
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidInput` if `k == 0`.
    pub async fn new(k: usize, trainset: Vec<Example>, embedder: Arc<E>) -> Result<Self, Error> {
        if k == 0 {
            return Err(Error::invalid_input(
                "k must be greater than 0 for KNN retrieval",
            ));
        }
        // Convert training examples to strings for embedding
        // Format: "key1: value1 | key2: value2 | ..."
        // Use only input fields (not outputs like "answer")
        let trainset_texts: Vec<String> = trainset
            .iter()
            .map(|example| {
                let parts: Vec<String> = example
                    .inputs()
                    .iter()
                    .map(|(key, value)| {
                        // Convert JSON value to string representation
                        let value_str = if let Some(s) = value.as_str() {
                            s.to_string()
                        } else {
                            value.to_string()
                        };
                        format!("{}: {}", key, value_str)
                    })
                    .collect();
                parts.join(" | ")
            })
            .collect();

        // Pre-compute embeddings for training set
        let trainset_embeddings = embedder
            ._embed_documents(&trainset_texts)
            .await
            .map_err(|e| {
                Error::other(format!(
                    "Failed to embed {} training examples for KNN: {}",
                    trainset_texts.len(),
                    e
                ))
            })?;

        Ok(Self {
            k,
            trainset,
            embedder,
            trainset_embeddings,
        })
    }

    /// Retrieve the k most similar examples from the training set.
    ///
    /// # Arguments
    ///
    /// * `query` - The query example containing input fields to match against
    ///
    /// # Returns
    ///
    /// A list of the k most similar training examples, ordered by similarity (highest first).
    pub async fn retrieve(&self, query: &Example) -> Result<Vec<Example>, Error> {
        // Convert query to string (same format as training set)
        let query_parts: Vec<String> = query
            .inputs()
            .iter()
            .map(|(key, value)| {
                let value_str = if let Some(s) = value.as_str() {
                    s.to_string()
                } else {
                    value.to_string()
                };
                format!("{}: {}", key, value_str)
            })
            .collect();
        let query_text = query_parts.join(" | ");

        // Embed the query
        let query_embedding = self
            .embedder
            ._embed_query(&query_text)
            .await
            .map_err(|e| {
                Error::other(format!("Failed to embed query for KNN retrieval: {}", e))
            })?;

        // Compute cosine similarity with all training examples
        let mut scores: Vec<(usize, f32)> = self
            .trainset_embeddings
            .iter()
            .enumerate()
            .map(|(idx, train_emb)| {
                let score = cosine_similarity(&query_embedding, train_emb);
                (idx, score)
            })
            .collect();

        // Sort by score (highest first) and take top k
        // Use unwrap_or(Equal) to handle NaN scores gracefully
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let top_k_indices: Vec<usize> = scores.iter().take(self.k).map(|(idx, _)| *idx).collect();

        // Return the corresponding training examples
        Ok(top_k_indices
            .into_iter()
            .map(|idx| self.trainset[idx].clone())
            .collect())
    }

    /// Get the number of neighbors this retriever will return.
    pub fn k(&self) -> usize {
        self.k
    }

    /// Get the number of examples in the training set.
    pub fn trainset_size(&self) -> usize {
        self.trainset.len()
    }
}

/// Compute cosine similarity between two vectors.
///
/// Cosine similarity = dot(a, b) / (||a|| * ||b||)
///
/// Returns a value between -1.0 and 1.0, where:
/// - 1.0 = identical direction
/// - 0.0 = orthogonal (or zero vector)
/// - -1.0 = opposite direction
///
/// # Arguments
///
/// * `a` - First vector
/// * `b` - Second vector (must have same length as `a`)
///
/// # Panics
///
/// In debug builds, panics if vectors have different lengths.
/// In release builds, returns 0.0 for mismatched vectors to avoid crashes.
///
/// # Note
///
/// This is a private function; callers must ensure vectors have the same
/// dimension (embeddings from the same model always have consistent dimensions).
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    // Debug assertion helps catch bugs during development
    debug_assert_eq!(a.len(), b.len(), "Vectors must have same length");

    // In release, gracefully handle dimension mismatch
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_partial() {
        let a = vec![1.0, 2.0];
        let b = vec![2.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        // cos(angle) = (1*2 + 2*1) / (sqrt(5) * sqrt(5)) = 4/5 = 0.8
        assert!((sim - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![0.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn test_cosine_similarity_dimension_mismatch() {
        // In release builds, mismatched dimensions return 0.0 gracefully.
        // In debug builds, this test is skipped because debug_assert_eq! panics.
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0]; // Different length
        let sim = cosine_similarity(&a, &b);
        // Should return 0.0 gracefully instead of panicking
        assert_eq!(sim, 0.0);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Vectors must have same length")]
    fn test_cosine_similarity_dimension_mismatch_debug() {
        // In debug builds, mismatched dimensions panic at debug_assert_eq.
        // This catches bugs during development.
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0]; // Different length
        let _ = cosine_similarity(&a, &b);
    }

    // Integration tests with mock embedder
    use crate::core::Error;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Mock embedding client for testing
    struct MockEmbedder {
        embeddings: Vec<Vec<f32>>,
        counter: Mutex<usize>,
    }

    impl MockEmbedder {
        fn new(embeddings: Vec<Vec<f32>>) -> Self {
            Self {
                embeddings,
                counter: Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl Embeddings for MockEmbedder {
        async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Error> {
            // Return pre-defined embeddings based on global counter
            let mut counter = self.counter.lock().unwrap();
            let mut result = Vec::new();
            for _ in texts {
                if *counter < self.embeddings.len() {
                    result.push(self.embeddings[*counter].clone());
                    *counter += 1;
                } else {
                    result.push(vec![0.0; self.embeddings[0].len()]);
                }
            }
            Ok(result)
        }

        async fn _embed_query(&self, _text: &str) -> Result<Vec<f32>, Error> {
            // Return next embedding from the list
            let mut counter = self.counter.lock().unwrap();
            if *counter < self.embeddings.len() {
                let embedding = self.embeddings[*counter].clone();
                *counter += 1;
                Ok(embedding)
            } else {
                Ok(vec![0.0; self.embeddings[0].len()])
            }
        }
    }

    #[tokio::test]
    async fn test_knn_creation() {
        let trainset = vec![
            Example::from([("input", "hello")]),
            Example::from([("input", "world")]),
        ];

        let embedder = Arc::new(MockEmbedder::new(vec![vec![1.0, 0.0], vec![0.0, 1.0]]));

        let knn = KNN::new(1, trainset, embedder).await.unwrap();
        assert_eq!(knn.k(), 1);
        assert_eq!(knn.trainset_size(), 2);
    }

    #[tokio::test]
    async fn test_knn_k_zero_validation() {
        let trainset = vec![
            Example::from([("input", "hello")]),
            Example::from([("input", "world")]),
        ];

        let embedder = Arc::new(MockEmbedder::new(vec![vec![1.0, 0.0], vec![0.0, 1.0]]));

        // k=0 should return an error
        let result = KNN::new(0, trainset, embedder).await;
        assert!(result.is_err(), "Expected error for k=0");
        let err = result.err().expect("Error should be present");
        assert!(
            err.to_string().contains("must be greater than 0"),
            "Expected validation error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_knn_retrieve() {
        let trainset = vec![
            Example::from([("input", "hello")]),
            Example::from([("input", "world")]),
            Example::from([("input", "foo")]),
        ];

        // Mock embeddings: [1,0] for "hello", [0,1] for "world", [0.5,0.5] for "foo", [0.9, 0.1] for query
        let embedder = Arc::new(MockEmbedder::new(vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![0.5, 0.5],
            vec![0.9, 0.1],
        ]));

        let knn = KNN::new(2, trainset, embedder).await.unwrap();

        // Query with embedding [0.9, 0.1] (closest to "hello")
        let query = Example::from([("input", "hi")]);
        let results = knn.retrieve(&query).await.unwrap();

        assert_eq!(results.len(), 2);
        // First result should be "hello" (most similar to [0.9, 0.1])
        assert_eq!(
            results[0].get("input").and_then(|v| v.as_str()),
            Some("hello")
        );
    }

    #[tokio::test]
    async fn test_knn_retrieve_all_k() {
        let trainset = vec![
            Example::from([("input", "a")]),
            Example::from([("input", "b")]),
            Example::from([("input", "c")]),
        ];

        let embedder = Arc::new(MockEmbedder::new(vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![0.5, 0.5],
            vec![1.0, 0.0],
        ]));

        let knn = KNN::new(3, trainset, embedder).await.unwrap();
        let query = Example::from([("input", "query")]);
        let results = knn.retrieve(&query).await.unwrap();

        // Should return all 3 training examples
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_knn_with_multiple_input_fields() {
        let trainset = vec![
            Example::from([("question", "What is AI?"), ("context", "Technology topic")]),
            Example::from([
                ("question", "What is ML?"),
                ("context", "Machine learning topic"),
            ]),
        ];

        // Mock embeddings: first 2 calls are for training set, 3rd is for query
        // Training: "AI" gets [1.0, 0.0], "ML" gets [0.0, 1.0]
        // Query: "deep learning" gets [0.1, 0.9] (closer to "ML" [0, 1])
        let embedder = Arc::new(MockEmbedder::new(vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![0.1, 0.9],
        ]));

        let knn = KNN::new(1, trainset, embedder).await.unwrap();

        // Query should concatenate both fields
        let query = Example::from([
            ("question", "What is deep learning?"),
            ("context", "ML concepts"),
        ]);
        let results = knn.retrieve(&query).await.unwrap();

        assert_eq!(results.len(), 1);
        // Should retrieve the ML example (embedding [0, 1] is closer to query [0.1, 0.9] than [1, 0])
        assert_eq!(
            results[0].get("question").and_then(|v| v.as_str()),
            Some("What is ML?")
        );
    }
}
