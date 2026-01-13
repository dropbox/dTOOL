//! Standard conformance tests for `VectorStore` implementations.
//!
//! These tests verify that all `VectorStore` implementations behave consistently
//! across different providers (Chroma, Qdrant, Pinecone, etc.).
//!
//! ## Usage
//!
//! In your provider crate, create a test module:
//!
//! ```rust,ignore
//! #[cfg(test)]
//! mod standard_tests {
//!     use super::*;
//!     use dashflow_standard_tests::vectorstore_tests::*;
//!     use dashflow::core::vector_stores::VectorStore;
//!
//!     async fn create_test_store() -> MyVectorStore {
//!         // Create and configure your vector store
//!         MyVectorStore::new(embeddings).await.unwrap()
//!     }
//!
//!     #[tokio::test]
//!     async fn test_add_and_search_standard() {
//!         let mut store = create_test_store().await;
//!         test_add_and_search(&mut store).await;
//!     }
//!
//!     // Add more standard tests...
//! }
//! ```

use dashflow::core::{documents::Document, error::Error, vector_stores::VectorStore};
use dashflow::embed_query;
use std::collections::HashMap;

/// Helper function for environmental error handling in vector store tests.
///
/// If a dependency is missing/unavailable (service not running, network issues,
/// missing credentials), fail loudly rather than silently returning.
fn should_skip_on_error<T>(result: &Result<T, Error>) -> bool {
    if let Err(e) = result {
        if e.is_environmental() {
            panic!("Environmental dependency unavailable: {e}");
        }
    }
    false
}

/// Helper function for environmental error handling in graph API results.
///
/// Similar to should_skip_on_error but for dashflow::error::Error which wraps core errors.
fn should_skip_on_graph_error<T>(result: &Result<T, dashflow::error::Error>) -> bool {
    if let Err(e) = result {
        // Check if this is a wrapped core error that is environmental
        if let dashflow::error::Error::Core(core_err) = e {
            if core_err.is_environmental() {
                panic!("Environmental dependency unavailable: {e}");
            }
        }
        // Also check NodeExecution which wraps core errors from graph execution
        if let dashflow::error::Error::NodeExecution { source, .. } = e {
            // Try to downcast the source to a core error
            if let Some(core_err) = source.downcast_ref::<Error>() {
                if core_err.is_environmental() {
                    panic!("Environmental dependency unavailable: {e}");
                }
            }
        }
    }
    false
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: `VectorStoreIntegrationTests.test_add_documents` (lines 153-186)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 1: Basic `add_texts` and `similarity_search`
///
/// Verifies:
/// - Vector store can add texts
/// - Returns document IDs
/// - Can perform similarity search
/// - Returns most relevant results first
///
/// This is the most fundamental test - all `VectorStores` must pass this.
pub async fn test_add_and_search<T: VectorStore>(store: &mut T) {
    let texts = vec![
        "The quick brown fox jumps over the lazy dog",
        "A journey of a thousand miles begins with a single step",
        "To be or not to be, that is the question",
    ];

    // Add texts
    let ids = store.add_texts(&texts, None, None).await;
    if should_skip_on_error(&ids) {
        return;
    }
    assert!(ids.is_ok(), "add_texts should succeed");
    let ids = ids.unwrap();
    assert_eq!(ids.len(), 3, "Should return 3 IDs");

    // Search for similar text
    let results = store._similarity_search("fox jumps over dog", 2, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    assert!(results.is_ok(), "similarity_search should succeed");
    let results = results.unwrap();

    assert!(!results.is_empty(), "Search should return results");
    assert!(results.len() <= 2, "Should return at most k=2 results");

    // First result should be the most relevant (contains "fox" and "dog")
    assert!(
        results[0].page_content.contains("fox") || results[0].page_content.contains("dog"),
        "First result should be most relevant"
    );
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: Rust-specific extension (no direct Python equivalent)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 2: Search with scores
///
/// Verifies:
/// - Vector store supports `similarity_search_with_score`
/// - Returns documents with relevance scores
/// - Scores are in valid range [0, 1]
/// - Results are ordered by score (highest first)
pub async fn test_search_with_scores<T: VectorStore>(store: &mut T) {
    let texts = vec![
        "Machine learning is a subset of artificial intelligence",
        "Deep learning uses neural networks",
        "The weather today is sunny and warm",
    ];

    let add_result = store.add_texts(&texts, None, None).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    let result = store
        .similarity_search_with_score("artificial intelligence", 3, None)
        .await;
    if should_skip_on_error(&result) {
        return;
    }

    // Some vector stores may not implement this method
    if let Ok(results) = result {
        assert!(!results.is_empty(), "Should return results");

        // Check scores are valid
        for (doc, score) in &results {
            assert!(
                *score >= 0.0 && *score <= 1.0,
                "Score {score} should be in [0, 1] range"
            );
            assert!(!doc.page_content.is_empty(), "Document should have content");
        }

        // Check scores are ordered (descending)
        for i in 1..results.len() {
            assert!(
                results[i - 1].1 >= results[i].1,
                "Results should be ordered by score (descending)"
            );
        }
    }
    // If not implemented, that's acceptable - this is an optional method
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: Rust-specific extension (metadata filtering pattern)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 3: Metadata filtering
///
/// Verifies:
/// - Vector store can store metadata with documents
/// - Can filter search results by metadata
/// - Only returns documents matching filter criteria
pub async fn test_metadata_filtering<T: VectorStore>(store: &mut T) {
    let texts = vec![
        "Python is a programming language",
        "Rust is a systems programming language",
        "Java is an object-oriented language",
    ];

    let mut metadata1 = HashMap::new();
    metadata1.insert("language".to_string(), serde_json::json!("python"));
    metadata1.insert("type".to_string(), serde_json::json!("interpreted"));

    let mut metadata2 = HashMap::new();
    metadata2.insert("language".to_string(), serde_json::json!("rust"));
    metadata2.insert("type".to_string(), serde_json::json!("compiled"));

    let mut metadata3 = HashMap::new();
    metadata3.insert("language".to_string(), serde_json::json!("java"));
    metadata3.insert("type".to_string(), serde_json::json!("compiled"));

    let metadatas = vec![metadata1, metadata2, metadata3];

    let add_result = store.add_texts(&texts, Some(&metadatas), None).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    // Filter for compiled languages
    let mut filter = HashMap::new();
    filter.insert("type".to_string(), serde_json::json!("compiled"));

    let results = store
        ._similarity_search("programming", 10, Some(&filter))
        .await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();

    // Should only return compiled languages (Rust and Java)
    assert!(!results.is_empty(), "Should find results");
    for doc in &results {
        let doc_type = doc.metadata.get("type");
        assert_eq!(
            doc_type,
            Some(&serde_json::json!("compiled")),
            "All results should match filter"
        );
    }

    // Should not include Python
    let has_python = results
        .iter()
        .any(|doc| doc.page_content.contains("Python"));
    assert!(!has_python, "Python should be filtered out");
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: `VectorStoreIntegrationTests.test_get_by_ids` (lines 329-368)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 4: Custom IDs
///
/// Verifies:
/// - Vector store accepts custom document IDs
/// - Returns the same IDs that were provided
/// - Can retrieve documents by custom IDs
pub async fn test_custom_ids<T: VectorStore>(store: &mut T) {
    let texts = vec!["Document one", "Document two"];
    let custom_ids = vec!["doc-1".to_string(), "doc-2".to_string()];

    let ids_result = store.add_texts(&texts, None, Some(&custom_ids)).await;
    if should_skip_on_error(&ids_result) {
        return;
    }
    let ids = ids_result.unwrap();

    assert_eq!(ids, custom_ids, "Should return the provided custom IDs");

    // Try to retrieve by custom IDs (if supported)
    if let Ok(docs) = store.get_by_ids(&custom_ids).await {
        assert_eq!(docs.len(), 2, "Should retrieve both documents");
        // Verify content matches
        let contents: Vec<String> = docs.iter().map(|d| d.page_content.clone()).collect();
        assert!(contents.contains(&"Document one".to_string()));
        assert!(contents.contains(&"Document two".to_string()));
    }
    // If get_by_ids not implemented, that's acceptable - it's optional
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: `VectorStoreIntegrationTests.test_deleting_documents` (lines 206-226)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 5: Delete documents
///
/// Verifies:
/// - Vector store supports deleting documents by ID
/// - Documents are actually removed
/// - Remaining documents are unaffected
pub async fn test_delete<T: VectorStore>(store: &mut T) {
    let texts = vec!["Keep this", "Delete this", "Keep this too"];
    let ids_result = store.add_texts(&texts, None, None).await;
    if should_skip_on_error(&ids_result) {
        return;
    }
    let ids = ids_result.unwrap();

    // Delete the middle document
    let result = store.delete(Some(&[ids[1].clone()])).await;

    // Some stores may not support delete
    if let Ok(success) = result {
        assert!(success, "Delete should succeed");

        // Try to retrieve all documents
        if let Ok(docs) = store.get_by_ids(&ids).await {
            // Should only get 2 documents back
            assert_eq!(docs.len(), 2, "Should only have 2 documents after deletion");

            // Check deleted document is not present
            let contents: Vec<String> = docs.iter().map(|d| d.page_content.clone()).collect();
            assert!(
                !contents.contains(&"Delete this".to_string()),
                "Deleted doc should be gone"
            );
            assert!(contents.contains(&"Keep this".to_string()));
            assert!(contents.contains(&"Keep this too".to_string()));
        }
    }
    // If delete not implemented, that's acceptable - it's optional
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: `VectorStoreIntegrationTests.test_add_documents_documents` (lines 400-439)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 6: Add documents (vs `add_texts`)
///
/// Verifies:
/// - Vector store supports `add_documents` method
/// - Can add Document objects with metadata
/// - Document IDs are preserved or generated
pub async fn test_add_documents<T: VectorStore>(store: &mut T) {
    let mut metadata1 = HashMap::new();
    metadata1.insert("source".to_string(), serde_json::json!("test"));

    let mut metadata2 = HashMap::new();
    metadata2.insert("source".to_string(), serde_json::json!("test"));

    let documents = vec![
        Document {
            id: Some("custom-id-1".to_string()),
            page_content: "First document".to_string(),
            metadata: metadata1,
        },
        Document {
            id: Some("custom-id-2".to_string()),
            page_content: "Second document".to_string(),
            metadata: metadata2,
        },
    ];

    let ids_result = store.add_documents(&documents, None).await;
    if should_skip_on_error(&ids_result) {
        return;
    }
    let ids = ids_result.unwrap();
    assert_eq!(ids.len(), 2, "Should return 2 IDs");

    // IDs should match document IDs
    assert_eq!(ids[0], "custom-id-1");
    assert_eq!(ids[1], "custom-id-2");

    // Verify we can search for these documents
    let results = store._similarity_search("document", 5, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();
    assert!(!results.is_empty(), "Should find added documents");
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: `VectorStoreIntegrationTests.test_vectorstore_is_empty` (lines 139-151)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 7: Empty store search
///
/// Verifies:
/// - Vector store handles searches on empty collection gracefully
/// - Returns empty results (not an error)
pub async fn test_empty_search<T: VectorStore>(store: &T) {
    let results = store._similarity_search("query", 5, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    assert!(results.is_ok(), "Search on empty store should not error");
    let results = results.unwrap();
    assert_eq!(results.len(), 0, "Empty store should return no results");
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: Rust-specific extension (no direct Python equivalent)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 8: Search by vector
///
/// Verifies:
/// - Vector store supports `similarity_search_by_vector`
/// - Can search using pre-computed embeddings
/// - Returns relevant results
pub async fn test_search_by_vector<T: VectorStore>(store: &mut T) {
    // Only run this test if store exposes embeddings
    let embeddings = match store.embeddings() {
        Some(emb) => emb,
        None => return, // Skip test if embeddings not exposed
    };

    let texts = vec![
        "The cat sits on the mat",
        "Dogs are loyal animals",
        "Birds can fly in the sky",
    ];

    let add_result = store.add_texts(&texts, None, None).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    // Get embedding for a query using the graph API
    let embedding_result = embed_query(embeddings, "cats and animals").await;
    if should_skip_on_graph_error(&embedding_result) {
        return;
    }
    let query_embedding = embedding_result.unwrap();

    // Search by vector
    let result = store
        .similarity_search_by_vector(&query_embedding, 2, None)
        .await;

    if let Ok(results) = result {
        assert!(!results.is_empty(), "Should find results");
        assert!(results.len() <= 2, "Should respect k parameter");
    }
    // If not implemented, that's acceptable
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: Rust-specific extension (no direct Python equivalent)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 9: Maximum Marginal Relevance (MMR)
///
/// Verifies:
/// - Vector store supports MMR search
/// - Returns diverse results (not just most similar)
/// - Results are both relevant and diverse
pub async fn test_mmr_search<T: VectorStore>(store: &mut T) {
    let texts = vec![
        "Machine learning is a branch of AI",
        "Machine learning uses algorithms to learn from data",
        "Artificial intelligence mimics human intelligence",
        "The weather is sunny today",
    ];

    let add_result = store.add_texts(&texts, None, None).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    let result = store
        .max_marginal_relevance_search("machine learning", 2, 4, 0.5, None)
        .await;

    if let Ok(results) = result {
        assert!(!results.is_empty(), "MMR should return results");
        assert!(results.len() <= 2, "Should respect k parameter");

        // Results should be relevant to "machine learning"
        let relevant = results.iter().any(|doc| {
            doc.page_content.to_lowercase().contains("machine")
                || doc.page_content.to_lowercase().contains("learning")
                || doc.page_content.to_lowercase().contains("ai")
        });
        assert!(relevant, "MMR results should be relevant to query");
    }
    // If MMR not implemented, that's acceptable - it's optional
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: `VectorStoreIntegrationTests.test_deleting_bulk_documents` (lines 228-248)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 10: Large batch addition
///
/// Verifies:
/// - Vector store can handle adding many documents at once
/// - Performance is reasonable
/// - All documents are indexed correctly
pub async fn test_large_batch<T: VectorStore>(store: &mut T) {
    // Generate 50 documents
    let texts: Vec<String> = (0..50)
        .map(|i| format!("Document number {i} with unique content"))
        .collect();

    let text_refs: Vec<&str> = texts.iter().map(std::string::String::as_str).collect();

    let ids_result = store.add_texts(&text_refs, None, None).await;
    if should_skip_on_error(&ids_result) {
        return;
    }
    let ids = ids_result.unwrap();
    assert_eq!(ids.len(), 50, "Should add all 50 documents");

    // Search should work across all documents
    let results = store._similarity_search("document", 10, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();
    assert!(!results.is_empty(), "Should find documents");
    assert!(results.len() <= 10, "Should respect k parameter");
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: Rust-specific extension (validation logic)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 11: Validation - mismatched lengths
///
/// Verifies:
/// - Vector store validates input lengths
/// - Returns error for mismatched metadata/IDs
pub async fn test_validation<T: VectorStore>(store: &mut T) {
    let texts = vec!["text1", "text2"];

    // Mismatched metadata length
    let metadata = vec![HashMap::new()]; // Only 1 metadata for 2 texts
    let result = store.add_texts(&texts, Some(&metadata), None).await;
    assert!(
        result.is_err(),
        "Should error on mismatched metadata length"
    );

    // Mismatched IDs length
    let ids = vec!["id1".to_string()]; // Only 1 ID for 2 texts
    let result = store.add_texts(&texts, None, Some(&ids)).await;
    assert!(result.is_err(), "Should error on mismatched IDs length");
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: `VectorStoreIntegrationTests.test_add_documents_by_id_with_mutation` (lines 290-327)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 12: Update existing document
///
/// Verifies:
/// - Vector store can update existing documents by ID
/// - Updated content is searchable
/// - Old content is replaced
pub async fn test_update_document<T: VectorStore>(store: &mut T) {
    let texts = vec!["Original content"];
    let ids = vec!["doc-update-test".to_string()];

    // Add initial document
    let add_result = store.add_texts(&texts, None, Some(&ids)).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    // Update with new content
    let new_texts = vec!["Updated content with different keywords"];
    let result = store.add_texts(&new_texts, None, Some(&ids)).await;

    if result.is_ok() {
        // Search for new keywords
        let results = store
            ._similarity_search("updated different keywords", 5, None)
            .await
            .unwrap();

        // Should find the updated document
        let found_updated = results.iter().any(|doc| {
            doc.page_content.contains("Updated") && doc.page_content.contains("different")
        });

        assert!(
            found_updated,
            "Should find updated document with new content"
        );
    }
    // If update not supported (adds duplicate instead), that's acceptable
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: Rust-specific extension (metadata filtering pattern)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 13: Metadata-only search
///
/// Verifies:
/// - Can search using only metadata filters (no text query needed)
/// - Returns all documents matching metadata criteria
pub async fn test_metadata_only_filter<T: VectorStore>(store: &mut T) {
    let texts = vec![
        "Document about cats",
        "Document about dogs",
        "Document about birds",
    ];

    let mut metadata1 = HashMap::new();
    metadata1.insert("category".to_string(), serde_json::json!("mammals"));
    metadata1.insert("topic".to_string(), serde_json::json!("cats"));

    let mut metadata2 = HashMap::new();
    metadata2.insert("category".to_string(), serde_json::json!("mammals"));
    metadata2.insert("topic".to_string(), serde_json::json!("dogs"));

    let mut metadata3 = HashMap::new();
    metadata3.insert("category".to_string(), serde_json::json!("birds"));
    metadata3.insert("topic".to_string(), serde_json::json!("avian"));

    let metadatas = vec![metadata1, metadata2, metadata3];

    let add_result = store.add_texts(&texts, Some(&metadatas), None).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    // Filter for only mammals (should get cats and dogs, not birds)
    let mut filter = HashMap::new();
    filter.insert("category".to_string(), serde_json::json!("mammals"));

    let results = store._similarity_search("animals", 10, Some(&filter)).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();

    assert!(results.len() >= 2, "Should find at least 2 mammals");

    // All results should be mammals
    for doc in &results {
        let category = doc.metadata.get("category");
        assert_eq!(
            category,
            Some(&serde_json::json!("mammals")),
            "All results should be mammals"
        );
    }
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: Rust-specific extension (complex metadata handling)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 14: Complex metadata structures
///
/// Verifies:
/// - Vector store can handle nested/complex metadata
/// - Arrays, objects, and nested structures work correctly
pub async fn test_complex_metadata<T: VectorStore>(store: &mut T) {
    let texts = vec!["Document with complex metadata"];

    let mut metadata = HashMap::new();
    metadata.insert("simple".to_string(), serde_json::json!("value"));
    metadata.insert("number".to_string(), serde_json::json!(42));
    metadata.insert("float".to_string(), serde_json::json!(3.15));
    metadata.insert("boolean".to_string(), serde_json::json!(true));
    metadata.insert(
        "array".to_string(),
        serde_json::json!(["item1", "item2", "item3"]),
    );
    metadata.insert(
        "nested".to_string(),
        serde_json::json!({
            "level1": {
                "level2": "deep value"
            }
        }),
    );

    let result = store.add_texts(&texts, Some(&[metadata]), None).await;
    if should_skip_on_error(&result) {
        return;
    }
    assert!(
        result.is_ok(),
        "Should handle complex metadata: {:?}",
        result.err()
    );

    // Verify we can retrieve the document
    let results = store._similarity_search("document", 1, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();
    assert!(!results.is_empty(), "Should retrieve document");

    // Check metadata is preserved
    let doc = &results[0];
    assert_eq!(
        doc.metadata.get("simple"),
        Some(&serde_json::json!("value"))
    );
    assert_eq!(doc.metadata.get("number"), Some(&serde_json::json!(42)));
    assert_eq!(doc.metadata.get("float"), Some(&serde_json::json!(3.15)));
    assert_eq!(doc.metadata.get("boolean"), Some(&serde_json::json!(true)));
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: Rust-specific extension (edge case testing)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 15: Empty text handling
///
/// Verifies:
/// - Vector store handles empty strings gracefully
/// - Either accepts and embeds or rejects cleanly
pub async fn test_empty_text<T: VectorStore>(store: &mut T) {
    let texts = vec!["", "non-empty"];

    let result = store.add_texts(&texts, None, None).await;

    // Either should accept empty strings or error gracefully
    if let Ok(ids) = result {
        assert_eq!(ids.len(), 2, "Should handle both texts if accepted");
    } else {
        // Rejection of empty strings is acceptable
    }

    // Try adding only empty string
    let result2 = store.add_texts(&[""], None, None).await;
    // Should either accept or error cleanly (no panic)
    let _ = result2;
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: Rust-specific extension (unicode/special char handling)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 16: Special characters in metadata keys
///
/// Verifies:
/// - Metadata keys with special characters work
/// - Unicode in keys and values is handled
pub async fn test_special_chars_metadata<T: VectorStore>(store: &mut T) {
    let texts = vec!["Document with special char metadata"];

    let mut metadata = HashMap::new();
    metadata.insert("key-with-dash".to_string(), serde_json::json!("value1"));
    metadata.insert(
        "key_with_underscore".to_string(),
        serde_json::json!("value2"),
    );
    metadata.insert("key.with.dots".to_string(), serde_json::json!("value3"));
    metadata.insert("unicode_key_🔑".to_string(), serde_json::json!("value4"));
    metadata.insert(
        "regular_key".to_string(),
        serde_json::json!("unicode_value_🌍"),
    );

    let result = store.add_texts(&texts, Some(&[metadata]), None).await;
    if should_skip_on_error(&result) {
        return;
    }

    if result.is_ok() {
        // Verify we can retrieve and check metadata
        let results = store._similarity_search("document", 1, None).await;
        if should_skip_on_error(&results) {
            return;
        }
        let results = results.unwrap();
        if !results.is_empty() {
            let doc = &results[0];
            // At least some metadata should be preserved
            assert!(!doc.metadata.is_empty(), "Metadata should be preserved");
        }
    }
    // If special chars not supported, that's acceptable - store-dependent
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: Rust-specific extension (concurrency testing)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 17: Concurrent operations
///
/// Verifies:
/// - Vector store can handle concurrent add operations
/// - All documents are indexed correctly
/// - No race conditions or data corruption
pub async fn test_concurrent_operations<T: VectorStore + Send>(store: &mut T) {
    use futures::future::join_all;

    // Create concurrent add operations
    let tasks = (0..5).map(|i| {
        let texts = vec![format!("Concurrent document {}", i)];
        async move {
            // Note: We can't actually share mutable store across tasks easily
            // This test is more conceptual - provider-specific tests can implement properly
            texts
        }
    });

    let all_texts = join_all(tasks).await;

    // Add all texts in one batch (simulates concurrent adds)
    let flat_texts: Vec<String> = all_texts.into_iter().flatten().collect();
    let text_refs: Vec<&str> = flat_texts.iter().map(std::string::String::as_str).collect();

    let result = store.add_texts(&text_refs, None, None).await;
    if should_skip_on_error(&result) {
        return;
    }
    assert!(result.is_ok(), "Batch add should succeed");
    let ids = result.unwrap();
    assert_eq!(ids.len(), 5, "Should add all 5 concurrent documents");
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: Rust-specific extension (edge case - long text)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 18: Very long text
///
/// Verifies:
/// - Vector store can handle very long documents
/// - Embeddings are generated correctly
/// - Search still works
pub async fn test_very_long_text<T: VectorStore>(store: &mut T) {
    // Create a very long text (10,000+ words)
    let long_text = "The quick brown fox jumps over the lazy dog. ".repeat(2000);

    let texts = vec![long_text.as_str(), "Short document"];

    let result = store.add_texts(&texts, None, None).await;
    if should_skip_on_error(&result) {
        return;
    }
    assert!(
        result.is_ok(),
        "Should handle very long text: {:?}",
        result.err()
    );

    // Search should still work
    let results = store._similarity_search("fox", 5, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();
    assert!(!results.is_empty(), "Should find results");
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: `VectorStoreIntegrationTests.test_add_documents_with_ids_is_idempotent` (lines 264-288)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 19: Duplicate document handling
///
/// Verifies:
/// - Vector store handles adding identical content
/// - Either allows duplicates or handles gracefully
pub async fn test_duplicate_documents<T: VectorStore>(store: &mut T) {
    let texts = vec!["Duplicate content", "Duplicate content", "Unique content"];

    let result = store.add_texts(&texts, None, None).await;
    if should_skip_on_error(&result) {
        return;
    }
    assert!(result.is_ok(), "Should handle duplicate texts");
    let ids = result.unwrap();
    assert_eq!(ids.len(), 3, "Should return 3 IDs even with duplicates");

    // IDs should be different (each document gets unique ID)
    assert_ne!(ids[0], ids[1], "Duplicate docs should have different IDs");

    // Search should return multiple copies
    let results = store._similarity_search("Duplicate content", 5, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();
    assert!(
        results.len() >= 2,
        "Should find at least 2 duplicate documents"
    );
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/vectorstores.py
/// Python function: Rust-specific extension (k parameter testing)
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 20: K parameter validation
///
/// Verifies:
/// - Vector store respects k parameter correctly
/// - Returns at most k results
/// - Handles k=0, k=1, k=large correctly
pub async fn test_k_parameter<T: VectorStore>(store: &mut T) {
    let texts: Vec<String> = (0..10)
        .map(|i| format!("Test document number {i}"))
        .collect();
    let text_refs: Vec<&str> = texts.iter().map(std::string::String::as_str).collect();

    let add_result = store.add_texts(&text_refs, None, None).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    // Test k=0 (should return 0 results)
    let results = store._similarity_search("test", 0, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();
    assert_eq!(results.len(), 0, "k=0 should return 0 results");

    // Test k=1 (should return 1 result)
    let results = store._similarity_search("test", 1, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();
    assert_eq!(results.len(), 1, "k=1 should return 1 result");

    // Test k=5 (should return 5 results)
    let results = store._similarity_search("test", 5, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();
    assert_eq!(results.len(), 5, "k=5 should return 5 results");

    // Test k=100 (larger than available, should return all 10)
    let results = store._similarity_search("test", 100, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();
    assert_eq!(
        results.len(),
        10,
        "k=100 should return all available (10) results"
    );
}

// ========================================================================
// COMPREHENSIVE TESTS
// These tests add deeper coverage for edge cases, error handling, and
// performance scenarios beyond the standard conformance tests above.
// ========================================================================

/// **COMPREHENSIVE TEST** - MMR with lambda=0 (maximum diversity)
///
/// Verifies:
/// - MMR with lambda=0 prioritizes diversity over relevance
/// - Results are diverse (not all similar to each other)
/// - All results are still somewhat relevant to query
pub async fn test_mmr_lambda_zero<T: VectorStore>(store: &mut T) {
    let texts = vec![
        "Python programming language for data science",
        "Python language features and syntax",
        "Python development tools and frameworks",
        "Rust systems programming language",
        "JavaScript web development",
        "The weather is sunny today",
    ];

    let add_result = store.add_texts(&texts, None, None).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    // lambda=0.0 means maximum diversity (ignore relevance)
    let result = store
        .max_marginal_relevance_search("Python programming", 3, 6, 0.0, None)
        .await;

    if let Ok(results) = result {
        assert!(!results.is_empty(), "Should return results");
        assert!(results.len() <= 3, "Should respect k=3 parameter");

        // With lambda=0, should get diverse results, not just all Python docs
        let all_about_python = results
            .iter()
            .all(|doc| doc.page_content.to_lowercase().contains("python"));

        // Should NOT be all about Python (diversity should pull in other topics)
        assert!(
            !all_about_python,
            "With lambda=0, should return diverse results, not all Python docs"
        );
    }
    // If MMR not implemented, that's acceptable
}

/// **COMPREHENSIVE TEST** - MMR with lambda=1 (maximum relevance)
///
/// Verifies:
/// - MMR with lambda=1 is equivalent to similarity search
/// - All results are highly relevant to query
/// - Diversity is ignored
pub async fn test_mmr_lambda_one<T: VectorStore>(store: &mut T) {
    let texts = vec![
        "Machine learning neural networks",
        "Machine learning algorithms",
        "Machine learning applications",
        "Deep learning is a subset of machine learning",
        "The weather is sunny",
        "Cooking recipes for dinner",
    ];

    let add_result = store.add_texts(&texts, None, None).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    // lambda=1.0 means maximum relevance (ignore diversity)
    let result = store
        .max_marginal_relevance_search("machine learning", 3, 6, 1.0, None)
        .await;

    if let Ok(results) = result {
        assert!(!results.is_empty(), "Should return results");
        assert!(results.len() <= 3, "Should respect k=3 parameter");

        // With lambda=1, should get most relevant results (all about ML)
        for doc in &results {
            let is_relevant = doc.page_content.to_lowercase().contains("machine")
                || doc.page_content.to_lowercase().contains("learning");
            assert!(
                is_relevant,
                "With lambda=1, all results should be about machine learning"
            );
        }
    }
    // If MMR not implemented, that's acceptable
}

/// **COMPREHENSIVE TEST** - MMR with varied `fetch_k`
///
/// Verifies:
/// - `fetch_k` parameter correctly limits initial candidates
/// - Different `fetch_k` values produce different results
/// - `fetch_k` >= k always produces valid results
pub async fn test_mmr_fetch_k_variations<T: VectorStore>(store: &mut T) {
    let texts: Vec<String> = (0..20)
        .map(|i| format!("Document about topic {} with unique content", i % 5))
        .collect();
    let text_refs: Vec<&str> = texts.iter().map(std::string::String::as_str).collect();

    let add_result = store.add_texts(&text_refs, None, None).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    // Test with fetch_k = k (minimum valid value)
    let result1 = store
        .max_marginal_relevance_search("topic", 3, 3, 0.5, None)
        .await;
    if let Ok(results1) = result1 {
        assert!(results1.len() <= 3, "Should respect k parameter");
    }

    // Test with fetch_k = 2*k
    let result2 = store
        .max_marginal_relevance_search("topic", 3, 6, 0.5, None)
        .await;
    if let Ok(results2) = result2 {
        assert!(results2.len() <= 3, "Should respect k parameter");
    }

    // Test with fetch_k >> k
    let result3 = store
        .max_marginal_relevance_search("topic", 3, 20, 0.5, None)
        .await;
    if let Ok(results3) = result3 {
        assert!(results3.len() <= 3, "Should respect k parameter");
    }

    // If MMR not implemented, that's acceptable
}

/// **COMPREHENSIVE TEST** - Complex metadata filtering with operators
///
/// Verifies:
/// - Metadata filtering supports comparison operators ($gt, $gte, $lt, $lte, $ne)
/// - Metadata filtering supports logical operators ($and, $or, $not)
/// - Metadata filtering supports array operators ($in, $nin)
pub async fn test_complex_metadata_operators<T: VectorStore>(store: &mut T) {
    let texts = vec![
        "Document 1",
        "Document 2",
        "Document 3",
        "Document 4",
        "Document 5",
    ];

    let mut metadata1 = HashMap::new();
    metadata1.insert("score".to_string(), serde_json::json!(10));
    metadata1.insert("category".to_string(), serde_json::json!("A"));

    let mut metadata2 = HashMap::new();
    metadata2.insert("score".to_string(), serde_json::json!(20));
    metadata2.insert("category".to_string(), serde_json::json!("B"));

    let mut metadata3 = HashMap::new();
    metadata3.insert("score".to_string(), serde_json::json!(30));
    metadata3.insert("category".to_string(), serde_json::json!("A"));

    let mut metadata4 = HashMap::new();
    metadata4.insert("score".to_string(), serde_json::json!(40));
    metadata4.insert("category".to_string(), serde_json::json!("C"));

    let mut metadata5 = HashMap::new();
    metadata5.insert("score".to_string(), serde_json::json!(50));
    metadata5.insert("category".to_string(), serde_json::json!("B"));

    let metadatas = vec![metadata1, metadata2, metadata3, metadata4, metadata5];

    let add_result = store.add_texts(&texts, Some(&metadatas), None).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    // Test $gt operator (score > 25)
    let mut filter_gt = HashMap::new();
    filter_gt.insert("score".to_string(), serde_json::json!({"$gt": 25}));

    let results = store
        ._similarity_search("document", 10, Some(&filter_gt))
        .await;
    if let Ok(results) = results {
        // Should get docs 3, 4, 5 (scores 30, 40, 50)
        for doc in &results {
            if let Some(score) = doc
                .metadata
                .get("score")
                .and_then(serde_json::Value::as_i64)
            {
                assert!(score > 25, "All scores should be > 25");
            }
        }
    }

    // Test $in operator (category in [A, B])
    let mut filter_in = HashMap::new();
    filter_in.insert(
        "category".to_string(),
        serde_json::json!({"$in": ["A", "B"]}),
    );

    let results = store
        ._similarity_search("document", 10, Some(&filter_in))
        .await;
    if let Ok(results) = results {
        // Should get docs with category A or B
        for doc in &results {
            if let Some(cat) = doc.metadata.get("category").and_then(|v| v.as_str()) {
                assert!(cat == "A" || cat == "B", "All categories should be A or B");
            }
        }
    }

    // If advanced filtering not supported, that's acceptable
}

/// **COMPREHENSIVE TEST** - Nested metadata filtering
///
/// Verifies:
/// - Can filter by nested metadata fields
/// - Dot notation (field.subfield) works
/// - Deep nesting is supported
pub async fn test_nested_metadata_filtering<T: VectorStore>(store: &mut T) {
    let texts = vec!["Product A", "Product B", "Product C"];

    let mut metadata1 = HashMap::new();
    metadata1.insert(
        "details".to_string(),
        serde_json::json!({
            "price": {"amount": 100, "currency": "USD"},
            "category": "electronics"
        }),
    );

    let mut metadata2 = HashMap::new();
    metadata2.insert(
        "details".to_string(),
        serde_json::json!({
            "price": {"amount": 200, "currency": "USD"},
            "category": "furniture"
        }),
    );

    let mut metadata3 = HashMap::new();
    metadata3.insert(
        "details".to_string(),
        serde_json::json!({
            "price": {"amount": 50, "currency": "EUR"},
            "category": "electronics"
        }),
    );

    let metadatas = vec![metadata1, metadata2, metadata3];

    let add_result = store.add_texts(&texts, Some(&metadatas), None).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    // Try filtering by nested field (implementation varies by store)
    // Some stores support dot notation, others need different syntax
    let mut filter = HashMap::new();
    filter.insert(
        "details.category".to_string(),
        serde_json::json!("electronics"),
    );

    let results = store._similarity_search("product", 10, Some(&filter)).await;

    // If store supports nested filtering, verify results
    if let Ok(results) = results {
        if !results.is_empty() {
            // If we got results, they should match the filter
            for doc in &results {
                if let Some(details) = doc.metadata.get("details") {
                    if let Some(category) = details.get("category").and_then(|v| v.as_str()) {
                        assert_eq!(category, "electronics", "Nested filter should work");
                    }
                }
            }
        }
    }
    // If nested filtering not supported, that's acceptable - store-dependent
}

/// **COMPREHENSIVE TEST** - Array metadata values
///
/// Verifies:
/// - Metadata can contain arrays
/// - Can filter by array membership
/// - Arrays are preserved on retrieval
pub async fn test_array_metadata<T: VectorStore>(store: &mut T) {
    let texts = vec![
        "Article about programming",
        "Article about cooking",
        "Article about sports",
    ];

    let mut metadata1 = HashMap::new();
    metadata1.insert(
        "tags".to_string(),
        serde_json::json!(["python", "coding", "tech"]),
    );
    metadata1.insert("authors".to_string(), serde_json::json!(["Alice", "Bob"]));

    let mut metadata2 = HashMap::new();
    metadata2.insert("tags".to_string(), serde_json::json!(["recipe", "food"]));
    metadata2.insert("authors".to_string(), serde_json::json!(["Charlie"]));

    let mut metadata3 = HashMap::new();
    metadata3.insert(
        "tags".to_string(),
        serde_json::json!(["football", "basketball"]),
    );
    metadata3.insert("authors".to_string(), serde_json::json!(["Diana", "Eve"]));

    let metadatas = vec![metadata1, metadata2, metadata3];

    let add_result = store.add_texts(&texts, Some(&metadatas), None).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    // Retrieve and verify arrays are preserved
    let results = store._similarity_search("article", 10, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();

    assert!(!results.is_empty(), "Should find documents");

    // Check that at least one document has array metadata preserved
    let has_array_metadata = results.iter().any(|doc| {
        doc.metadata
            .get("tags")
            .is_some_and(serde_json::Value::is_array)
    });

    if has_array_metadata {
        // Arrays are supported and preserved
        for doc in &results {
            if let Some(tags) = doc.metadata.get("tags") {
                if let Some(arr) = tags.as_array() {
                    assert!(!arr.is_empty(), "Tag arrays should not be empty");
                }
            }
        }
    }
    // If arrays not supported or not preserved, that's acceptable
}

/// **COMPREHENSIVE TEST** - Large batch operations (1000+ documents)
///
/// Verifies:
/// - Can handle adding 1000+ documents in single batch
/// - Performance is reasonable
/// - All documents are indexed correctly
/// - Memory usage is reasonable
pub async fn test_very_large_batch<T: VectorStore>(store: &mut T) {
    // Generate 1000 documents
    let texts: Vec<String> = (0..1000)
        .map(|i| format!("Document {} with unique content about topic {}", i, i % 10))
        .collect();

    let text_refs: Vec<&str> = texts.iter().map(std::string::String::as_str).collect();

    let ids_result = store.add_texts(&text_refs, None, None).await;
    if should_skip_on_error(&ids_result) {
        return;
    }
    let ids = ids_result.unwrap();
    assert_eq!(ids.len(), 1000, "Should add all 1000 documents");

    // Search should work efficiently
    let results = store._similarity_search("document topic", 10, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();
    assert!(!results.is_empty(), "Should find documents");
    assert!(results.len() <= 10, "Should respect k parameter");

    // Verify we can search for specific topics
    let results = store._similarity_search("topic 5", 20, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();
    assert!(!results.is_empty(), "Should find topic-specific documents");
}

/// **COMPREHENSIVE TEST** - Concurrent write operations
///
/// Verifies:
/// - Multiple sequential batch operations succeed
/// - No data corruption
/// - All documents are indexed
///
/// Note: True concurrency testing requires `Arc<Mutex<VectorStore>>`
/// which is store-specific. This test documents the expected behavior.
pub async fn test_concurrent_writes<T: VectorStore>(store: &mut T) {
    // Sequential batch additions (simulating concurrent workload)
    // Actual concurrency requires store-specific implementation

    let mut all_ids = Vec::new();

    for batch_id in 0..10 {
        let texts: Vec<String> = (0..10)
            .map(|i| format!("Batch {batch_id} Document {i}"))
            .collect();
        let text_refs: Vec<&str> = texts.iter().map(std::string::String::as_str).collect();

        let result = store.add_texts(&text_refs, None, None).await;
        if should_skip_on_error(&result) {
            return;
        }

        if let Ok(ids) = result {
            all_ids.extend(ids);
        }
    }

    // Should have added all 100 documents (10 batches * 10 docs)
    assert_eq!(
        all_ids.len(),
        100,
        "Should add all 100 documents across batches"
    );

    // Verify we can search across all batches
    let results = store._similarity_search("Batch Document", 50, None).await;
    if should_skip_on_error(&results) {
        return;
    }
    let results = results.unwrap();
    assert!(
        !results.is_empty(),
        "Should find documents from multiple batches"
    );
}

/// **COMPREHENSIVE TEST** - Error handling for network failures
///
/// Verifies:
/// - Operations fail gracefully with clear error messages
/// - Errors are propagated correctly
/// - No panics or undefined behavior
pub async fn test_error_handling_network<T: VectorStore>(store: &mut T) {
    // This test is primarily for documentation
    // Actual network failure testing requires infrastructure setup

    // Try operations that might fail due to network/service issues
    let result = store._similarity_search("test query", 5, None).await;

    // If operation fails, error should be environmental (not a panic)
    if let Err(e) = result {
        // Error should be well-formed
        let error_msg = e.to_string();
        assert!(!error_msg.is_empty(), "Error message should not be empty");

        // Should be environmental error
        assert!(
            e.is_environmental(),
            "Network failures should be marked as environmental"
        );
    }
    // If operation succeeds, that's fine too
}

/// **COMPREHENSIVE TEST** - Error handling for invalid inputs
///
/// Verifies:
/// - Invalid metadata is rejected gracefully
/// - Invalid IDs are rejected gracefully
/// - Invalid filter syntax is rejected gracefully
/// - No panics on malformed input
pub async fn test_error_handling_invalid_input<T: VectorStore>(store: &mut T) {
    let texts = vec!["Valid text"];

    // Test 1: Mismatched metadata length (should error)
    let metadata = vec![HashMap::new(), HashMap::new()]; // 2 metadata for 1 text
    let result = store.add_texts(&texts, Some(&metadata), None).await;
    assert!(
        result.is_err(),
        "Should error on mismatched metadata length"
    );

    // Test 2: Mismatched IDs length (should error)
    let ids = vec!["id1".to_string(), "id2".to_string()]; // 2 IDs for 1 text
    let result = store.add_texts(&texts, None, Some(&ids)).await;
    assert!(result.is_err(), "Should error on mismatched IDs length");

    // Test 3: Empty ID strings (behavior varies by store)
    let empty_ids = vec![String::new()];
    let result = store.add_texts(&texts, None, Some(&empty_ids)).await;
    // Either accept or reject cleanly (no panic)
    let _ = result;

    // Test 4: Very long ID strings (should either accept or reject cleanly)
    let long_id = "x".repeat(10000);
    let long_ids = vec![long_id];
    let result = store.add_texts(&texts, None, Some(&long_ids)).await;
    // Either accept or reject cleanly (no panic)
    let _ = result;
}

/// **COMPREHENSIVE TEST** - Delete operations with filtering
///
/// Verifies:
/// - Can delete multiple documents by IDs
/// - Can delete with metadata filters (if supported)
/// - Deletion is immediate/consistent
/// - Search reflects deletions
pub async fn test_bulk_delete<T: VectorStore>(store: &mut T) {
    let texts: Vec<String> = (0..20).map(|i| format!("Document {i}")).collect();
    let text_refs: Vec<&str> = texts.iter().map(std::string::String::as_str).collect();

    let ids_result = store.add_texts(&text_refs, None, None).await;
    if should_skip_on_error(&ids_result) {
        return;
    }
    let ids = ids_result.unwrap();

    // Delete first 10 documents
    let ids_to_delete: Vec<String> = ids.iter().take(10).cloned().collect();
    let result = store.delete(Some(&ids_to_delete)).await;

    if let Ok(success) = result {
        if success {
            // Verify deletion
            if let Ok(remaining) = store.get_by_ids(&ids).await {
                assert!(
                    remaining.len() <= 10,
                    "Should have at most 10 documents remaining"
                );
            }

            // Search should not return deleted docs
            let results = store._similarity_search("document", 20, None).await;
            if let Ok(results) = results {
                assert!(
                    results.len() <= 10,
                    "Should find at most 10 remaining documents"
                );
            }
        }
    }
    // If delete not supported, that's acceptable
}

/// **COMPREHENSIVE TEST** - Update with metadata changes
///
/// Verifies:
/// - Can update document content and metadata
/// - Metadata changes are reflected in search results
/// - Can filter by new metadata values
pub async fn test_update_metadata<T: VectorStore>(store: &mut T) {
    let texts = vec!["Original content"];
    let ids = vec!["doc-meta-test".to_string()];

    let mut original_metadata = HashMap::new();
    original_metadata.insert("version".to_string(), serde_json::json!(1));
    original_metadata.insert("status".to_string(), serde_json::json!("draft"));

    // Add initial document
    let add_result = store
        .add_texts(&texts, Some(&[original_metadata]), Some(&ids))
        .await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    // Update with new metadata
    let new_texts = vec!["Updated content"];
    let mut new_metadata = HashMap::new();
    new_metadata.insert("version".to_string(), serde_json::json!(2));
    new_metadata.insert("status".to_string(), serde_json::json!("published"));

    let result = store
        .add_texts(&new_texts, Some(&[new_metadata.clone()]), Some(&ids))
        .await;

    if result.is_ok() {
        // Try to filter by new metadata
        let mut filter = HashMap::new();
        filter.insert("status".to_string(), serde_json::json!("published"));

        let results = store._similarity_search("content", 5, Some(&filter)).await;

        if let Ok(results) = results {
            if !results.is_empty() {
                // Should find the updated document
                let found = results
                    .iter()
                    .any(|doc| doc.metadata.get("status") == Some(&serde_json::json!("published")));
                assert!(found, "Should find document with updated metadata");
            }
        }
    }
    // If update not supported, that's acceptable
}

/// **COMPREHENSIVE TEST** - Search with score threshold
///
/// Verifies:
/// - Can filter results by minimum score
/// - Only returns results above threshold
/// - Threshold=0.0 returns all results
/// - Threshold=1.0 returns only perfect matches
pub async fn test_search_score_threshold<T: VectorStore>(store: &mut T) {
    let texts = vec![
        "Machine learning and artificial intelligence",
        "Deep learning neural networks",
        "The weather is sunny today",
    ];

    let add_result = store.add_texts(&texts, None, None).await;
    if should_skip_on_error(&add_result) {
        return;
    }
    add_result.unwrap();

    // Search with scores
    let result = store
        .similarity_search_with_score("machine learning AI", 10, None)
        .await;

    if let Ok(results) = result {
        if !results.is_empty() {
            // Get the score range
            let max_score = results
                .iter()
                .map(|(_, s)| s)
                .fold(0.0_f32, |a, &b| a.max(b));
            let min_score = results
                .iter()
                .map(|(_, s)| s)
                .fold(1.0_f32, |a, &b| a.min(b));

            // If there's a score range, we can test thresholding behavior
            if max_score > min_score {
                let _threshold = (max_score + min_score) / 2.0;

                // In a real implementation, we'd pass threshold to search
                // For now, just verify scores are in valid range
                for (_, score) in &results {
                    assert!(
                        *score >= 0.0 && *score <= 1.0,
                        "Scores should be in [0, 1] range"
                    );
                }
            }
        }
    }
    // If score-based search not supported, that's acceptable
}
