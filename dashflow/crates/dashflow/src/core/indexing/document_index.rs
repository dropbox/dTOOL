// Allow clippy warnings for document index
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Document index abstraction for vector stores with ID-based operations
//!
//! The `DocumentIndex` trait extends retrievers with upsert/delete/get operations
//! that use document IDs. This enables intelligent indexing with change detection.

use crate::core::documents::Document;
use crate::core::retrievers::Retriever;
use async_trait::async_trait;

/// Response from an upsert operation
///
/// Tracks which document IDs succeeded and which failed during indexing.
#[derive(Debug, Clone, Default)]
pub struct UpsertResponse {
    /// Document IDs that were successfully added or updated
    pub succeeded: Vec<String>,
    /// Document IDs that failed to be added or updated
    pub failed: Vec<String>,
}

impl UpsertResponse {
    /// Create a new upsert response with all IDs marked as succeeded
    #[must_use]
    pub fn all_succeeded(ids: Vec<String>) -> Self {
        Self {
            succeeded: ids,
            failed: Vec::new(),
        }
    }

    /// Create a new upsert response with all IDs marked as failed
    #[must_use]
    pub fn all_failed(ids: Vec<String>) -> Self {
        Self {
            succeeded: Vec::new(),
            failed: ids,
        }
    }

    /// Check if all operations succeeded
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.failed.is_empty()
    }

    /// Get total number of operations attempted
    #[must_use]
    pub fn total(&self) -> usize {
        self.succeeded.len() + self.failed.len()
    }
}

/// Response from a delete operation
///
/// Provides details about which documents were deleted and which failed.
#[derive(Debug, Clone, Default)]
pub struct DeleteResponse {
    /// Number of documents actually deleted (not including non-existent IDs)
    pub num_deleted: Option<usize>,
    /// Document IDs that were successfully deleted
    pub succeeded: Option<Vec<String>>,
    /// Document IDs that failed to be deleted
    pub failed: Option<Vec<String>>,
    /// Number of delete operations that failed
    pub num_failed: Option<usize>,
}

impl DeleteResponse {
    /// Create a simple response with just the deletion count
    #[must_use]
    pub fn with_count(count: usize) -> Self {
        Self {
            num_deleted: Some(count),
            succeeded: None,
            failed: None,
            num_failed: None,
        }
    }

    /// Create a detailed response with all fields
    #[must_use]
    pub fn detailed(succeeded: Vec<String>, failed: Vec<String>) -> Self {
        Self {
            num_deleted: Some(succeeded.len()),
            succeeded: Some(succeeded.clone()),
            failed: Some(failed.clone()),
            num_failed: Some(failed.len()),
        }
    }

    /// Check if the operation had any failures
    #[must_use]
    pub fn has_failures(&self) -> bool {
        self.num_failed.is_some_and(|n| n > 0)
            || self.failed.as_ref().is_some_and(|f| !f.is_empty())
    }
}

/// A document retriever that supports indexing operations
///
/// `DocumentIndex` extends the Retriever trait with ID-based CRUD operations:
/// - **upsert**: Add or update documents by ID
/// - **delete**: Remove documents by ID
/// - **get**: Retrieve documents by ID
///
/// This interface is designed to be implementation-agnostic and works with
/// any storage backend (vector databases, search engines, key-value stores).
///
/// # Design Philosophy
///
/// - **ID-Centric**: All operations use document IDs for precise control
/// - **Upsert Semantics**: Insert if new, update if exists (no separate APIs)
/// - **Batch Operations**: Efficient bulk operations for large datasets
/// - **Graceful Degradation**: Missing documents don't cause errors
///
/// # Usage with Indexing API
///
/// The `index()` function uses `DocumentIndex` to:
/// 1. Check which documents already exist (via `RecordManager`)
/// 2. Upsert new or changed documents
/// 3. Delete outdated documents based on cleanup mode
///
/// # Implementation Notes
///
/// Vector stores can implement this trait to enable intelligent indexing.
/// The trait is already implemented for `VectorStore` types that support
/// ID-based operations.
#[async_trait]
pub trait DocumentIndex: Retriever {
    /// Add or update documents in the index
    ///
    /// Uses document IDs to determine whether to insert (new) or update (existing).
    /// Document IDs should be set in the `id` field before calling this method.
    ///
    /// # Arguments
    ///
    /// * `items` - Documents to upsert (must have IDs set)
    ///
    /// # Returns
    ///
    /// Response indicating which IDs succeeded and which failed
    ///
    /// # Errors
    ///
    /// Returns error if the operation cannot be completed. Individual document
    /// failures should be reported in the response, not as errors.
    async fn upsert(
        &self,
        items: &[Document],
    ) -> Result<UpsertResponse, Box<dyn std::error::Error + Send + Sync>>;

    /// Delete documents by ID
    ///
    /// Removes the specified documents from the index. Deleting a non-existent
    /// ID is **not** considered a failure.
    ///
    /// # Arguments
    ///
    /// * `ids` - Document IDs to delete (None means delete all, if supported)
    ///
    /// # Returns
    ///
    /// Response with deletion statistics
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - IDs is None/empty and implementation requires explicit IDs
    /// - The delete operation fails at the storage level
    async fn delete(
        &self,
        ids: Option<&[String]>,
    ) -> Result<DeleteResponse, Box<dyn std::error::Error + Send + Sync>>;

    /// Retrieve documents by ID
    ///
    /// Fetches the specified documents from the index. This is a direct lookup
    /// by ID, not a semantic search.
    ///
    /// # Arguments
    ///
    /// * `ids` - Document IDs to retrieve
    ///
    /// # Returns
    ///
    /// List of found documents (may be fewer than requested)
    ///
    /// # Important Behaviors
    ///
    /// - **No Exceptions**: Missing IDs are silently skipped, not errors
    /// - **No Order Guarantee**: Returned docs may not match input order
    /// - **Deduplication**: Duplicate input IDs may return single document
    ///
    /// # Errors
    ///
    /// Returns error only if the operation fails at the storage level,
    /// not for missing documents.
    async fn get(
        &self,
        ids: &[String],
    ) -> Result<Vec<Document>, Box<dyn std::error::Error + Send + Sync>>;
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_upsert_response_all_succeeded() {
        let response = UpsertResponse::all_succeeded(vec!["id1".to_string(), "id2".to_string()]);
        assert!(response.is_success());
        assert_eq!(response.total(), 2);
        assert_eq!(response.succeeded.len(), 2);
        assert_eq!(response.failed.len(), 0);
        assert_eq!(response.succeeded[0], "id1");
        assert_eq!(response.succeeded[1], "id2");
    }

    #[test]
    fn test_upsert_response_all_failed() {
        let response = UpsertResponse::all_failed(vec!["id3".to_string()]);
        assert!(!response.is_success());
        assert_eq!(response.total(), 1);
        assert_eq!(response.failed.len(), 1);
        assert_eq!(response.succeeded.len(), 0);
        assert_eq!(response.failed[0], "id3");
    }

    #[test]
    fn test_upsert_response_mixed() {
        let response = UpsertResponse {
            succeeded: vec!["id1".to_string(), "id2".to_string()],
            failed: vec!["id3".to_string()],
        };
        assert!(!response.is_success());
        assert_eq!(response.total(), 3);
        assert_eq!(response.succeeded.len(), 2);
        assert_eq!(response.failed.len(), 1);
    }

    #[test]
    fn test_upsert_response_empty() {
        let response = UpsertResponse::default();
        assert!(response.is_success());
        assert_eq!(response.total(), 0);
        assert_eq!(response.succeeded.len(), 0);
        assert_eq!(response.failed.len(), 0);
    }

    #[test]
    fn test_upsert_response_large_batch() {
        let succeeded_ids: Vec<String> = (0..1000).map(|i| format!("id{}", i)).collect();
        let response = UpsertResponse::all_succeeded(succeeded_ids.clone());
        assert!(response.is_success());
        assert_eq!(response.total(), 1000);
        assert_eq!(response.succeeded.len(), 1000);
    }

    #[test]
    fn test_delete_response_with_count() {
        let response = DeleteResponse::with_count(5);
        assert_eq!(response.num_deleted, Some(5));
        assert!(!response.has_failures());
        assert!(response.succeeded.is_none());
        assert!(response.failed.is_none());
        assert!(response.num_failed.is_none());
    }

    #[test]
    fn test_delete_response_with_count_zero() {
        let response = DeleteResponse::with_count(0);
        assert_eq!(response.num_deleted, Some(0));
        assert!(!response.has_failures());
    }

    #[test]
    fn test_delete_response_detailed_success() {
        let response = DeleteResponse::detailed(vec!["id1".to_string()], vec![]);
        assert_eq!(response.num_deleted, Some(1));
        assert_eq!(response.num_failed, Some(0));
        assert!(!response.has_failures());
        assert_eq!(response.succeeded.as_ref().unwrap().len(), 1);
        assert_eq!(response.failed.as_ref().unwrap().len(), 0);
    }

    #[test]
    fn test_delete_response_detailed_with_failures() {
        let response = DeleteResponse::detailed(
            vec!["id1".to_string()],
            vec!["id2".to_string(), "id3".to_string()],
        );
        assert_eq!(response.num_deleted, Some(1));
        assert_eq!(response.num_failed, Some(2));
        assert!(response.has_failures());
        assert_eq!(response.succeeded.as_ref().unwrap().len(), 1);
        assert_eq!(response.failed.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_delete_response_detailed_all_failed() {
        let response = DeleteResponse::detailed(vec![], vec!["id1".to_string(), "id2".to_string()]);
        assert_eq!(response.num_deleted, Some(0));
        assert_eq!(response.num_failed, Some(2));
        assert!(response.has_failures());
    }

    #[test]
    fn test_delete_response_default() {
        let response = DeleteResponse::default();
        assert!(!response.has_failures());
        assert!(response.num_deleted.is_none());
        assert!(response.succeeded.is_none());
        assert!(response.failed.is_none());
        assert!(response.num_failed.is_none());
    }

    #[test]
    fn test_delete_response_has_failures_with_num_failed() {
        let response = DeleteResponse {
            num_deleted: Some(5),
            succeeded: None,
            failed: None,
            num_failed: Some(3),
        };
        assert!(response.has_failures());
    }

    #[test]
    fn test_delete_response_has_failures_with_failed_vec() {
        let response = DeleteResponse {
            num_deleted: Some(2),
            succeeded: None,
            failed: Some(vec!["id1".to_string()]),
            num_failed: None,
        };
        assert!(response.has_failures());
    }

    #[test]
    fn test_delete_response_has_failures_empty_failed_vec() {
        let response = DeleteResponse {
            num_deleted: Some(2),
            succeeded: None,
            failed: Some(vec![]),
            num_failed: None,
        };
        assert!(!response.has_failures());
    }

    #[test]
    fn test_upsert_response_clone() {
        let response = UpsertResponse::all_succeeded(vec!["id1".to_string()]);
        let cloned = response.clone();
        assert_eq!(response.succeeded, cloned.succeeded);
        assert_eq!(response.failed, cloned.failed);
    }

    #[test]
    fn test_delete_response_clone() {
        let response = DeleteResponse::with_count(10);
        let cloned = response.clone();
        assert_eq!(response.num_deleted, cloned.num_deleted);
    }

    #[test]
    fn test_upsert_response_debug() {
        let response = UpsertResponse::all_succeeded(vec!["test".to_string()]);
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("UpsertResponse"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_delete_response_debug() {
        let response = DeleteResponse::with_count(5);
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("DeleteResponse"));
        assert!(debug_str.contains("5"));
    }

    #[test]
    fn test_upsert_response_total_calculation() {
        let response = UpsertResponse {
            succeeded: vec!["id1".to_string(), "id2".to_string(), "id3".to_string()],
            failed: vec!["id4".to_string(), "id5".to_string()],
        };
        assert_eq!(response.total(), 5);
        assert_eq!(
            response.succeeded.len() + response.failed.len(),
            response.total()
        );
    }

    #[test]
    fn test_delete_response_consistency() {
        let succeeded = vec!["id1".to_string(), "id2".to_string()];
        let failed = vec!["id3".to_string()];
        let response = DeleteResponse::detailed(succeeded.clone(), failed.clone());

        assert_eq!(response.num_deleted, Some(succeeded.len()));
        assert_eq!(response.num_failed, Some(failed.len()));
        assert_eq!(response.succeeded.as_ref().unwrap(), &succeeded);
        assert_eq!(response.failed.as_ref().unwrap(), &failed);
    }

    #[test]
    fn test_upsert_response_partial_success() {
        let response = UpsertResponse {
            succeeded: vec!["id1".to_string()],
            failed: vec!["id2".to_string(), "id3".to_string(), "id4".to_string()],
        };
        assert!(!response.is_success());
        assert_eq!(response.total(), 4);
        assert!(response.succeeded.len() < response.failed.len());
    }

    #[test]
    fn test_upsert_response_single_success() {
        let response = UpsertResponse::all_succeeded(vec!["single".to_string()]);
        assert!(response.is_success());
        assert_eq!(response.total(), 1);
    }

    #[test]
    fn test_delete_response_large_deletion() {
        let response = DeleteResponse::with_count(100000);
        assert_eq!(response.num_deleted, Some(100000));
        assert!(!response.has_failures());
    }

    #[test]
    fn test_upsert_response_with_empty_strings() {
        let response = UpsertResponse::all_succeeded(vec!["".to_string()]);
        assert!(response.is_success());
        assert_eq!(response.succeeded[0], "");
    }

    #[test]
    fn test_delete_response_detailed_empty_both() {
        let response = DeleteResponse::detailed(vec![], vec![]);
        assert_eq!(response.num_deleted, Some(0));
        assert_eq!(response.num_failed, Some(0));
        assert!(!response.has_failures());
    }

    #[test]
    fn test_upsert_response_special_characters() {
        let ids = vec![
            "id:with:colons".to_string(),
            "id/with/slashes".to_string(),
            "id-with-dashes".to_string(),
            "id_with_underscores".to_string(),
        ];
        let response = UpsertResponse::all_succeeded(ids.clone());
        assert_eq!(response.succeeded, ids);
        assert!(response.is_success());
    }

    #[test]
    fn test_delete_response_partial_failure_consistency() {
        let succeeded = vec!["id1".to_string(), "id2".to_string(), "id3".to_string()];
        let failed = vec!["id4".to_string()];
        let response = DeleteResponse::detailed(succeeded.clone(), failed.clone());

        assert_eq!(response.num_deleted.unwrap(), succeeded.len());
        assert_eq!(response.num_failed.unwrap(), failed.len());
        assert!(response.has_failures());
    }
}
