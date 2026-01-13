// Allow deprecated usage within this module - the struct is deprecated but we need to implement it
#![allow(deprecated)]
//! Pinecone Hybrid Search Retriever (Configuration Stub)
//!
//! **NOTE: This is a configuration-only stub.** For the full working implementation,
//! use the `dashflow-pinecone` crate which provides:
//! - Real Pinecone API integration
//! - Sparse encoding support
//! - Production-ready hybrid search
//!
//! ```rust,ignore
//! // Use this instead:
//! use dashflow_pinecone::PineconeHybridSearchRetriever;
//! ```
//!
//! This stub exists only for configuration types and API documentation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::core::config::RunnableConfig;
use crate::core::documents::Document;
use crate::core::error::Error;
use crate::core::retrievers::{Retriever, RetrieverOutput};
use crate::core::runnable::Runnable;

/// Configuration for Pinecone hybrid search.
///
/// Hybrid search combines dense (vector) and sparse (keyword) search
/// using Pinecone's hybrid search capabilities.
///
/// # Algorithm
///
/// 1. **Dense Vector Generation**: Embed query using embeddings model
/// 2. **Sparse Vector Generation**: Encode query using sparse encoder (e.g., BM25, SPLADE)
/// 3. **Hybrid Scaling**: Scale dense and sparse vectors using alpha parameter
///    - alpha = 1.0: Pure dense (semantic) search
///    - alpha = 0.0: Pure sparse (keyword) search
///    - alpha = 0.5: Balanced hybrid search
/// 4. **Query Pinecone**: Send both vectors to Pinecone index
/// 5. **Return Results**: Top-k documents with scores
///
/// # Python Baseline
///
/// ```python
/// # From pinecone_hybrid_search.py:102-186
/// class PineconeHybridSearchRetriever(BaseRetriever):
///     embeddings: Embeddings
///     sparse_encoder: Any  # e.g., BM25, SPLADE
///     index: Any  # Pinecone index
///     top_k: int = 4
///     alpha: float = 0.5  # Hybrid scaling factor
///     namespace: Optional[str] = None
///     text_key: str = "context"
///
///     def _get_relevant_documents(self, query: str, **kwargs) -> List[Document]:
///         # Create sparse vector
///         sparse_vec = self.sparse_encoder.encode_queries(query)
///         # Create dense vector
///         dense_vec = self.embeddings.embed_query(query)
///         # Scale with alpha
///         dense_vec, sparse_vec = hybrid_convex_scale(dense_vec, sparse_vec, self.alpha)
///         # Query Pinecone
///         result = self.index.query(
///             vector=dense_vec,
///             sparse_vector=sparse_vec,
///             top_k=self.top_k,
///             include_metadata=True,
///             namespace=self.namespace,
///             **kwargs
///         )
///         return [Document(page_content=res["metadata"][text_key], metadata=res["metadata"])
///                 for res in result["matches"]]
/// ```
///
/// # Usage (when implemented)
///
/// ```ignore
/// use dashflow_pinecone::PineconeHybridSearchRetriever;
/// use dashflow_openai::OpenAIEmbeddings;
///
/// let retriever = PineconeHybridSearchRetriever::new(
///     embeddings,
///     sparse_encoder,
///     index,
///     PineconeHybridConfig {
///         top_k: 4,
///         alpha: 0.5,
///         namespace: None,
///         text_key: "context".to_string(),
///     }
/// );
///
/// let docs = retriever._get_relevant_documents("query").await?;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PineconeHybridConfig {
    /// Number of documents to return
    pub top_k: usize,

    /// Alpha value for hybrid search (0.0 = sparse only, 1.0 = dense only)
    /// Default: 0.5 (balanced hybrid)
    pub alpha: f32,

    /// Namespace for index partition (optional)
    pub namespace: Option<String>,

    /// Key for text content in metadata
    /// Default: "context"
    pub text_key: String,
}

impl Default for PineconeHybridConfig {
    fn default() -> Self {
        Self {
            top_k: 4,
            alpha: 0.5,
            namespace: None,
            text_key: "context".to_string(),
        }
    }
}

/// Pinecone Hybrid Search Retriever (STUB).
///
/// **DEPRECATED**: This is a configuration-only stub. The Pinecone hybrid search
/// feature requires sparse encoders that are not yet implemented.
/// Use `dashflow_pinecone::PineconeVectorStore` for vector (semantic) search.
///
/// # Migration
///
/// ```rust,ignore
/// // For vector search (not hybrid):
/// use dashflow_pinecone::PineconeVectorStore;
///
/// let store = PineconeVectorStore::new("index", embeddings, None, None).await?;
/// let results = store._similarity_search("query", 4, None).await?;
/// ```
///
/// # Note
///
/// True hybrid search requires sparse encoders (BM25, SPLADE) which are not
/// yet available in the Rust ecosystem. For hybrid search, consider
/// Elasticsearch or Weaviate alternatives.
#[deprecated(
    since = "1.11.0",
    note = "Hybrid search not implemented. Use dashflow_pinecone::PineconeVectorStore for vector search."
)]
#[derive(Clone)]
pub struct PineconeHybridSearchRetriever {
    config: PineconeHybridConfig,
}

impl fmt::Debug for PineconeHybridSearchRetriever {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PineconeHybridSearchRetriever")
            .field("config", &self.config)
            .finish()
    }
}

impl PineconeHybridSearchRetriever {
    /// Create a new Pinecone hybrid search retriever.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for hybrid search
    ///
    /// # Note
    ///
    /// This is a stub implementation. Use `dashflow-pinecone` crate for actual implementation.
    #[must_use]
    pub fn new(config: PineconeHybridConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(PineconeHybridConfig::default())
    }

    /// Get the configuration.
    #[must_use]
    pub fn config(&self) -> &PineconeHybridConfig {
        &self.config
    }
}

#[async_trait]
impl Retriever for PineconeHybridSearchRetriever {
    async fn _get_relevant_documents(
        &self,
        _query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>, Error> {
        Err(Error::other(
            "PineconeHybridSearchRetriever is a stub implementation. \
             Use dashflow-pinecone crate for actual Pinecone integration.",
        ))
    }
}

#[async_trait]
impl Runnable for PineconeHybridSearchRetriever {
    type Input = String;
    type Output = RetrieverOutput;

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> Result<Self::Output, Error> {
        self._get_relevant_documents(&input, config.as_ref()).await
    }
}

#[cfg(test)]
#[allow(deprecated)] // Testing deprecated stub behavior
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_create_pinecone_hybrid_retriever() {
        let retriever = PineconeHybridSearchRetriever::with_defaults();
        assert_eq!(retriever.config().top_k, 4);
        assert_eq!(retriever.config().alpha, 0.5);
    }

    #[test]
    fn test_custom_config() {
        let config = PineconeHybridConfig {
            top_k: 10,
            alpha: 0.7,
            namespace: Some("test-namespace".to_string()),
            text_key: "content".to_string(),
        };
        let retriever = PineconeHybridSearchRetriever::new(config);
        assert_eq!(retriever.config().top_k, 10);
        assert_eq!(retriever.config().alpha, 0.7);
        assert_eq!(
            retriever.config().namespace.as_ref().unwrap(),
            "test-namespace"
        );
        assert_eq!(retriever.config().text_key, "content");
    }

    #[tokio::test]
    async fn test_get_relevant_documents_not_implemented() {
        let retriever = PineconeHybridSearchRetriever::with_defaults();
        let result = retriever._get_relevant_documents("test query", None).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("stub implementation"));
    }

    #[tokio::test]
    async fn test_runnable_invoke_not_implemented() {
        let retriever = PineconeHybridSearchRetriever::with_defaults();
        let result = retriever.invoke("test query".to_string(), None).await;
        assert!(result.is_err());
    }
}
