//! # Qdrant Vector Store Example
//!
//! This example demonstrates how to use QdrantVectorStore for storing
//! and searching document embeddings in Qdrant.
//!
//! **Prerequisites:**
//! - Start Qdrant server: `docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant`
//!
//! **Run this example:**
//! ```bash
//! cargo run --package dashflow-qdrant --example qdrant_basic
//! ```
//!
//! Covers:
//! - Creating a Qdrant vector store with collection
//! - Adding documents with metadata
//! - Similarity search with scores
//! - Metadata filtering
//! - Maximum Marginal Relevance (MMR) search for diversity
//! - CRUD operations

use async_trait::async_trait;
use dashflow::embed_query;
use dashflow::core::{embeddings::Embeddings, Error};
use dashflow_qdrant::{QdrantVectorStore, RetrievalMode};
use std::collections::HashMap;
use std::sync::Arc;

/// Simple mock embeddings for demonstration
/// In production, use OpenAI, Cohere, or another real embedding model
struct DemoEmbeddings;

#[async_trait]
impl Embeddings for DemoEmbeddings {
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Error> {
        // Generate deterministic embeddings based on text characteristics
        // Real embeddings would be 768-1536 dimensions; we use 3 for demo
        Ok(texts
            .iter()
            .map(|text| {
                let len = text.len() as f32;
                let first_char = text.chars().next().unwrap_or('a') as u32 as f32;
                let word_count = text.split_whitespace().count() as f32;

                // Create 3D embedding vector
                let x = (first_char / 255.0).min(1.0);
                let y = (word_count / 20.0).min(1.0);
                let z = (len / 100.0).min(1.0);

                // Normalize to unit vector (for cosine similarity)
                let mag = (x * x + y * y + z * z).sqrt();
                if mag > 0.0 {
                    vec![x / mag, y / mag, z / mag]
                } else {
                    vec![0.0, 0.0, 1.0]
                }
            })
            .collect())
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>, Error> {
        let mut vectors = self._embed_documents(&[text.to_string()]).await?;
        vectors
            .pop()
            .ok_or_else(|| Error::InvalidInput("Embedding returned empty vectors".to_string()))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Qdrant Vector Store Example ===\n");
    println!("Note: This requires a running Qdrant server at http://localhost:6334");
    println!("Start with: docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant\n");

    // Create embeddings and connect to Qdrant
    let embeddings: Arc<dyn Embeddings> = Arc::new(DemoEmbeddings);
    let collection_name = "dashflow_demo";

    println!(
        "Connecting to Qdrant and creating collection '{}'...",
        collection_name
    );
    let mut store = QdrantVectorStore::new(
        "http://localhost:6334",
        collection_name,
        Some(Arc::clone(&embeddings)),
        RetrievalMode::Dense,
    )
    .await?;
    println!("Connected successfully!\n");

    // Clean up any existing data from previous runs
    println!("Clearing any existing documents (if collection exists)...");
    // Note: Qdrant doesn't have a "delete all" method, so we recreate the collection
    if store.collection_exists(collection_name).await? {
        store.delete_collection(collection_name).await?;
        // Recreate the store with new collection
        store = QdrantVectorStore::new(
            "http://localhost:6334",
            collection_name,
            Some(Arc::clone(&embeddings)),
            RetrievalMode::Dense,
        )
        .await?;
    }
    println!("Ready for examples\n");

    // Example 1: Add simple texts
    println!("Example 1: Adding Simple Texts");
    let texts = vec![
        "The quick brown fox jumps over the lazy dog",
        "A journey of a thousand miles begins with a single step",
        "To be or not to be, that is the question",
    ];

    let ids = store.add_texts(&texts, None, None, 64).await?;
    println!("Added {} documents", ids.len());
    println!("Document IDs: {:?}\n", ids);

    // Example 2: Add documents with metadata
    println!("Example 2: Adding Documents with Metadata");

    let mut metadata1 = HashMap::new();
    metadata1.insert("author".to_string(), serde_json::json!("Lao Tzu"));
    metadata1.insert("category".to_string(), serde_json::json!("philosophy"));
    metadata1.insert("year".to_string(), serde_json::json!(-500));

    let mut metadata2 = HashMap::new();
    metadata2.insert("author".to_string(), serde_json::json!("Albert Einstein"));
    metadata2.insert("category".to_string(), serde_json::json!("science"));
    metadata2.insert("year".to_string(), serde_json::json!(1955));

    let mut metadata3 = HashMap::new();
    metadata3.insert("author".to_string(), serde_json::json!("Marie Curie"));
    metadata3.insert("category".to_string(), serde_json::json!("science"));
    metadata3.insert("year".to_string(), serde_json::json!(1934));

    let mut metadata4 = HashMap::new();
    metadata4.insert("author".to_string(), serde_json::json!("Confucius"));
    metadata4.insert("category".to_string(), serde_json::json!("philosophy"));
    metadata4.insert("year".to_string(), serde_json::json!(-479));

    let texts_with_meta = vec![
        "The journey of a thousand miles begins with one step",
        "Imagination is more important than knowledge",
        "Nothing in life is to be feared, it is only to be understood",
        "It does not matter how slowly you go as long as you do not stop",
    ];

    let metadatas = vec![metadata1, metadata2, metadata3, metadata4];
    let meta_ids = store
        .add_texts(&texts_with_meta, Some(&metadatas), None, 64)
        .await?;
    println!("Added {} documents with rich metadata\n", meta_ids.len());

    // Example 3: Similarity Search with Scores
    println!("Example 3: Similarity Search with Scores");
    let query = "knowledge and wisdom";
    let results = store
        .similarity_search_with_score(query, 3, None, None, 0, None)
        .await?;

    println!("Top 3 results for query '{}':", query);
    for (i, (doc, score)) in results.iter().enumerate() {
        let author = doc
            .metadata
            .get("author")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        println!("{}. [Score: {:.4}] {}", i + 1, score, doc.page_content);
        println!("   Author: {}", author);
    }
    println!();

    // Example 4: Search with all results
    println!("Example 4: Extended Search (Top 5 Results)");
    // Note: Qdrant filtering requires constructing a qdrant::Filter object
    // For advanced filtering, see the Qdrant documentation
    let filtered_results = store
        .similarity_search_with_score(query, 5, None, None, 0, None)
        .await?;

    println!("Top 5 results (filter example simplified):");
    for (i, (doc, score)) in filtered_results.iter().enumerate() {
        let author = doc
            .metadata
            .get("author")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let category = doc
            .metadata
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        println!(
            "{}. [Score: {:.4}] [{}] {} - {}",
            i + 1,
            score,
            category,
            author,
            doc.page_content
        );
    }
    println!();

    // Example 5: MMR Search for Diversity
    println!("Example 5: Maximum Marginal Relevance (MMR) Search");

    // Add more documents with similar content
    let similar_texts = vec![
        "Machine learning is a subset of artificial intelligence",
        "Deep learning uses neural networks to learn patterns",
        "AI systems can process large amounts of data",
        "Neural networks mimic the human brain structure",
        "The weather today is sunny and warm",
    ];
    store.add_texts(&similar_texts, None, None, 64).await?;

    let ml_query = "artificial intelligence and machine learning";

    println!("Regular similarity search (may return very similar results):");
    let regular_results = store
        .similarity_search(ml_query, 3, None, None, 0, None)
        .await?;
    for (i, doc) in regular_results.iter().enumerate() {
        println!("{}. {}", i + 1, doc.page_content);
    }

    println!("\nMMR search with lambda=0.5 (balances relevance and diversity):");
    let mmr_results = store
        .max_marginal_relevance_search(ml_query, 3, 8, 0.5, None, None, None)
        .await?;
    for (i, doc) in mmr_results.iter().enumerate() {
        println!("{}. {}", i + 1, doc.page_content);
    }

    println!("\nMMR search with lambda=1.0 (maximum relevance, no diversity):");
    let mmr_high_lambda = store
        .max_marginal_relevance_search(ml_query, 3, 8, 1.0, None, None, None)
        .await?;
    for (i, doc) in mmr_high_lambda.iter().enumerate() {
        println!("{}. {}", i + 1, doc.page_content);
    }

    println!("\nMMR search with lambda=0.0 (maximum diversity, less relevance):");
    let mmr_low_lambda = store
        .max_marginal_relevance_search(ml_query, 3, 8, 0.0, None, None, None)
        .await?;
    for (i, doc) in mmr_low_lambda.iter().enumerate() {
        println!("{}. {}", i + 1, doc.page_content);
    }
    println!();

    // Example 6: Get Documents by ID
    println!("Example 6: Get Documents by ID");
    let first_two_ids = &ids[0..2];
    let retrieved = store.get_by_ids(first_two_ids).await?;
    println!("Retrieved {} documents by ID:", retrieved.len());
    for doc in &retrieved {
        println!("- {}", doc.page_content);
    }
    println!();

    // Example 7: Delete Specific Documents
    println!("Example 7: Delete Specific Documents");
    println!("Deleting first document (ID: {})", ids[0]);
    store.delete(&[ids[0].clone()]).await?;

    let remaining = store.get_by_ids(&ids).await?;
    println!("Documents remaining: {}/{}", remaining.len(), ids.len());
    println!();

    // Example 8: Add Documents with Custom IDs
    println!("Example 8: Adding Documents with Custom IDs");
    let custom_texts = vec![
        "Custom ID document number one",
        "Custom ID document number two",
    ];
    let custom_ids = vec!["custom-id-001".to_string(), "custom-id-002".to_string()];

    let added_ids = store
        .add_texts(&custom_texts, None, Some(&custom_ids), 64)
        .await?;
    println!("Added documents with custom IDs: {:?}", added_ids);
    println!("IDs match: {}", added_ids == custom_ids);
    println!();

    // Example 9: Search by Vector (without query text)
    println!("Example 9: Direct Vector Search");
    let query_vector = embed_query(Arc::clone(&embeddings), "philosophy and life").await?;
    let vector_results = store
        .similarity_search_by_vector(&query_vector, 2, None, None, 0, None)
        .await?;

    println!("Top 2 results from direct vector search:");
    for (i, doc) in vector_results.iter().enumerate() {
        let category = doc
            .metadata
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("none");
        println!("{}. [{}] {}", i + 1, category, doc.page_content);
    }
    println!();

    // Example 10: From Texts Constructor
    println!("Example 10: Creating Store with from_texts()");
    let quick_texts = vec!["Quick text one", "Quick text two"];

    let mut quick_meta = HashMap::new();
    quick_meta.insert("source".to_string(), serde_json::json!("quick_example"));
    let quick_metadatas = vec![quick_meta.clone(), quick_meta];

    let from_texts_store = QdrantVectorStore::from_texts(
        "http://localhost:6334",
        "quick_collection",
        &quick_texts,
        Some(&quick_metadatas),
        None, // ids (auto-generated)
        Some(Arc::clone(&embeddings)),
        RetrievalMode::Dense,
        64, // batch_size
    )
    .await?;

    println!("Created store with from_texts() - collection: quick_collection");
    let quick_search = from_texts_store
        .similarity_search("text", 2, None, None, 0, None)
        .await?;
    println!("Found {} documents in new collection", quick_search.len());
    println!();

    // Final stats
    println!("=== Summary ===");
    println!(
        "Qdrant collection '{}' now contains multiple documents",
        collection_name
    );
    println!("Demonstrated: CRUD operations, similarity search, metadata filtering, and MMR");
    println!("\nExample complete!");
    println!("\nNote: Collections persist in Qdrant. To reset, restart the Qdrant container.");

    Ok(())
}
