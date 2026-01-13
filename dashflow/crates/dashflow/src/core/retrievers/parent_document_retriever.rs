//! Parent document retriever - retrieves small chunks, returns parent documents.
//!
//! This module implements the `ParentDocumentRetriever` pattern, which solves a key
//! challenge in retrieval-augmented generation (RAG):
//!
//! 1. **Small chunks** → Accurate embeddings (specific meaning)
//! 2. **Large chunks** → Sufficient context (surrounding information)
//!
//! The `ParentDocumentRetriever` stores small chunks for embedding/retrieval, but
//! returns the larger parent documents that contain those chunks.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │         Parent Document                 │
//! │  (stored in docstore with UUID)         │
//! │                                         │
//! │  ┌──────────┐  ┌──────────┐  ┌────────┐│
//! │  │ Child 1  │  │ Child 2  │  │ Child 3││
//! │  │(indexed) │  │(indexed) │  │(indexed│││
//! │  └──────────┘  └──────────┘  └────────┘│
//! └─────────────────────────────────────────┘
//!
//! Query → VectorStore → Child chunks → Parent UUIDs → Docstore → Parent docs
//! ```
//!
//! # Use Cases
//!
//! - **Code search**: Index function bodies, return full files
//! - **Document QA**: Index paragraphs, return full pages/sections
//! - **Legal/compliance**: Index clauses, return full contracts
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::retrievers::ParentDocumentRetriever;
//! use dashflow::core::stores::InMemoryStore;
//! use dashflow_text_splitters::RecursiveCharacterTextSplitter;
//!
//! // Create text splitters
//! let parent_splitter = RecursiveCharacterTextSplitter::new(2000, 0, true);
//! let child_splitter = RecursiveCharacterTextSplitter::new(400, 0, true);
//!
//! // Create retriever
//! let retriever = ParentDocumentRetriever::new(
//!     vectorstore,
//!     InMemoryStore::new(),
//!     child_splitter,
//!     Some(parent_splitter),
//!     "doc_id",
//!     None, // Keep all child metadata
//! );
//!
//! // Add documents - automatically splits and stores
//! retriever.add_documents(documents, None, true).await?;
//!
//! // Search - returns parent documents, not children
//! let results = retriever._get_relevant_documents("query", None).await?;
//! ```

use crate::core::{
    config::RunnableConfig,
    documents::Document,
    error::{Error, Result},
    retrievers::{Retriever, SearchType},
    stores::BaseStore,
    vector_stores::VectorStore,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Text splitter trait for document splitting.
///
/// This is a minimal trait definition to avoid circular dependencies with
/// dashflow-text-splitters. Implementations in dashflow-text-splitters
/// should implement this trait.
pub trait TextSplitter: Send + Sync {
    /// Split documents into smaller chunks.
    fn split_documents(&self, documents: &[Document]) -> Vec<Document>;
}

/// Multi-vector retriever that stores multiple embeddings per document.
///
/// This is the base class for retrievers that:
/// - Store small chunks in a vector store for similarity search
/// - Store full documents in a separate docstore
/// - Link chunks to documents via metadata keys
///
/// The typical flow:
/// 1. Query → Vector store → Retrieve child chunks
/// 2. Extract parent IDs from chunk metadata
/// 3. Fetch parent documents from docstore
/// 4. Return parent documents (not chunks)
///
/// # Type Parameters
///
/// - `VS`: `VectorStore` implementation
/// - `V`: Value type stored in docstore (typically Document)
pub struct MultiVectorRetriever<VS, V>
where
    VS: VectorStore + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Vector store containing child chunks with embeddings
    pub vectorstore: Arc<VS>,

    /// Document store containing parent documents
    pub docstore: Arc<tokio::sync::RwLock<Box<dyn BaseStore<String, V>>>>,

    /// Metadata key used to link children to parents (default: "`doc_id`")
    pub id_key: String,

    /// Search type for vector store queries
    pub search_type: SearchType,

    /// Search parameters (k, threshold, lambda, etc.)
    pub search_kwargs: HashMap<String, serde_json::Value>,
}

impl<VS, V> MultiVectorRetriever<VS, V>
where
    VS: VectorStore + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Create a new `MultiVectorRetriever`.
    ///
    /// # Arguments
    ///
    /// * `vectorstore` - Vector store for child chunks
    /// * `docstore` - Document store for parent documents
    /// * `id_key` - Metadata key linking children to parents
    /// * `search_type` - Type of vector search (similarity, MMR, etc.)
    /// * `search_kwargs` - Additional search parameters
    pub fn new(
        vectorstore: Arc<VS>,
        docstore: Arc<tokio::sync::RwLock<Box<dyn BaseStore<String, V>>>>,
        id_key: String,
        search_type: SearchType,
        search_kwargs: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            vectorstore,
            docstore,
            id_key,
            search_type,
            search_kwargs,
        }
    }

    /// Perform vector search and return child chunks.
    ///
    /// Internal helper that queries the vector store using the configured search type.
    async fn search_vectorstore(&self, query: &str, k: usize) -> Result<Vec<Document>> {
        match self.search_type {
            SearchType::Similarity => self.vectorstore._similarity_search(query, k, None).await,
            SearchType::MMR => {
                let lambda = self
                    .search_kwargs
                    .get("lambda_mult")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.5) as f32;
                let fetch_k = self
                    .search_kwargs
                    .get("fetch_k")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(20) as usize;
                self.vectorstore
                    .max_marginal_relevance_search(query, k, fetch_k, lambda, None)
                    .await
            }
            SearchType::SimilarityScoreThreshold => {
                let threshold = self
                    .search_kwargs
                    .get("score_threshold")
                    .and_then(serde_json::Value::as_f64)
                    .ok_or_else(|| {
                        Error::config(
                            "score_threshold required for SimilarityScoreThreshold search",
                        )
                    })? as f32;
                let docs_with_scores = self
                    .vectorstore
                    .similarity_search_with_score(query, k, None)
                    .await
                    .map_err(|e| {
                        Error::other(format!(
                            "Similarity search with score failed in multi-vector retriever: {e}"
                        ))
                    })?;
                Ok(docs_with_scores
                    .into_iter()
                    .filter(|(_, score)| *score >= threshold)
                    .map(|(doc, _)| doc)
                    .collect())
            }
        }
    }

    /// Extract parent IDs from child chunks, preserving order and removing duplicates.
    fn extract_parent_ids(&self, sub_docs: &[Document]) -> Vec<String> {
        let mut ids = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for doc in sub_docs {
            if let Some(id_value) = doc.metadata.get(&self.id_key) {
                let id = match id_value {
                    serde_json::Value::String(s) => s.clone(),
                    _ => id_value.to_string(),
                };
                if !seen.contains(&id) {
                    seen.insert(id.clone());
                    ids.push(id);
                }
            }
        }

        ids
    }
}

#[async_trait]
impl<VS> Retriever for MultiVectorRetriever<VS, Document>
where
    VS: VectorStore + Send + Sync + 'static,
{
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // Get default k from search_kwargs or use 4
        let k = self
            .search_kwargs
            .get("k")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(4) as usize;

        // Search vector store for child chunks
        let sub_docs = self.search_vectorstore(query, k).await.map_err(|e| {
            Error::other(format!(
                "Multi-vector retriever vector store search failed: {e}"
            ))
        })?;

        // Extract parent IDs from child chunks
        let ids = self.extract_parent_ids(&sub_docs);

        // Retrieve parent documents from docstore
        let docstore = self.docstore.read().await;
        let docs = docstore.mget(ids.clone()).await.map_err(|e| {
            Error::other(format!(
                "Failed to retrieve {} parent documents from docstore: {e}",
                ids.len()
            ))
        })?;

        // Filter out None values
        Ok(docs.into_iter().flatten().collect())
    }

    fn name(&self) -> String {
        "MultiVectorRetriever".to_string()
    }
}

/// Parent document retriever - balances embedding accuracy with context.
///
/// Splits documents into small chunks for accurate embeddings, but returns
/// larger parent documents for sufficient context.
///
/// # Two-Level Splitting
///
/// 1. **Parent splitter** (optional): Splits raw documents into parent chunks
///    - If None, raw documents are the parents
///    - Example: 2000 character chunks
///
/// 2. **Child splitter** (required): Splits parents into child chunks
///    - These are indexed in the vector store
///    - Example: 400 character chunks
///
/// # Workflow
///
/// ```text
/// Input Doc → [Parent Splitter] → Parent Docs (stored in docstore)
///                                       ↓
///                                  Child Splitter
///                                       ↓
///                               Child Chunks (indexed in vectorstore)
/// ```
///
/// During retrieval:
/// ```text
/// Query → VectorStore → Child Chunks → Parent IDs → Docstore → Parent Docs
/// ```
///
/// # Metadata Filtering
///
/// Use `child_metadata_fields` to control which metadata fields are copied
/// to child chunks. This prevents metadata bloat in the vector store.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::retrievers::ParentDocumentRetriever;
/// use dashflow::core::stores::InMemoryStore;
/// use dashflow_text_splitters::RecursiveCharacterTextSplitter;
///
/// let parent_splitter = RecursiveCharacterTextSplitter::new(2000, 0, true);
/// let child_splitter = RecursiveCharacterTextSplitter::new(400, 0, true);
///
/// let retriever = ParentDocumentRetriever::new(
///     Arc::new(vectorstore),
///     Arc::new(InMemoryStore::new()),
///     child_splitter,
///     Some(parent_splitter),
///     "doc_id".to_string(),
///     None, // Keep all metadata in children
/// );
///
/// // Add documents
/// retriever.add_documents(docs, None, true).await?;
///
/// // Search - returns parent documents
/// let results = retriever._get_relevant_documents("query", None).await?;
/// ```
pub struct ParentDocumentRetriever<VS>
where
    VS: VectorStore + Send + Sync,
{
    /// Base multi-vector retriever
    base: MultiVectorRetriever<VS, Document>,

    /// Text splitter for creating child chunks (required)
    pub child_splitter: Box<dyn TextSplitter>,

    /// Text splitter for creating parent documents (optional)
    pub parent_splitter: Option<Box<dyn TextSplitter>>,

    /// Metadata fields to keep in child documents
    /// If None, all parent metadata is copied to children
    pub child_metadata_fields: Option<Vec<String>>,
}

impl<VS> ParentDocumentRetriever<VS>
where
    VS: VectorStore + Send + Sync,
{
    /// Create a new `ParentDocumentRetriever`.
    ///
    /// # Arguments
    ///
    /// * `vectorstore` - Vector store for child chunks
    /// * `docstore` - Document store for parent documents
    /// * `child_splitter` - Splitter for creating child chunks
    /// * `parent_splitter` - Optional splitter for creating parent documents
    /// * `id_key` - Metadata key linking children to parents (default: "`doc_id`")
    /// * `child_metadata_fields` - Optional list of metadata fields to keep in children
    #[allow(clippy::too_many_arguments)] // Parent-child config: stores, splitters, id_key, metadata fields
    pub fn new(
        vectorstore: Arc<VS>,
        docstore: Box<dyn BaseStore<String, Document>>,
        child_splitter: Box<dyn TextSplitter>,
        parent_splitter: Option<Box<dyn TextSplitter>>,
        id_key: String,
        child_metadata_fields: Option<Vec<String>>,
    ) -> Self {
        let docstore = Arc::new(tokio::sync::RwLock::new(docstore));
        let base = MultiVectorRetriever::new(
            vectorstore,
            docstore,
            id_key,
            SearchType::Similarity,
            HashMap::new(),
        );

        Self {
            base,
            child_splitter,
            parent_splitter,
            child_metadata_fields,
        }
    }

    /// Set the search type for vector store queries.
    #[must_use]
    pub fn with_search_type(mut self, search_type: SearchType) -> Self {
        self.base.search_type = search_type;
        self
    }

    /// Set search parameters (k, threshold, lambda, etc.).
    #[must_use]
    pub fn with_search_kwargs(mut self, kwargs: HashMap<String, serde_json::Value>) -> Self {
        self.base.search_kwargs = kwargs;
        self
    }

    /// Split documents for adding to retriever.
    ///
    /// Internal method that:
    /// 1. Optionally splits documents into parent chunks
    /// 2. Generates UUIDs for parent documents
    /// 3. Splits parents into child chunks
    /// 4. Filters child metadata if configured
    /// 5. Links children to parents via `id_key` metadata
    ///
    /// # Returns
    ///
    /// - Child documents for vector store
    /// - (ID, parent document) pairs for docstore
    #[allow(clippy::type_complexity)] // Returns (child docs for vectorstore, (id, parent) pairs for docstore)
    fn split_docs_for_adding(
        &self,
        documents: Vec<Document>,
        ids: Option<Vec<String>>,
        add_to_docstore: bool,
    ) -> Result<(Vec<Document>, Vec<(String, Document)>)> {
        // Apply parent splitter if configured
        let parent_docs = if let Some(ref parent_splitter) = self.parent_splitter {
            parent_splitter.split_documents(&documents)
        } else {
            documents
        };

        // Generate or validate IDs
        let doc_ids = if let Some(ids) = ids {
            if ids.len() != parent_docs.len() {
                return Err(Error::config(format!(
                    "Got uneven list of documents and ids. \
                     If `ids` is provided, should be same length as `documents`. \
                     Got {} documents and {} ids",
                    parent_docs.len(),
                    ids.len()
                )));
            }
            ids
        } else {
            if !add_to_docstore {
                return Err(Error::config(
                    "If ids are not passed in, `add_to_docstore` MUST be true",
                ));
            }
            // Generate UUIDs for each parent document
            (0..parent_docs.len())
                .map(|_| Uuid::new_v4().to_string())
                .collect()
        };

        let mut child_docs = Vec::new();
        let mut full_docs = Vec::new();

        for (i, parent_doc) in parent_docs.into_iter().enumerate() {
            let id = &doc_ids[i];

            // Split parent into children
            let sub_docs = self
                .child_splitter
                .split_documents(std::slice::from_ref(&parent_doc));

            for mut child_doc in sub_docs {
                // Filter metadata if configured
                if let Some(ref fields) = self.child_metadata_fields {
                    let filtered: HashMap<String, serde_json::Value> = child_doc
                        .metadata
                        .iter()
                        .filter(|(k, _)| fields.contains(k))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    child_doc.metadata = filtered;
                }

                // Link child to parent via id_key
                child_doc
                    .metadata
                    .insert(self.base.id_key.clone(), serde_json::json!(id));

                child_docs.push(child_doc);
            }

            full_docs.push((id.clone(), parent_doc));
        }

        Ok((child_docs, full_docs))
    }

    /// Add documents to the retriever.
    ///
    /// This method:
    /// 1. Splits documents into parents and children
    /// 2. Adds children to vector store (for retrieval)
    /// 3. Adds parents to docstore (for return values)
    ///
    /// # Arguments
    ///
    /// * `documents` - Raw documents to add
    /// * `ids` - Optional pre-generated IDs (must match document count)
    /// * `add_to_docstore` - Whether to add parents to docstore
    ///   - Must be true if ids is None
    ///   - Can be false if parents already exist in docstore
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Simple case: add documents, generate IDs automatically
    /// retriever.add_documents(docs, None, true).await?;
    ///
    /// // Advanced: reuse existing parent IDs
    /// retriever.add_documents(docs, Some(existing_ids), false).await?;
    /// ```
    pub async fn add_documents(
        &mut self,
        documents: Vec<Document>,
        ids: Option<Vec<String>>,
        add_to_docstore: bool,
    ) -> Result<()> {
        let (child_docs, full_docs) =
            self.split_docs_for_adding(documents, ids, add_to_docstore)?;

        // Add children to vector store
        // Get mutable reference to the inner vector store
        let vs = Arc::get_mut(&mut self.base.vectorstore).ok_or_else(|| {
            Error::Other(
                "Cannot get mutable reference to vectorstore - multiple references exist"
                    .to_string(),
            )
        })?;
        vs.add_documents(&child_docs, None).await.map_err(|e| {
            Error::other(format!(
                "Failed to add {} child documents to vector store: {e}",
                child_docs.len()
            ))
        })?;

        // Add parents to docstore if requested
        if add_to_docstore {
            let mut docstore = self.base.docstore.write().await;
            docstore.mset(full_docs.clone()).await.map_err(|e| {
                Error::other(format!(
                    "Failed to add {} parent documents to docstore: {e}",
                    full_docs.len()
                ))
            })?;
        }

        Ok(())
    }
}

#[async_trait]
impl<VS> Retriever for ParentDocumentRetriever<VS>
where
    VS: VectorStore + Send + Sync + 'static,
{
    async fn _get_relevant_documents(
        &self,
        query: &str,
        config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        self.base._get_relevant_documents(query, config).await
    }

    fn name(&self) -> String {
        "ParentDocumentRetriever".to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::core::embeddings::Embeddings;
    use crate::core::retrievers::SearchType;
    use crate::core::stores::InMemoryStore;
    use crate::core::vector_stores::InMemoryVectorStore;
    use crate::test_prelude::*;

    // Simple character-based text splitter for testing
    struct SimpleTextSplitter {
        chunk_size: usize,
    }

    impl SimpleTextSplitter {
        fn new(chunk_size: usize) -> Self {
            Self { chunk_size }
        }
    }

    impl TextSplitter for SimpleTextSplitter {
        fn split_documents(&self, documents: &[Document]) -> Vec<Document> {
            let mut result = Vec::new();
            for doc in documents {
                let text = &doc.page_content;
                let chunks: Vec<String> = text
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(self.chunk_size)
                    .map(|chunk| chunk.iter().collect())
                    .collect();

                for chunk in chunks {
                    if !chunk.is_empty() {
                        result.push(Document {
                            page_content: chunk,
                            metadata: doc.metadata.clone(),
                            id: None,
                        });
                    }
                }
            }
            result
        }
    }

    // Mock embeddings for testing
    struct MockEmbeddings;

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .enumerate()
                .map(|(i, _)| vec![i as f32, 0.5, 0.1])
                .collect())
        }

        async fn _embed_query(&self, text: &str) -> Result<Vec<f32>> {
            Ok(vec![text.len() as f32, 0.5, 0.1])
        }
    }

    #[tokio::test]
    async fn test_parent_document_retriever_basic() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());

        // Create splitters: parent=50 chars, child=15 chars
        let parent_splitter = Box::new(SimpleTextSplitter::new(50));
        let child_splitter = Box::new(SimpleTextSplitter::new(15));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            Some(parent_splitter),
            "doc_id".to_string(),
            None,
        );

        // Add a long document
        let doc = Document::new(
            "This is a long document with multiple sentences. It should be split into parent and child chunks.",
        );
        retriever
            .add_documents(vec![doc], None, true)
            .await
            .unwrap();

        // Search should return parent document, not children
        let results = retriever
            ._get_relevant_documents("document", None)
            .await
            .unwrap();

        // Should get back the parent document (or a parent chunk)
        assert!(!results.is_empty());
        // Parent should be longer than child chunks (15 chars)
        assert!(results[0].page_content.len() > 15);
    }

    #[tokio::test]
    async fn test_parent_document_retriever_no_parent_splitter() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());

        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        // No parent splitter - raw docs are parents
        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        let doc = Document::new("Short document text");
        let original_content = doc.page_content.clone();

        retriever
            .add_documents(vec![doc], None, true)
            .await
            .unwrap();

        let results = retriever
            ._get_relevant_documents("Short", None)
            .await
            .unwrap();

        // Should return the full original document
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].page_content, original_content);
    }

    #[tokio::test]
    async fn test_parent_document_retriever_metadata_filtering() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());

        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        // Only keep "source" field in children
        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            Some(vec!["source".to_string()]),
        );

        let doc = Document::new("Test content")
            .with_metadata("source", "file.txt")
            .with_metadata("author", "Alice")
            .with_metadata("date", "2025-01-01");

        retriever
            .add_documents(vec![doc], None, true)
            .await
            .unwrap();

        let results = retriever
            ._get_relevant_documents("Test", None)
            .await
            .unwrap();

        // Parent should have all metadata
        assert_eq!(results[0].metadata.len(), 3);
        assert_eq!(results[0].metadata.get("source").unwrap(), "file.txt");
    }

    #[tokio::test]
    async fn test_split_docs_for_adding_with_ids() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        let retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        let doc = Document::new("Test content");
        let custom_id = "custom-uuid-123".to_string();

        let (child_docs, full_docs) = retriever
            .split_docs_for_adding(vec![doc], Some(vec![custom_id.clone()]), false)
            .unwrap();

        // Should use provided ID
        assert_eq!(full_docs[0].0, custom_id);

        // Child should have parent ID in metadata
        assert_eq!(
            child_docs[0].metadata.get("doc_id").unwrap(),
            &serde_json::json!(custom_id)
        );
    }

    #[tokio::test]
    async fn test_split_docs_for_adding_validation() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        let retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        // Error: mismatched IDs count
        let result = retriever.split_docs_for_adding(
            vec![Document::new("doc1"), Document::new("doc2")],
            Some(vec!["id1".to_string()]), // Only 1 ID for 2 docs
            false,
        );
        assert!(result.is_err());

        // Error: ids=None but add_to_docstore=false
        let result = retriever.split_docs_for_adding(
            vec![Document::new("doc1")],
            None,
            false, // Can't skip docstore without IDs
        );
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multi_vector_retriever_extract_parent_ids() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore: Box<dyn BaseStore<String, Document>> = Box::new(InMemoryStore::new());
        let docstore = Arc::new(tokio::sync::RwLock::new(docstore));

        let retriever = MultiVectorRetriever::new(
            vectorstore,
            docstore,
            "parent_id".to_string(),
            SearchType::Similarity,
            HashMap::new(),
        );

        let docs = vec![
            Document::new("chunk1").with_metadata("parent_id", "doc1"),
            Document::new("chunk2").with_metadata("parent_id", "doc2"),
            Document::new("chunk3").with_metadata("parent_id", "doc1"), // Duplicate
            Document::new("chunk4").with_metadata("parent_id", "doc3"),
        ];

        let ids = retriever.extract_parent_ids(&docs);

        // Should preserve order and remove duplicates
        assert_eq!(ids, vec!["doc1", "doc2", "doc3"]);
    }

    // ============================================================================
    // Additional Comprehensive Tests
    // ============================================================================

    // ----------------------------------------------------------------------------
    // Edge Cases - ParentDocumentRetriever
    // ----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_parent_retriever_empty_documents() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        // Add empty documents list
        retriever.add_documents(vec![], None, true).await.unwrap();

        // Should handle gracefully
        let results = retriever
            ._get_relevant_documents("query", None)
            .await
            .unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_parent_retriever_single_document() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(5));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        let doc = Document::new("Hello");
        let original = doc.page_content.clone();

        retriever
            .add_documents(vec![doc], None, true)
            .await
            .unwrap();

        let results = retriever
            ._get_relevant_documents("Hello", None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].page_content, original);
    }

    #[tokio::test]
    async fn test_parent_retriever_multiple_parents_same_child() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(20));
        let parent_splitter = Box::new(SimpleTextSplitter::new(50));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            Some(parent_splitter),
            "doc_id".to_string(),
            None,
        );

        // Long document that splits into multiple parents with overlapping children
        let long_doc = Document::new("A".repeat(150));

        retriever
            .add_documents(vec![long_doc], None, true)
            .await
            .unwrap();

        let results = retriever._get_relevant_documents("A", None).await.unwrap();

        // Should get multiple parent documents
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_parent_retriever_empty_string_document() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        let doc = Document::new("");

        retriever
            .add_documents(vec![doc], None, true)
            .await
            .unwrap();

        // Query should not crash
        let results = retriever
            ._get_relevant_documents("anything", None)
            .await
            .unwrap();

        // May or may not return the empty doc depending on vector store behavior
        assert!(results.len() <= 1);
    }

    #[tokio::test]
    async fn test_parent_retriever_very_small_chunks() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());

        // 1-character chunks
        let child_splitter = Box::new(SimpleTextSplitter::new(1));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        let doc = Document::new("Hello");

        retriever
            .add_documents(vec![doc.clone()], None, true)
            .await
            .unwrap();

        let results = retriever._get_relevant_documents("H", None).await.unwrap();

        // Should return parent, not individual characters
        assert_eq!(results[0].page_content, doc.page_content);
    }

    // ----------------------------------------------------------------------------
    // Builder Methods
    // ----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_parent_retriever_with_search_type() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        let retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        )
        .with_search_type(SearchType::MMR);

        assert!(matches!(retriever.base.search_type, SearchType::MMR));
    }

    #[tokio::test]
    async fn test_parent_retriever_with_search_kwargs() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        let mut kwargs = HashMap::new();
        kwargs.insert("k".to_string(), serde_json::json!(10));
        kwargs.insert("lambda_mult".to_string(), serde_json::json!(0.7));

        let retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        )
        .with_search_kwargs(kwargs);

        assert_eq!(
            retriever.base.search_kwargs.get("k").unwrap(),
            &serde_json::json!(10)
        );
        assert_eq!(
            retriever.base.search_kwargs.get("lambda_mult").unwrap(),
            &serde_json::json!(0.7)
        );
    }

    // ----------------------------------------------------------------------------
    // Search Parameters
    // ----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_parent_retriever_custom_k() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        let mut kwargs = HashMap::new();
        kwargs.insert("k".to_string(), serde_json::json!(2));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        )
        .with_search_kwargs(kwargs);

        // Add multiple documents
        let docs = vec![
            Document::new("First document content"),
            Document::new("Second document content"),
            Document::new("Third document content"),
            Document::new("Fourth document content"),
        ];

        retriever.add_documents(docs, None, true).await.unwrap();

        let results = retriever
            ._get_relevant_documents("document", None)
            .await
            .unwrap();

        // Should respect k parameter (though may be less if deduplicated)
        assert!(results.len() <= 2);
    }

    #[tokio::test]
    async fn test_parent_retriever_default_k() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        // Add 5 documents
        let docs = (0..5)
            .map(|i| Document::new(format!("Document number {}", i)))
            .collect();

        retriever.add_documents(docs, None, true).await.unwrap();

        let results = retriever
            ._get_relevant_documents("Document", None)
            .await
            .unwrap();

        // Default k=4, so should get at most 4 results
        assert!(results.len() <= 4);
    }

    // ----------------------------------------------------------------------------
    // Metadata Handling
    // ----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_parent_retriever_all_metadata_preserved() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        // No metadata filtering
        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        let doc = Document::new("Test")
            .with_metadata("source", "file.txt")
            .with_metadata("author", "Alice")
            .with_metadata("version", 1);

        retriever
            .add_documents(vec![doc], None, true)
            .await
            .unwrap();

        let results = retriever
            ._get_relevant_documents("Test", None)
            .await
            .unwrap();

        // Parent should have all original metadata
        assert_eq!(results[0].metadata.len(), 3);
        assert!(results[0].metadata.contains_key("source"));
        assert!(results[0].metadata.contains_key("author"));
        assert!(results[0].metadata.contains_key("version"));
    }

    #[tokio::test]
    async fn test_parent_retriever_empty_metadata_fields() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        // Empty metadata filter list - no fields kept
        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            Some(vec![]),
        );

        let doc = Document::new("Test")
            .with_metadata("source", "file.txt")
            .with_metadata("author", "Alice");

        retriever
            .add_documents(vec![doc], None, true)
            .await
            .unwrap();

        let results = retriever
            ._get_relevant_documents("Test", None)
            .await
            .unwrap();

        // Parent should still have all metadata
        assert_eq!(results[0].metadata.len(), 2);
    }

    #[tokio::test]
    async fn test_parent_retriever_multiple_metadata_fields() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        // Keep multiple fields
        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            Some(vec!["source".to_string(), "author".to_string()]),
        );

        let doc = Document::new("Test")
            .with_metadata("source", "file.txt")
            .with_metadata("author", "Alice")
            .with_metadata("date", "2025-01-01")
            .with_metadata("version", 1);

        retriever
            .add_documents(vec![doc], None, true)
            .await
            .unwrap();

        let results = retriever
            ._get_relevant_documents("Test", None)
            .await
            .unwrap();

        // Parent should have all 4 fields
        assert_eq!(results[0].metadata.len(), 4);
    }

    // ----------------------------------------------------------------------------
    // Custom ID Key
    // ----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_parent_retriever_custom_id_key() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "custom_parent_id".to_string(),
            None,
        );

        let doc = Document::new("Test content");

        retriever
            .add_documents(vec![doc], None, true)
            .await
            .unwrap();

        // Should work with custom id key
        let results = retriever
            ._get_relevant_documents("Test", None)
            .await
            .unwrap();

        assert!(!results.is_empty());
    }

    // ----------------------------------------------------------------------------
    // UUID Generation
    // ----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_parent_retriever_uuid_uniqueness() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        let retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        // Split 3 documents without providing IDs
        let docs = vec![
            Document::new("Doc 1"),
            Document::new("Doc 2"),
            Document::new("Doc 3"),
        ];

        let (_, full_docs) = retriever.split_docs_for_adding(docs, None, true).unwrap();

        // All IDs should be unique
        let ids: Vec<String> = full_docs.iter().map(|(id, _)| id.clone()).collect();
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique_ids.len());

        // All IDs should be valid UUIDs
        for id in &ids {
            assert!(Uuid::parse_str(id).is_ok());
        }
    }

    // ----------------------------------------------------------------------------
    // Multiple Documents
    // ----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_parent_retriever_many_documents() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(20));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        // Add 20 documents
        let docs: Vec<Document> = (0..20)
            .map(|i| Document::new(format!("Document number {} with unique content", i)))
            .collect();

        retriever.add_documents(docs, None, true).await.unwrap();

        let results = retriever
            ._get_relevant_documents("Document", None)
            .await
            .unwrap();

        // Should return some results
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_parent_retriever_incremental_additions() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        // Add documents in multiple batches
        retriever
            .add_documents(vec![Document::new("First batch doc 1")], None, true)
            .await
            .unwrap();

        retriever
            .add_documents(vec![Document::new("Second batch doc 2")], None, true)
            .await
            .unwrap();

        retriever
            .add_documents(vec![Document::new("Third batch doc 3")], None, true)
            .await
            .unwrap();

        let results = retriever
            ._get_relevant_documents("batch", None)
            .await
            .unwrap();

        // Should be able to retrieve from all batches
        assert!(!results.is_empty());
    }

    // ----------------------------------------------------------------------------
    // MultiVectorRetriever Tests
    // ----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_multi_vector_retriever_similarity_search() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore: Box<dyn BaseStore<String, Document>> = Box::new(InMemoryStore::new());
        let docstore = Arc::new(tokio::sync::RwLock::new(docstore));

        let retriever = MultiVectorRetriever::new(
            vectorstore,
            docstore,
            "doc_id".to_string(),
            SearchType::Similarity,
            HashMap::new(),
        );

        assert!(matches!(retriever.search_type, SearchType::Similarity));
    }

    #[tokio::test]
    async fn test_multi_vector_retriever_mmr_search() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore: Box<dyn BaseStore<String, Document>> = Box::new(InMemoryStore::new());
        let docstore = Arc::new(tokio::sync::RwLock::new(docstore));

        let mut kwargs = HashMap::new();
        kwargs.insert("lambda_mult".to_string(), serde_json::json!(0.7));
        kwargs.insert("fetch_k".to_string(), serde_json::json!(30));

        let retriever = MultiVectorRetriever::new(
            vectorstore,
            docstore,
            "doc_id".to_string(),
            SearchType::MMR,
            kwargs,
        );

        assert!(matches!(retriever.search_type, SearchType::MMR));
    }

    #[tokio::test]
    async fn test_multi_vector_retriever_extract_parent_ids_empty() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore: Box<dyn BaseStore<String, Document>> = Box::new(InMemoryStore::new());
        let docstore = Arc::new(tokio::sync::RwLock::new(docstore));

        let retriever = MultiVectorRetriever::new(
            vectorstore,
            docstore,
            "parent_id".to_string(),
            SearchType::Similarity,
            HashMap::new(),
        );

        let docs = vec![];
        let ids = retriever.extract_parent_ids(&docs);

        assert_eq!(ids.len(), 0);
    }

    #[tokio::test]
    async fn test_multi_vector_retriever_extract_parent_ids_no_matches() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore: Box<dyn BaseStore<String, Document>> = Box::new(InMemoryStore::new());
        let docstore = Arc::new(tokio::sync::RwLock::new(docstore));

        let retriever = MultiVectorRetriever::new(
            vectorstore,
            docstore,
            "parent_id".to_string(),
            SearchType::Similarity,
            HashMap::new(),
        );

        // Documents without parent_id metadata
        let docs = vec![
            Document::new("chunk1").with_metadata("other_field", "value1"),
            Document::new("chunk2").with_metadata("other_field", "value2"),
        ];

        let ids = retriever.extract_parent_ids(&docs);

        assert_eq!(ids.len(), 0);
    }

    #[tokio::test]
    async fn test_multi_vector_retriever_extract_parent_ids_preserves_order() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore: Box<dyn BaseStore<String, Document>> = Box::new(InMemoryStore::new());
        let docstore = Arc::new(tokio::sync::RwLock::new(docstore));

        let retriever = MultiVectorRetriever::new(
            vectorstore,
            docstore,
            "parent_id".to_string(),
            SearchType::Similarity,
            HashMap::new(),
        );

        let docs = vec![
            Document::new("chunk1").with_metadata("parent_id", "doc_C"),
            Document::new("chunk2").with_metadata("parent_id", "doc_A"),
            Document::new("chunk3").with_metadata("parent_id", "doc_B"),
        ];

        let ids = retriever.extract_parent_ids(&docs);

        // Should preserve first-seen order
        assert_eq!(ids, vec!["doc_C", "doc_A", "doc_B"]);
    }

    #[tokio::test]
    async fn test_multi_vector_retriever_extract_non_string_ids() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore: Box<dyn BaseStore<String, Document>> = Box::new(InMemoryStore::new());
        let docstore = Arc::new(tokio::sync::RwLock::new(docstore));

        let retriever = MultiVectorRetriever::new(
            vectorstore,
            docstore,
            "parent_id".to_string(),
            SearchType::Similarity,
            HashMap::new(),
        );

        // Non-string parent IDs (numbers, booleans, etc.)
        let docs = vec![
            Document::new("chunk1").with_metadata("parent_id", 123),
            Document::new("chunk2").with_metadata("parent_id", true),
            Document::new("chunk3").with_metadata("parent_id", "string_id"),
        ];

        let ids = retriever.extract_parent_ids(&docs);

        // Should convert all to strings
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0], "123");
        assert_eq!(ids[1], "true");
        assert_eq!(ids[2], "string_id");
    }

    // ----------------------------------------------------------------------------
    // Trait Implementation Tests
    // ----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_parent_retriever_name() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(10));

        let retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        assert_eq!(retriever.name(), "ParentDocumentRetriever");
    }

    #[tokio::test]
    async fn test_multi_vector_retriever_name() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore: Box<dyn BaseStore<String, Document>> = Box::new(InMemoryStore::new());
        let docstore = Arc::new(tokio::sync::RwLock::new(docstore));

        let retriever = MultiVectorRetriever::new(
            vectorstore,
            docstore,
            "doc_id".to_string(),
            SearchType::Similarity,
            HashMap::new(),
        );

        assert_eq!(retriever.name(), "MultiVectorRetriever");
    }

    // ----------------------------------------------------------------------------
    // Stress Tests
    // ----------------------------------------------------------------------------

    #[tokio::test]
    async fn test_parent_retriever_large_document() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(100));
        let parent_splitter = Box::new(SimpleTextSplitter::new(500));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            Some(parent_splitter),
            "doc_id".to_string(),
            None,
        );

        // 10,000 character document
        let large_doc = Document::new("X".repeat(10000));

        retriever
            .add_documents(vec![large_doc], None, true)
            .await
            .unwrap();

        let results = retriever._get_relevant_documents("X", None).await.unwrap();

        // Should handle large documents
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_parent_retriever_many_small_chunks() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());

        // Very small chunks: 5 chars each
        let child_splitter = Box::new(SimpleTextSplitter::new(5));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        // 100-char document = 20 chunks
        let doc = Document::new("A".repeat(100));

        retriever
            .add_documents(vec![doc.clone()], None, true)
            .await
            .unwrap();

        let results = retriever._get_relevant_documents("A", None).await.unwrap();

        // Should return parent, not individual chunks
        assert_eq!(results[0].page_content, doc.page_content);
    }

    #[tokio::test]
    async fn test_parent_retriever_mixed_document_sizes() {
        let embeddings = Arc::new(MockEmbeddings);
        let vectorstore = Arc::new(InMemoryVectorStore::new(embeddings));
        let docstore = Box::new(InMemoryStore::new());
        let child_splitter = Box::new(SimpleTextSplitter::new(20));

        let mut retriever = ParentDocumentRetriever::new(
            vectorstore,
            docstore,
            child_splitter,
            None,
            "doc_id".to_string(),
            None,
        );

        // Mix of small, medium, and large documents
        let docs = vec![
            Document::new("Short"),
            Document::new("Medium length document ".repeat(5)),
            Document::new("Very long document with lots of content ".repeat(20)),
        ];

        retriever.add_documents(docs, None, true).await.unwrap();

        let results = retriever
            ._get_relevant_documents("document", None)
            .await
            .unwrap();

        // Should handle mixed sizes
        assert!(!results.is_empty());
    }
}
