//! Integration tests for Redis vector store.
//!
//! These tests require Redis Stack running on localhost:6379 (or custom URL via env).
//!
//! To run Redis Stack with Docker:
//! ```bash
//! docker run -d -p 6379:6379 redis/redis-stack:latest
//! ```
//!
//! Configure Redis URL (optional):
//! ```bash
//! export REDIS_URL=redis://myhost:6379
//! ```
//!
//! Run tests with:
//! ```bash
//! cargo test -p dashflow-redis --test integration_tests -- --ignored
//! ```

use dashflow::core::embeddings::Embeddings;
use dashflow::core::error::Result;
use dashflow::core::vector_stores::VectorStore;
use dashflow_redis::RedisVectorStore;
use redis::aio::ConnectionManager;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Get Redis URL from environment or default to localhost.
fn get_redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string())
}

fn unique_index_name(prefix: &str) -> String {
    format!("{}_{}", prefix, Uuid::new_v4().simple())
}

/// Mock embeddings for testing (deterministic based on text length).
#[derive(Clone)]
struct MockEmbeddings {
    dims: usize,
}

impl MockEmbeddings {
    fn new(dims: usize) -> Self {
        Self { dims }
    }

    /// Generate a deterministic embedding based on text content.
    fn generate_embedding(&self, text: &str) -> Vec<f32> {
        let mut embedding = vec![0.0; self.dims];

        // Simple deterministic embedding: use character codes
        for (i, ch) in text.chars().enumerate() {
            let idx = i % self.dims;
            embedding[idx] += (ch as u32 as f32) / 1000.0;
        }

        // Normalize
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for val in &mut embedding {
                *val /= magnitude;
            }
        }

        embedding
    }
}

#[async_trait::async_trait]
impl Embeddings for MockEmbeddings {
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|t| self.generate_embedding(t)).collect())
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>> {
        Ok(self.generate_embedding(text))
    }
}

/// Helper to create a test store with a unique index name.
async fn create_test_store(index_name: &str) -> Result<RedisVectorStore> {
    let embeddings = Arc::new(MockEmbeddings::new(128));
    RedisVectorStore::new(&get_redis_url(), index_name, embeddings, None, None).await
}

/// Helper to clean up test index.
async fn cleanup_index(index_name: &str) -> Result<()> {
    let client = redis::Client::open(get_redis_url().as_str()).map_err(|e| {
        dashflow::core::error::Error::config(format!("Failed to connect to Redis: {}", e))
    })?;
    let mut conn = ConnectionManager::new(client).await.map_err(|e| {
        dashflow::core::error::Error::config(format!("Failed to create connection manager: {}", e))
    })?;

    // Drop index (keep data)
    let _: std::result::Result<(), redis::RedisError> = redis::cmd("FT.DROPINDEX")
        .arg(index_name)
        .query_async(&mut conn)
        .await;

    // Delete all keys with prefix
    let pattern = format!("doc:{}:*", index_name);
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg(&pattern)
        .query_async(&mut conn)
        .await
        .map_err(|e| {
            dashflow::core::error::Error::config(format!("Failed to query keys: {}", e))
        })?;

    if !keys.is_empty() {
        let _: () = redis::cmd("DEL")
            .arg(&keys)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                dashflow::core::error::Error::config(format!("Failed to delete keys: {}", e))
            })?;
    }

    Ok(())
}

#[tokio::test]
#[ignore = "requires Redis Stack"]
async fn test_basic_workflow() -> Result<()> {
    let index_name = unique_index_name("test_basic_workflow");
    cleanup_index(&index_name).await?;

    let mut store = create_test_store(&index_name).await?;

    // Add documents
    let texts = vec!["Hello world", "Goodbye world", "Hello universe"];
    let ids = store.add_texts(&texts, None, None).await?;

    assert_eq!(ids.len(), 3);
    assert!(ids[0].contains(&format!("doc:{}:", index_name)));

    // Search
    let results = store._similarity_search("Hello", 2, None).await?;
    assert_eq!(results.len(), 2);

    // Results should be ordered by similarity (deterministic with MockEmbeddings)
    assert!(results[0].page_content.contains("Hello"));

    // Get by IDs
    let fetched = store.get_by_ids(&ids[0..1]).await?;
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].page_content, texts[0]);

    // Delete
    let deleted = store.delete(Some(&[ids[0].clone()])).await?;
    assert!(deleted);

    // Verify deletion
    let fetched_after = store.get_by_ids(&[ids[0].clone()]).await?;
    assert_eq!(fetched_after.len(), 0);

    cleanup_index(&index_name).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires Redis Stack"]
async fn test_with_metadata() -> Result<()> {
    let index_name = unique_index_name("test_with_metadata");
    cleanup_index(&index_name).await?;

    let mut store = create_test_store(&index_name).await?;

    // Add documents with metadata
    let texts = vec!["Product A", "Product B", "Product C"];

    let mut meta1 = HashMap::new();
    meta1.insert("category".to_string(), serde_json::json!("electronics"));
    meta1.insert("price".to_string(), serde_json::json!(99.99));

    let mut meta2 = HashMap::new();
    meta2.insert("category".to_string(), serde_json::json!("books"));
    meta2.insert("price".to_string(), serde_json::json!(19.99));

    let mut meta3 = HashMap::new();
    meta3.insert("category".to_string(), serde_json::json!("electronics"));
    meta3.insert("price".to_string(), serde_json::json!(149.99));

    let metadatas = vec![meta1, meta2, meta3];
    let ids = store.add_texts(&texts, Some(&metadatas), None).await?;

    assert_eq!(ids.len(), 3);

    // Search and verify metadata
    let results = store._similarity_search("Product", 3, None).await?;
    assert_eq!(results.len(), 3);

    // Check that metadata is present
    for result in &results {
        assert!(result.metadata.contains_key("category"));
        assert!(result.metadata.contains_key("price"));
    }

    cleanup_index(&index_name).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires Redis Stack"]
async fn test_custom_ids() -> Result<()> {
    let index_name = unique_index_name("test_custom_ids");
    cleanup_index(&index_name).await?;

    let mut store = create_test_store(&index_name).await?;

    // Add documents with custom IDs
    let texts = vec!["Doc 1", "Doc 2"];
    let custom_ids = vec!["my-id-1".to_string(), "my-id-2".to_string()];

    let returned_ids = store.add_texts(&texts, None, Some(&custom_ids)).await?;

    assert_eq!(returned_ids.len(), 2);
    assert!(returned_ids[0].contains("my-id-1"));
    assert!(returned_ids[1].contains("my-id-2"));

    // Fetch by custom IDs
    let fetched = store.get_by_ids(&custom_ids).await?;
    assert_eq!(fetched.len(), 2);
    assert_eq!(fetched[0].page_content, "Doc 1");
    assert_eq!(fetched[1].page_content, "Doc 2");

    cleanup_index(&index_name).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires Redis Stack"]
async fn test_empty_operations() -> Result<()> {
    let index_name = unique_index_name("test_empty_operations");
    cleanup_index(&index_name).await?;

    let mut store = create_test_store(&index_name).await?;

    // Add empty texts
    let empty_texts: Vec<String> = vec![];
    let ids = store.add_texts(&empty_texts, None, None).await?;
    assert_eq!(ids.len(), 0);

    // Search with no documents
    let results = store._similarity_search("query", 5, None).await?;
    assert_eq!(results.len(), 0);

    // Get by empty IDs
    let fetched = store.get_by_ids(&[]).await?;
    assert_eq!(fetched.len(), 0);

    // Delete with no IDs
    let deleted = store.delete(None).await?;
    assert!(!deleted);

    cleanup_index(&index_name).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires Redis Stack"]
async fn test_large_batch() -> Result<()> {
    let index_name = unique_index_name("test_large_batch");
    cleanup_index(&index_name).await?;

    let mut store = create_test_store(&index_name).await?;

    // Add 100 documents
    let texts: Vec<String> = (0..100).map(|i| format!("Document {}", i)).collect();
    let ids = store.add_texts(&texts, None, None).await?;

    assert_eq!(ids.len(), 100);

    // Search should work
    let results = store._similarity_search("Document 50", 10, None).await?;
    assert!(!results.is_empty());
    assert!(results.len() <= 10);

    cleanup_index(&index_name).await?;
    Ok(())
}
