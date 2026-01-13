//! Retrieval modes for Qdrant vector store.
//!
//! Qdrant supports multiple types of vector retrieval, which can be combined
//! for hybrid search strategies.

use std::fmt;

/// Retrieval mode for Qdrant vector store searches.
///
/// Qdrant supports three types of vector retrieval:
///
/// - **Dense**: Traditional embedding-based similarity search using dense vectors
///   (e.g., from models like `OpenAI`'s `text-embedding-3-small` or sentence transformers).
///   This is the most common and well-supported mode.
///
/// - **Sparse**: Keyword-based retrieval using sparse vectors (e.g., BM25, SPLADE).
///   Sparse vectors have most dimensions set to zero and are efficient for
///   keyword matching and term-based search. **Note**: This mode requires
///   sparse embeddings support, which is planned for future implementation.
///
/// - **Hybrid**: Combines both dense and sparse vectors to leverage semantic
///   understanding (dense) and exact keyword matching (sparse). This often
///   produces the best results but requires both embedding types.
///   **Note**: This mode is planned for future implementation.
///
/// # Examples
///
/// ```
/// use dashflow_qdrant::RetrievalMode;
///
/// let mode = RetrievalMode::Dense;
/// assert_eq!(mode.to_string(), "dense");
/// assert!(mode.is_dense());
/// ```
///
/// # Python Baseline Compatibility
///
/// This enum corresponds to the Python `RetrievalMode` enum in
/// `dashflow_qdrant.qdrant.RetrievalMode`:
///
/// ```python
/// class RetrievalMode(str, Enum):
///     DENSE = "dense"
///     SPARSE = "sparse"
///     HYBRID = "hybrid"
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RetrievalMode {
    /// Dense vector retrieval using traditional embeddings.
    ///
    /// This mode uses dense vectors (typically 384-1536 dimensions) from
    /// embedding models like `OpenAI`, sentence transformers, or other dense
    /// embedding providers. Dense vectors capture semantic meaning and work
    /// well for similarity search based on meaning rather than exact keywords.
    ///
    /// **Status**: Fully implemented and supported.
    Dense,

    /// Sparse vector retrieval using keyword-based embeddings (e.g., BM25, SPLADE).
    ///
    /// Sparse vectors have most dimensions set to zero and are efficient for
    /// keyword matching. They work well for exact term matching and can be
    /// combined with dense vectors for hybrid search.
    ///
    /// **Status**: Planned for future implementation. Using this mode will
    /// currently return an error.
    ///
    /// # Future Implementation
    ///
    /// To use this mode in the future, you will need:
    /// - A sparse embeddings provider implementing `SparseEmbeddings` trait
    /// - Qdrant collection configured with named sparse vector support
    /// - Sparse vector indexing enabled
    Sparse,

    /// Hybrid retrieval combining dense and sparse vectors.
    ///
    /// This mode performs a fusion search that combines:
    /// - Dense vectors for semantic similarity
    /// - Sparse vectors for keyword matching
    ///
    /// Hybrid search often provides the best results by leveraging both
    /// semantic understanding and exact keyword matching. Results are
    /// typically combined using Reciprocal Rank Fusion (RRF) or similar
    /// algorithms.
    ///
    /// **Status**: Planned for future implementation. Using this mode will
    /// currently return an error.
    ///
    /// # Future Implementation
    ///
    /// To use this mode in the future, you will need:
    /// - Both dense and sparse embeddings providers
    /// - Qdrant collection configured with both vector types
    /// - Fusion search parameters (alpha, normalization strategy)
    Hybrid,
}

impl RetrievalMode {
    /// Returns `true` if this is the Dense mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_qdrant::RetrievalMode;
    ///
    /// assert!(RetrievalMode::Dense.is_dense());
    /// assert!(!RetrievalMode::Sparse.is_dense());
    /// assert!(!RetrievalMode::Hybrid.is_dense());
    /// ```
    #[must_use]
    pub fn is_dense(&self) -> bool {
        matches!(self, RetrievalMode::Dense)
    }

    /// Returns `true` if this is the Sparse mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_qdrant::RetrievalMode;
    ///
    /// assert!(!RetrievalMode::Dense.is_sparse());
    /// assert!(RetrievalMode::Sparse.is_sparse());
    /// assert!(!RetrievalMode::Hybrid.is_sparse());
    /// ```
    #[must_use]
    pub fn is_sparse(&self) -> bool {
        matches!(self, RetrievalMode::Sparse)
    }

    /// Returns `true` if this is the Hybrid mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_qdrant::RetrievalMode;
    ///
    /// assert!(!RetrievalMode::Dense.is_hybrid());
    /// assert!(!RetrievalMode::Sparse.is_hybrid());
    /// assert!(RetrievalMode::Hybrid.is_hybrid());
    /// ```
    #[must_use]
    pub fn is_hybrid(&self) -> bool {
        matches!(self, RetrievalMode::Hybrid)
    }

    /// Returns `true` if this mode requires dense embeddings.
    ///
    /// Dense embeddings are required for Dense and Hybrid modes.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_qdrant::RetrievalMode;
    ///
    /// assert!(RetrievalMode::Dense.requires_dense_embeddings());
    /// assert!(!RetrievalMode::Sparse.requires_dense_embeddings());
    /// assert!(RetrievalMode::Hybrid.requires_dense_embeddings());
    /// ```
    #[must_use]
    pub fn requires_dense_embeddings(&self) -> bool {
        matches!(self, RetrievalMode::Dense | RetrievalMode::Hybrid)
    }

    /// Returns `true` if this mode requires sparse embeddings.
    ///
    /// Sparse embeddings are required for Sparse and Hybrid modes.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_qdrant::RetrievalMode;
    ///
    /// assert!(!RetrievalMode::Dense.requires_sparse_embeddings());
    /// assert!(RetrievalMode::Sparse.requires_sparse_embeddings());
    /// assert!(RetrievalMode::Hybrid.requires_sparse_embeddings());
    /// ```
    #[must_use]
    pub fn requires_sparse_embeddings(&self) -> bool {
        matches!(self, RetrievalMode::Sparse | RetrievalMode::Hybrid)
    }

    /// Returns `true` if this mode is currently implemented.
    ///
    /// Only Dense mode is currently implemented. Sparse and Hybrid modes
    /// are planned for future releases.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_qdrant::RetrievalMode;
    ///
    /// assert!(RetrievalMode::Dense.is_implemented());
    /// assert!(!RetrievalMode::Sparse.is_implemented());
    /// assert!(!RetrievalMode::Hybrid.is_implemented());
    /// ```
    #[must_use]
    pub fn is_implemented(&self) -> bool {
        matches!(self, RetrievalMode::Dense)
    }
}

impl fmt::Display for RetrievalMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RetrievalMode::Dense => write!(f, "dense"),
            RetrievalMode::Sparse => write!(f, "sparse"),
            RetrievalMode::Hybrid => write!(f, "hybrid"),
        }
    }
}

impl Default for RetrievalMode {
    /// Returns the default retrieval mode, which is Dense.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow_qdrant::RetrievalMode;
    ///
    /// let mode: RetrievalMode = Default::default();
    /// assert_eq!(mode, RetrievalMode::Dense);
    /// ```
    fn default() -> Self {
        RetrievalMode::Dense
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrieval_mode_display() {
        assert_eq!(RetrievalMode::Dense.to_string(), "dense");
        assert_eq!(RetrievalMode::Sparse.to_string(), "sparse");
        assert_eq!(RetrievalMode::Hybrid.to_string(), "hybrid");
    }

    #[test]
    fn test_retrieval_mode_equality() {
        assert_eq!(RetrievalMode::Dense, RetrievalMode::Dense);
        assert_ne!(RetrievalMode::Dense, RetrievalMode::Sparse);
        assert_ne!(RetrievalMode::Dense, RetrievalMode::Hybrid);
    }

    #[test]
    fn test_retrieval_mode_clone() {
        let mode = RetrievalMode::Dense;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    #[test]
    fn test_retrieval_mode_copy() {
        let mode = RetrievalMode::Dense;
        let copied = mode;
        assert_eq!(mode, copied);
    }

    #[test]
    fn test_is_dense() {
        assert!(RetrievalMode::Dense.is_dense());
        assert!(!RetrievalMode::Sparse.is_dense());
        assert!(!RetrievalMode::Hybrid.is_dense());
    }

    #[test]
    fn test_is_sparse() {
        assert!(!RetrievalMode::Dense.is_sparse());
        assert!(RetrievalMode::Sparse.is_sparse());
        assert!(!RetrievalMode::Hybrid.is_sparse());
    }

    #[test]
    fn test_is_hybrid() {
        assert!(!RetrievalMode::Dense.is_hybrid());
        assert!(!RetrievalMode::Sparse.is_hybrid());
        assert!(RetrievalMode::Hybrid.is_hybrid());
    }

    #[test]
    fn test_requires_dense_embeddings() {
        assert!(RetrievalMode::Dense.requires_dense_embeddings());
        assert!(!RetrievalMode::Sparse.requires_dense_embeddings());
        assert!(RetrievalMode::Hybrid.requires_dense_embeddings());
    }

    #[test]
    fn test_requires_sparse_embeddings() {
        assert!(!RetrievalMode::Dense.requires_sparse_embeddings());
        assert!(RetrievalMode::Sparse.requires_sparse_embeddings());
        assert!(RetrievalMode::Hybrid.requires_sparse_embeddings());
    }

    #[test]
    fn test_is_implemented() {
        assert!(RetrievalMode::Dense.is_implemented());
        assert!(!RetrievalMode::Sparse.is_implemented());
        assert!(!RetrievalMode::Hybrid.is_implemented());
    }

    #[test]
    fn test_default() {
        let mode: RetrievalMode = Default::default();
        assert_eq!(mode, RetrievalMode::Dense);
    }

    #[test]
    fn test_debug() {
        let mode = RetrievalMode::Dense;
        let debug_str = format!("{:?}", mode);
        assert_eq!(debug_str, "Dense");
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(RetrievalMode::Dense);
        set.insert(RetrievalMode::Sparse);
        set.insert(RetrievalMode::Hybrid);

        assert_eq!(set.len(), 3);
        assert!(set.contains(&RetrievalMode::Dense));
        assert!(set.contains(&RetrievalMode::Sparse));
        assert!(set.contains(&RetrievalMode::Hybrid));
    }

    #[test]
    fn test_all_modes_coverage() {
        // Ensure all modes are covered in tests
        let modes = [
            RetrievalMode::Dense,
            RetrievalMode::Sparse,
            RetrievalMode::Hybrid,
        ];

        for mode in &modes {
            // Each mode should have a valid string representation
            let _s = mode.to_string();
            // Each mode should be debuggable
            let _d = format!("{:?}", mode);
            // Each mode should be cloneable
            let _c = *mode;
        }
    }
}
