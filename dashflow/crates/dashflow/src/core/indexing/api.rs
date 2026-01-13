//! Main indexing API for managing vector store content
//!
//! Provides the `index()` function for intelligently indexing documents with
//! change detection, deduplication, and cleanup.

use super::document_index::DocumentIndex;
use super::hashing::{
    deduplicate_documents, hash_document, hash_document_with_encoder, HashAlgorithm,
};
use super::record_manager::RecordManager;
use crate::core::documents::Document;
use std::collections::HashSet;
use thiserror::Error;

/// Cleanup strategy for outdated documents
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupMode {
    /// No cleanup - only add/update documents
    None,

    /// Incremental cleanup during indexing
    ///
    /// Deletes documents that:
    /// - Have the same `source_id` as documents in this batch
    /// - Were not updated in this indexing run
    ///
    /// Cleanup happens continuously during indexing, minimizing the window
    /// where users might see duplicates.
    ///
    /// **Requirements**: `source_id_key` must be provided
    Incremental,

    /// Full cleanup after indexing
    ///
    /// Deletes ALL documents that were not returned by the loader in this run.
    /// Use carefully - the loader must return the complete dataset, not a subset.
    ///
    /// Cleanup happens after all documents are indexed, so users may temporarily
    /// see duplicates during indexing.
    Full,

    /// Scoped full cleanup after indexing
    ///
    /// Like Full, but only deletes documents with `source_ids` seen in this run.
    /// Safer than Full when the loader might return a subset of data.
    ///
    /// **Requirements**: `source_id_key` must be provided
    ScopedFull,
}

/// Error types for indexing operations
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum IndexingError {
    /// Source ID key is required for the specified cleanup mode.
    #[error("Source ID key is required for cleanup mode {0:?}")]
    SourceIdRequired(CleanupMode),

    /// A document is missing the required source ID in its metadata.
    #[error("Document missing source ID: {0}")]
    MissingSourceId(String),

    /// The specified cleanup mode is not recognized.
    #[error("Invalid cleanup mode: {0}")]
    InvalidCleanupMode(String),

    /// An error occurred in the record manager backend.
    #[error("Record manager error: {0}")]
    RecordManager(String),

    /// An error occurred in the document index backend.
    #[error("Document index error: {0}")]
    DocumentIndex(String),

    /// Failed to serialize document metadata.
    #[error("Metadata serialization error: {0}")]
    MetadataSerialization(String),

    /// A delete operation failed during cleanup.
    #[error("Delete operation failed")]
    DeleteFailed,
}

/// Result of an indexing operation
///
/// Provides detailed statistics about what happened during indexing.
#[derive(Debug, Clone, Default)]
pub struct IndexingResult {
    /// Number of new documents added
    pub num_added: usize,
    /// Number of existing documents updated (because they changed)
    pub num_updated: usize,
    /// Number of documents skipped (because they're already up-to-date)
    pub num_skipped: usize,
    /// Number of outdated documents deleted during cleanup
    pub num_deleted: usize,
}

impl IndexingResult {
    /// Total number of documents processed (added + updated + skipped)
    #[must_use]
    pub fn total_processed(&self) -> usize {
        self.num_added + self.num_updated + self.num_skipped
    }

    /// Check if any work was done
    #[must_use]
    pub fn has_changes(&self) -> bool {
        self.num_added > 0 || self.num_updated > 0 || self.num_deleted > 0
    }
}

/// Function type for extracting source ID from a document
pub type SourceIdExtractor = Box<dyn Fn(&Document) -> Option<String> + Send + Sync>;

/// Function type for custom document hashing
pub type KeyEncoder = Box<dyn Fn(&Document) -> String + Send + Sync>;

/// Index documents into a vector store with intelligent change detection
///
/// This function orchestrates the complete indexing workflow:
/// 1. Hash documents to generate unique IDs
/// 2. Check which documents already exist (via `RecordManager`)
/// 3. Upsert new or changed documents
/// 4. Update record timestamps
/// 5. Clean up outdated documents (based on cleanup mode)
///
/// # Arguments
///
/// * `docs` - Documents to index
/// * `record_manager` - Tracks indexed documents and timestamps
/// * `document_index` - Vector store or index to write to
/// * `cleanup` - Cleanup strategy for outdated documents
/// * `source_id_key` - Metadata key for source identification (required for incremental/scoped cleanup)
/// * `batch_size` - Number of documents to process per batch
/// * `cleanup_batch_size` - Number of documents to delete per batch during cleanup
/// * `force_update` - Re-index all documents even if unchanged
/// * `hash_algorithm` - Algorithm for hashing documents
/// * `key_encoder` - Optional custom function for generating document IDs
///
/// # Returns
///
/// Indexing result with statistics about operations performed
///
/// # Errors
///
/// Returns error if:
/// - Cleanup mode requires `source_id_key` but it's not provided
/// - Documents are missing required source IDs
/// - Record manager or document index operations fail
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::indexing::{index, InMemoryRecordManager, CleanupMode, HashAlgorithm};
/// use dashflow::core::documents::Document;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let record_manager = InMemoryRecordManager::new("my_index");
/// let docs = vec![
///     Document::new("First document").with_metadata("source", "file1.txt"),
///     Document::new("Second document").with_metadata("source", "file2.txt"),
/// ];
///
/// // Index with incremental cleanup
/// // let result = index(
/// //     docs,
/// //     &record_manager,
/// //     &document_index,
/// //     CleanupMode::Incremental,
/// //     Some("source"),
/// //     100,
/// //     1000,
/// //     false,
/// //     HashAlgorithm::Sha256,
/// //     None,
/// // ).await?;
/// # Ok(())
/// # }
/// ```
#[allow(clippy::too_many_arguments)] // Indexing config: cleanup mode, batch sizes, hashing, force update
pub async fn index<R, D>(
    docs: Vec<Document>,
    record_manager: &R,
    document_index: &D,
    cleanup: CleanupMode,
    source_id_key: Option<&str>,
    batch_size: usize,
    cleanup_batch_size: usize,
    force_update: bool,
    hash_algorithm: HashAlgorithm,
    key_encoder: Option<KeyEncoder>,
) -> Result<IndexingResult, IndexingError>
where
    R: RecordManager + ?Sized,
    D: DocumentIndex + ?Sized,
{
    // Validate cleanup mode requirements
    if matches!(cleanup, CleanupMode::Incremental | CleanupMode::ScopedFull)
        && source_id_key.is_none()
    {
        return Err(IndexingError::SourceIdRequired(cleanup));
    }

    // Create source ID extractor
    let source_id_extractor: SourceIdExtractor = if let Some(key) = source_id_key {
        let key = key.to_string();
        Box::new(move |doc: &Document| {
            doc.metadata
                .get(&key)
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string)
        })
    } else {
        Box::new(|_| None)
    };

    // Mark indexing start time
    let index_start_time = record_manager.get_time().await;

    let mut result = IndexingResult::default();
    let mut scoped_full_source_ids: HashSet<String> = HashSet::new();

    // Process documents in batches
    let batches: Vec<_> = docs.chunks(batch_size).collect();

    for batch in batches {
        // Hash documents and assign IDs
        let mut hashed_docs: Vec<Document> = batch
            .iter()
            .map(|doc| {
                let id = if let Some(ref encoder) = key_encoder {
                    hash_document_with_encoder(doc, encoder.as_ref())
                } else {
                    hash_document(doc, hash_algorithm)
                };

                let mut new_doc = doc.clone();
                new_doc.id = Some(id);
                new_doc
            })
            .collect();

        // Track original batch size before deduplication
        let original_batch_size = hashed_docs.len();

        // Deduplicate within batch
        hashed_docs = deduplicate_documents(hashed_docs);
        result.num_skipped += original_batch_size - hashed_docs.len();

        // Extract source IDs
        let source_ids: Vec<Option<String>> =
            hashed_docs.iter().map(&source_id_extractor).collect();

        // Validate source IDs if required by cleanup mode
        if matches!(cleanup, CleanupMode::Incremental | CleanupMode::ScopedFull) {
            for (doc, source_id) in hashed_docs.iter().zip(source_ids.iter()) {
                if source_id.is_none() {
                    let preview = if doc.page_content.len() > 100 {
                        &doc.page_content[..100]
                    } else {
                        &doc.page_content
                    };
                    return Err(IndexingError::MissingSourceId(preview.to_string()));
                }
                if cleanup == CleanupMode::ScopedFull {
                    if let Some(ref sid) = source_id {
                        scoped_full_source_ids.insert(sid.clone());
                    }
                }
            }
        }

        // Check which documents already exist
        let doc_ids: Vec<String> = hashed_docs
            .iter()
            .filter_map(|doc| doc.id.clone())
            .collect();

        let exists_batch = record_manager
            .exists(&doc_ids)
            .await
            .map_err(|e| IndexingError::RecordManager(e.to_string()))?;

        // Separate documents into: to_index (new/changed) and to_refresh (unchanged)
        let mut docs_to_index = Vec::new();
        let mut ids_to_index = Vec::new();
        let mut ids_to_refresh = Vec::new();
        let mut seen_doc_ids = HashSet::new();

        for (doc, exists) in hashed_docs.iter().zip(exists_batch.iter()) {
            let Some(doc_id) = doc.id.clone() else {
                continue;
            };

            if *exists {
                if force_update {
                    // Force update: treat as new document
                    seen_doc_ids.insert(doc_id.clone());
                    docs_to_index.push(doc.clone());
                    ids_to_index.push(doc_id);
                } else {
                    // Already exists and unchanged: just refresh timestamp
                    ids_to_refresh.push(doc_id);
                }
            } else {
                // New document
                docs_to_index.push(doc.clone());
                ids_to_index.push(doc_id);
            }
        }

        // Update refresh timestamps for unchanged documents
        if !ids_to_refresh.is_empty() {
            record_manager
                .update(&ids_to_refresh, None, Some(index_start_time))
                .await
                .map_err(|e| IndexingError::RecordManager(e.to_string()))?;
            result.num_skipped += ids_to_refresh.len();
        }

        // Upsert new/changed documents to vector store
        if !docs_to_index.is_empty() {
            let upsert_response = document_index
                .upsert(&docs_to_index)
                .await
                .map_err(|e| IndexingError::DocumentIndex(e.to_string()))?;

            // Check for failures
            if !upsert_response.is_success() {
                return Err(IndexingError::DocumentIndex(format!(
                    "{} documents failed to upsert",
                    upsert_response.failed.len()
                )));
            }

            // Update statistics
            result.num_added += docs_to_index.len() - seen_doc_ids.len();
            result.num_updated += seen_doc_ids.len();
        }

        // Update record manager with all document IDs (including refreshed)
        let all_source_ids: Option<Vec<Option<String>>> = if source_id_key.is_some() {
            Some(source_ids.clone())
        } else {
            None
        };

        record_manager
            .update(&doc_ids, all_source_ids.as_deref(), Some(index_start_time))
            .await
            .map_err(|e| IndexingError::RecordManager(e.to_string()))?;

        // Incremental cleanup: delete outdated docs with same source_id
        if cleanup == CleanupMode::Incremental {
            let source_id_strings: Vec<String> = source_ids
                .iter()
                .filter_map(std::clone::Clone::clone)
                .collect();

            if !source_id_strings.is_empty() {
                loop {
                    let ids_to_delete = record_manager
                        .list_keys(
                            Some(index_start_time),
                            None,
                            Some(&source_id_strings),
                            Some(cleanup_batch_size),
                        )
                        .await
                        .map_err(|e| IndexingError::RecordManager(e.to_string()))?;

                    if ids_to_delete.is_empty() {
                        break;
                    }

                    // Delete from vector store
                    delete_from_index(document_index, &ids_to_delete).await?;

                    // Delete from record manager
                    record_manager
                        .delete_keys(&ids_to_delete)
                        .await
                        .map_err(|e| IndexingError::RecordManager(e.to_string()))?;

                    result.num_deleted += ids_to_delete.len();

                    // Break if we got fewer than requested (no more to delete)
                    if ids_to_delete.len() < cleanup_batch_size {
                        break;
                    }
                }
            }
        }
    }

    // Full or scoped full cleanup: delete all outdated documents
    if matches!(cleanup, CleanupMode::Full | CleanupMode::ScopedFull) {
        let group_ids: Option<Vec<String>> = if cleanup == CleanupMode::ScopedFull {
            Some(scoped_full_source_ids.into_iter().collect())
        } else {
            None
        };

        loop {
            let ids_to_delete = record_manager
                .list_keys(
                    Some(index_start_time),
                    None,
                    group_ids.as_deref(),
                    Some(cleanup_batch_size),
                )
                .await
                .map_err(|e| IndexingError::RecordManager(e.to_string()))?;

            if ids_to_delete.is_empty() {
                break;
            }

            // Delete from vector store
            delete_from_index(document_index, &ids_to_delete).await?;

            // Delete from record manager
            record_manager
                .delete_keys(&ids_to_delete)
                .await
                .map_err(|e| IndexingError::RecordManager(e.to_string()))?;

            result.num_deleted += ids_to_delete.len();

            if ids_to_delete.len() < cleanup_batch_size {
                break;
            }
        }
    }

    Ok(result)
}

/// Helper function to delete documents from index and check for failures
async fn delete_from_index<D>(document_index: &D, ids: &[String]) -> Result<(), IndexingError>
where
    D: DocumentIndex + ?Sized,
{
    let delete_response = document_index
        .delete(Some(ids))
        .await
        .map_err(|e| IndexingError::DocumentIndex(e.to_string()))?;

    if delete_response.has_failures() {
        return Err(IndexingError::DeleteFailed);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{IndexingError, KeyEncoder};
    use crate::core::config::RunnableConfig;
    use crate::core::documents::Document;
    use crate::core::error::Result as LcResult;
    use crate::core::indexing::{DeleteResponse, InMemoryRecordManager, UpsertResponse};
    use crate::core::retrievers::Retriever;
    use crate::test_prelude::*;
    use async_trait::async_trait;

    // Mock DocumentIndex for testing
    struct MockDocumentIndex {
        documents: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, Document>>>,
    }

    impl MockDocumentIndex {
        fn new() -> Self {
            Self {
                documents: std::sync::Arc::new(tokio::sync::RwLock::new(
                    std::collections::HashMap::new(),
                )),
            }
        }

        async fn count(&self) -> usize {
            self.documents.read().await.len()
        }
    }

    #[async_trait]
    impl Retriever for MockDocumentIndex {
        async fn _get_relevant_documents(
            &self,
            _query: &str,
            _config: Option<&RunnableConfig>,
        ) -> LcResult<Vec<Document>> {
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl DocumentIndex for MockDocumentIndex {
        async fn upsert(
            &self,
            items: &[Document],
        ) -> std::result::Result<UpsertResponse, Box<dyn std::error::Error + Send + Sync>> {
            let mut docs = self.documents.write().await;
            let mut succeeded = Vec::new();

            for doc in items {
                if let Some(ref id) = doc.id {
                    docs.insert(id.clone(), doc.clone());
                    succeeded.push(id.clone());
                }
            }

            Ok(UpsertResponse {
                succeeded,
                failed: Vec::new(),
            })
        }

        async fn delete(
            &self,
            ids: Option<&[String]>,
        ) -> std::result::Result<DeleteResponse, Box<dyn std::error::Error + Send + Sync>> {
            let mut docs = self.documents.write().await;

            if let Some(ids) = ids {
                let mut count = 0;
                for id in ids {
                    if docs.remove(id).is_some() {
                        count += 1;
                    }
                }
                Ok(DeleteResponse::with_count(count))
            } else {
                let count = docs.len();
                docs.clear();
                Ok(DeleteResponse::with_count(count))
            }
        }

        async fn get(
            &self,
            ids: &[String],
        ) -> std::result::Result<Vec<Document>, Box<dyn std::error::Error + Send + Sync>> {
            let docs = self.documents.read().await;
            Ok(ids.iter().filter_map(|id| docs.get(id).cloned()).collect())
        }
    }

    #[tokio::test]
    async fn test_index_basic() {
        let record_manager = InMemoryRecordManager::new("test_index_basic");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![
            Document::new("First document"),
            Document::new("Second document"),
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 2);
        assert_eq!(result.num_updated, 0);
        assert_eq!(result.num_skipped, 0);
        assert_eq!(result.num_deleted, 0);
        assert_eq!(doc_index.count().await, 2);
        assert_eq!(record_manager.len(), 2);
    }

    #[tokio::test]
    async fn test_index_deduplication() {
        let record_manager = InMemoryRecordManager::new("test_index_deduplication");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![
            Document::new("Same content"),
            Document::new("Same content"), // Duplicate
            Document::new("Different content"),
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        // Duplicate should be skipped
        assert_eq!(result.num_added, 2);
        assert_eq!(result.num_skipped, 1);
        assert_eq!(doc_index.count().await, 2);
    }

    #[tokio::test]
    async fn test_index_skip_unchanged() {
        let record_manager = InMemoryRecordManager::new("test_index_skip_unchanged");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![Document::new("First"), Document::new("Second")];

        // First indexing
        let result1 = index(
            docs.clone(),
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result1.num_added, 2);

        // Second indexing with same documents
        let result2 = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        // Should skip both documents
        assert_eq!(result2.num_added, 0);
        assert_eq!(result2.num_skipped, 2);
    }

    #[tokio::test]
    async fn test_index_force_update() {
        let record_manager = InMemoryRecordManager::new("test_index_force_update");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![Document::new("Content")];

        // First indexing
        index(
            docs.clone(),
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        // Second indexing with force_update=true
        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            true, // Force update
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_updated, 1);
        assert_eq!(result.num_added, 0);
        assert_eq!(result.num_skipped, 0);
    }

    #[tokio::test]
    async fn test_index_incremental_cleanup_requires_source_id() {
        let record_manager =
            InMemoryRecordManager::new("test_index_incremental_cleanup_requires_source_id");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![Document::new("Content")];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            None, // No source_id_key
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            IndexingError::SourceIdRequired(_)
        ));
    }

    #[tokio::test]
    async fn test_index_scoped_full_cleanup_requires_source_id() {
        let record_manager =
            InMemoryRecordManager::new("test_index_scoped_full_cleanup_requires_source_id");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![Document::new("Content")];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::ScopedFull,
            None, // No source_id_key
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            IndexingError::SourceIdRequired(_)
        ));
    }

    #[tokio::test]
    async fn test_index_incremental_cleanup_missing_source_id() {
        let record_manager =
            InMemoryRecordManager::new("test_index_incremental_cleanup_missing_source_id");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![
            Document::new("Doc with source").with_metadata("source", "file1.txt"),
            Document::new("Doc without source"), // Missing source
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            IndexingError::MissingSourceId(_)
        ));
    }

    #[tokio::test]
    async fn test_index_incremental_cleanup_success() {
        let record_manager = InMemoryRecordManager::new("test_index_incremental_cleanup_success");
        let doc_index = MockDocumentIndex::new();

        // First batch: index two documents from source1
        let docs1 = vec![
            Document::new("First doc v1").with_metadata("source", "source1"),
            Document::new("Second doc v1").with_metadata("source", "source1"),
        ];

        let result1 = index(
            docs1,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result1.num_added, 2);
        assert_eq!(result1.num_deleted, 0);
        assert_eq!(doc_index.count().await, 2);

        // Small delay to ensure index_start_time differs from first batch timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Second batch: replace with one updated doc (should delete the old one)
        let docs2 = vec![Document::new("First doc v2").with_metadata("source", "source1")];

        let result2 = index(
            docs2,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result2.num_added, 1);
        assert_eq!(result2.num_deleted, 2); // Old docs deleted
        assert_eq!(doc_index.count().await, 1);
    }

    #[tokio::test]
    async fn test_index_full_cleanup_success() {
        let record_manager = InMemoryRecordManager::new("test_index_full_cleanup_success");
        let doc_index = MockDocumentIndex::new();

        // First batch: index three documents
        let docs1 = vec![
            Document::new("Doc 1"),
            Document::new("Doc 2"),
            Document::new("Doc 3"),
        ];

        let result1 = index(
            docs1,
            &record_manager,
            &doc_index,
            CleanupMode::Full,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result1.num_added, 3);
        assert_eq!(doc_index.count().await, 3);

        // Small delay to ensure index_start_time differs from first batch timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Second batch: only one document (should delete the other two)
        let docs2 = vec![Document::new("Doc 1")]; // Only Doc 1

        let result2 = index(
            docs2,
            &record_manager,
            &doc_index,
            CleanupMode::Full,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result2.num_skipped, 1); // Doc 1 already exists
        assert_eq!(result2.num_deleted, 2); // Doc 2 and 3 deleted
        assert_eq!(doc_index.count().await, 1);
    }

    #[tokio::test]
    async fn test_index_scoped_full_cleanup_success() {
        let record_manager = InMemoryRecordManager::new("test_index_scoped_full_cleanup_success");
        let doc_index = MockDocumentIndex::new();

        // Index documents from two different sources
        let docs1 = vec![
            Document::new("Source1 Doc1").with_metadata("source", "source1"),
            Document::new("Source1 Doc2").with_metadata("source", "source1"),
            Document::new("Source2 Doc1").with_metadata("source", "source2"),
        ];

        index(
            docs1,
            &record_manager,
            &doc_index,
            CleanupMode::ScopedFull,
            Some("source"),
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(doc_index.count().await, 3);

        // Small delay to ensure index_start_time differs from first batch timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Re-index only source1 with one document
        // Should only delete old source1 docs, leaving source2 untouched
        let docs2 = vec![Document::new("Source1 Doc1").with_metadata("source", "source1")];

        let result = index(
            docs2,
            &record_manager,
            &doc_index,
            CleanupMode::ScopedFull,
            Some("source"),
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_skipped, 1); // Doc1 already exists
        assert_eq!(result.num_deleted, 1); // Only Source1 Doc2 deleted
        assert_eq!(doc_index.count().await, 2); // Source1 Doc1 and Source2 Doc1 remain
    }

    #[tokio::test]
    async fn test_index_batch_size() {
        let record_manager = InMemoryRecordManager::new("test_index_batch_size");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![
            Document::new("Doc 1"),
            Document::new("Doc 2"),
            Document::new("Doc 3"),
            Document::new("Doc 4"),
            Document::new("Doc 5"),
        ];

        // Use small batch size to test batching
        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            2, // Batch size of 2
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 5);
        assert_eq!(doc_index.count().await, 5);
    }

    #[tokio::test]
    async fn test_index_hash_algorithms() {
        let record_manager = InMemoryRecordManager::new("test_index_hash_algorithms");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![Document::new("Test content")];

        // Test with SHA-1
        let result_sha1 = index(
            docs.clone(),
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha1,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result_sha1.num_added, 1);

        // Test with SHA256 (should generate different hash)
        let record_manager2 = InMemoryRecordManager::new("test2");
        let result_sha = index(
            docs,
            &record_manager2,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result_sha.num_added, 1);
    }

    #[tokio::test]
    async fn test_index_custom_key_encoder() {
        let record_manager = InMemoryRecordManager::new("test_index_custom_key_encoder");
        let doc_index = MockDocumentIndex::new();

        // Custom encoder that uses page_content length as ID
        let key_encoder: KeyEncoder =
            Box::new(|doc: &Document| format!("custom_{}", doc.page_content.len()));

        let docs = vec![
            Document::new("Short"),       // custom_5
            Document::new("Much longer"), // custom_11
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            Some(key_encoder),
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 2);

        // Verify custom IDs were used
        let stored_docs = doc_index.get(&["custom_5".to_string()]).await.unwrap();
        assert_eq!(stored_docs.len(), 1);
        assert_eq!(stored_docs[0].page_content, "Short");
    }

    #[tokio::test]
    async fn test_indexing_result_methods() {
        let result = IndexingResult {
            num_added: 5,
            num_updated: 3,
            num_skipped: 2,
            num_deleted: 1,
        };

        assert_eq!(result.total_processed(), 10); // 5 + 3 + 2
        assert!(result.has_changes()); // Has adds, updates, and deletes

        let no_changes = IndexingResult {
            num_added: 0,
            num_updated: 0,
            num_skipped: 5,
            num_deleted: 0,
        };

        assert_eq!(no_changes.total_processed(), 5);
        assert!(!no_changes.has_changes());
    }

    #[tokio::test]
    async fn test_cleanup_mode_equality() {
        assert_eq!(CleanupMode::None, CleanupMode::None);
        assert_eq!(CleanupMode::Incremental, CleanupMode::Incremental);
        assert_eq!(CleanupMode::Full, CleanupMode::Full);
        assert_eq!(CleanupMode::ScopedFull, CleanupMode::ScopedFull);

        assert_ne!(CleanupMode::None, CleanupMode::Incremental);
        assert_ne!(CleanupMode::Full, CleanupMode::ScopedFull);
    }

    #[tokio::test]
    async fn test_index_with_metadata() {
        let record_manager = InMemoryRecordManager::new("test_index_with_metadata");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![
            Document::new("Doc 1")
                .with_metadata("author", "Alice")
                .with_metadata("category", "tech"),
            Document::new("Doc 2")
                .with_metadata("author", "Bob")
                .with_metadata("category", "science"),
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 2);

        // Verify metadata is preserved
        let all_docs = doc_index
            .get(
                &record_manager
                    .list_keys(None, None, None, None)
                    .await
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(all_docs.len(), 2);
        assert!(all_docs
            .iter()
            .any(|d| d.metadata.get("author").and_then(|v| v.as_str()) == Some("Alice")));
        assert!(all_docs
            .iter()
            .any(|d| d.metadata.get("author").and_then(|v| v.as_str()) == Some("Bob")));
    }

    #[tokio::test]
    async fn test_index_empty_documents() {
        let record_manager = InMemoryRecordManager::new("test_index_empty_documents");
        let doc_index = MockDocumentIndex::new();

        let docs: Vec<Document> = vec![];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 0);
        assert_eq!(result.num_updated, 0);
        assert_eq!(result.num_skipped, 0);
        assert_eq!(result.num_deleted, 0);
        assert_eq!(doc_index.count().await, 0);
    }

    #[tokio::test]
    async fn test_index_large_batch_with_cleanup() {
        let record_manager = InMemoryRecordManager::new("test_index_large_batch_with_cleanup");
        let doc_index = MockDocumentIndex::new();

        // Index a batch of documents
        let docs1: Vec<Document> = (0..20)
            .map(|i| Document::new(format!("Document {}", i)).with_metadata("source", "batch1"))
            .collect();

        let result1 = index(
            docs1,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            10, // Small batch size
            50, // Cleanup batch size larger than expected deletions
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result1.num_added, 20);
        assert_eq!(doc_index.count().await, 20);

        // Small delay to ensure index_start_time differs from first batch timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Replace with half the documents
        let docs2: Vec<Document> = (0..10)
            .map(|i| Document::new(format!("Document {}", i)).with_metadata("source", "batch1"))
            .collect();

        let result2 = index(
            docs2,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            10,
            50,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result2.num_skipped, 10); // First 10 unchanged
        assert_eq!(result2.num_deleted, 10); // Last 10 deleted
        assert_eq!(doc_index.count().await, 10);
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[tokio::test]
    async fn test_index_single_document() {
        let record_manager = InMemoryRecordManager::new("test_index_single_document");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![Document::new("Single document")];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 1);
        assert_eq!(result.num_updated, 0);
        assert_eq!(result.num_skipped, 0);
        assert_eq!(result.num_deleted, 0);
        assert_eq!(doc_index.count().await, 1);
    }

    #[tokio::test]
    async fn test_index_batch_size_one() {
        let record_manager = InMemoryRecordManager::new("test_index_batch_size_one");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![
            Document::new("Doc 1"),
            Document::new("Doc 2"),
            Document::new("Doc 3"),
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            1, // Batch size of 1
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 3);
        assert_eq!(doc_index.count().await, 3);
    }

    #[tokio::test]
    async fn test_index_stress_1000_documents() {
        let record_manager = InMemoryRecordManager::new("test_index_stress_1000_documents");
        let doc_index = MockDocumentIndex::new();

        let docs: Vec<Document> = (0..1000)
            .map(|i| Document::new(format!("Document {}", i)))
            .collect();

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 1000);
        assert_eq!(doc_index.count().await, 1000);
        assert_eq!(record_manager.len(), 1000);
    }

    #[tokio::test]
    async fn test_index_very_large_document() {
        let record_manager = InMemoryRecordManager::new("test_index_very_large_document");
        let doc_index = MockDocumentIndex::new();

        // 1MB document
        let large_content = "X".repeat(1_000_000);
        let docs = vec![Document::new(large_content)];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 1);
        assert_eq!(doc_index.count().await, 1);
    }

    #[tokio::test]
    async fn test_index_unicode_content() {
        let record_manager = InMemoryRecordManager::new("test_index_unicode_content");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![
            Document::new("Hello 世界 🌍"),
            Document::new("Привет мир 🚀"),
            Document::new("مرحبا بالعالم 🎉"),
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 3);
        assert_eq!(doc_index.count().await, 3);
    }

    #[tokio::test]
    async fn test_index_empty_content() {
        let record_manager = InMemoryRecordManager::new("test_index_empty_content");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![Document::new(""), Document::new("Not empty")];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 2);
        assert_eq!(doc_index.count().await, 2);
    }

    // ========================================================================
    // Cleanup Batch Size Edge Cases
    // ========================================================================

    #[tokio::test]
    async fn test_index_cleanup_batch_size_one() {
        let record_manager = InMemoryRecordManager::new("test_index_cleanup_batch_size_one");
        let doc_index = MockDocumentIndex::new();

        // Index 5 documents
        let docs1: Vec<Document> = (0..5)
            .map(|i| Document::new(format!("Doc {}", i)).with_metadata("source", "src1"))
            .collect();

        index(
            docs1,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            100,
            1, // Cleanup batch size of 1
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(doc_index.count().await, 5);

        // Small delay to ensure index_start_time differs from first batch timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Replace with 2 documents (should delete 5 old ones, 1 at a time)
        let docs2: Vec<Document> = (0..2)
            .map(|i| Document::new(format!("New Doc {}", i)).with_metadata("source", "src1"))
            .collect();

        let result = index(
            docs2,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            100,
            1, // Cleanup batch size of 1
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 2);
        assert_eq!(result.num_deleted, 5); // Old docs deleted one by one
        assert_eq!(doc_index.count().await, 2);
    }

    #[tokio::test]
    async fn test_index_full_cleanup_large_batch_size() {
        let record_manager = InMemoryRecordManager::new("test_index_full_cleanup_large_batch_size");
        let doc_index = MockDocumentIndex::new();

        // Index 100 documents
        let docs1: Vec<Document> = (0..100)
            .map(|i| Document::new(format!("Doc {}", i)))
            .collect();

        index(
            docs1,
            &record_manager,
            &doc_index,
            CleanupMode::Full,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        // Small delay to ensure index_start_time differs from first batch timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Replace with 10 documents, cleanup in large batches
        let docs2: Vec<Document> = (0..10)
            .map(|i| Document::new(format!("Doc {}", i)))
            .collect();

        let result = index(
            docs2,
            &record_manager,
            &doc_index,
            CleanupMode::Full,
            None,
            100,
            1000, // Large cleanup batch
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_skipped, 10);
        assert_eq!(result.num_deleted, 90);
        assert_eq!(doc_index.count().await, 10);
    }

    // ========================================================================
    // Source ID Edge Cases
    // ========================================================================

    #[tokio::test]
    async fn test_index_empty_string_source_id() {
        let record_manager = InMemoryRecordManager::new("test_index_empty_string_source_id");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![Document::new("Doc with empty source").with_metadata("source", "")];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 1);
        assert_eq!(doc_index.count().await, 1);
    }

    #[tokio::test]
    async fn test_index_special_characters_source_id() {
        let record_manager = InMemoryRecordManager::new("test_index_special_characters_source_id");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![
            Document::new("Doc 1").with_metadata("source", "file://path/to/file.txt"),
            Document::new("Doc 2")
                .with_metadata("source", "https://example.com/doc?id=123&foo=bar"),
            Document::new("Doc 3").with_metadata("source", "source_with_!@#$%^&*()"),
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 3);
        assert_eq!(doc_index.count().await, 3);
    }

    #[tokio::test]
    async fn test_index_source_id_with_unicode() {
        let record_manager = InMemoryRecordManager::new("test_index_source_id_with_unicode");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![
            Document::new("Doc 1").with_metadata("source", "文件.txt"),
            Document::new("Doc 2").with_metadata("source", "файл.doc"),
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 2);
        assert_eq!(doc_index.count().await, 2);
    }

    // ========================================================================
    // Deduplication Tests
    // ========================================================================

    #[tokio::test]
    async fn test_index_deduplication_across_batches() {
        let record_manager = InMemoryRecordManager::new("test_index_deduplication_across_batches");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![
            Document::new("Same content"),      // Batch 1
            Document::new("Different content"), // Batch 1
            Document::new("Same content"),      // Batch 2 - duplicate of batch 1
            Document::new("Another content"),   // Batch 2
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            2, // Batch size of 2
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        // First batch: 2 added
        // Second batch: "Same content" already exists (skipped), "Another content" added
        assert_eq!(result.num_added, 3);
        assert_eq!(result.num_skipped, 1);
        assert_eq!(doc_index.count().await, 3);
    }

    #[tokio::test]
    async fn test_index_all_duplicates() {
        let record_manager = InMemoryRecordManager::new("test_index_all_duplicates");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![
            Document::new("Same"),
            Document::new("Same"),
            Document::new("Same"),
            Document::new("Same"),
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 1);
        assert_eq!(result.num_skipped, 3);
        assert_eq!(doc_index.count().await, 1);
    }

    #[tokio::test]
    async fn test_index_deduplication_within_batch() {
        let record_manager = InMemoryRecordManager::new("test_index_deduplication_within_batch");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![
            Document::new("Unique 1"),
            Document::new("Duplicate"),
            Document::new("Duplicate"), // Within same batch
            Document::new("Unique 2"),
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100, // All in one batch
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 3);
        assert_eq!(result.num_skipped, 1);
        assert_eq!(doc_index.count().await, 3);
    }

    // ========================================================================
    // Force Update with Cleanup Modes
    // ========================================================================

    #[tokio::test]
    async fn test_index_force_update_with_incremental_cleanup() {
        let record_manager =
            InMemoryRecordManager::new("test_index_force_update_with_incremental_cleanup");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![
            Document::new("Doc 1").with_metadata("source", "src1"),
            Document::new("Doc 2").with_metadata("source", "src1"),
        ];

        // First indexing
        index(
            docs.clone(),
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        // Small delay to ensure index_start_time differs from first batch timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Force update with same documents
        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            100,
            1000,
            true, // Force update
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_updated, 2);
        assert_eq!(result.num_added, 0);
        assert_eq!(result.num_skipped, 0);
        assert_eq!(result.num_deleted, 0); // No old documents to delete
    }

    #[tokio::test]
    async fn test_index_force_update_with_full_cleanup() {
        let record_manager =
            InMemoryRecordManager::new("test_index_force_update_with_full_cleanup");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![Document::new("Doc 1"), Document::new("Doc 2")];

        // First indexing
        index(
            docs.clone(),
            &record_manager,
            &doc_index,
            CleanupMode::Full,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        // Small delay to ensure index_start_time differs from first batch timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Force update with same documents
        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::Full,
            None,
            100,
            1000,
            true, // Force update
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_updated, 2);
        assert_eq!(result.num_added, 0);
        assert_eq!(result.num_skipped, 0);
        assert_eq!(result.num_deleted, 0);
    }

    // ========================================================================
    // Concurrent Operations
    // ========================================================================

    #[tokio::test]
    async fn test_index_concurrent_different_sources() {
        let record_manager = std::sync::Arc::new(InMemoryRecordManager::new("test"));
        let doc_index = std::sync::Arc::new(MockDocumentIndex::new());

        let mut handles = vec![];

        for i in 0..10 {
            let rm = record_manager.clone();
            let di = doc_index.clone();

            let handle = tokio::spawn(async move {
                let docs = vec![
                    Document::new(format!("Doc {}-1", i))
                        .with_metadata("source", format!("source{}", i)),
                    Document::new(format!("Doc {}-2", i))
                        .with_metadata("source", format!("source{}", i)),
                ];

                index(
                    docs,
                    rm.as_ref(),
                    di.as_ref(),
                    CleanupMode::Incremental,
                    Some("source"),
                    100,
                    1000,
                    false,
                    HashAlgorithm::Sha256,
                    None,
                )
                .await
                .unwrap()
            });

            handles.push(handle);
        }

        let results: Vec<_> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // Each concurrent operation should add 2 documents
        for result in &results {
            assert_eq!(result.num_added, 2);
        }

        assert_eq!(doc_index.count().await, 20); // 10 sources × 2 docs
    }

    // ========================================================================
    // Batch Processing Validation
    // ========================================================================

    #[tokio::test]
    async fn test_index_exact_batch_boundary() {
        let record_manager = InMemoryRecordManager::new("test_index_exact_batch_boundary");
        let doc_index = MockDocumentIndex::new();

        // Exactly 3 batches of 10 documents each
        let docs: Vec<Document> = (0..30)
            .map(|i| Document::new(format!("Doc {}", i)))
            .collect();

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            10, // Batch size exactly divides document count
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 30);
        assert_eq!(doc_index.count().await, 30);
    }

    #[tokio::test]
    async fn test_index_remainder_batch() {
        let record_manager = InMemoryRecordManager::new("test_index_remainder_batch");
        let doc_index = MockDocumentIndex::new();

        // 3 full batches + 1 remainder batch
        let docs: Vec<Document> = (0..31)
            .map(|i| Document::new(format!("Doc {}", i)))
            .collect();

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            10, // Last batch will have 1 document
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 31);
        assert_eq!(doc_index.count().await, 31);
    }

    // ========================================================================
    // IndexingResult Tests
    // ========================================================================

    #[tokio::test]
    async fn test_indexing_result_default() {
        let result = IndexingResult::default();

        assert_eq!(result.num_added, 0);
        assert_eq!(result.num_updated, 0);
        assert_eq!(result.num_skipped, 0);
        assert_eq!(result.num_deleted, 0);
        assert_eq!(result.total_processed(), 0);
        assert!(!result.has_changes());
    }

    #[tokio::test]
    async fn test_indexing_result_has_changes_added_only() {
        let result = IndexingResult {
            num_added: 5,
            num_updated: 0,
            num_skipped: 0,
            num_deleted: 0,
        };

        assert!(result.has_changes());
    }

    #[tokio::test]
    async fn test_indexing_result_has_changes_updated_only() {
        let result = IndexingResult {
            num_added: 0,
            num_updated: 3,
            num_skipped: 0,
            num_deleted: 0,
        };

        assert!(result.has_changes());
    }

    #[tokio::test]
    async fn test_indexing_result_has_changes_deleted_only() {
        let result = IndexingResult {
            num_added: 0,
            num_updated: 0,
            num_skipped: 5,
            num_deleted: 2,
        };

        assert!(result.has_changes());
    }

    // ========================================================================
    // Multiple Cleanup Iterations
    // ========================================================================

    #[tokio::test]
    async fn test_index_multiple_incremental_cleanups() {
        let record_manager = InMemoryRecordManager::new("test_index_multiple_incremental_cleanups");
        let doc_index = MockDocumentIndex::new();

        // Iteration 1: Index 10 documents
        let docs1: Vec<Document> = (0..10)
            .map(|i| Document::new(format!("Ver1 Doc {}", i)).with_metadata("source", "src1"))
            .collect();

        index(
            docs1,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(doc_index.count().await, 10);

        // Small delay to ensure index_start_time differs from first batch timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Iteration 2: Replace with 5 documents
        let docs2: Vec<Document> = (0..5)
            .map(|i| Document::new(format!("Ver2 Doc {}", i)).with_metadata("source", "src1"))
            .collect();

        let result2 = index(
            docs2,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result2.num_added, 5);
        assert_eq!(result2.num_deleted, 10);
        assert_eq!(doc_index.count().await, 5);

        // Small delay to ensure index_start_time differs from previous batch timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Iteration 3: Replace with 3 documents
        let docs3: Vec<Document> = (0..3)
            .map(|i| Document::new(format!("Ver3 Doc {}", i)).with_metadata("source", "src1"))
            .collect();

        let result3 = index(
            docs3,
            &record_manager,
            &doc_index,
            CleanupMode::Incremental,
            Some("source"),
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result3.num_added, 3);
        assert_eq!(result3.num_deleted, 5);
        assert_eq!(doc_index.count().await, 3);
    }

    // ========================================================================
    // Mixed Metadata Tests
    // ========================================================================

    #[tokio::test]
    async fn test_index_complex_metadata() {
        let record_manager = InMemoryRecordManager::new("test_index_complex_metadata");
        let doc_index = MockDocumentIndex::new();

        let docs = vec![Document::new("Doc 1")
            .with_metadata("source", "file1.txt")
            .with_metadata("author", "Alice")
            .with_metadata("timestamp", "2024-01-01")
            .with_metadata("tags", "rust,programming")
            .with_metadata("version", "1.0")];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.num_added, 1);

        // Verify metadata is preserved
        let stored_docs = doc_index
            .get(
                &record_manager
                    .list_keys(None, None, None, None)
                    .await
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(stored_docs.len(), 1);
        let doc = &stored_docs[0];
        assert_eq!(
            doc.metadata.get("author").and_then(|v| v.as_str()),
            Some("Alice")
        );
        assert_eq!(
            doc.metadata.get("version").and_then(|v| v.as_str()),
            Some("1.0")
        );
    }

    // ========================================================================
    // Hash Algorithm Variations
    // ========================================================================

    #[tokio::test]
    async fn test_index_all_hash_algorithms() {
        let algorithms = [
            HashAlgorithm::Sha1,
            HashAlgorithm::Sha256,
            HashAlgorithm::Sha512,
            HashAlgorithm::Blake2b,
        ];

        for (i, algorithm) in algorithms.iter().enumerate() {
            let record_manager = InMemoryRecordManager::new(format!("test_{}", i));
            let doc_index = MockDocumentIndex::new();

            let docs = vec![Document::new("Test content")];

            let result = index(
                docs,
                &record_manager,
                &doc_index,
                CleanupMode::None,
                None,
                100,
                1000,
                false,
                *algorithm,
                None,
            )
            .await
            .unwrap();

            assert_eq!(result.num_added, 1, "Failed for algorithm: {:?}", algorithm);
        }
    }

    // ========================================================================
    // Custom Key Encoder Edge Cases
    // ========================================================================

    #[tokio::test]
    async fn test_index_custom_key_encoder_collision() {
        let record_manager = InMemoryRecordManager::new("test_index_custom_key_encoder_collision");
        let doc_index = MockDocumentIndex::new();

        // Encoder that always returns same ID (forces collision)
        let key_encoder: KeyEncoder = Box::new(|_doc: &Document| "constant_id".to_string());

        let docs = vec![
            Document::new("Different content 1"),
            Document::new("Different content 2"),
            Document::new("Different content 3"),
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            Some(key_encoder),
        )
        .await
        .unwrap();

        // All documents have same ID, so only first is indexed
        assert_eq!(result.num_added, 1);
        assert_eq!(result.num_skipped, 2);
        assert_eq!(doc_index.count().await, 1);
    }

    #[tokio::test]
    async fn test_index_custom_key_encoder_with_metadata() {
        let record_manager =
            InMemoryRecordManager::new("test_index_custom_key_encoder_with_metadata");
        let doc_index = MockDocumentIndex::new();

        // Encoder that uses metadata for ID
        let key_encoder: KeyEncoder = Box::new(|doc: &Document| {
            let source = doc
                .metadata
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!("id_{}", source)
        });

        let docs = vec![
            Document::new("Content 1").with_metadata("source", "file1"),
            Document::new("Content 2").with_metadata("source", "file2"),
            Document::new("Content 3").with_metadata("source", "file1"), // Same source as first
        ];

        let result = index(
            docs,
            &record_manager,
            &doc_index,
            CleanupMode::None,
            None,
            100,
            1000,
            false,
            HashAlgorithm::Sha256,
            Some(key_encoder),
        )
        .await
        .unwrap();

        // First and third have same ID (from metadata), so third is deduplicated
        assert_eq!(result.num_added, 2);
        assert_eq!(result.num_skipped, 1);
    }
}
