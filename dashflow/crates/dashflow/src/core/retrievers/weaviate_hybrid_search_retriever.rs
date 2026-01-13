// Allow deprecated usage within this module - the struct is deprecated but we need to implement it
#![allow(deprecated)]
//! Weaviate Hybrid Search Retriever (Configuration Stub)
//!
//! **NOTE: This is a configuration-only stub.** For the full working implementation,
//! use the `dashflow-weaviate` crate which provides:
//! - Real Weaviate client integration
//! - Schema management
//! - Production-ready hybrid search
//!
//! ```rust,ignore
//! // Use this instead:
//! use dashflow_weaviate::WeaviateHybridSearchRetriever;
//! ```
//!
//! This stub exists only for configuration types and API documentation.
//!
//! Note: The Python baseline was deprecated since LangChain 0.3.18.
//! Consider using `WeaviateVectorStore` for new projects.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::core::config::RunnableConfig;
use crate::core::documents::Document;
use crate::core::error::Error;
use crate::core::retrievers::{Retriever, RetrieverOutput};
use crate::core::runnable::Runnable;

/// Configuration for Weaviate hybrid search.
///
/// Weaviate hybrid search combines BM25 (sparse) and vector (dense) search.
///
/// # Algorithm
///
/// 1. **BM25 Search**: Keyword-based search using BM25 algorithm
/// 2. **Vector Search**: Semantic search using vector embeddings
/// 3. **Fusion**: Combine results using weighted fusion (alpha parameter)
///    - alpha = 1.0: Pure vector search
///    - alpha = 0.0: Pure BM25 search
///    - alpha = 0.5: Balanced hybrid
/// 4. **Ranking**: Fuse rankings using selected method (relative score, etc.)
///
/// # Python Baseline
///
/// ```python
/// # From weaviate_hybrid_search.py:18-169
/// # NOTE: Deprecated since 0.3.18, use WeaviateVectorStore instead
/// class WeaviateHybridSearchRetriever(BaseRetriever):
///     client: Any  # Weaviate client
///     index_name: str
///     text_key: str
///     alpha: float = 0.5
///     k: int = 4
///     attributes: List[str]
///     create_schema_if_missing: bool = True
///
///     def _get_relevant_documents(
///         self,
///         query: str,
///         where_filter: Optional[Dict] = None,
///         score: bool = False,
///         hybrid_search_kwargs: Optional[Dict] = None,
///     ) -> List[Document]:
///         query_obj = self.client.query.get(self.index_name, self.attributes)
///         if where_filter:
///             query_obj = query_obj.with_where(where_filter)
///         if score:
///             query_obj = query_obj.with_additional(["score", "explainScore"])
///
///         result = (
///             query_obj.with_hybrid(query, alpha=self.alpha, **hybrid_search_kwargs)
///             .with_limit(self.k)
///             .do()
///         )
///         return [Document(page_content=res[text_key], metadata=res)
///                 for res in result["data"]["Get"][index_name]]
/// ```
///
/// # Fusion Methods
///
/// - **Relative Score**: Normalize scores to \[0,1\] and combine
/// - **Ranked**: Combine based on rank positions
///
/// # Usage (when implemented)
///
/// ```ignore
/// use dashflow_weaviate::WeaviateHybridSearchRetriever;
///
/// let retriever = WeaviateHybridSearchRetriever::new(
///     client,
///     WeaviateHybridConfig {
///         index_name: "Articles".to_string(),
///         text_key: "content".to_string(),
///         alpha: 0.5,
///         k: 4,
///         attributes: vec!["content".to_string(), "title".to_string()],
///         create_schema_if_missing: true,
///     }
/// );
///
/// let docs = retriever._get_relevant_documents("query").await?;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaviateHybridConfig {
    /// Name of the Weaviate class/index
    pub index_name: String,

    /// Key for text content in properties
    pub text_key: String,

    /// Alpha value for hybrid search (0.0 = BM25 only, 1.0 = vector only)
    /// Default: 0.5 (balanced hybrid)
    pub alpha: f32,

    /// Number of documents to return
    pub k: usize,

    /// Attributes/properties to return in results
    pub attributes: Vec<String>,

    /// Whether to create schema if it doesn't exist
    pub create_schema_if_missing: bool,
}

impl Default for WeaviateHybridConfig {
    fn default() -> Self {
        Self {
            index_name: "Document".to_string(),
            text_key: "text".to_string(),
            alpha: 0.5,
            k: 4,
            attributes: vec!["text".to_string()],
            create_schema_if_missing: true,
        }
    }
}

/// Weaviate Hybrid Search Retriever (STUB).
///
/// **DEPRECATED**: This is a configuration-only stub. The Python baseline was
/// deprecated in LangChain 0.3.18. Use `dashflow_weaviate::WeaviateVectorStore`
/// for vector search with Weaviate.
///
/// # Migration
///
/// ```rust,ignore
/// // Use WeaviateVectorStore for vector search:
/// use dashflow_weaviate::WeaviateVectorStore;
///
/// let store = WeaviateVectorStore::new("http://localhost:8080", "MyClass", embeddings).await?;
/// let results = store._similarity_search("query", 4, None).await?;
/// ```
///
/// # Note
///
/// Weaviate's native hybrid search is available via GraphQL queries in the
/// `dashflow_weaviate` crate, but the dedicated retriever is not yet implemented.
#[deprecated(
    since = "1.11.0",
    note = "Python baseline deprecated. Use dashflow_weaviate::WeaviateVectorStore instead."
)]
#[derive(Clone)]
pub struct WeaviateHybridSearchRetriever {
    config: WeaviateHybridConfig,
}

impl fmt::Debug for WeaviateHybridSearchRetriever {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WeaviateHybridSearchRetriever")
            .field("config", &self.config)
            .finish()
    }
}

impl WeaviateHybridSearchRetriever {
    /// Create a new Weaviate hybrid search retriever.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for hybrid search
    ///
    /// # Note
    ///
    /// This is a stub implementation. Use `dashflow-weaviate` crate for actual implementation.
    #[must_use]
    pub fn new(config: WeaviateHybridConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(WeaviateHybridConfig::default())
    }

    /// Get the configuration.
    #[must_use]
    pub fn config(&self) -> &WeaviateHybridConfig {
        &self.config
    }
}

#[async_trait]
impl Retriever for WeaviateHybridSearchRetriever {
    async fn _get_relevant_documents(
        &self,
        _query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>, Error> {
        Err(Error::other(
            "WeaviateHybridSearchRetriever is a stub implementation. \
             Use dashflow-weaviate crate for actual Weaviate integration. \
             Note: Python baseline deprecated since 0.3.18, use WeaviateVectorStore instead.",
        ))
    }
}

#[async_trait]
impl Runnable for WeaviateHybridSearchRetriever {
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
    fn test_create_weaviate_hybrid_retriever() {
        let retriever = WeaviateHybridSearchRetriever::with_defaults();
        assert_eq!(retriever.config().k, 4);
        assert_eq!(retriever.config().alpha, 0.5);
        assert_eq!(retriever.config().index_name, "Document");
        assert_eq!(retriever.config().text_key, "text");
    }

    #[test]
    fn test_custom_config() {
        let config = WeaviateHybridConfig {
            index_name: "Articles".to_string(),
            text_key: "content".to_string(),
            alpha: 0.7,
            k: 10,
            attributes: vec!["content".to_string(), "title".to_string()],
            create_schema_if_missing: false,
        };
        let retriever = WeaviateHybridSearchRetriever::new(config);
        assert_eq!(retriever.config().index_name, "Articles");
        assert_eq!(retriever.config().text_key, "content");
        assert_eq!(retriever.config().alpha, 0.7);
        assert_eq!(retriever.config().k, 10);
        assert_eq!(retriever.config().attributes.len(), 2);
        assert!(!retriever.config().create_schema_if_missing);
    }

    #[tokio::test]
    async fn test_get_relevant_documents_not_implemented() {
        let retriever = WeaviateHybridSearchRetriever::with_defaults();
        let result = retriever._get_relevant_documents("test query", None).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("stub implementation"));
    }

    #[tokio::test]
    async fn test_runnable_invoke_not_implemented() {
        let retriever = WeaviateHybridSearchRetriever::with_defaults();
        let result = retriever.invoke("test query".to_string(), None).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_default_attributes() {
        let retriever = WeaviateHybridSearchRetriever::with_defaults();
        assert_eq!(retriever.config().attributes, vec!["text".to_string()]);
    }
}
