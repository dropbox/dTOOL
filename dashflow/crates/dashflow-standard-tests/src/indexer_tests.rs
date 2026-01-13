//! Standard tests for `DocumentIndex` implementations
//!
//! These tests ensure that `DocumentIndex` implementations conform to the expected behavior
//! defined in the Python `DashFlow` standard tests.
//!
//! All tests are marked with STANDARD TEST labels to indicate they are ports
//! from Python `DashFlow` and should not be removed without careful consideration.
//!
//! Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/indexer.py

use dashflow::core::documents::Document;
use dashflow::core::indexing::document_index::DocumentIndex;

/// Trait for `DocumentIndex` standard tests
///
/// Implementations should provide a fixture method that returns a fresh, empty
/// `DocumentIndex` for each test. All tests will be run against this index.
#[async_trait::async_trait]
pub trait DocumentIndexTests {
    /// Returns a fresh, empty `DocumentIndex` for testing
    ///
    /// The index must be completely empty before each test.
    async fn index(&self) -> Box<dyn DocumentIndex>;

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/indexer.py
    /// Python function: `test_upsert_documents_has_no_ids` (line 31)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Verify that upsert signature does not include "ids" parameter.
    /// IDs should come from the documents themselves, not as a separate parameter.
    async fn test_upsert_documents_has_no_ids(&self) {
        // This is a signature test in Python using inspect.signature
        // In Rust, this is enforced by the type system - DocumentIndex::upsert()
        // takes only documents, not a separate ids parameter.
        // This test passes by compilation.
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/indexer.py
    /// Python function: `test_upsert_no_ids` (line 36)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Upsert works with documents that do not have IDs.
    /// The index should auto-generate IDs for documents without them.
    async fn test_upsert_no_ids(&self) {
        let index = self.index().await;

        let documents = vec![
            Document::new("foo").with_metadata("id", 1),
            Document::new("bar").with_metadata("id", 2),
        ];

        let response = index
            .upsert(&documents)
            .await
            .expect("upsert should succeed");
        let mut ids = response.succeeded.clone();
        ids.sort();

        assert_eq!(response.failed.len(), 0, "No documents should fail");
        assert_eq!(ids.len(), 2, "Should have 2 IDs");

        // Retrieve and verify documents
        let mut retrieved = index.get(&ids).await.expect("get should succeed");
        retrieved.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(retrieved.len(), 2);
        // Order is not guaranteed, check both possibilities
        if retrieved[0].page_content == "bar" {
            assert_eq!(retrieved[0].page_content, "bar");
            assert_eq!(retrieved[1].page_content, "foo");
        } else {
            assert_eq!(retrieved[0].page_content, "foo");
            assert_eq!(retrieved[1].page_content, "bar");
        }
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/indexer.py
    /// Python function: `test_upsert_some_ids` (line 67)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test an upsert where some docs have ids and some don't.
    async fn test_upsert_some_ids(&self) {
        let index = self.index().await;

        let foo_uuid = "00000000-0000-0000-0000-000000000007".to_string();

        let documents = vec![
            Document::new("foo")
                .with_id(foo_uuid.clone())
                .with_metadata("id", 1),
            Document::new("bar").with_metadata("id", 1),
        ];

        let response = index
            .upsert(&documents)
            .await
            .expect("upsert should succeed");

        assert_eq!(response.failed.len(), 0, "No documents should fail");
        assert!(
            response.succeeded.contains(&foo_uuid),
            "Should contain foo_uuid"
        );

        let other_id = response
            .succeeded
            .iter()
            .find(|id| *id != &foo_uuid)
            .expect("Should have another ID")
            .clone();

        // Retrieve and verify
        let retrieved = index
            .get(&response.succeeded)
            .await
            .expect("get should succeed");
        assert_eq!(retrieved.len(), 2);

        // Check both docs are present (order not guaranteed)
        let has_foo = retrieved
            .iter()
            .any(|d| d.id == Some(foo_uuid.clone()) && d.page_content == "foo");
        let has_bar = retrieved
            .iter()
            .any(|d| d.id == Some(other_id.clone()) && d.page_content == "bar");
        assert!(has_foo && has_bar, "Both documents should be present");
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/indexer.py
    /// Python function: `test_upsert_overwrites` (line 93)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test that upsert overwrites existing content.
    async fn test_upsert_overwrites(&self) {
        let index = self.index().await;

        let foo_uuid = "00000000-0000-0000-0000-000000000007".to_string();

        let documents = vec![Document::new("foo")
            .with_id(foo_uuid.clone())
            .with_metadata("id", 1)];

        let response = index
            .upsert(&documents)
            .await
            .expect("upsert should succeed");
        assert_eq!(response.failed.len(), 0);
        assert_eq!(response.succeeded, vec![foo_uuid.clone()]);

        let retrieved = index
            .get(std::slice::from_ref(&foo_uuid))
            .await
            .expect("get should succeed");
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].page_content, "foo");

        // Now overwrite
        let documents2 = vec![Document::new("foo2")
            .with_id(foo_uuid.clone())
            .with_metadata("id", 1)];

        index
            .upsert(&documents2)
            .await
            .expect("second upsert should succeed");

        let retrieved2 = index
            .get(std::slice::from_ref(&foo_uuid))
            .await
            .expect("second get should succeed");
        assert_eq!(retrieved2.len(), 1);
        assert_eq!(retrieved2[0].page_content, "foo2");
        assert_eq!(
            retrieved2[0].metadata.get("meow"),
            Some(&serde_json::json!(2))
        );
        assert_eq!(
            retrieved2[0].metadata.get("bar"),
            None,
            "Old metadata should be gone"
        );
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/indexer.py
    /// Python function: `test_delete_missing_docs` (line 114)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Verify that we can delete docs that aren't there without errors.
    async fn test_delete_missing_docs(&self) {
        let index = self.index().await;

        let retrieved = index
            .get(&["1".to_string()])
            .await
            .expect("get should succeed");
        assert_eq!(retrieved.len(), 0, "Index should be empty");

        let delete_response = index
            .delete(Some(&["1".to_string()]))
            .await
            .expect("delete should succeed");

        if let Some(num_deleted) = delete_response.num_deleted {
            assert_eq!(num_deleted, 0, "Nothing should be deleted");
        }

        if let Some(num_failed) = delete_response.num_failed {
            assert_eq!(num_failed, 0, "Deleting missing ID is not a failure");
        }

        if let Some(ref succeeded) = delete_response.succeeded {
            assert_eq!(succeeded.len(), 0, "Nothing to delete");
        }

        if let Some(ref failed) = delete_response.failed {
            assert_eq!(failed.len(), 0, "Nothing should fail");
        }
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/indexer.py
    /// Python function: `test_delete_semantics` (line 134)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test deletion of content has appropriate semantics.
    async fn test_delete_semantics(&self) {
        let index = self.index().await;

        let foo_uuid = "00000000-0000-0000-0000-000000000007".to_string();
        let documents = vec![Document::new("foo").with_id(foo_uuid.clone())];

        let upsert_response = index
            .upsert(&documents)
            .await
            .expect("upsert should succeed");
        assert_eq!(upsert_response.succeeded, vec![foo_uuid.clone()]);
        assert_eq!(upsert_response.failed.len(), 0);

        // Delete existing and missing ID
        let delete_response = index
            .delete(Some(&["missing_id".to_string(), foo_uuid.clone()]))
            .await
            .expect("delete should succeed");

        if let Some(num_deleted) = delete_response.num_deleted {
            assert_eq!(num_deleted, 1, "Only one document should be deleted");
        }

        if let Some(num_failed) = delete_response.num_failed {
            assert_eq!(num_failed, 0, "Deleting missing ID is not a failure");
        }

        if let Some(ref succeeded) = delete_response.succeeded {
            assert_eq!(
                succeeded,
                &vec![foo_uuid.clone()],
                "foo_uuid should be deleted"
            );
        }

        if let Some(ref failed) = delete_response.failed {
            assert_eq!(failed.len(), 0, "Nothing should fail");
        }
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/indexer.py
    /// Python function: `test_bulk_delete` (line 160)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test that we can delete several documents at once.
    async fn test_bulk_delete(&self) {
        let index = self.index().await;

        let documents = vec![
            Document::new("foo")
                .with_id("1".to_string())
                .with_metadata("id", 1),
            Document::new("bar")
                .with_id("2".to_string())
                .with_metadata("id", 1),
            Document::new("baz")
                .with_id("3".to_string())
                .with_metadata("id", 1),
        ];

        index
            .upsert(&documents)
            .await
            .expect("upsert should succeed");
        index
            .delete(Some(&["1".to_string(), "2".to_string()]))
            .await
            .expect("delete should succeed");

        let retrieved = index
            .get(&["1".to_string(), "2".to_string(), "3".to_string()])
            .await
            .expect("get should succeed");

        assert_eq!(retrieved.len(), 1, "Only doc 3 should remain");
        assert_eq!(retrieved[0].page_content, "baz");
        assert_eq!(retrieved[0].id, Some("3".to_string()));
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/indexer.py
    /// Python function: `test_delete_no_args` (line 174)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test delete with no args raises error.
    async fn test_delete_no_args(&self) {
        let index = self.index().await;

        // In Rust, delete() requires a Vec<String>, so calling with empty vec
        let result = index.delete(Some(&[])).await;

        // Should return an error (in Python this raises ValueError)
        assert!(result.is_err(), "Delete with no IDs should return an error");
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/indexer.py
    /// Python function: `test_delete_missing_content` (line 179)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Deleting missing content should not raise an exception.
    async fn test_delete_missing_content(&self) {
        let index = self.index().await;

        // These should not error
        index
            .delete(Some(&["1".to_string()]))
            .await
            .expect("delete should succeed");
        index
            .delete(Some(&["1".to_string(), "2".to_string(), "3".to_string()]))
            .await
            .expect("delete should succeed");
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/indexer.py
    /// Python function: `test_get_with_missing_ids` (line 184)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test get with missing IDs returns only found documents.
    async fn test_get_with_missing_ids(&self) {
        let index = self.index().await;

        let documents = vec![
            Document::new("foo")
                .with_id("1".to_string())
                .with_metadata("id", 1),
            Document::new("bar")
                .with_id("2".to_string())
                .with_metadata("id", 1),
        ];

        let upsert_response = index
            .upsert(&documents)
            .await
            .expect("upsert should succeed");
        assert_eq!(
            upsert_response.succeeded,
            vec!["1".to_string(), "2".to_string()]
        );
        assert_eq!(upsert_response.failed.len(), 0);

        // Get with some missing IDs
        let mut retrieved = index
            .get(&[
                "1".to_string(),
                "2".to_string(),
                "3".to_string(),
                "4".to_string(),
            ])
            .await
            .expect("get should succeed");

        retrieved.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(retrieved.len(), 2, "Should only get docs 1 and 2");
        assert_eq!(retrieved[0].page_content, "foo");
        assert_eq!(retrieved[0].id, Some("1".to_string()));
        assert_eq!(retrieved[1].page_content, "bar");
        assert_eq!(retrieved[1].id, Some("2".to_string()));
    }

    /// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
    /// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/indexer.py
    /// Python function: `test_get_missing` (line 202)
    /// Port date: 2025-10-30
    /// DO NOT REMOVE - This ensures upstream compatibility
    ///
    /// Test get by IDs with all missing IDs returns empty.
    async fn test_get_missing(&self) {
        let index = self.index().await;

        let retrieved = index
            .get(&["1".to_string(), "2".to_string(), "3".to_string()])
            .await
            .expect("get should succeed");

        assert_eq!(
            retrieved.len(),
            0,
            "Should return empty list for missing IDs"
        );
    }
}
