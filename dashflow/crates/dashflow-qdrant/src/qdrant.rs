//! Qdrant vector store implementation.
//!
//! This module provides the main `QdrantVectorStore` struct for interacting with
//! a Qdrant vector database.

use dashflow::core::config::RunnableConfig;
use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::error::{Error, Result};
use dashflow::core::indexing::document_index::{DeleteResponse, DocumentIndex, UpsertResponse};
use dashflow::core::retrievers::Retriever;
use dashflow::core::vector_stores::DistanceMetric;
use dashflow::{embed, embed_query};
use qdrant_client::qdrant;
use qdrant_client::qdrant::{
    Condition, Distance, FieldCondition, Filter, Match, UpsertPointsBuilder,
};
use qdrant_client::Payload;
use qdrant_client::Qdrant;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

use crate::RetrievalMode;

mod collections;
mod traits;

#[cfg(test)]
use traits::hashmap_to_qdrant_filter;

/// A vector store backed by Qdrant.
///
/// Qdrant is a high-performance vector search engine that supports:
/// - Dense vector search (traditional embeddings)
/// - Sparse vector search (BM25-style keyword matching)
/// - Hybrid search (combining dense and sparse)
/// - Advanced filtering with nested conditions
/// - Multiple distance metrics
///
/// # Architecture
///
/// - **Client**: Uses `qdrant-client` for gRPC communication with Qdrant server
/// - **Collections**: Data is stored in named collections with configurable vector dimensions
/// - **Points**: Each document becomes a "point" with an ID, vector(s), and payload (metadata)
/// - **Payloads**: Store document content and metadata as key-value pairs
///
/// # Current Status
///
/// - ✅ Dense vector search (fully implemented)
/// - ⏳ Sparse vector search(future feature)
/// - ⏳ Hybrid search(future feature)
///
/// # Examples
///
/// ## Basic Usage
///
/// ```ignore
/// use dashflow_qdrant::{QdrantVectorStore, RetrievalMode};
/// use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
/// // Connect to Qdrant and create a vector store
/// let store = QdrantVectorStore::new(
///     "http://localhost:6334",
///     "my_collection",
///     Some(embeddings),
///     RetrievalMode::Dense,
/// ).await?;
///
/// // Add documents
/// let texts = vec!["Hello world", "Goodbye world"];
/// let ids = store.add_texts(&texts, None, None).await?;
///
/// // Search
/// let results = store._similarity_search("Hello", 2, None).await?;
/// # Ok(())
/// # }
/// ```
///
/// ## With Custom Configuration
///
/// ```ignore
/// use dashflow_qdrant::{QdrantVectorStore, RetrievalMode};
/// use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
/// use dashflow::core::vector_stores::DistanceMetric;
/// use qdrant_client::Qdrant;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
/// // Create a Qdrant client with custom configuration
/// let client = Qdrant::from_url("http://localhost:6334")
///     .api_key(Some("your-api-key".to_string()))
///     .timeout(std::time::Duration::from_secs(30))
///     .build()?;
///
/// // Create store from existing client
/// let store = QdrantVectorStore::from_client(
///     client,
///     "my_collection",
///     Some(embeddings),
///     RetrievalMode::Dense,
/// )
/// .with_distance_metric(DistanceMetric::Euclidean)
/// .with_content_key("text")
/// .with_metadata_key("meta");
///
/// # Ok(())
/// # }
/// ```
///
/// # Python Baseline Compatibility
///
/// This implementation matches the Python `QdrantVectorStore` in
/// `dashflow_qdrant.qdrant.QdrantVectorStore`.
#[derive(Clone)]
pub struct QdrantVectorStore {
    /// The Qdrant client for gRPC communication
    client: Qdrant,

    /// Name of the collection in Qdrant
    collection_name: String,

    /// Embeddings provider for dense vectors (required for Dense and Hybrid modes)
    embeddings: Option<Arc<dyn Embeddings>>,

    /// Embeddings provider for sparse vectors (required for Sparse and Hybrid modes)
    /// Sparse embeddings placeholder - becomes `Arc<dyn SparseEmbeddings>` when trait exists
    #[allow(dead_code)] // Architectural: Reserved for Sparse/Hybrid retrieval modes
    sparse_embeddings: Option<()>,

    /// Retrieval mode (Dense, Sparse, or Hybrid)
    retrieval_mode: RetrievalMode,

    /// Distance metric for similarity calculations
    distance_metric: DistanceMetric,

    // Configuration fields
    /// Name of the vector field in Qdrant (default: "" for unnamed/default vector)
    vector_name: String,

    /// Name of the sparse vector field in Qdrant (default: "dashflow-sparse")
    sparse_vector_name: String,

    /// Key for document content in point payloads (default: "`page_content`")
    content_key: String,

    /// Key for document metadata in point payloads (default: "metadata")
    metadata_key: String,
}

impl std::fmt::Debug for QdrantVectorStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QdrantVectorStore")
            .field("collection_name", &self.collection_name)
            .field("retrieval_mode", &self.retrieval_mode)
            .field("distance_metric", &self.distance_metric)
            .field("vector_name", &self.vector_name)
            .field("sparse_vector_name", &self.sparse_vector_name)
            .field("content_key", &self.content_key)
            .field("metadata_key", &self.metadata_key)
            .field("has_embeddings", &self.embeddings.is_some())
            .finish_non_exhaustive()
    }
}

impl QdrantVectorStore {
    /// Creates a new `QdrantVectorStore` by connecting to a Qdrant server.
    ///
    /// # Arguments
    ///
    /// * `url` - URL of the Qdrant server (e.g., "<http://localhost:6334>")
    /// * `collection_name` - Name of the collection to use
    /// * `embeddings` - Embeddings provider (required for Dense and Hybrid modes)
    /// * `retrieval_mode` - Retrieval mode (Dense, Sparse, or Hybrid)
    ///
    /// # Returns
    ///
    /// Returns a `Result` with the created `QdrantVectorStore` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to connect to Qdrant server
    /// - Invalid URL format
    /// - Embeddings are None but required for the retrieval mode
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use dashflow_qdrant::{QdrantVectorStore, RetrievalMode};
    /// use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
    /// let store = QdrantVectorStore::new(
    ///     "http://localhost:6334",
    ///     "my_collection",
    ///     Some(embeddings),
    ///     RetrievalMode::Dense,
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(
        url: &str,
        collection_name: impl Into<String>,
        embeddings: Option<Arc<dyn Embeddings>>,
        retrieval_mode: RetrievalMode,
    ) -> Result<Self> {
        let client = Qdrant::from_url(url)
            .build()
            .map_err(|e| Error::config(format!("Failed to create Qdrant client: {e}")))?;

        Ok(Self::from_client(
            client,
            collection_name,
            embeddings,
            retrieval_mode,
        ))
    }

    /// Creates a new `QdrantVectorStore` from an existing Qdrant client.
    ///
    /// Use this constructor when you need custom client configuration (e.g., API keys,
    /// timeouts, custom headers).
    ///
    /// # Arguments
    ///
    /// * `client` - An already-configured Qdrant client
    /// * `collection_name` - Name of the collection to use
    /// * `embeddings` - Embeddings provider (required for Dense and Hybrid modes)
    /// * `retrieval_mode` - Retrieval mode (Dense, Sparse, or Hybrid)
    ///
    /// # Returns
    ///
    /// Returns the created `QdrantVectorStore`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use dashflow_qdrant::{QdrantVectorStore, RetrievalMode};
    /// use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
    /// use qdrant_client::Qdrant;
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
    /// let client = Qdrant::from_url("http://localhost:6334")
    ///     .api_key(Some("your-api-key".to_string()))
    ///     .build()?;
    ///
    /// let store = QdrantVectorStore::from_client(
    ///     client,
    ///     "my_collection",
    ///     Some(embeddings),
    ///     RetrievalMode::Dense,
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_client(
        client: Qdrant,
        collection_name: impl Into<String>,
        embeddings: Option<Arc<dyn Embeddings>>,
        retrieval_mode: RetrievalMode,
    ) -> Self {
        Self {
            client,
            collection_name: collection_name.into(),
            embeddings,
            sparse_embeddings: None,
            retrieval_mode,
            distance_metric: DistanceMetric::Cosine, // Default to Cosine (most common)
            vector_name: String::new(),              // Empty string = unnamed/default vector
            sparse_vector_name: "dashflow-sparse".to_string(), // Matches Python baseline
            content_key: "page_content".to_string(), // Matches Python baseline
            metadata_key: "metadata".to_string(),    // Matches Python baseline
        }
    }

    /// Creates a new `QdrantVectorStore` from an existing collection.
    ///
    /// This method connects to an existing Qdrant collection without creating or
    /// modifying it. Use this when you want to connect to a collection that was
    /// created externally or by another process.
    ///
    /// # Arguments
    ///
    /// * `url` - URL of the Qdrant server (e.g., "<http://localhost:6334>")
    /// * `collection_name` - Name of the existing collection
    /// * `embeddings` - Embeddings provider (required for Dense and Hybrid modes)
    /// * `retrieval_mode` - Retrieval mode (Dense, Sparse, or Hybrid)
    ///
    /// # Returns
    ///
    /// Returns a `Result` with the created `QdrantVectorStore` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to connect to Qdrant server
    /// - Invalid URL format
    /// - Embeddings are None but required for the retrieval mode
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `from_existing_collection()` in `dashflow_qdrant/qdrant.py:434-494`.
    ///
    /// The Python version:
    /// 1. Creates a `QdrantClient` with connection parameters
    /// 2. Returns a new `QdrantVectorStore` instance with the client
    /// 3. Does NOT validate collection config or create collections
    /// 4. Validation happens lazily on first use (if enabled)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use dashflow_qdrant::{QdrantVectorStore, RetrievalMode};
    /// use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
    /// // Connect to an existing collection
    /// let store = QdrantVectorStore::from_existing_collection(
    ///     "http://localhost:6334",
    ///     "my_existing_collection",
    ///     Some(embeddings),
    ///     RetrievalMode::Dense,
    /// ).await?;
    ///
    /// // Now you can search the existing collection
    /// let results = store._similarity_search("query", 5, None, None, 0, None).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn from_existing_collection(
        url: &str,
        collection_name: impl Into<String>,
        embeddings: Option<Arc<dyn Embeddings>>,
        retrieval_mode: RetrievalMode,
    ) -> Result<Self> {
        // Validate embeddings for the retrieval mode
        Self::validate_embeddings(retrieval_mode, embeddings.clone())?;

        // Create Qdrant client
        let client = Qdrant::from_url(url)
            .build()
            .map_err(|e| Error::config(format!("Failed to create Qdrant client: {e}")))?;

        // Return instance (validation will happen lazily on first use)
        Ok(Self::from_client(
            client,
            collection_name,
            embeddings,
            retrieval_mode,
        ))
    }

    /// Creates a new `QdrantVectorStore` and initializes it with documents.
    ///
    /// This is a convenience method that:
    /// 1. Connects to Qdrant
    /// 2. Assumes the collection already exists (does not create it)
    /// 3. Adds the provided texts as documents
    ///
    /// **Note**: This implementation assumes the collection exists. For full
    /// collection creation logic (matching Python's `construct_instance`), use the
    /// Qdrant client directly to create the collection first.
    ///
    /// # Arguments
    ///
    /// * `url` - URL of the Qdrant server (e.g., "<http://localhost:6334>")
    /// * `collection_name` - Name of the collection (must already exist)
    /// * `texts` - Texts to add as documents
    /// * `metadatas` - Optional metadata for each text
    /// * `ids` - Optional IDs for each text (generated if None)
    /// * `embeddings` - Embeddings provider (required for Dense and Hybrid modes)
    /// * `retrieval_mode` - Retrieval mode (Dense, Sparse, or Hybrid)
    /// * `batch_size` - Number of documents to upload in each batch (default: 64)
    ///
    /// # Returns
    ///
    /// Returns a `Result` with the created and initialized `QdrantVectorStore` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to connect to Qdrant server
    /// - Collection does not exist
    /// - Failed to add texts
    /// - Invalid embeddings configuration
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `from_texts()` in `dashflow_qdrant/qdrant.py:339-433`.
    ///
    /// The Python version:
    /// 1. Calls `construct_instance()` to create/connect to collection
    /// 2. Calls `add_texts()` to add initial documents
    /// 3. Returns the initialized instance
    ///
    /// **Implementation Note**: The Python version includes full collection creation
    /// logic with vector configuration, distance metrics, etc. This Rust version
    /// assumes the collection already exists.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use dashflow_qdrant::{QdrantVectorStore, RetrievalMode};
    /// use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
    /// // Create and initialize a vector store with documents
    /// let texts = vec!["Hello world", "Goodbye world"];
    /// let store = QdrantVectorStore::from_texts(
    ///     "http://localhost:6334",
    ///     "my_collection",
    ///     &texts,
    ///     None, // No metadata
    ///     None, // Auto-generate IDs
    ///     Some(embeddings),
    ///     RetrievalMode::Dense,
    ///     64, // Batch size
    /// ).await?;
    ///
    /// // Collection now contains the documents
    /// let results = store._similarity_search("Hello", 2, None, None, 0, None).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::too_many_arguments)] // Factory method with required configuration options for vector store creation
    pub async fn from_texts(
        url: &str,
        collection_name: impl Into<String>,
        texts: &[impl AsRef<str>],
        metadatas: Option<&[HashMap<String, JsonValue>]>,
        ids: Option<&[String]>,
        embeddings: Option<Arc<dyn Embeddings>>,
        retrieval_mode: RetrievalMode,
        batch_size: usize,
    ) -> Result<Self> {
        // Python: qdrant = cls.construct_instance(...)
        // Python: qdrant.add_texts(texts, metadatas, ids, batch_size)
        // Python: return qdrant

        // Use construct_instance to handle collection creation
        let mut store = Self::construct_instance(
            url,
            Some(collection_name.into()),
            embeddings,
            retrieval_mode,
            qdrant_client::qdrant::Distance::Cosine, // Default distance metric
            false,                                   // Don't force recreate
            true,                                    // Validate collection config
        )
        .await?;

        // Add texts to the collection
        store.add_texts(texts, metadatas, ids, batch_size).await?;

        Ok(store)
    }

    /// Returns a reference to the Qdrant client.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) {
    /// let client = store.client();
    /// // Use client for direct Qdrant operations
    /// # }
    /// ```
    #[must_use]
    pub fn client(&self) -> &Qdrant {
        &self.client
    }

    /// Returns the collection name.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) {
    /// let name = store.collection_name();
    /// println!("Collection: {}", name);
    /// # }
    /// ```
    #[must_use]
    pub fn collection_name(&self) -> &str {
        &self.collection_name
    }

    /// Returns the retrieval mode.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::{QdrantVectorStore, RetrievalMode};
    /// # async fn example(store: &QdrantVectorStore) {
    /// let mode = store.retrieval_mode();
    /// assert_eq!(mode, RetrievalMode::Dense);
    /// # }
    /// ```
    #[must_use]
    pub fn retrieval_mode(&self) -> RetrievalMode {
        self.retrieval_mode
    }

    /// Returns the distance metric.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # use dashflow::core::vector_stores::DistanceMetric;
    /// # async fn example(store: &QdrantVectorStore) {
    /// let metric = store.distance_metric();
    /// assert_eq!(metric, DistanceMetric::Cosine);
    /// # }
    /// ```
    #[must_use]
    pub fn distance_metric(&self) -> DistanceMetric {
        self.distance_metric
    }

    /// Returns a reference to the embeddings provider.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) {
    /// if let Some(embeddings) = store.embeddings() {
    ///     // Use embeddings
    /// }
    /// # }
    /// ```
    #[must_use]
    pub fn embeddings(&self) -> Option<&Arc<dyn Embeddings>> {
        self.embeddings.as_ref()
    }

    /// Returns the content key used in payloads.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) {
    /// let key = store.content_key();
    /// assert_eq!(key, "page_content");
    /// # }
    /// ```
    #[must_use]
    pub fn content_key(&self) -> &str {
        &self.content_key
    }

    /// Returns the metadata key used in payloads.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) {
    /// let key = store.metadata_key();
    /// assert_eq!(key, "metadata");
    /// # }
    /// ```
    #[must_use]
    pub fn metadata_key(&self) -> &str {
        &self.metadata_key
    }

    /// Returns the vector name (empty string for default/unnamed vector).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) {
    /// let name = store.vector_name();
    /// assert_eq!(name, ""); // Default vector
    /// # }
    /// ```
    #[must_use]
    pub fn vector_name(&self) -> &str {
        &self.vector_name
    }

    /// Returns the sparse vector name.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) {
    /// let name = store.sparse_vector_name();
    /// assert_eq!(name, "dashflow-sparse");
    /// # }
    /// ```
    #[must_use]
    pub fn sparse_vector_name(&self) -> &str {
        &self.sparse_vector_name
    }

    /// Sets the distance metric.
    ///
    /// **Note**: This should match the distance metric configured for the
    /// collection in Qdrant. No automatic validation is performed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # use dashflow::core::vector_stores::DistanceMetric;
    /// # async fn example(mut store: QdrantVectorStore) {
    /// store = store.with_distance_metric(DistanceMetric::Euclidean);
    /// # }
    /// ```
    #[must_use]
    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Sets the content key used in payloads.
    ///
    /// Default is "`page_content`" (matches Python baseline).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(mut store: QdrantVectorStore) {
    /// store = store.with_content_key("text");
    /// # }
    /// ```
    pub fn with_content_key(mut self, key: impl Into<String>) -> Self {
        self.content_key = key.into();
        self
    }

    /// Sets the metadata key used in payloads.
    ///
    /// Default is "metadata" (matches Python baseline).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(mut store: QdrantVectorStore) {
    /// store = store.with_metadata_key("meta");
    /// # }
    /// ```
    pub fn with_metadata_key(mut self, key: impl Into<String>) -> Self {
        self.metadata_key = key.into();
        self
    }

    /// Sets the vector name.
    ///
    /// Default is "" (empty string = unnamed/default vector).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(mut store: QdrantVectorStore) {
    /// store = store.with_vector_name("dense_vector");
    /// # }
    /// ```
    pub fn with_vector_name(mut self, name: impl Into<String>) -> Self {
        self.vector_name = name.into();
        self
    }

    /// Sets the sparse vector name.
    ///
    /// Default is "dashflow-sparse" (matches Python baseline).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(mut store: QdrantVectorStore) {
    /// store = store.with_sparse_vector_name("bm25_vector");
    /// # }
    /// ```
    pub fn with_sparse_vector_name(mut self, name: impl Into<String>) -> Self {
        self.sparse_vector_name = name.into();
        self
    }

    // ========== Validation Methods ==========

    /// Validates that embeddings are provided based on retrieval mode.
    ///
    /// # Arguments
    ///
    /// * `retrieval_mode` - The retrieval mode being used
    /// * `embeddings` - Optional embeddings provider
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Dense mode requires embeddings but None provided
    /// - Sparse mode requires sparse embeddings (deferred: requires SparseEmbeddings trait)
    /// - Hybrid mode requires both embeddings (deferred: requires both Dense and Sparse support)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::{QdrantVectorStore, RetrievalMode};
    /// # use dashflow::core::embeddings::Embeddings;
    /// # use std::sync::Arc;
    /// # fn example(embeddings: Arc<dyn Embeddings>) -> Result<(), Box<dyn std::error::Error>> {
    /// QdrantVectorStore::validate_embeddings(
    ///     RetrievalMode::Dense,
    ///     Some(embeddings),
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::needless_pass_by_value)] // API consistency - takes ownership to match constructor patterns
    pub fn validate_embeddings(
        retrieval_mode: RetrievalMode,
        embeddings: Option<Arc<dyn Embeddings>>,
    ) -> Result<()> {
        match retrieval_mode {
            RetrievalMode::Dense => {
                if embeddings.is_none() {
                    return Err(Error::config(
                        "'embeddings' cannot be None when retrieval mode is Dense",
                    ));
                }
            }
            RetrievalMode::Sparse => {
                // Sparse mode: Deferred - requires SparseEmbeddings trait definition
                return Err(Error::config(
                    "Sparse retrieval mode deferred (requires SparseEmbeddings trait) - use Dense mode",
                ));
            }
            RetrievalMode::Hybrid => {
                // Hybrid mode: Deferred - requires both Dense and Sparse embeddings
                return Err(Error::config(
                    "Hybrid retrieval mode deferred (requires both embedding types) - use Dense mode",
                ));
            }
        }
        Ok(())
    }

    /// Validates the collection configuration matches the expected configuration.
    ///
    /// This method checks that:
    /// - The collection exists
    /// - Vector dimensions match the embeddings
    /// - Distance metric matches the configuration
    /// - Retrieval mode is compatible with collection configuration
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Collection does not exist
    /// - Vector dimensions mismatch
    /// - Distance metric mismatch
    /// - Retrieval mode incompatible with collection
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// store.validate_collection_config().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn validate_collection_config(&self) -> Result<()> {
        // Get collection info from Qdrant
        let collection_info = self
            .client
            .collection_info(&self.collection_name)
            .await
            .map_err(|e| {
                Error::config(format!(
                    "Failed to get collection info for '{}': {}. \
                     Collection may not exist. Create the collection first.",
                    self.collection_name, e
                ))
            })?;

        // Validate based on retrieval mode
        match self.retrieval_mode {
            RetrievalMode::Dense => {
                self.validate_collection_for_dense(
                    collection_info
                        .result
                        .as_ref()
                        .and_then(|r| r.config.as_ref())
                        .ok_or_else(|| {
                            Error::config(format!(
                                "Collection '{}' has no config",
                                self.collection_name
                            ))
                        })?,
                )
                .await?;
            }
            RetrievalMode::Sparse => {
                // Sparse mode: Deferred - requires SparseEmbeddings trait
                return Err(Error::config(
                    "Sparse retrieval mode deferred - use Dense mode",
                ));
            }
            RetrievalMode::Hybrid => {
                // Hybrid mode: Deferred - requires both Dense and Sparse support
                return Err(Error::config(
                    "Hybrid retrieval mode deferred - use Dense mode",
                ));
            }
        }

        Ok(())
    }

    /// Validates collection configuration for dense vector retrieval.
    async fn validate_collection_for_dense(
        &self,
        config: &qdrant_client::qdrant::CollectionConfig,
    ) -> Result<()> {
        // Get vector config
        let vectors_config = config
            .params
            .as_ref()
            .and_then(|p| p.vectors_config.as_ref())
            .ok_or_else(|| {
                Error::config(format!(
                    "Collection '{}' has no vector configuration",
                    self.collection_name
                ))
            })?;

        // Extract vector params based on whether vectors are named or unnamed
        let vector_params = match vectors_config.config.as_ref() {
            Some(qdrant_client::qdrant::vectors_config::Config::Params(params)) => {
                // Single unnamed vector
                if !self.vector_name.is_empty() {
                    return Err(Error::config(format!(
                        "Existing Qdrant collection '{}' is built with unnamed dense vector. \
                         If you want to reuse it, set `vector_name` to '' (empty string). \
                         If you want to recreate the collection, create a new collection.",
                        self.collection_name
                    )));
                }
                params
            }
            Some(qdrant_client::qdrant::vectors_config::Config::ParamsMap(map)) => {
                // Named vectors
                map.map.get(&self.vector_name).ok_or_else(|| {
                    let available: Vec<_> = map.map.keys().collect();
                    Error::config(format!(
                        "Existing Qdrant collection '{}' does not contain dense vector named '{}'. \
                         Did you mean one of the existing vectors: {:?}? \
                         If you want to recreate the collection, create a new collection.",
                        self.collection_name, self.vector_name, available
                    ))
                })?
            }
            None => {
                return Err(Error::config(format!(
                    "Collection '{}' has invalid vector configuration",
                    self.collection_name
                )));
            }
        };

        // Validate vector dimensions
        if let Some(embeddings) = &self.embeddings {
            // Embed a dummy text to get vector size using graph API
            let dummy_vector = embed_query(Arc::clone(embeddings), "dummy_text")
                .await
                .map_err(|e| Error::config(format!("Failed to get embedding dimensions: {e}")))?;
            let vector_size = dummy_vector.len() as u64;

            if vector_params.size != vector_size {
                return Err(Error::config(format!(
                    "Existing Qdrant collection is configured for dense vectors with {} dimensions. \
                     Selected embeddings are {}-dimensional. \
                     If you want to recreate the collection, create a new collection with correct dimensions.",
                    vector_params.size, vector_size
                )));
            }
        }

        // Validate distance metric
        let expected_distance = self.distance_metric_to_qdrant();
        let actual_distance = Distance::try_from(vector_params.distance)
            .map_err(|e| Error::config(format!("Invalid distance metric in collection: {e:?}")))?;

        if actual_distance != expected_distance {
            return Err(Error::config(format!(
                "Existing Qdrant collection is configured for {actual_distance:?} similarity, \
                 but requested {expected_distance:?}. Please set distance metric to {actual_distance:?} if you want to reuse it. \
                 If you want to recreate the collection, create a new collection with correct metric."
            )));
        }

        Ok(())
    }

    /// Converts our `DistanceMetric` to Qdrant's Distance enum.
    fn distance_metric_to_qdrant(&self) -> Distance {
        match self.distance_metric {
            DistanceMetric::Cosine => Distance::Cosine,
            DistanceMetric::Euclidean => Distance::Euclid,
            DistanceMetric::DotProduct => Distance::Dot,
            DistanceMetric::MaxInnerProduct => Distance::Dot, // MIP uses Dot in Qdrant
        }
    }

    /// Converts document content and metadata to a Qdrant Payload.
    ///
    /// This method builds a Qdrant Payload object with the document content stored
    /// under `content_key` (default: "`page_content`") and metadata stored under
    /// `metadata_key` (default: "metadata").
    ///
    /// # Arguments
    ///
    /// * `content` - The document text content
    /// * `metadata` - Optional metadata as a `HashMap` of JSON values
    ///
    /// # Returns
    ///
    /// A `Payload` object suitable for insertion into Qdrant. The structure is:
    /// ```json
    /// {
    ///   "page_content": "document text",
    ///   "metadata": {
    ///     "key1": "value1",
    ///     "key2": 123,
    ///     ...
    ///   }
    /// }
    /// ```
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches the payload structure created by Python's `_build_payloads()` function
    /// in `dashflow_qdrant.qdrant.py`. The qdrant-client crate provides automatic
    /// conversion from `serde_json::Value` to `qdrant::Value`, so this method
    /// uses that conversion for all nested metadata values.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::{QdrantVectorStore, RetrievalMode};
    /// # use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
    /// # use std::collections::HashMap;
    /// # use std::sync::Arc;
    /// # use serde_json::json;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
    /// # let store = QdrantVectorStore::new("http://localhost:6334", "test", Some(embeddings), RetrievalMode::Dense).await?;
    /// let mut metadata = HashMap::new();
    /// metadata.insert("author".to_string(), json!("Alice"));
    /// metadata.insert("year".to_string(), json!(2024));
    ///
    /// let payload = store.build_payload("Hello world", Some(&metadata));
    /// # Ok(())
    /// # }
    /// ```
    fn build_payload(
        &self,
        content: &str,
        metadata: Option<&HashMap<String, JsonValue>>,
    ) -> Payload {
        let mut payload = Payload::new();

        // Add document content
        payload.insert(&self.content_key, content.to_string());

        // Add metadata (either the provided metadata or null)
        if let Some(metadata) = metadata {
            // Convert HashMap<String, JsonValue> to serde_json::Value::Object
            let metadata_object =
                serde_json::Map::from_iter(metadata.iter().map(|(k, v)| (k.clone(), v.clone())));
            payload.insert(&self.metadata_key, JsonValue::Object(metadata_object));
        } else {
            // Python baseline stores None as null in the payload
            payload.insert(&self.metadata_key, JsonValue::Null);
        }

        payload
    }

    /// Converts a Qdrant payload back to document content and metadata.
    ///
    /// This is the reverse operation of the internal payload building process.
    /// It extracts the document content and metadata from a Qdrant point's payload,
    /// following the same key configuration.
    ///
    /// # Arguments
    ///
    /// * `payload` - The Qdrant payload as a `HashMap` of `qdrant::Value` entries
    ///
    /// # Returns
    ///
    /// Returns a tuple of `(content, metadata)`:
    /// * `content` - The document text extracted from `content_key`
    /// * `metadata` - The metadata as `HashMap<String, JsonValue>`
    ///
    /// # Behavior
    ///
    /// **Python Baseline Compatibility**:
    /// Matches `_document_from_scored_point()` in `dashflow_qdrant/qdrant.py:1030-1036`:
    /// - Missing content → empty string `""`
    /// - Missing metadata → empty `HashMap` `{}`
    /// - Null metadata → empty `HashMap` `{}`
    ///
    /// **Content Extraction**:
    /// - Extracts value from `payload[content_key]`
    /// - If missing or not a string: returns empty string `""`
    /// - Non-string types (number, bool, null, etc.) are invalid → empty string
    ///
    /// **Metadata Extraction**:
    /// - Extracts value from `payload[metadata_key]`
    /// - If missing: returns empty `HashMap`
    /// - If null: returns empty `HashMap` (matches Python `or {}` behavior)
    /// - If not an object/map: returns empty `HashMap`
    /// - Converts `qdrant::Value` → `serde_json::Value` for each field
    ///
    /// # Type Conversion
    ///
    /// Qdrant values are automatically converted to JSON values:
    /// - `StringValue` → `JsonValue::String`
    /// - `IntegerValue` → `JsonValue::Number` (i64)
    /// - `DoubleValue` → `JsonValue::Number` (f64)
    /// - `BoolValue` → `JsonValue::Bool`
    /// - `NullValue` → `JsonValue::Null`
    /// - `ListValue` → `JsonValue::Array` (recursive)
    /// - `StructValue` → `JsonValue::Object` (recursive)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # use std::collections::HashMap;
    /// # use qdrant_client::Payload;
    /// # async fn example(store: &QdrantVectorStore, payload: HashMap<String, qdrant::Value>) {
    /// let (content, metadata) = store.payload_to_document(&payload);
    /// println!("Content: {}", content);
    /// println!("Metadata: {:?}", metadata);
    /// # }
    /// ```
    ///
    /// # Related Methods
    ///
    /// - [`with_content_key()`](Self::with_content_key) - Sets custom content key
    /// - [`with_metadata_key()`](Self::with_metadata_key) - Sets custom metadata key
    fn payload_to_document(
        &self,
        payload: &HashMap<String, qdrant::Value>,
    ) -> (String, HashMap<String, JsonValue>) {
        use qdrant::value::Kind;

        // Extract content from payload[content_key]
        // Python: payload.get(content_payload_key, "")
        let content = payload
            .get(&self.content_key)
            .and_then(|v| {
                // Extract string value from qdrant::Value
                if let Some(Kind::StringValue(s)) = &v.kind {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        // Extract metadata from payload[metadata_key]
        // Python: payload.get(metadata_payload_key) or {}
        let metadata = payload
            .get(&self.metadata_key)
            .and_then(|v| self.qdrant_value_to_json(v))
            .and_then(|json_val| {
                // Convert JsonValue::Object to HashMap
                if let JsonValue::Object(map) = json_val {
                    Some(map.into_iter().collect())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        (content, metadata)
    }

    /// Converts a `qdrant::Value` to `serde_json::Value` recursively.
    ///
    /// This handles all Qdrant value types including nested structures.
    /// Used by [`payload_to_document()`](Self::payload_to_document) for metadata conversion.
    ///
    /// # Arguments
    ///
    /// * `value` - The `qdrant::Value` to convert
    ///
    /// # Returns
    ///
    /// The equivalent `serde_json::Value`, or `None` if conversion fails
    ///
    /// # Type Mapping
    ///
    /// - `NullValue` → `Null`
    /// - `BoolValue` → `Bool`
    /// - `IntegerValue` → `Number` (i64)
    /// - `DoubleValue` → `Number` (f64)
    /// - `StringValue` → `String`
    /// - `ListValue` → `Array` (recursive conversion)
    /// - `StructValue` → `Object` (recursive conversion)
    #[allow(clippy::only_used_in_recursion)] // Method for API consistency; self used only in recursive calls
    fn qdrant_value_to_json(&self, value: &qdrant::Value) -> Option<JsonValue> {
        use qdrant::value::Kind;

        match &value.kind {
            Some(Kind::NullValue(_)) => Some(JsonValue::Null),
            Some(Kind::BoolValue(b)) => Some(JsonValue::Bool(*b)),
            Some(Kind::IntegerValue(i)) => Some(JsonValue::Number(serde_json::Number::from(*i))),
            Some(Kind::DoubleValue(f)) => serde_json::Number::from_f64(*f).map(JsonValue::Number),
            Some(Kind::StringValue(s)) => Some(JsonValue::String(s.clone())),
            Some(Kind::ListValue(list)) => {
                let values: Vec<JsonValue> = list
                    .values
                    .iter()
                    .filter_map(|v| self.qdrant_value_to_json(v))
                    .collect();
                Some(JsonValue::Array(values))
            }
            Some(Kind::StructValue(struct_val)) => {
                let map: serde_json::Map<String, JsonValue> = struct_val
                    .fields
                    .iter()
                    .filter_map(|(k, v)| {
                        self.qdrant_value_to_json(v)
                            .map(|json_v| (k.clone(), json_v))
                    })
                    .collect();
                Some(JsonValue::Object(map))
            }
            None => None,
        }
    }

    /// Adds texts to the Qdrant vector store with optional metadata and IDs.
    ///
    /// This is the primary method for inserting documents into Qdrant. It handles:
    /// - Embedding texts into vectors (dense/sparse/hybrid based on retrieval mode)
    /// - Converting texts and metadata into Qdrant payloads
    /// - Generating or validating IDs for each point
    /// - Upserting points to the Qdrant collection
    ///
    /// # Arguments
    ///
    /// * `texts` - Texts to add to the vector store. Each text becomes a separate point.
    /// * `metadatas` - Optional metadata for each text. If provided, must have same length as `texts`.
    /// * `ids` - Optional IDs for each point. If None, UUIDs are generated automatically.
    /// * `batch_size` - Number of points to upsert in each batch. Default is 64 (matches Python).
    ///
    /// # Returns
    ///
    /// Returns a `Vec<String>` of IDs for the added points. These IDs can be used to
    /// retrieve or delete the points later.
    ///
    /// # Behavior
    ///
    /// **Python Baseline Compatibility**:
    /// Matches `add_texts()` in `dashflow_qdrant/qdrant.py:495-518`:
    /// - Accepts texts, metadatas, and ids
    /// - Returns list of IDs (Python returns `list[str | int]`, we return `Vec<String>`)
    /// - Python supports batching with `batch_size` parameter (not implemented yet)
    /// - Python uses `_generate_batches()` which creates batches and calls `client.upsert()`
    /// - Currently implements single-batch version (all points in one upsert)
    ///
    /// **Processing Steps**:
    /// 1. Convert texts to `Vec<String>` for processing
    /// 2. Validate metadata count matches text count (if provided)
    /// 3. Generate or validate IDs (using internal ID generation)
    /// 4. Build vectors by embedding texts
    /// 5. Build payloads with metadata for each text
    /// 6. Create `PointStruct` for each (id, vector, payload) tuple
    /// 7. Call `client.upsert()` to insert all points
    /// 8. Return the IDs that were inserted
    ///
    /// **Batching**:
    /// - Python implementation supports batching (default `batch_size=64`)
    /// - Rust implementation matches Python batching behavior
    /// - Splits texts/metadata/IDs into batches and processes each separately
    /// - Each batch is upserted independently to prevent timeouts with large datasets
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Metadata count does not match text count (when metadata provided)
    /// - ID count does not match text count (when IDs provided)
    /// - Embeddings provider is not configured (DENSE mode)
    /// - Embedding process fails
    /// - Qdrant client upsert fails
    /// - SPARSE or HYBRID mode requested (requires sparse vector encoders)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # use std::collections::HashMap;
    /// # async fn example(mut store: QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// // Add texts without metadata or IDs (UUIDs generated automatically)
    /// let texts = vec!["Hello world", "Goodbye world"];
    /// let ids = store.add_texts(&texts, None, None, 64).await?;
    /// assert_eq!(ids.len(), 2);
    ///
    /// // Add texts with metadata
    /// let mut meta1 = HashMap::new();
    /// meta1.insert("category".to_string(), serde_json::json!("greeting"));
    /// let mut meta2 = HashMap::new();
    /// meta2.insert("category".to_string(), serde_json::json!("farewell"));
    /// let metadatas = vec![meta1, meta2];
    /// let ids = store.add_texts(&texts, Some(&metadatas), None, 64).await?;
    ///
    /// // Add texts with custom IDs and custom batch size
    /// let custom_ids = vec!["id1".to_string(), "id2".to_string()];
    /// let ids = store.add_texts(&texts, None, Some(&custom_ids), 32).await?;
    /// assert_eq!(ids, custom_ids);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # ID Format
    ///
    /// When IDs are auto-generated (ids=None):
    /// - Uses UUID v4 in simple format (32 hex chars, no dashes)
    /// - Example: "550e8400e29b41d4a716446655440000"
    /// - Matches Python's `uuid.uuid4().hex` format
    ///
    /// # Related Methods
    ///
    /// - `similarity_search()` - Search for similar documents
    /// - `delete()` - Delete documents by IDs
    pub async fn add_texts(
        &mut self,
        texts: &[impl AsRef<str>],
        metadatas: Option<&[HashMap<String, JsonValue>]>,
        ids: Option<&[String]>,
        batch_size: usize,
    ) -> Result<Vec<String>> {
        // Convert texts to Vec<String>
        let text_strings: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();

        // Handle empty texts
        if text_strings.is_empty() {
            return Ok(Vec::new());
        }

        // Validate metadata count matches text count
        if let Some(metas) = metadatas {
            if metas.len() != text_strings.len() {
                return Err(Error::InvalidInput(format!(
                    "Metadata count ({}) does not match texts count ({})",
                    metas.len(),
                    text_strings.len()
                )));
            }
        }

        // Generate or validate IDs for all texts
        let point_ids = self.generate_ids(&text_strings, ids)?;

        // Process texts in batches
        let mut added_ids = Vec::with_capacity(point_ids.len());

        for batch_start in (0..text_strings.len()).step_by(batch_size) {
            let batch_end = (batch_start + batch_size).min(text_strings.len());

            // Get batch slices
            let batch_texts = &text_strings[batch_start..batch_end];
            let batch_ids = &point_ids[batch_start..batch_end];
            let batch_metadatas = metadatas.map(|metas| &metas[batch_start..batch_end]);

            // Build vectors for batch
            let batch_vectors = self.build_vectors(batch_texts).await?;

            // Build payloads for batch
            let mut batch_payloads: Vec<Payload> = Vec::with_capacity(batch_texts.len());
            for (i, text) in batch_texts.iter().enumerate() {
                let metadata = batch_metadatas.and_then(|metas| metas.get(i));
                batch_payloads.push(self.build_payload(text, metadata));
            }

            // Create PointStruct for each (id, vector, payload) in batch
            let batch_points: Vec<qdrant::PointStruct> = batch_ids
                .iter()
                .zip(batch_vectors.iter())
                .zip(batch_payloads.iter())
                .map(|((id, vector), payload)| qdrant::PointStruct {
                    id: Some(qdrant::PointId::from(id.as_str())),
                    vectors: Some(vector.clone()),
                    payload: payload.clone().into(),
                })
                .collect();

            // Upsert batch to Qdrant using UpsertPointsBuilder
            let upsert_request = UpsertPointsBuilder::new(&self.collection_name, batch_points);
            self.client
                .upsert_points(upsert_request)
                .await
                .map_err(|e| Error::other(format!("Failed to upsert batch to Qdrant: {e}")))?;

            // Collect IDs from this batch
            added_ids.extend_from_slice(batch_ids);
        }

        // Return all IDs
        Ok(added_ids)
    }

    /// Performs similarity search using a pre-computed embedding vector and returns documents with scores.
    ///
    /// This method queries Qdrant for the most similar documents to the provided embedding vector.
    /// Returns both the matching documents and their similarity scores.
    ///
    /// # Arguments
    ///
    /// * `embedding` - The query embedding vector (dense vector for DENSE mode)
    /// * `k` - Number of most similar documents to return (default: 4)
    /// * `filter` - Optional Qdrant filter to restrict search space
    /// * `search_params` - Optional Qdrant search parameters for tuning
    /// * `offset` - Number of results to skip (for pagination, default: 0)
    /// * `score_threshold` - Optional minimum similarity score threshold
    ///
    /// # Returns
    ///
    /// Returns a `Vec<(Document, f32)>` where each tuple contains:
    /// - `Document`: The matching document with content and metadata
    /// - `f32`: The similarity score (higher is more similar)
    ///
    /// # Behavior
    ///
    /// **Python Baseline Compatibility**:
    /// Matches `similarity_search_with_score_by_vector()` in `dashflow_qdrant/qdrant.py:645-697`:
    /// - Accepts embedding, k, filter, `search_params`, offset, `score_threshold`
    /// - Calls `_validate_collection_for_dense()` to validate configuration
    /// - Uses `client.query_points()` to query Qdrant
    /// - Returns list of `(Document, score)` tuples
    ///
    /// **Processing Steps**:
    /// 1. Validate collection configuration (DENSE mode only)
    /// 2. Call `client.query_points()` with query vector and parameters
    /// 3. Convert each `ScoredPoint` to `(Document, score)` tuple:
    ///    - Extract content from payload using `content_key`
    ///    - Extract metadata from payload using `metadata_key`
    ///    - Add `_id` and `_collection_name` to metadata
    ///    - Create `Document` with content and metadata
    ///    - Include similarity score from Qdrant response
    /// 4. Return list of `(Document, score)` tuples
    ///
    /// **Metadata Injection**:
    /// The Python baseline adds two special fields to each document's metadata:
    /// - `_id`: The Qdrant point ID (string or integer)
    /// - `_collection_name`: The Qdrant collection name
    ///
    /// This allows downstream code to track document origins and IDs.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Collection configuration validation fails
    /// - Qdrant query fails
    /// - Payload extraction fails
    /// - SPARSE or HYBRID mode requested (requires sparse vector encoders)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// // Basic similarity search with pre-computed embedding
    /// let embedding = vec![0.1, 0.2, 0.3, /* ... */];
    /// let results = store.similarity_search_with_score_by_vector(
    ///     &embedding,
    ///     5,      // top 5 results
    ///     None,   // no filter
    ///     None,   // default search params
    ///     0,      // no offset
    ///     None,   // no score threshold
    /// ).await?;
    ///
    /// for (doc, score) in results {
    ///     println!("Score: {}, Content: {}", score, doc.page_content);
    ///     println!("ID: {:?}", doc.metadata.get("_id"));
    /// }
    ///
    /// // With score threshold to filter low-quality matches
    /// let results = store.similarity_search_with_score_by_vector(
    ///     &embedding,
    ///     10,
    ///     None,
    ///     None,
    ///     0,
    ///     Some(0.7),  // only return results with score >= 0.7
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Python Baseline Reference
    ///
    /// Python `similarity_search_with_score_by_vector()` in `dashflow_qdrant/qdrant.py:645-697`
    ///
    /// # See Also
    ///
    /// - [`similarity_search_by_vector()`](Self::similarity_search_by_vector) - Same search without scores
    /// - [`similarity_search()`](Self::similarity_search) - Search with query text (embeds automatically)
    /// - [`add_texts()`](Self::add_texts) - Add documents to the vector store
    pub async fn similarity_search_with_score_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<qdrant::Filter>,
        search_params: Option<qdrant::SearchParams>,
        offset: usize,
        score_threshold: Option<f32>,
    ) -> Result<Vec<(Document, f32)>> {
        // Step 1: Validate collection configuration for DENSE mode
        // Python: self._validate_collection_for_dense(...)
        // Rust: self.validate_collection_config() -> validate_collection_for_dense()
        self.validate_collection_config().await?;

        // Step 2: Query Qdrant for similar points
        // Python: results = self.client.query_points(...)
        let query_result = self
            .client
            .query(
                qdrant::QueryPointsBuilder::new(&self.collection_name)
                    .query(embedding.to_vec())
                    .limit(k as u64)
                    .offset(offset as u64)
                    .filter(filter.unwrap_or_default())
                    .with_payload(true)
                    .with_vectors(false)
                    .params(search_params.unwrap_or_default())
                    .score_threshold(score_threshold.unwrap_or(0.0)),
            )
            .await
            .map_err(|e| Error::other(format!("Failed to query Qdrant: {e}")))?;

        // Step 3: Convert ScoredPoints to (Document, score) tuples
        // Python: return [(self._document_from_point(...), result.score) for result in results]
        let results = query_result
            .result
            .into_iter()
            .map(|scored_point| {
                // Extract payload
                let payload: HashMap<String, qdrant::Value> =
                    scored_point.payload.into_iter().collect();

                // Convert payload to document (content, metadata)
                let (content, mut metadata) = self.payload_to_document(&payload);

                // Add special metadata fields (_id, _collection_name)
                // Python: metadata["_id"] = scored_point.id
                // Python: metadata["_collection_name"] = collection_name
                if let Some(point_id) = &scored_point.id {
                    // Convert PointId to string for metadata
                    let id_str = match point_id.point_id_options {
                        Some(qdrant::point_id::PointIdOptions::Num(n)) => n.to_string(),
                        Some(qdrant::point_id::PointIdOptions::Uuid(ref s)) => s.clone(),
                        None => String::new(),
                    };
                    metadata.insert("_id".to_string(), JsonValue::String(id_str));
                }
                metadata.insert(
                    "_collection_name".to_string(),
                    JsonValue::String(self.collection_name.clone()),
                );

                // Create Document with content and metadata
                let document = Document {
                    page_content: content,
                    metadata,
                    id: None,
                };

                // Return (Document, score) tuple
                (document, scored_point.score)
            })
            .collect();

        Ok(results)
    }

    /// Searches for documents similar to the given embedding vector.
    ///
    /// This is a convenience wrapper around [`similarity_search_with_score_by_vector()`](Self::similarity_search_with_score_by_vector)
    /// that returns only the documents without their similarity scores.
    ///
    /// # Arguments
    ///
    /// * `embedding` - The query embedding vector to search for
    /// * `k` - Number of documents to return (default: 4 in Python, required here)
    /// * `filter` - Optional Qdrant filter to restrict search space
    /// * `search_params` - Optional Qdrant search parameters for tuning
    /// * `offset` - Offset for pagination (default: 0)
    /// * `score_threshold` - Optional minimum similarity score threshold
    ///
    /// # Returns
    ///
    /// Returns a `Vec<Document>` containing the k most similar documents.
    /// Documents are ordered by similarity (most similar first).
    ///
    /// # Behavior
    ///
    /// **Python Baseline Compatibility**:
    /// Matches `similarity_search_by_vector()` in `dashflow_qdrant/qdrant.py:699-726`:
    /// - Calls `similarity_search_with_score_by_vector()` with all parameters
    /// - Uses `map(itemgetter(0), results)` to extract documents (ignore scores)
    /// - Returns `list[Document]` (no scores)
    ///
    /// **Processing Steps**:
    /// 1. Calls `similarity_search_with_score_by_vector()` with all parameters
    /// 2. Extracts documents from `(Document, score)` tuples
    /// 3. Returns `Vec<Document>`
    ///
    /// **Metadata**:
    /// Each returned document includes metadata fields:
    /// - `_id`: Point ID from Qdrant
    /// - `_collection_name`: Name of the collection
    /// - All original metadata from the document payload
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Collection configuration is invalid (see [`validate_collection_config()`](Self::validate_collection_config))
    /// - Qdrant query fails
    /// - Collection does not exist
    ///
    /// # Examples
    ///
    /// ## Basic Search
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// let embedding = vec![0.1, 0.2, 0.3, 0.4]; // Query vector
    /// let docs = store.similarity_search_by_vector(&embedding, 5, None, None, 0, None).await?;
    /// println!("Found {} documents", docs.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With Score Threshold
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// let embedding = vec![0.1, 0.2, 0.3, 0.4];
    /// let docs = store
    ///     .similarity_search_by_vector(&embedding, 10, None, None, 0, Some(0.8))
    ///     .await?;
    /// // Returns only documents with similarity >= 0.8
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With Pagination
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// let embedding = vec![0.1, 0.2, 0.3, 0.4];
    /// // Get first page (results 0-4)
    /// let page1 = store.similarity_search_by_vector(&embedding, 5, None, None, 0, None).await?;
    /// // Get second page (results 5-9)
    /// let page2 = store.similarity_search_by_vector(&embedding, 5, None, None, 5, None).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    ///
    /// - [`similarity_search_with_score_by_vector()`](Self::similarity_search_with_score_by_vector) - Returns documents with scores
    /// - [`similarity_search()`](Self::similarity_search) - Searches by text query (embeds query first)
    /// - [`similarity_search_with_score()`](Self::similarity_search_with_score) - Searches by text query with scores
    ///
    /// # See Also
    ///
    /// Python `similarity_search_by_vector()` in `dashflow_qdrant/qdrant.py:699-726`
    ///
    /// Searches for documents similar to the given query text, returning documents with similarity scores.
    ///
    /// This method embeds the query text using the configured embeddings provider,
    /// then searches for similar documents in the Qdrant collection.
    ///
    /// # Arguments
    ///
    /// * `query` - The query text to search for
    /// * `k` - Number of documents to return (default: 4 in Python, required here)
    /// * `filter` - Optional Qdrant filter to restrict search space
    /// * `search_params` - Optional Qdrant search parameters for tuning
    /// * `offset` - Offset for pagination (default: 0)
    /// * `score_threshold` - Optional minimum similarity score threshold
    ///
    /// # Returns
    ///
    /// Returns a `Vec<(Document, f32)>` where each tuple contains:
    /// - `Document`: The document with content and metadata
    /// - `f32`: The similarity score (higher = more similar)
    ///
    /// Documents are ordered by similarity (most similar first).
    ///
    /// # Behavior
    ///
    /// **Python Baseline Compatibility**:
    /// Matches `similarity_search_with_score()` in `dashflow_qdrant/qdrant.py:551-643`:
    /// - Validates embeddings provider exists: `embeddings = self._require_embeddings("DENSE mode")` (line 582)
    /// - Embeds query text: `query_dense_embedding = embeddings.embed_query(query)` (line 583)
    /// - Calls Qdrant query with embedded vector (line 584-588)
    /// - Returns `list[tuple[Document, float]]` with scores (line 632-643)
    ///
    /// **Processing Steps**:
    /// 1. Validates embeddings provider exists (returns Error if None)
    /// 2. Embeds query text using `embeddings.embed_query(query)`
    /// 3. Calls `similarity_search_with_score_by_vector()` with embedded query
    /// 4. Returns `Vec<(Document, f32)>`
    ///
    /// **Metadata**:
    /// Each returned document includes metadata fields:
    /// - `_id`: Point ID from Qdrant
    /// - `_collection_name`: Name of the collection
    /// - All original metadata from the document payload
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - No embeddings provider configured
    /// - Embeddings provider fails to embed query text
    /// - Collection configuration is invalid (see [`validate_collection_config()`](Self::validate_collection_config))
    /// - Qdrant query fails
    /// - Collection does not exist
    ///
    /// # Examples
    ///
    /// ## Basic Search with Scores
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// let results = store
    ///     .similarity_search_with_score("machine learning", 5, None, None, 0, None)
    ///     .await?;
    ///
    /// for (doc, score) in results {
    ///     println!("Document: {} (score: {})", doc.page_content, score);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With Score Threshold
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// let results = store
    ///     .similarity_search_with_score("rust programming", 10, None, None, 0, Some(0.8))
    ///     .await?;
    /// // Returns only documents with similarity >= 0.8
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With Pagination
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// // Get first page (results 0-4)
    /// let page1 = store
    ///     .similarity_search_with_score("deep learning", 5, None, None, 0, None)
    ///     .await?;
    /// // Get second page (results 5-9)
    /// let page2 = store
    ///     .similarity_search_with_score("deep learning", 5, None, None, 5, None)
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    ///
    /// - [`similarity_search()`](Self::similarity_search) - Returns documents without scores
    /// - [`similarity_search_by_vector()`](Self::similarity_search_by_vector) - Searches by embedding vector
    /// - [`similarity_search_with_score_by_vector()`](Self::similarity_search_with_score_by_vector) - Searches by vector with scores
    ///
    /// # See Also
    ///
    /// Python `similarity_search_with_score()` in `dashflow_qdrant/qdrant.py:551-643`
    pub async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<qdrant::Filter>,
        search_params: Option<qdrant::SearchParams>,
        offset: usize,
        score_threshold: Option<f32>,
    ) -> Result<Vec<(Document, f32)>> {
        // Step 1: Validate embeddings provider exists
        // Python: embeddings = self._require_embeddings("DENSE mode")
        let embeddings = self.embeddings.as_ref().ok_or_else(|| {
            Error::config("Embeddings provider required for similarity_search_with_score()")
        })?;

        // Step 2: Embed query text using graph API
        // Python: query_dense_embedding = embeddings.embed_query(query)
        let embedding = embed_query(Arc::clone(embeddings), query)
            .await
            .map_err(|e| Error::other(format!("Failed to embed query: {e}")))?;

        // Step 3: Call similarity_search_with_score_by_vector with embedded query
        // Python: results = self.client.query_points(query=query_dense_embedding, ...)
        self.similarity_search_with_score_by_vector(
            &embedding,
            k,
            filter,
            search_params,
            offset,
            score_threshold,
        )
        .await
    }

    /// Searches for documents similar to the given query text.
    ///
    /// This is a convenience wrapper that embeds the query text, searches for similar
    /// documents, and returns only the documents without similarity scores.
    ///
    /// # Arguments
    ///
    /// * `query` - The query text to search for
    /// * `k` - Number of documents to return (default: 4 in Python, required here)
    /// * `filter` - Optional Qdrant filter to restrict search space
    /// * `search_params` - Optional Qdrant search parameters for tuning
    /// * `offset` - Offset for pagination (default: 0)
    /// * `score_threshold` - Optional minimum similarity score threshold
    ///
    /// # Returns
    ///
    /// Returns a `Vec<Document>` containing the k most similar documents.
    /// Documents are ordered by similarity (most similar first).
    ///
    /// # Behavior
    ///
    /// **Python Baseline Compatibility**:
    /// Matches `similarity_search()` in `dashflow_qdrant/qdrant.py:520-549`:
    /// - Calls `similarity_search_with_score()` with all parameters (line 538-548)
    /// - Uses `map(itemgetter(0), results)` to extract documents (line 549)
    /// - Returns `list[Document]` (no scores)
    ///
    /// **Processing Steps**:
    /// 1. Calls `similarity_search_with_score()` with all parameters
    /// 2. Extracts documents from `(Document, score)` tuples
    /// 3. Returns `Vec<Document>`
    ///
    /// **Metadata**:
    /// Each returned document includes metadata fields:
    /// - `_id`: Point ID from Qdrant
    /// - `_collection_name`: Name of the collection
    /// - All original metadata from the document payload
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - No embeddings provider configured
    /// - Embeddings provider fails to embed query text
    /// - Collection configuration is invalid (see [`validate_collection_config()`](Self::validate_collection_config))
    /// - Qdrant query fails
    /// - Collection does not exist
    ///
    /// # Examples
    ///
    /// ## Basic Search
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// let docs = store._similarity_search("machine learning", 5, None, None, 0, None).await?;
    /// for doc in docs {
    ///     println!("Document: {}", doc.page_content);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With Score Threshold
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// let docs = store
    ///     ._similarity_search("rust programming", 10, None, None, 0, Some(0.8))
    ///     .await?;
    /// // Returns only documents with similarity >= 0.8
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With Pagination
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// // Get first page (results 0-4)
    /// let page1 = store._similarity_search("deep learning", 5, None, None, 0, None).await?;
    /// // Get second page (results 5-9)
    /// let page2 = store._similarity_search("deep learning", 5, None, None, 5, None).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    ///
    /// - [`similarity_search_with_score()`](Self::similarity_search_with_score) - Returns documents with scores
    /// - [`similarity_search_by_vector()`](Self::similarity_search_by_vector) - Searches by embedding vector
    /// - [`similarity_search_with_score_by_vector()`](Self::similarity_search_with_score_by_vector) - Searches by vector with scores
    ///
    /// # See Also
    ///
    /// Python `similarity_search()` in `dashflow_qdrant/qdrant.py:520-549`
    pub async fn similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<qdrant::Filter>,
        search_params: Option<qdrant::SearchParams>,
        offset: usize,
        score_threshold: Option<f32>,
    ) -> Result<Vec<Document>> {
        // Call similarity_search_with_score and extract documents
        // Python: list(map(itemgetter(0), results))
        let results = self
            .similarity_search_with_score(query, k, filter, search_params, offset, score_threshold)
            .await?;

        // Extract documents from (Document, score) tuples
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    pub async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<qdrant::Filter>,
        search_params: Option<qdrant::SearchParams>,
        offset: usize,
        score_threshold: Option<f32>,
    ) -> Result<Vec<Document>> {
        // Call similarity_search_with_score_by_vector and extract documents
        // Python: list(map(itemgetter(0), results))
        let results = self
            .similarity_search_with_score_by_vector(
                embedding,
                k,
                filter,
                search_params,
                offset,
                score_threshold,
            )
            .await?;

        // Extract documents from (Document, score) tuples
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    /// Searches for documents using maximal marginal relevance with a text query.
    ///
    /// Maximal marginal relevance (MMR) optimizes for similarity to the query AND
    /// diversity among selected documents. This is useful when you want results that
    /// are relevant but not redundant.
    ///
    /// # Arguments
    ///
    /// * `query` - Text query to search for
    /// * `k` - Number of documents to return (default: 4 in Python)
    /// * `fetch_k` - Number of candidates to fetch for MMR re-ranking (default: 20 in Python)
    /// * `lambda_mult` - Diversity parameter in [0, 1]. Higher values favor diversity (default: 0.5)
    ///   - 1.0 = maximum diversity (ignores relevance)
    ///   - 0.0 = maximum relevance (ignores diversity)
    /// * `filter` - Optional Qdrant filter to restrict search space
    /// * `search_params` - Optional Qdrant search parameters
    /// * `score_threshold` - Optional minimum similarity score threshold
    ///
    /// # Returns
    ///
    /// Returns a `Vec<Document>` containing the k documents selected by MMR.
    ///
    /// # Behavior
    ///
    /// **Python Baseline Compatibility**:
    /// Matches `max_marginal_relevance_search()` in `dashflow_qdrant/qdrant.py:728-769`:
    /// - Validates collection configuration for DENSE mode
    /// - Embeds query text using `embeddings.embed_query()`
    /// - Calls `max_marginal_relevance_search_by_vector()` with query embedding
    ///
    /// **Algorithm**:
    /// 1. Validates embeddings provider exists
    /// 2. Embeds query text to vector
    /// 3. Uses Qdrant's built-in MMR algorithm via `query_points` with `NearestQuery`
    ///    containing `Mmr` configuration
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Get diverse results about "machine learning"
    /// let docs = store.max_marginal_relevance_search(
    ///     "machine learning",
    ///     5,      // Return 5 documents
    ///     20,     // Consider 20 candidates
    ///     0.7,    // Favor diversity
    ///     None,
    ///     None,
    ///     None,
    /// ).await?;
    /// ```
    #[allow(clippy::too_many_arguments)] // MMR search requires k, fetch_k, lambda, filter, params, threshold
    pub async fn max_marginal_relevance_search(
        &self,
        query: &str,
        k: usize,
        fetch_k: usize,
        lambda_mult: f32,
        filter: Option<qdrant::Filter>,
        search_params: Option<qdrant::SearchParams>,
        score_threshold: Option<f32>,
    ) -> Result<Vec<Document>> {
        // Step 1: Validate collection configuration for DENSE mode
        // Python: self._validate_collection_for_dense(...)
        self.validate_collection_config().await?;

        // Step 2: Get embeddings provider (required for MMR with text query)
        // Python: embeddings = self._require_embeddings("max_marginal_relevance_search")
        let embeddings = self.embeddings.as_ref().ok_or_else(|| {
            Error::other("Embeddings provider required for max_marginal_relevance_search")
        })?;

        // Step 3: Embed query text to vector using graph API
        // Python: query_embedding = embeddings.embed_query(query)
        let query_embedding = embed_query(Arc::clone(embeddings), query).await?;

        // Step 4: Call MMR search with query vector
        // Python: return self.max_marginal_relevance_search_by_vector(...)
        self.max_marginal_relevance_search_by_vector(
            &query_embedding,
            k,
            fetch_k,
            lambda_mult,
            filter,
            search_params,
            score_threshold,
        )
        .await
    }

    /// Searches for documents using maximal marginal relevance with a vector query.
    ///
    /// This is a convenience wrapper around [`max_marginal_relevance_search_with_score_by_vector()`](Self::max_marginal_relevance_search_with_score_by_vector)
    /// that returns only the documents without their similarity scores.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Query embedding vector to search for
    /// * `k` - Number of documents to return (default: 4 in Python)
    /// * `fetch_k` - Number of candidates to fetch for MMR re-ranking (default: 20 in Python)
    /// * `lambda_mult` - Diversity parameter in [0, 1]. Higher values favor diversity (default: 0.5)
    /// * `filter` - Optional Qdrant filter to restrict search space
    /// * `search_params` - Optional Qdrant search parameters
    /// * `score_threshold` - Optional minimum similarity score threshold
    ///
    /// # Returns
    ///
    /// Returns a `Vec<Document>` containing the k documents selected by MMR.
    ///
    /// # Behavior
    ///
    /// **Python Baseline Compatibility**:
    /// Matches `max_marginal_relevance_search_by_vector()` in `dashflow_qdrant/qdrant.py:771-803`:
    /// - Calls `max_marginal_relevance_search_with_score_by_vector()`
    /// - Extracts documents from (Document, score) tuples: `list(map(itemgetter(0), results))`
    #[allow(clippy::too_many_arguments)] // MMR search requires k, fetch_k, lambda, filter, params, threshold
    pub async fn max_marginal_relevance_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
        fetch_k: usize,
        lambda_mult: f32,
        filter: Option<qdrant::Filter>,
        search_params: Option<qdrant::SearchParams>,
        score_threshold: Option<f32>,
    ) -> Result<Vec<Document>> {
        // Call max_marginal_relevance_search_with_score_by_vector and extract documents
        // Python: results = self.max_marginal_relevance_search_with_score_by_vector(...)
        // Python: return list(map(itemgetter(0), results))
        let results = self
            .max_marginal_relevance_search_with_score_by_vector(
                embedding,
                k,
                fetch_k,
                lambda_mult,
                filter,
                search_params,
                score_threshold,
            )
            .await?;

        // Extract documents from (Document, score) tuples
        Ok(results.into_iter().map(|(doc, _score)| doc).collect())
    }

    /// Searches for documents using maximal marginal relevance with a vector query,
    /// returning both documents and their similarity scores.
    ///
    /// Maximal marginal relevance (MMR) optimizes for similarity to the query AND
    /// diversity among selected documents using Qdrant's built-in MMR algorithm.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Query embedding vector to search for
    /// * `k` - Number of documents to return (default: 4 in Python)
    /// * `fetch_k` - Number of candidates to fetch for MMR re-ranking (default: 20 in Python)
    /// * `lambda_mult` - Diversity parameter in [0, 1]. Higher values favor diversity (default: 0.5)
    ///   - 1.0 = maximum diversity (ignores relevance)
    ///   - 0.0 = maximum relevance (ignores diversity)
    /// * `filter` - Optional Qdrant filter to restrict search space
    /// * `search_params` - Optional Qdrant search parameters
    /// * `score_threshold` - Optional minimum similarity score threshold
    ///
    /// # Returns
    ///
    /// Returns a `Vec<(Document, f32)>` where each tuple contains:
    /// - Document: The retrieved document with content and metadata
    /// - f32: The similarity score (higher is more similar)
    ///
    /// # Behavior
    ///
    /// **Python Baseline Compatibility**:
    /// Matches `max_marginal_relevance_search_with_score_by_vector()` in `dashflow_qdrant/qdrant.py:805-854`:
    /// - Uses `client.query_points()` with MMR configuration
    /// - MMR parameters: `diversity=lambda_mult`, `candidates_limit=fetch_k`
    /// - Returns documents with similarity scores
    ///
    /// **Algorithm (Qdrant Built-in MMR)**:
    /// 1. Queries Qdrant with `NearestQuery` containing `Mmr` configuration
    /// 2. Qdrant internally:
    ///    - Fetches `fetch_k` candidates by vector similarity
    ///    - Re-ranks candidates using MMR algorithm
    ///    - Returns top `k` diverse results
    /// 3. Converts Qdrant points to (Document, score) tuples
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Embed a query
    /// let query_vec = embeddings.embed_query("machine learning").await?;
    ///
    /// // Get diverse results with scores
    /// let results = store.max_marginal_relevance_search_with_score_by_vector(
    ///     &query_vec,
    ///     5,      // Return 5 documents
    ///     20,     // Consider 20 candidates
    ///     0.7,    // Favor diversity
    ///     None,
    ///     None,
    ///     None,
    /// ).await?;
    ///
    /// for (doc, score) in results {
    ///     println!("Score: {}, Content: {}", score, doc.page_content);
    /// }
    /// ```
    #[allow(clippy::too_many_arguments)] // MMR search requires k, fetch_k, lambda, filter, params, threshold
    pub async fn max_marginal_relevance_search_with_score_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
        fetch_k: usize,
        lambda_mult: f32,
        filter: Option<qdrant::Filter>,
        search_params: Option<qdrant::SearchParams>,
        score_threshold: Option<f32>,
    ) -> Result<Vec<(Document, f32)>> {
        // Step 1: Build MMR configuration
        // Python: mmr=models.Mmr(diversity=lambda_mult, candidates_limit=fetch_k)
        let mmr = qdrant::MmrBuilder::with_params(lambda_mult, fetch_k as u32).build();

        // Step 2: Build query with MMR using new_nearest_with_mmr
        // Python: query=models.NearestQuery(nearest=embedding, mmr=mmr)
        let query = qdrant::Query::new_nearest_with_mmr(embedding.to_vec(), mmr);

        // Step 3: Query Qdrant with MMR
        // Python: results = self.client.query_points(
        //     collection_name=self.collection_name,
        //     query=models.NearestQuery(nearest=embedding, mmr=...),
        //     query_filter=filter,
        //     search_params=search_params,
        //     limit=k,
        //     with_payload=True,
        //     with_vectors=True,
        //     score_threshold=score_threshold,
        //     consistency=consistency,
        //     using=self.vector_name,
        // ).points
        let mut query_builder = qdrant::QueryPointsBuilder::new(&self.collection_name)
            .query(query)
            .limit(k as u64)
            .with_payload(true)
            .with_vectors(true) // MMR needs vectors for diversity calculation
            .filter(filter.unwrap_or_default())
            .params(search_params.unwrap_or_default());

        // Add score threshold if provided
        if let Some(threshold) = score_threshold {
            query_builder = query_builder.score_threshold(threshold);
        }

        // Add vector name if specified (for named vectors)
        if !self.vector_name.is_empty() {
            query_builder = query_builder.using(&self.vector_name);
        }

        let query_result = self
            .client
            .query(query_builder)
            .await
            .map_err(|e| Error::other(format!("Failed to query Qdrant with MMR: {e}")))?;

        // Step 4: Convert ScoredPoints to (Document, score) tuples
        // Python: return [(self._document_from_point(...), result.score) for result in results]
        let results = query_result
            .result
            .into_iter()
            .map(|scored_point| {
                // Extract payload
                let payload: HashMap<String, qdrant::Value> =
                    scored_point.payload.into_iter().collect();

                // Convert payload to document (content, metadata)
                let (content, mut metadata) = self.payload_to_document(&payload);

                // Add special metadata fields (_id, _collection_name)
                if let Some(point_id) = &scored_point.id {
                    let id_str = match point_id.point_id_options {
                        Some(qdrant::point_id::PointIdOptions::Num(n)) => n.to_string(),
                        Some(qdrant::point_id::PointIdOptions::Uuid(ref s)) => s.clone(),
                        None => String::new(),
                    };
                    metadata.insert("_id".to_string(), JsonValue::String(id_str));
                }
                metadata.insert(
                    "_collection_name".to_string(),
                    JsonValue::String(self.collection_name.clone()),
                );

                // Create Document
                let document = Document {
                    page_content: content,
                    metadata,
                    id: None,
                };

                // Return (Document, score) tuple
                (document, scored_point.score)
            })
            .collect();

        Ok(results)
    }

    /// Deletes documents from the vector store by their IDs.
    ///
    /// # Arguments
    ///
    /// * `ids` - List of document IDs to delete. IDs can be either strings or integers.
    ///
    /// # Returns
    ///
    /// Returns `true` if the deletion was successful, `false` otherwise.
    ///
    /// # Behavior
    ///
    /// **Python Baseline Compatibility**:
    /// Matches `delete()` in `dashflow_qdrant/qdrant.py:856-875`:
    /// - Uses `client.delete(collection_name=..., points_selector=ids)`
    /// - Returns `result.status == models.UpdateStatus.COMPLETED`
    ///
    /// **Algorithm**:
    /// 1. Converts IDs to Qdrant `PointId` format (supports both string UUIDs and numeric IDs)
    /// 2. Calls `client.delete_points()` with `PointsIdsList`
    /// 3. Returns success status based on `UpdateStatus`
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Delete documents by their IDs
    /// let success = store.delete(&["id1", "id2", "id3"]).await?;
    /// assert!(success);
    /// ```
    pub async fn delete(&self, ids: &[impl AsRef<str>]) -> Result<bool> {
        // Step 1: Convert string IDs to PointId
        // Python: points_selector=ids (where ids is list[str | int])
        let point_ids: Vec<qdrant::PointId> = ids
            .iter()
            .map(|id| {
                let id_str = id.as_ref();
                // Try to parse as integer first, otherwise treat as UUID string
                if let Ok(num) = id_str.parse::<u64>() {
                    num.into()
                } else {
                    id_str.to_string().into()
                }
            })
            .collect();

        // Step 2: Delete points from Qdrant
        // Python: result = self.client.delete(
        //     collection_name=self.collection_name,
        //     points_selector=ids,
        // )
        let result = self
            .client
            .delete_points(
                qdrant::DeletePointsBuilder::new(&self.collection_name)
                    .points(qdrant::PointsIdsList { ids: point_ids })
                    .wait(true), // Wait for operation to complete
            )
            .await
            .map_err(|e| Error::other(format!("Failed to delete points from Qdrant: {e}")))?;

        // Step 3: Check if deletion was successful
        // Python: return result.status == models.UpdateStatus.COMPLETED
        Ok(result
            .result
            .is_some_and(|r| r.status == qdrant::UpdateStatus::Completed as i32))
    }

    /// Retrieves documents from the vector store by their IDs.
    ///
    /// # Arguments
    ///
    /// * `ids` - List of document IDs to retrieve. IDs can be either strings or integers.
    ///
    /// # Returns
    ///
    /// Returns a `Vec<Document>` containing the documents with the specified IDs.
    /// Documents are returned in the same order as the IDs, but missing IDs are skipped.
    ///
    /// # Behavior
    ///
    /// **Python Baseline Compatibility**:
    /// Matches `get_by_ids()` in `dashflow_qdrant/qdrant.py:877-888`:
    /// - Uses `client.retrieve(collection_name, ids, with_payload=True)`
    /// - Converts each point to a Document using `_document_from_point()`
    ///
    /// **Algorithm**:
    /// 1. Converts IDs to Qdrant `PointId` format (supports both string UUIDs and numeric IDs)
    /// 2. Calls `client.get_points()` with `GetPointsBuilder`
    /// 3. Converts each retrieved point to a Document with content and metadata
    /// 4. Adds special metadata fields: `_id`, `_collection_name`
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Retrieve documents by their IDs
    /// let docs = store.get_by_ids(&["id1", "id2", "id3"]).await?;
    /// for doc in &docs {
    ///     println!("Content: {}", doc.page_content);
    /// }
    /// ```
    pub async fn get_by_ids(&self, ids: &[impl AsRef<str>]) -> Result<Vec<Document>> {
        // Step 1: Convert string IDs to PointId
        // Python: ids: Sequence[str | int]
        let point_ids: Vec<qdrant::PointId> = ids
            .iter()
            .map(|id| {
                let id_str = id.as_ref();
                // Try to parse as integer first, otherwise treat as UUID string
                if let Ok(num) = id_str.parse::<u64>() {
                    num.into()
                } else {
                    id_str.to_string().into()
                }
            })
            .collect();

        // Step 2: Retrieve points from Qdrant
        // Python: results = self.client.retrieve(self.collection_name, ids, with_payload=True)
        let response = self
            .client
            .get_points(
                qdrant::GetPointsBuilder::new(&self.collection_name, point_ids).with_payload(true), // Include payload data
            )
            .await
            .map_err(|e| Error::other(format!("Failed to retrieve points from Qdrant: {e}")))?;

        // Step 3: Convert points to Documents
        // Python: return [self._document_from_point(...) for result in results]
        let documents: Vec<Document> = response
            .result
            .into_iter()
            .map(|point| {
                // Extract payload
                let payload: HashMap<String, qdrant::Value> = point.payload.into_iter().collect();

                // Convert payload to document (content, metadata)
                let (content, mut metadata) = self.payload_to_document(&payload);

                // Add special metadata fields (_id, _collection_name)
                // Python: metadata["_id"] = point.id
                // Python: metadata["_collection_name"] = collection_name
                if let Some(point_id) = &point.id {
                    let id_str = match point_id.point_id_options {
                        Some(qdrant::point_id::PointIdOptions::Num(n)) => n.to_string(),
                        Some(qdrant::point_id::PointIdOptions::Uuid(ref s)) => s.clone(),
                        None => String::new(),
                    };
                    metadata.insert("_id".to_string(), JsonValue::String(id_str));
                }
                metadata.insert(
                    "_collection_name".to_string(),
                    JsonValue::String(self.collection_name.clone()),
                );

                // Create Document
                Document {
                    page_content: content,
                    metadata,
                    id: None,
                }
            })
            .collect();

        Ok(documents)
    }

    /// Builds Qdrant vectors from texts by embedding them.
    ///
    /// This method handles the embedding process based on the configured retrieval mode.
    /// Currently only DENSE mode is implemented.
    ///
    /// # Arguments
    ///
    /// * `texts` - Texts to embed and convert to vectors
    ///
    /// # Returns
    ///
    /// Returns a `Vec<qdrant::Vectors>` where each element corresponds to one input text.
    /// The vector format depends on the retrieval mode:
    /// - **DENSE**: Single dense vector per text (supported)
    /// - **SPARSE**: Single sparse vector per text (requires sparse encoders - planned)
    /// - **HYBRID**: Both dense and sparse vectors per text (requires sparse encoders - planned)
    ///
    /// # Behavior
    ///
    /// **Python Baseline Compatibility**:
    /// Matches `_build_vectors()` in `dashflow_qdrant/qdrant.py:1098-1147`:
    /// - DENSE mode: Embeds texts with `embeddings.embed_documents()`
    /// - Returns list of `VectorStruct` (dict with `vector_name` → vector)
    /// - Uses `self.vector_name` as key (default: "" for unnamed vector)
    ///
    /// **DENSE Mode**:
    /// 1. Validates embeddings provider exists (returns Error if None)
    /// 2. Calls `embeddings.embed_documents(texts)` to get embeddings
    /// 3. Creates one vector per embedding
    /// 4. If `vector_name` is empty (""), creates unnamed default vector
    /// 5. If `vector_name` is non-empty, creates named vector with that name
    ///
    /// **SPARSE and HYBRID Modes**:
    /// - Requires sparse vector encoders (BM25/SPLADE) not yet available in Rust
    /// - Returns Error with descriptive message when requested
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - DENSE mode but no embeddings provider configured
    /// - Embeddings provider fails to embed texts
    /// - SPARSE or HYBRID mode (not implemented)
    ///
    /// # Vector Naming
    ///
    /// Qdrant supports both unnamed and named vectors:
    /// - **Unnamed** (default): `vector_name = ""` → Single default vector per point
    /// - **Named**: `vector_name = "text"` → Named vector accessible by key
    ///
    /// Python uses unnamed vectors by default (`vector_name` = ""), which creates
    /// simpler collection configurations and search queries.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// let texts = vec!["Hello world".to_string(), "Goodbye world".to_string()];
    /// let vectors = store.build_vectors(&texts).await?;
    /// assert_eq!(vectors.len(), 2);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Related Methods
    ///
    /// - [`with_vector_name()`](Self::with_vector_name) - Sets custom vector name
    /// - [`validate_embeddings()`](Self::validate_embeddings) - Validates embeddings provider
    async fn build_vectors(&self, texts: &[String]) -> Result<Vec<qdrant::Vectors>> {
        match self.retrieval_mode {
            RetrievalMode::Dense => {
                // Validate embeddings provider exists
                let embeddings = self.embeddings.as_ref().ok_or_else(|| {
                    Error::InvalidInput(
                        "DENSE mode requires embeddings provider. Use with_embeddings() or provide embeddings in constructor.".to_string()
                    )
                })?;

                // Embed all texts using graph API
                let batch_embeddings = embed(Arc::clone(embeddings), texts).await?;

                // Verify we got the right number of embeddings
                if batch_embeddings.len() != texts.len() {
                    return Err(Error::InvalidInput(format!(
                        "Embeddings count mismatch: got {} embeddings for {} texts",
                        batch_embeddings.len(),
                        texts.len()
                    )));
                }

                // Convert embeddings to Qdrant Vectors
                // If vector_name is empty (""), create unnamed default vector
                // If vector_name is non-empty, create named vector
                let vectors = if self.vector_name.is_empty() {
                    // Unnamed vector (default) - directly convert Vec<f32> to Vectors
                    // Python: {self.vector_name: vector} where vector_name = ""
                    batch_embeddings
                        .into_iter()
                        .map(|embedding| {
                            // Vec<f32> implements Into<Vectors> for unnamed vectors
                            embedding.into()
                        })
                        .collect()
                } else {
                    // Named vector - create NamedVectors with custom key
                    // Python: {self.vector_name: vector} where vector_name != ""
                    batch_embeddings
                        .into_iter()
                        .map(|embedding| {
                            // Create Vector from f32 data (using new API)
                            let vector = qdrant::Vector {
                                #[allow(deprecated)]
                                data: embedding,
                                #[allow(deprecated)]
                                indices: None,
                                #[allow(deprecated)]
                                vectors_count: None,
                                vector: None,
                            };

                            // Create NamedVectors with custom key
                            let mut named_vectors = HashMap::new();
                            named_vectors.insert(self.vector_name.clone(), vector);

                            qdrant::Vectors {
                                vectors_options: Some(qdrant::vectors::VectorsOptions::Vectors(
                                    qdrant::NamedVectors {
                                        vectors: named_vectors,
                                    },
                                )),
                            }
                        })
                        .collect()
                };

                Ok(vectors)
            }
            RetrievalMode::Sparse => Err(Error::InvalidInput(
                "SPARSE mode requires sparse vector encoders (BM25/SPLADE) which are not yet \
                 available in the Rust ecosystem. Use DENSE mode for semantic search."
                    .to_string(),
            )),
            RetrievalMode::Hybrid => Err(Error::InvalidInput(
                "HYBRID mode requires sparse vector encoders (BM25/SPLADE) which are not yet \
                 available in the Rust ecosystem. Use DENSE mode for semantic search."
                    .to_string(),
            )),
        }
    }

    /// Generates or validates IDs for documents.
    ///
    /// This method handles ID generation for points in Qdrant. If IDs are provided,
    /// they are validated and used. If not provided, UUIDs are generated automatically.
    ///
    /// # Arguments
    ///
    /// * `texts` - The texts being added (used to determine count)
    /// * `provided_ids` - Optional pre-generated IDs
    ///
    /// # Returns
    ///
    /// Returns a `Vec<String>` with exactly one ID per text.
    ///
    /// # Behavior
    ///
    /// **Python Baseline Compatibility**:
    /// Matches `_generate_batches()` in `dashflow_qdrant/qdrant.py:1038-1069`:
    /// - If `ids` is None, generates UUIDs: `uuid.uuid4().hex`
    /// - If `ids` provided, uses them directly
    /// - Python accepts both str and int IDs (Qdrant supports both)
    /// - No explicit count validation (iterator handles it)
    ///
    /// **ID Generation**:
    /// - If `provided_ids` is None: Generate UUID for each text
    /// - If `provided_ids` is Some: Validate count matches text count
    /// - UUIDs use `.simple()` format (lowercase hex without dashes)
    /// - Python uses `uuid.uuid4().hex` which is equivalent to `.simple()`
    ///
    /// **Count Validation**:
    /// - Rust validates count explicitly (Python uses iterators)
    /// - If provided ID count mismatches text count, returns error
    /// - This catches user errors early before attempting insertion
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Provided IDs count does not match texts count
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// let texts = vec!["Hello".to_string(), "World".to_string()];
    ///
    /// // Generate IDs automatically
    /// let ids1 = store.generate_ids(&texts, None)?;
    /// assert_eq!(ids1.len(), 2);
    ///
    /// // Use provided IDs
    /// let provided = vec!["id1".to_string(), "id2".to_string()];
    /// let ids2 = store.generate_ids(&texts, Some(&provided))?;
    /// assert_eq!(ids2, provided);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # UUID Format
    ///
    /// UUIDs are generated using `uuid::Uuid::new_v4().simple().to_string()`:
    /// - Example: "550e8400e29b41d4a716446655440000"
    /// - Lowercase hexadecimal, no dashes or hyphens
    /// - 32 characters long
    /// - Matches Python's `uuid.uuid4().hex` format
    fn generate_ids(
        &self,
        texts: &[String],
        provided_ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
        match provided_ids {
            Some(ids) => {
                // Validate count matches
                if ids.len() != texts.len() {
                    return Err(Error::InvalidInput(format!(
                        "Provided IDs count ({}) does not match texts count ({})",
                        ids.len(),
                        texts.len()
                    )));
                }
                // Use provided IDs
                Ok(ids.to_vec())
            }
            None => {
                // Generate UUIDs for each text
                // Python: uuid.uuid4().hex
                // Rust: Uuid::new_v4().simple().to_string()
                Ok(texts
                    .iter()
                    .map(|_| uuid::Uuid::new_v4().simple().to_string())
                    .collect())
            }
        }
    }

    /// Constructs a new `QdrantVectorStore` with full collection management.
    ///
    /// This method provides complete control over collection creation and configuration,
    /// including:
    /// - Automatic collection creation if it doesn't exist
    /// - Collection recreation if `force_recreate` is true
    /// - Automatic vector dimension detection from embeddings
    /// - Collection validation if it already exists
    ///
    /// This is the recommended way to create a new `QdrantVectorStore` when you want
    /// automatic collection management. It matches the Python `construct_instance()`
    /// classmethod behavior.
    ///
    /// # Arguments
    ///
    /// * `url` - URL of the Qdrant server
    /// * `collection_name` - Optional collection name (generates UUID if None)
    /// * `embeddings` - Embeddings provider (required for Dense mode)
    /// * `retrieval_mode` - Retrieval mode (Dense, Sparse, or Hybrid)
    /// * `distance` - Distance metric for similarity calculations
    /// * `force_recreate` - If true, deletes and recreates existing collection
    /// * `validate_collection_config` - If true, validates existing collection configuration
    ///
    /// # Returns
    ///
    /// Returns a `Result` with the created `QdrantVectorStore` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to connect to Qdrant server
    /// - Embeddings required but not provided
    /// - Collection validation fails
    /// - Collection creation fails
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `construct_instance()` classmethod in Python baseline at
    /// `dashflow_qdrant/qdrant.py:891-997`.
    ///
    /// Python implementation:
    /// ```python
    /// @classmethod
    /// def construct_instance(
    ///     cls,
    ///     embedding: Embeddings | None = None,
    ///     retrieval_mode: RetrievalMode = RetrievalMode.DENSE,
    ///     sparse_embedding: SparseEmbeddings | None = None,
    ///     client_options: dict[str, Any] | None = None,
    ///     collection_name: str | None = None,
    ///     distance: models.Distance = models.Distance.COSINE,
    ///     content_payload_key: str = CONTENT_KEY,
    ///     metadata_payload_key: str = METADATA_KEY,
    ///     vector_name: str = VECTOR_NAME,
    ///     sparse_vector_name: str = SPARSE_VECTOR_NAME,
    ///     force_recreate: bool = False,
    ///     collection_create_options: dict[str, Any] | None = None,
    ///     vector_params: dict[str, Any] | None = None,
    ///     sparse_vector_params: dict[str, Any] | None = None,
    ///     validate_embeddings: bool = True,
    ///     validate_collection_config: bool = True,
    /// ) -> QdrantVectorStore:
    ///     # 1. Validate embeddings
    ///     # 2. Generate collection name if needed
    ///     # 3. Create Qdrant client
    ///     # 4. Check if collection exists
    ///     # 5. Delete collection if force_recreate
    ///     # 6. Create collection if doesn't exist (with auto dimension detection)
    ///     # 7. Validate collection config if exists
    ///     # 8. Return QdrantVectorStore
    /// ```
    ///
    /// **Key Features**:
    /// - Automatic dimension detection: Embeds "`dummy_text`" to get vector size
    /// - Collection recreation: `force_recreate=True` deletes existing collection
    /// - Config validation: Validates distance metric and vector dimensions match
    /// - UUID generation: Uses `uuid.uuid4().hex` if `collection_name` is None
    ///
    /// # Algorithm
    ///
    /// 1. Validate embeddings match retrieval mode requirements
    /// 2. Generate collection name if None (using UUID)
    /// 3. Create Qdrant client and connect to server
    /// 4. Check if collection exists
    /// 5. If exists and `force_recreate`: Delete collection
    /// 6. If not exists: Create collection with auto-detected dimensions
    ///    - Embed "`dummy_text`" to detect vector dimensions
    ///    - Create `VectorParams` with detected size and distance
    /// 7. If exists and `validate_collection_config`: Validate configuration
    /// 8. Return configured `QdrantVectorStore`
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use dashflow_qdrant::{QdrantVectorStore, RetrievalMode};
    /// use dashflow::core::embeddings::{Embeddings, MockEmbeddings};
    /// use qdrant_client::qdrant::Distance;
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new(384));
    /// // Create new collection (auto-generates name, auto-detects dimensions)
    /// let store = QdrantVectorStore::construct_instance(
    ///     "http://localhost:6334",
    ///     None, // Auto-generate collection name
    ///     Some(embeddings),
    ///     RetrievalMode::Dense,
    ///     Distance::Cosine,
    ///     false, // Don't force recreate
    ///     true,  // Validate config
    /// ).await?;
    ///
    /// println!("Collection: {}", store.collection_name());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Notes
    ///
    /// - This method embeds a dummy text to detect vector dimensions, which makes one
    ///   API call to the embeddings provider during initialization.
    /// - For production use with large collections, consider caching the vector dimensions
    ///   and using [`from_existing_collection()`](Self::from_existing_collection) instead.
    /// - SPARSE and HYBRID modes require sparse encoders (planned feature).
    pub async fn construct_instance(
        url: &str,
        collection_name: Option<String>,
        embeddings: Option<Arc<dyn Embeddings>>,
        retrieval_mode: RetrievalMode,
        distance: Distance,
        force_recreate: bool,
        validate_collection_config: bool,
    ) -> Result<Self> {
        // Python: cls._validate_embeddings(retrieval_mode, embedding, sparse_embedding)
        Self::validate_embeddings(retrieval_mode, embeddings.clone())?;

        // Python: collection_name = collection_name or uuid.uuid4().hex
        let collection_name =
            collection_name.unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());

        // Python: client = QdrantClient(**client_options)
        let client = Qdrant::from_url(url)
            .build()
            .map_err(|e| Error::Other(format!("Failed to create Qdrant client: {e}")))?;

        // Python: collection_exists = client.collection_exists(collection_name)
        let collection_exists = client
            .collection_exists(&collection_name)
            .await
            .map_err(|e| Error::Other(format!("Failed to check if collection exists: {e}")))?;

        // Python: if collection_exists and force_recreate: client.delete_collection(collection_name)
        let mut collection_exists = collection_exists;
        if collection_exists && force_recreate {
            client
                .delete_collection(&collection_name)
                .await
                .map_err(|e| Error::Other(format!("Failed to delete collection: {e}")))?;
            collection_exists = false;
        }

        // Create store instance for method access
        let distance_metric = match distance {
            Distance::Cosine => DistanceMetric::Cosine,
            Distance::Euclid => DistanceMetric::Euclidean,
            Distance::Dot => DistanceMetric::DotProduct,
            _ => DistanceMetric::Cosine,
        };

        let store = Self {
            client: client.clone(),
            collection_name: collection_name.clone(),
            embeddings: embeddings.clone(),
            sparse_embeddings: None,
            retrieval_mode,
            distance_metric,
            vector_name: String::new(),
            sparse_vector_name: "dashflow-sparse".to_string(),
            content_key: "page_content".to_string(),
            metadata_key: "metadata".to_string(),
        };

        if !collection_exists {
            // Python: Create collection with auto-detected dimensions
            // Python: partial_embeddings = embedding.embed_documents(["dummy_text"])
            // Python: vector_params["size"] = len(partial_embeddings[0])

            match retrieval_mode {
                RetrievalMode::Dense => {
                    let embeddings_ref = embeddings.as_ref().ok_or_else(|| {
                        Error::InvalidInput("Embeddings required for Dense mode".to_string())
                    })?;

                    // Embed dummy text to detect dimensions using graph API
                    // Python: embedding.embed_documents(["dummy_text"])
                    let dummy_embeddings = embed(Arc::clone(embeddings_ref), &["dummy_text".to_string()])
                        .await?;

                    if dummy_embeddings.is_empty() {
                        return Err(Error::InvalidInput(
                            "Embeddings returned empty result".to_string(),
                        ));
                    }

                    let vector_size = dummy_embeddings[0].len() as u64;

                    // Create collection with detected dimensions
                    store
                        .create_collection(&collection_name, vector_size, distance)
                        .await?;
                }
                RetrievalMode::Sparse => {
                    return Err(Error::InvalidInput(
                        "SPARSE mode requires sparse vector encoders. Use DENSE mode.".to_string(),
                    ));
                }
                RetrievalMode::Hybrid => {
                    return Err(Error::InvalidInput(
                        "HYBRID mode requires sparse vector encoders. Use DENSE mode.".to_string(),
                    ));
                }
            }
        } else if validate_collection_config {
            // Python: cls._validate_collection_config(...)
            store.validate_collection_config().await?;
        }

        Ok(store)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::clone_on_ref_ptr,
    clippy::float_cmp
)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::clone_on_ref_ptr,
    clippy::float_cmp
)]
#[path = "standard_tests.rs"]
mod standard_tests;
