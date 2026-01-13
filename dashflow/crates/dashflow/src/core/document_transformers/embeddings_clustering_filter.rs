//! Embeddings clustering filter transformer.
//!
//! This transformer performs K-means clustering on document embeddings and returns
//! the documents closest to each cluster center. This is useful for selecting
//! representative documents from a large corpus.

use crate::core::document_transformers::DocumentTransformer;
use crate::core::documents::Document;
use crate::core::embeddings::Embeddings;
use crate::core::error::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// Perform K-means clustering on document vectors.
///
/// This transformer:
/// 1. Embeds all documents using the provided embeddings model
/// 2. Performs K-means clustering on the embeddings
/// 3. For each cluster, selects the N closest documents to the cluster center
/// 4. Returns the selected documents
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::document_transformers::{EmbeddingsClusteringFilter, DocumentTransformer};
/// use dashflow::core::documents::Document;
/// use dashflow_openai::OpenAIEmbeddings;
///
/// let embeddings = OpenAIEmbeddings::new();
/// let filter = EmbeddingsClusteringFilter::new(Arc::new(embeddings))
///     .with_num_clusters(3)
///     .with_num_closest(1)
///     .with_sorted(true);
///
/// let docs = vec![
///     Document::new("Cat document 1"),
///     Document::new("Cat document 2"),
///     Document::new("Dog document 1"),
///     Document::new("Dog document 2"),
///     Document::new("Bird document 1"),
///     Document::new("Bird document 2"),
/// ];
///
/// let filtered = filter.atransform_documents(docs).await?;
/// // Result: 3 documents (1 from each cluster)
/// ```
///
/// # Python Baseline
///
/// Python: `dashflow_community/document_transformers/embeddings_redundant_filter.py:174-226`
pub struct EmbeddingsClusteringFilter {
    /// Embeddings model to use for embedding document contents
    pub embeddings: Arc<dyn Embeddings>,
    /// Number of clusters (default: 5)
    pub num_clusters: usize,
    /// Number of closest documents to return per cluster (default: 1)
    pub num_closest: usize,
    /// Random seed for cluster initialization (default: 42)
    pub random_state: u64,
    /// Whether to sort results by original position (default: false)
    ///
    /// If false, results are grouped by cluster.
    /// If true, results are sorted by original document position.
    pub sorted: bool,
    /// Whether to remove duplicate selections (default: false)
    ///
    /// If false, duplicates are skipped and replaced by next closest.
    /// If true, no replacement is done (may reduce results).
    pub remove_duplicates: bool,
}

impl EmbeddingsClusteringFilter {
    /// Create a new `EmbeddingsClusteringFilter`.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - The embeddings model to use
    pub fn new(embeddings: Arc<dyn Embeddings>) -> Self {
        Self {
            embeddings,
            num_clusters: 5,
            num_closest: 1,
            random_state: 42,
            sorted: false,
            remove_duplicates: false,
        }
    }

    /// Set the number of clusters.
    #[must_use]
    pub fn with_num_clusters(mut self, num_clusters: usize) -> Self {
        self.num_clusters = num_clusters;
        self
    }

    /// Set the number of closest documents per cluster.
    #[must_use]
    pub fn with_num_closest(mut self, num_closest: usize) -> Self {
        self.num_closest = num_closest;
        self
    }

    /// Set the random seed for cluster initialization.
    #[must_use]
    pub fn with_random_state(mut self, random_state: u64) -> Self {
        self.random_state = random_state;
        self
    }

    /// Set whether to sort results by original position.
    #[must_use]
    pub fn with_sorted(mut self, sorted: bool) -> Self {
        self.sorted = sorted;
        self
    }

    /// Set whether to remove duplicate selections.
    #[must_use]
    pub fn with_remove_duplicates(mut self, remove_duplicates: bool) -> Self {
        self.remove_duplicates = remove_duplicates;
        self
    }

    /// Calculate Euclidean distance between two vectors.
    fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// Simple K-means clustering implementation.
    ///
    /// Returns cluster centers and assignments.
    fn kmeans_clustering(
        embeddings: &[Vec<f32>],
        num_clusters: usize,
        random_state: u64,
        max_iterations: usize,
    ) -> (Vec<Vec<f32>>, Vec<usize>) {
        if embeddings.is_empty() {
            return (Vec::new(), Vec::new());
        }

        let n_samples = embeddings.len();
        let n_features = embeddings[0].len();
        let k = num_clusters.min(n_samples);

        // Initialize cluster centers using random selection (simplified k-means++)
        use rand::Rng;
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(random_state);

        let mut centers = Vec::new();
        let mut selected_indices = std::collections::HashSet::new();

        for _ in 0..k {
            loop {
                let idx = rng.gen_range(0..n_samples);
                if !selected_indices.contains(&idx) {
                    selected_indices.insert(idx);
                    centers.push(embeddings[idx].clone());
                    break;
                }
            }
        }

        let mut assignments = vec![0; n_samples];

        // K-means iterations
        for _ in 0..max_iterations {
            let mut changed = false;

            // Assign each point to nearest center
            for (i, embedding) in embeddings.iter().enumerate() {
                let mut min_dist = f32::INFINITY;
                let mut best_cluster = 0;

                for (cluster_idx, center) in centers.iter().enumerate() {
                    let dist = Self::euclidean_distance(embedding, center);
                    if dist < min_dist {
                        min_dist = dist;
                        best_cluster = cluster_idx;
                    }
                }

                if assignments[i] != best_cluster {
                    assignments[i] = best_cluster;
                    changed = true;
                }
            }

            if !changed {
                break;
            }

            // Update cluster centers
            for (cluster_idx, center) in centers.iter_mut().enumerate() {
                let cluster_points: Vec<&Vec<f32>> = embeddings
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| assignments[*i] == cluster_idx)
                    .map(|(_, e)| e)
                    .collect();

                if !cluster_points.is_empty() {
                    let mut new_center = vec![0.0; n_features];
                    for point in &cluster_points {
                        for (j, val) in point.iter().enumerate() {
                            new_center[j] += val;
                        }
                    }
                    for val in &mut new_center {
                        *val /= cluster_points.len() as f32;
                    }
                    *center = new_center;
                }
            }
        }

        (centers, assignments)
    }

    /// Filter documents by clustering.
    ///
    /// # Python Algorithm (from Python baseline)
    ///
    /// ```python
    /// def _filter_cluster_embeddings(
    ///     embedded_documents: List[List[float]],
    ///     num_clusters: int,
    ///     num_closest: int,
    ///     random_state: int,
    ///     remove_duplicates: bool,
    /// ) -> List[int]:
    ///     kmeans = KMeans(n_clusters=num_clusters, random_state=random_state).fit(
    ///         embedded_documents
    ///     )
    ///     closest_indices = []
    ///
    ///     for i in range(num_clusters):
    ///         distances = np.linalg.norm(
    ///             embedded_documents - kmeans.cluster_centers_[i], axis=1
    ///         )
    ///
    ///         if remove_duplicates:
    ///             closest_indices_sorted = [
    ///                 x for x in np.argsort(distances)[:num_closest]
    ///                 if x not in closest_indices
    ///             ]
    ///         else:
    ///             closest_indices_sorted = [
    ///                 x for x in np.argsort(distances) if x not in closest_indices
    ///             ][:num_closest]
    ///
    ///         closest_indices.extend(closest_indices_sorted)
    ///
    ///     return closest_indices
    /// ```
    fn filter_cluster_embeddings(
        embeddings: &[Vec<f32>],
        num_clusters: usize,
        num_closest: usize,
        random_state: u64,
        remove_duplicates: bool,
    ) -> Vec<usize> {
        if embeddings.is_empty() {
            return Vec::new();
        }

        // Perform K-means clustering
        let (centers, _assignments) =
            Self::kmeans_clustering(embeddings, num_clusters, random_state, 100);

        let mut closest_indices = Vec::new();
        let mut used_indices = std::collections::HashSet::new();

        // For each cluster center, find the closest documents
        for center in &centers {
            // Calculate distances from all documents to this center
            let mut distances: Vec<(usize, f32)> = embeddings
                .iter()
                .enumerate()
                .map(|(idx, embedding)| (idx, Self::euclidean_distance(embedding, center)))
                .collect();

            // Sort by distance (ascending)
            distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            if remove_duplicates {
                // Only add documents not already in the result
                let selected: Vec<usize> = distances
                    .iter()
                    .map(|(idx, _)| *idx)
                    .filter(|idx| !closest_indices.contains(idx))
                    .take(num_closest)
                    .collect();
                closest_indices.extend(selected);
            } else {
                // Skip duplicates and replace with next closest
                let selected: Vec<usize> = distances
                    .iter()
                    .map(|(idx, _)| *idx)
                    .filter(|idx| !used_indices.contains(idx))
                    .take(num_closest)
                    .collect();

                for idx in &selected {
                    used_indices.insert(*idx);
                }
                closest_indices.extend(selected);
            }
        }

        closest_indices
    }
}

#[async_trait]
impl DocumentTransformer for EmbeddingsClusteringFilter {
    fn transform_documents(&self, _documents: Vec<Document>) -> Result<Vec<Document>> {
        // Synchronous version not supported - embeddings are async
        Err(crate::core::error::Error::InvalidInput(
            "EmbeddingsClusteringFilter requires async operation. Use atransform_documents instead."
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

        // Filter using clustering
        let mut included_idxs = Self::filter_cluster_embeddings(
            &embedded_documents,
            self.num_clusters,
            self.num_closest,
            self.random_state,
            self.remove_duplicates,
        );

        // Sort if requested
        if self.sorted {
            included_idxs.sort_unstable();
        }

        // Return documents at included indices
        let result = included_idxs
            .into_iter()
            .filter_map(|i| documents.get(i).cloned())
            .collect();

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::core::embeddings::MockEmbeddings;
    use crate::test_prelude::*;

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let dist = EmbeddingsClusteringFilter::euclidean_distance(&a, &b);
        assert!((dist - 1.0).abs() < 0.001);

        let a = vec![0.0, 0.0, 0.0];
        let b = vec![3.0, 4.0, 0.0];
        let dist = EmbeddingsClusteringFilter::euclidean_distance(&a, &b);
        assert!((dist - 5.0).abs() < 0.001); // 3-4-5 triangle
    }

    #[tokio::test]
    async fn test_embeddings_clustering_filter_basic() {
        let mock = MockEmbeddings::new(384);
        let filter = EmbeddingsClusteringFilter::new(Arc::new(mock))
            .with_num_clusters(2)
            .with_num_closest(1);

        let docs = vec![
            Document::new("text1"),
            Document::new("text2"),
            Document::new("text3"),
            Document::new("text4"),
        ];

        let result = filter.atransform_documents(docs).await.unwrap();
        // Should return 2 documents (1 from each cluster)
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn test_embeddings_clustering_filter_empty() {
        let mock = MockEmbeddings::new(384);
        let filter = EmbeddingsClusteringFilter::new(Arc::new(mock));

        let docs: Vec<Document> = vec![];
        let result = filter.atransform_documents(docs).await.unwrap();
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_embeddings_clustering_filter_single() {
        let mock = MockEmbeddings::new(384);
        let filter = EmbeddingsClusteringFilter::new(Arc::new(mock))
            .with_num_clusters(1)
            .with_num_closest(1);

        let docs = vec![Document::new("only")];
        let result = filter.atransform_documents(docs).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_embeddings_clustering_filter_sorted() {
        let mock = MockEmbeddings::new(384);
        let filter = EmbeddingsClusteringFilter::new(Arc::new(mock))
            .with_num_clusters(2)
            .with_num_closest(1)
            .with_sorted(true);

        let docs = vec![
            Document::new("text1").with_metadata("pos", 0),
            Document::new("text2").with_metadata("pos", 1),
            Document::new("text3").with_metadata("pos", 2),
            Document::new("text4").with_metadata("pos", 3),
        ];

        let result = filter.atransform_documents(docs).await.unwrap();
        // Should be sorted by original position
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn test_embeddings_clustering_filter_preserves_metadata() {
        let mock = MockEmbeddings::new(384);
        let filter = EmbeddingsClusteringFilter::new(Arc::new(mock))
            .with_num_clusters(1)
            .with_num_closest(2);

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
    fn test_kmeans_clustering() {
        // Create simple embeddings for testing
        let embeddings = vec![
            vec![0.0, 0.0],   // Cluster 1
            vec![0.1, 0.1],   // Cluster 1
            vec![10.0, 10.0], // Cluster 2
            vec![10.1, 10.1], // Cluster 2
        ];

        let (centers, assignments) =
            EmbeddingsClusteringFilter::kmeans_clustering(&embeddings, 2, 42, 100);

        assert_eq!(centers.len(), 2);
        assert_eq!(assignments.len(), 4);

        // Check that similar points are assigned to same cluster
        assert_eq!(assignments[0], assignments[1]);
        assert_eq!(assignments[2], assignments[3]);
        assert_ne!(assignments[0], assignments[2]);
    }

    #[test]
    fn test_sync_not_supported() {
        let mock = MockEmbeddings::new(384);
        let filter = EmbeddingsClusteringFilter::new(Arc::new(mock));

        let docs = vec![Document::new("test")];
        let result = filter.transform_documents(docs);
        assert!(result.is_err());
    }
}
