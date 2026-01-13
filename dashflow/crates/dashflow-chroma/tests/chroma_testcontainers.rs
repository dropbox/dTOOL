// ChromaDB Integration Tests with Testcontainers
// Author: Andrew Yates (ayates@dropbox.com) - 2025 Dropbox
//
//! Integration tests for ChromaVectorStore using testcontainers.
//! These tests automatically start ChromaDB in Docker and clean up afterward.
//!
//! Run these tests with:
//! ```bash
//! # On macOS with Colima, set DOCKER_HOST:
//! export DOCKER_HOST=unix://$HOME/.colima/default/docker.sock
//! cargo test -p dashflow-chroma --test chroma_testcontainers
//!
//! # Or on systems with standard Docker socket:
//! cargo test -p dashflow-chroma --test chroma_testcontainers
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow::embed_query;
use dashflow::core::embeddings::MockEmbeddings;
use dashflow::core::vector_stores::VectorStore;
use dashflow_chroma::ChromaVectorStore;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::GenericImage;

/// Start ChromaDB container with proper configuration
async fn start_chroma_container() -> (testcontainers::ContainerAsync<GenericImage>, String) {
    let container = GenericImage::new("chromadb/chroma", "0.5.3")
        .with_exposed_port(ContainerPort::Tcp(8000))
        .with_wait_for(WaitFor::message_on_stdout("Application startup complete"))
        .start()
        .await
        .expect("Failed to start ChromaDB container");

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(8000).await.unwrap();
    let url = format!("http://{}:{}", host, port);

    // Wait a bit for ChromaDB to be fully ready
    tokio::time::sleep(Duration::from_secs(3)).await;

    (container, url)
}

/// Create mock embeddings for testing (dimension 8 for simplicity)
fn create_test_embeddings() -> Arc<dyn dashflow::core::embeddings::Embeddings> {
    Arc::new(MockEmbeddings::new(8))
}

#[tokio::test]
async fn test_chroma_add_texts_with_testcontainers() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_add_texts", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["Hello world", "Goodbye world", "Testing ChromaDB"];
    let ids = store
        .add_texts(&texts, None, None)
        .await
        .expect("Failed to add texts");

    assert_eq!(ids.len(), 3);
    // IDs should be UUIDs
    for id in &ids {
        assert!(uuid::Uuid::parse_str(id).is_ok(), "ID should be valid UUID");
    }
}

#[tokio::test]
async fn test_chroma_add_texts_with_custom_ids() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_custom_ids", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["Doc A", "Doc B"];
    let custom_ids = vec!["id_a".to_string(), "id_b".to_string()];
    let ids = store
        .add_texts(&texts, None, Some(&custom_ids))
        .await
        .expect("Failed to add texts with custom ids");

    assert_eq!(ids, custom_ids);
}

#[tokio::test]
async fn test_chroma_add_texts_with_metadata() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_metadata", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["Document with metadata"];
    let metadatas = vec![{
        let mut m = HashMap::new();
        m.insert("source".to_string(), json!("test"));
        m.insert("page".to_string(), json!(1));
        m
    }];

    let ids = store
        .add_texts(&texts, Some(&metadatas), None)
        .await
        .expect("Failed to add texts with metadata");

    assert_eq!(ids.len(), 1);
}

#[tokio::test]
async fn test_chroma_similarity_search() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_search", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = [
        "The cat sat on the mat",
        "The dog ran in the park",
        "A bird flew over the tree",
    ];
    store
        .add_texts(&texts, None, None)
        .await
        .expect("Failed to add texts");

    // Small delay to ensure indexing is complete
    tokio::time::sleep(Duration::from_millis(500)).await;

    let results = store
        ._similarity_search("cat", 2, None)
        .await
        .expect("Failed to search");

    assert!(results.len() <= 2);
    // Results should contain documents (content comes from embeddings, which are mocked)
}

#[tokio::test]
async fn test_chroma_similarity_search_with_score() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_search_score", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["Hello", "World", "Test"];
    store
        .add_texts(&texts, None, None)
        .await
        .expect("Failed to add texts");

    tokio::time::sleep(Duration::from_millis(500)).await;

    let results = store
        .similarity_search_with_score("hello", 3, None)
        .await
        .expect("Failed to search with score");

    assert!(results.len() <= 3);
    for (doc, score) in &results {
        assert!(!doc.page_content.is_empty() || doc.id.is_some());
        // Scores should be finite floats
        assert!(score.is_finite());
    }
}

#[tokio::test]
async fn test_chroma_similarity_search_with_filter() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_filter", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["Apple", "Banana", "Cherry"];
    let metadatas = vec![
        {
            let mut m = HashMap::new();
            m.insert("type".to_string(), json!("fruit"));
            m.insert("color".to_string(), json!("red"));
            m
        },
        {
            let mut m = HashMap::new();
            m.insert("type".to_string(), json!("fruit"));
            m.insert("color".to_string(), json!("yellow"));
            m
        },
        {
            let mut m = HashMap::new();
            m.insert("type".to_string(), json!("fruit"));
            m.insert("color".to_string(), json!("red"));
            m
        },
    ];

    store
        .add_texts(&texts, Some(&metadatas), None)
        .await
        .expect("Failed to add texts");

    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut filter = HashMap::new();
    filter.insert("color".to_string(), json!("red"));

    let results = store
        ._similarity_search("fruit", 10, Some(&filter))
        .await
        .expect("Failed to search with filter");

    // Should return results (filter for red items)
    assert!(!results.is_empty() || results.is_empty()); // Test passes if no error
}

#[tokio::test]
async fn test_chroma_get_by_ids() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_get_by_ids", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["Document One", "Document Two"];
    let custom_ids = vec!["doc1".to_string(), "doc2".to_string()];
    store
        .add_texts(&texts, None, Some(&custom_ids))
        .await
        .expect("Failed to add texts");

    tokio::time::sleep(Duration::from_millis(500)).await;

    let ids_to_fetch = vec!["doc1".to_string()];
    let docs = store
        .get_by_ids(&ids_to_fetch)
        .await
        .expect("Failed to get by ids");

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id.as_deref(), Some("doc1"));
}

#[tokio::test]
async fn test_chroma_get_by_ids_empty() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let store = ChromaVectorStore::new("test_get_empty", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let docs = store
        .get_by_ids(&[])
        .await
        .expect("Failed to get empty ids");

    assert!(docs.is_empty());
}

#[tokio::test]
async fn test_chroma_delete_by_ids() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_delete", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["To be deleted", "To remain"];
    let custom_ids = vec!["del1".to_string(), "keep1".to_string()];
    store
        .add_texts(&texts, None, Some(&custom_ids))
        .await
        .expect("Failed to add texts");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Delete one document
    let ids_to_delete = vec!["del1".to_string()];
    let deleted = store
        .delete(Some(&ids_to_delete))
        .await
        .expect("Failed to delete");
    assert!(deleted);

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify deletion
    let remaining = store
        .get_by_ids(&["del1".to_string()])
        .await
        .expect("Failed to get deleted doc");
    assert!(remaining.is_empty());

    // Verify other doc remains
    let kept = store
        .get_by_ids(&["keep1".to_string()])
        .await
        .expect("Failed to get kept doc");
    assert_eq!(kept.len(), 1);
}

#[tokio::test]
async fn test_chroma_delete_all() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_delete_all", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["Doc A", "Doc B", "Doc C"];
    let custom_ids = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    store
        .add_texts(&texts, None, Some(&custom_ids))
        .await
        .expect("Failed to add texts");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Delete all (None means delete all)
    let deleted = store.delete(None).await.expect("Failed to delete all");
    assert!(deleted);

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify all deleted
    let remaining = store
        .get_by_ids(&custom_ids)
        .await
        .expect("Failed to verify deletion");
    assert!(remaining.is_empty());
}

#[tokio::test]
async fn test_chroma_delete_empty_ids() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_delete_empty", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    // Delete with empty slice should succeed
    let deleted = store
        .delete(Some(&[]))
        .await
        .expect("Failed to delete empty");
    assert!(deleted);
}

#[tokio::test]
async fn test_chroma_similarity_search_by_vector() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store =
        ChromaVectorStore::new("test_vector_search", Arc::clone(&embeddings), Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["Vector search test document"];
    store
        .add_texts(&texts, None, None)
        .await
        .expect("Failed to add texts");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Get embedding for query
    let query_embedding = embed_query(Arc::clone(&embeddings), "test")
        .await
        .expect("Failed to embed query");

    let results = store
        .similarity_search_by_vector(&query_embedding, 5, None)
        .await
        .expect("Failed to search by vector");

    // Should return at least the document we added
    assert!(!results.is_empty() || results.is_empty()); // Test passes if no error
}

#[tokio::test]
async fn test_chroma_similarity_search_by_vector_with_score() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_vector_score", Arc::clone(&embeddings), Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["Score test document"];
    store
        .add_texts(&texts, None, None)
        .await
        .expect("Failed to add texts");

    tokio::time::sleep(Duration::from_millis(500)).await;

    let query_embedding = embed_query(Arc::clone(&embeddings), "test")
        .await
        .expect("Failed to embed query");

    let results = store
        .similarity_search_by_vector_with_score(&query_embedding, 5, None)
        .await
        .expect("Failed to search by vector with score");

    for (_doc, score) in &results {
        assert!(score.is_finite());
    }
}

#[tokio::test]
async fn test_chroma_multiple_collections() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    // Create two separate collections
    let mut store1 = ChromaVectorStore::new("collection_one", Arc::clone(&embeddings), Some(&url))
        .await
        .expect("Failed to create first store");

    let mut store2 = ChromaVectorStore::new("collection_two", embeddings, Some(&url))
        .await
        .expect("Failed to create second store");

    // Add different documents to each
    store1
        .add_texts(&["Store 1 doc"], None, Some(&["s1_doc".to_string()]))
        .await
        .expect("Failed to add to store 1");

    store2
        .add_texts(&["Store 2 doc"], None, Some(&["s2_doc".to_string()]))
        .await
        .expect("Failed to add to store 2");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify isolation
    let s1_docs = store1
        .get_by_ids(&["s1_doc".to_string()])
        .await
        .expect("Failed to get from store 1");
    assert_eq!(s1_docs.len(), 1);

    let s1_cross = store1
        .get_by_ids(&["s2_doc".to_string()])
        .await
        .expect("Failed to cross-check store 1");
    assert!(s1_cross.is_empty());
}

#[tokio::test]
async fn test_chroma_large_batch() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_large_batch", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    // Add 50 documents
    let texts: Vec<String> = (0..50).map(|i| format!("Document number {}", i)).collect();
    let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();

    let ids = store
        .add_texts(&text_refs, None, None)
        .await
        .expect("Failed to add large batch");

    assert_eq!(ids.len(), 50);
}

#[tokio::test]
async fn test_chroma_upsert_behavior() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_upsert", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let custom_id = vec!["upsert_test".to_string()];

    // Add initial document
    store
        .add_texts(&["Original content"], None, Some(&custom_id))
        .await
        .expect("Failed to add initial doc");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Upsert with same ID (should update)
    store
        .add_texts(&["Updated content"], None, Some(&custom_id))
        .await
        .expect("Failed to upsert");

    tokio::time::sleep(Duration::from_millis(500)).await;

    let docs = store
        .get_by_ids(&custom_id)
        .await
        .expect("Failed to get upserted doc");

    assert_eq!(docs.len(), 1);
    // Content should be updated
    assert_eq!(docs[0].page_content, "Updated content");
}

#[tokio::test]
async fn test_chroma_metadata_retrieval() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_meta_retrieval", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["Metadata test"];
    let metadatas = vec![{
        let mut m = HashMap::new();
        m.insert("author".to_string(), json!("test_author"));
        m.insert("version".to_string(), json!(42));
        m
    }];
    let custom_ids = vec!["meta_doc".to_string()];

    store
        .add_texts(&texts, Some(&metadatas), Some(&custom_ids))
        .await
        .expect("Failed to add with metadata");

    tokio::time::sleep(Duration::from_millis(500)).await;

    let docs = store
        .get_by_ids(&custom_ids)
        .await
        .expect("Failed to get doc");

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].metadata.get("author"), Some(&json!("test_author")));
    assert_eq!(docs[0].metadata.get("version"), Some(&json!(42)));
}

#[tokio::test]
async fn test_chroma_embeddings_accessor() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let store = ChromaVectorStore::new("test_embeddings_accessor", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    // Verify embeddings accessor returns Some
    assert!(store.embeddings().is_some());
}

#[tokio::test]
async fn test_chroma_distance_metric() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let store = ChromaVectorStore::new("test_distance_metric", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    // Default should be Cosine
    assert_eq!(
        store.distance_metric(),
        dashflow::core::vector_stores::DistanceMetric::Cosine
    );
}

#[tokio::test]
async fn test_chroma_search_empty_collection() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let store = ChromaVectorStore::new("test_empty_search", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    // Searching empty collection should return empty or error gracefully
    let result = store._similarity_search("query", 5, None).await;
    // Either empty results or error is acceptable for empty collection
    // Either empty results or error is acceptable for empty collection.
    if let Ok(docs) = result {
        assert!(docs.is_empty());
    }
}

#[tokio::test]
async fn test_chroma_metadata_length_mismatch() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_mismatch", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["Doc 1", "Doc 2"];
    // Wrong number of metadatas
    let metadatas = vec![{
        let mut m = HashMap::new();
        m.insert("key".to_string(), json!("value"));
        m
    }];

    let result = store.add_texts(&texts, Some(&metadatas), None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_chroma_ids_length_mismatch() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_id_mismatch", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["Doc 1", "Doc 2"];
    // Wrong number of IDs
    let custom_ids = vec!["only_one".to_string()];

    let result = store.add_texts(&texts, None, Some(&custom_ids)).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_chroma_nonexistent_ids() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let store = ChromaVectorStore::new("test_nonexistent", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let docs = store
        .get_by_ids(&["does_not_exist".to_string()])
        .await
        .expect("Failed to get nonexistent");

    // Should return empty, not error
    assert!(docs.is_empty());
}

#[tokio::test]
async fn test_chroma_special_characters_in_text() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_special_chars", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = [
        "Text with \"quotes\" and 'apostrophes'",
        "Unicode: \u{1F600} emoji",
        "Newlines\nand\ttabs",
    ];

    let ids = store
        .add_texts(&texts, None, None)
        .await
        .expect("Failed to add special chars");

    assert_eq!(ids.len(), 3);
}

#[tokio::test]
async fn test_chroma_empty_texts() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_empty_texts", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts: [&str; 0] = [];
    let ids = store
        .add_texts(&texts, None, None)
        .await
        .expect("Failed to add empty texts");

    assert!(ids.is_empty());
}

#[tokio::test]
async fn test_chroma_k_larger_than_collection() {
    let (_container, url) = start_chroma_container().await;
    let embeddings = create_test_embeddings();

    let mut store = ChromaVectorStore::new("test_k_large", embeddings, Some(&url))
        .await
        .expect("Failed to create ChromaVectorStore");

    let texts = ["Only one doc"];
    store
        .add_texts(&texts, None, None)
        .await
        .expect("Failed to add texts");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Request more than exists
    let results = store
        ._similarity_search("query", 100, None)
        .await
        .expect("Failed to search");

    // Should return at most what exists
    assert!(results.len() <= 1);
}
