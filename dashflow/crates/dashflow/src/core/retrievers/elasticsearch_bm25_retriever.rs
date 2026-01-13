// Allow deprecated usage within this module - the struct is deprecated but we need to implement it
#![allow(deprecated)]
//! Elasticsearch BM25 retriever stub.
//!
//! This is a **configuration-only stub** for BM25 settings. For the full working
//! implementation, use `dashflow_elasticsearch::ElasticsearchBM25Retriever`.
//!
//! # Full Implementation
//!
//! ```rust,ignore
//! // Add dashflow-elasticsearch to your Cargo.toml:
//! // [dependencies]
//! // dashflow-elasticsearch = "1.11"
//!
//! use dashflow_elasticsearch::ElasticsearchBM25Retriever;
//! use dashflow::core::retrievers::Retriever;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create BM25 retriever
//! let mut retriever = ElasticsearchBM25Retriever::new(
//!     "documents",
//!     "http://localhost:9200",
//! ).await?;
//!
//! // Add documents (no embeddings needed - BM25 uses keywords)
//! retriever.add_texts(&["First document", "Second document"]).await?;
//!
//! // Search using BM25 scoring
//! let results = retriever._get_relevant_documents("document", None).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # When to Use BM25 vs Vector Search
//!
//! | Use Case | Recommended |
//! |----------|-------------|
//! | Keyword matching (exact terms) | BM25 (`ElasticsearchBM25Retriever`) |
//! | Semantic similarity | Vector (`ElasticsearchVectorStore`) |
//! | Known terminology (legal, medical) | BM25 |
//! | Natural language queries | Vector |
//!
//! # Elasticsearch Cloud
//!
//! To connect to Elasticsearch Cloud, use the format:
//! ```text
//! https://username:password@cluster_id.region.cloud.es.io:9243
//! ```

use crate::core::{
    config::RunnableConfig,
    documents::Document,
    error::{Error, Result},
    retrievers::{Retriever, RetrieverInput, RetrieverOutput},
    runnable::Runnable,
};
use async_trait::async_trait;
use serde_json::json;

/// Elasticsearch BM25 retriever.
///
/// **DEPRECATED**: This is a configuration-only stub. Use
/// `dashflow_elasticsearch::ElasticsearchBM25Retriever` for a working implementation.
///
/// # Migration
///
/// ```rust,ignore
/// // Old (stub):
/// use dashflow::core::retrievers::ElasticSearchBM25Retriever;
///
/// // New (working):
/// use dashflow_elasticsearch::ElasticsearchBM25Retriever;
///
/// let retriever = ElasticsearchBM25Retriever::new("index", "http://localhost:9200").await?;
/// ```
///
/// # Original Documentation
///
/// Connects to an Elasticsearch instance and performs BM25-scored full-text
/// search. Documents are indexed with custom BM25 similarity settings.
///
/// # Authentication
///
/// For Elasticsearch Cloud or secured clusters, include credentials in the URL:
/// - Format: `https://username:password@host:port`
/// - Example: `https://elastic:mypassword@my-deployment.es.io:9243`
///
/// # BM25 Parameters
///
/// - `k1`: Controls term frequency saturation (default: 2.0 in Elasticsearch)
/// - `b`: Controls length normalization (default: 0.75)
///
/// # Fields
///
/// - `client`: Elasticsearch client instance
/// - `index_name`: Name of the Elasticsearch index
/// - `k`: Number of documents to return (default: 4)
#[deprecated(
    since = "1.11.0",
    note = "Use dashflow_elasticsearch::ElasticsearchBM25Retriever instead"
)]
pub struct ElasticSearchBM25Retriever {
    /// Elasticsearch URL (for serialization/documentation)
    elasticsearch_url: String,

    /// Elasticsearch index name
    index_name: String,

    /// Number of documents to return (placeholder - will be used in search queries when Elasticsearch client is implemented)
    ///
    /// Architectural field: Reserved for future Elasticsearch search query implementation.
    /// Set via constructor (line 130, default 4), configurable via builder (line 125).
    /// Documented in struct docs (line 74), tested (lines 276, 292).
    /// Comment at line 229 indicates future usage: "Return up to k results".
    /// Not accessed in current in-memory BM25 implementation.
    /// Reserved for Elasticsearch client integration (future feature).
    /// Cannot remove without breaking public API and planned search functionality.
    // Placeholder field - will be used in Elasticsearch search query implementation
    #[allow(dead_code)] // Architectural: Reserved for Elasticsearch query integration
    k: usize,

    /// BM25 k1 parameter
    k1: f64,

    /// BM25 b parameter
    b: f64,
}

impl ElasticSearchBM25Retriever {
    /// Create a new `ElasticSearchBM25Retriever`.
    ///
    /// This method creates the Elasticsearch index with custom BM25 settings
    /// if it doesn't already exist.
    ///
    /// # Arguments
    ///
    /// * `elasticsearch_url` - URL of Elasticsearch instance (with optional auth)
    /// * `index_name` - Name of the index to create/use
    /// * `k1` - BM25 k1 parameter (default: 2.0)
    /// * `b` - BM25 b parameter (default: 0.75)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Cannot connect to Elasticsearch
    /// - Index creation fails
    /// - Invalid URL or credentials
    ///
    /// # Note
    ///
    /// This method is marked as returning a Result but does not actually
    /// create the Elasticsearch client or index. The actual implementation
    /// would require the elasticsearch crate to be available. This is a
    /// stub implementation for the core retriever trait.
    pub fn create(
        elasticsearch_url: String,
        index_name: String,
        k1: f64,
        b: f64,
        k: Option<usize>,
    ) -> Result<Self> {
        Ok(Self {
            elasticsearch_url,
            index_name,
            k: k.unwrap_or(4),
            k1,
            b,
        })
    }

    /// Create a retriever with default BM25 parameters.
    pub fn new(elasticsearch_url: String, index_name: String) -> Result<Self> {
        Self::create(elasticsearch_url, index_name, 2.0, 0.75, None)
    }

    /// Get the Elasticsearch index settings for BM25.
    ///
    /// Returns JSON settings that configure the index with custom BM25 similarity.
    #[must_use]
    pub fn index_settings(&self) -> serde_json::Value {
        json!({
            "settings": {
                "analysis": {
                    "analyzer": {
                        "default": {
                            "type": "standard"
                        }
                    }
                },
                "similarity": {
                    "custom_bm25": {
                        "type": "BM25",
                        "k1": self.k1,
                        "b": self.b
                    }
                }
            },
            "mappings": {
                "properties": {
                    "content": {
                        "type": "text",
                        "similarity": "custom_bm25"
                    }
                }
            }
        })
    }

    /// Get the Elasticsearch URL.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.elasticsearch_url
    }

    /// Get the index name.
    #[must_use]
    pub fn index(&self) -> &str {
        &self.index_name
    }
}

/// Note: The actual Elasticsearch client interaction would be implemented
/// in the dashflow-elasticsearch crate. This core implementation provides
/// the trait definitions and configuration.
///
/// Full implementation would include:
/// ```rust,ignore
/// use elasticsearch::Elasticsearch;
/// use elasticsearch::http::transport::Transport;
///
/// impl ElasticSearchBM25Retriever {
///     pub async fn create_with_client(...) -> Result<Self> {
///         let transport = Transport::single_node(&url)?;
///         let client = Elasticsearch::new(transport);
///
///         // Create index with BM25 settings
///         client.indices()
///             .create(...)
///             .body(self.index_settings())
///             .send()
///             .await?;
///
///         Ok(Self { client, ... })
///     }
///
///     pub async fn add_texts(&self, texts: Vec<&str>) -> Result<Vec<String>> {
///         // Use bulk API to index documents
///         ...
///     }
/// }
/// ```

#[async_trait]
impl Retriever for ElasticSearchBM25Retriever {
    async fn _get_relevant_documents(
        &self,
        _query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // This is a stub implementation. The actual implementation would:
        // 1. Create a match query with the content field
        // 2. Execute the query against Elasticsearch
        // 3. Parse the response and extract documents
        // 4. Return up to k results
        //
        // For now, return an error indicating this needs the elasticsearch crate
        Err(Error::Other(
            "This is a stub. Use dashflow_elasticsearch::ElasticsearchBM25Retriever \
             for a working BM25 retriever. Add dashflow-elasticsearch = \"1.11\" to Cargo.toml."
                .to_string(),
        ))
    }

    fn name(&self) -> String {
        "ElasticSearchBM25Retriever".to_string()
    }
}

#[async_trait]
impl Runnable for ElasticSearchBM25Retriever {
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
        "ElasticSearchBM25Retriever".to_string()
    }
}

#[cfg(test)]
#[allow(deprecated)] // Testing deprecated stub behavior
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_create_retriever() {
        let retriever = ElasticSearchBM25Retriever::new(
            "http://localhost:9200".to_string(),
            "test_index".to_string(),
        )
        .unwrap();

        assert_eq!(retriever.url(), "http://localhost:9200");
        assert_eq!(retriever.index(), "test_index");
        assert_eq!(retriever.k, 4);
    }

    #[test]
    fn test_create_with_custom_params() {
        let retriever = ElasticSearchBM25Retriever::create(
            "http://localhost:9200".to_string(),
            "test_index".to_string(),
            1.5,
            0.5,
            Some(10),
        )
        .unwrap();

        assert_eq!(retriever.k1, 1.5);
        assert_eq!(retriever.b, 0.5);
        assert_eq!(retriever.k, 10);
    }

    #[test]
    fn test_index_settings() {
        let retriever = ElasticSearchBM25Retriever::create(
            "http://localhost:9200".to_string(),
            "test_index".to_string(),
            2.0,
            0.75,
            None,
        )
        .unwrap();

        let settings = retriever.index_settings();

        // Verify BM25 parameters are in settings
        assert_eq!(settings["settings"]["similarity"]["custom_bm25"]["k1"], 2.0);
        assert_eq!(settings["settings"]["similarity"]["custom_bm25"]["b"], 0.75);
        assert_eq!(
            settings["mappings"]["properties"]["content"]["similarity"],
            "custom_bm25"
        );
    }

    #[tokio::test]
    async fn test_retriever_stub_returns_error() {
        let retriever = ElasticSearchBM25Retriever::new(
            "http://localhost:9200".to_string(),
            "test_index".to_string(),
        )
        .unwrap();

        // Should return error directing to dashflow-elasticsearch crate
        let result = retriever._get_relevant_documents("test query", None).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("dashflow_elasticsearch"));
    }

    #[test]
    fn test_name() {
        let retriever = ElasticSearchBM25Retriever::new(
            "http://localhost:9200".to_string(),
            "test_index".to_string(),
        )
        .unwrap();

        use crate::core::retrievers::Retriever;
        assert_eq!(Retriever::name(&retriever), "ElasticSearchBM25Retriever");
    }
}
