//! # PostgreSQL pgvector Vector Store Example
//!
//! This example demonstrates how to use PgVectorStore for storing
//! and searching document embeddings in PostgreSQL with pgvector extension.
//!
//! **Prerequisites:**
//! - Start PostgreSQL with pgvector:
//!   ```bash
//!   docker run --name postgres-pgvector -e POSTGRES_PASSWORD=postgres \
//!     -p 5432:5432 -d pgvector/pgvector:pg16
//!   ```
//!
//! **Run this example:**
//! ```bash
//! cargo run --package dashflow-pgvector --example pgvector_basic
//! ```
//!
//! Covers:
//! - Creating a PostgreSQL pgvector store with collection
//! - Adding documents with metadata
//! - Similarity search with scores
//! - Metadata filtering
//! - CRUD operations

use async_trait::async_trait;
use dashflow::core::{embeddings::Embeddings, vector_stores::VectorStore, Error};
use dashflow_pgvector::PgVectorStore;
use std::collections::HashMap;
use std::sync::Arc;

/// Simple mock embeddings for demonstration
/// In production, use OpenAI, Cohere, or another real embedding model
struct DemoEmbeddings;

#[async_trait]
impl Embeddings for DemoEmbeddings {
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Error> {
        // Generate deterministic embeddings based on text characteristics
        // Real embeddings would be 768-1536 dimensions; we use 1536 to match OpenAI default
        Ok(texts
            .iter()
            .map(|text| {
                let len = text.len() as f32;
                let first_char = text.chars().next().unwrap_or('a') as u32 as f32;
                let word_count = text.split_whitespace().count() as f32;

                // Create 1536D embedding vector (matching OpenAI dimensions)
                let mut embedding = vec![0.0f32; 1536];

                // Use text features to set the first few dimensions
                embedding[0] = (first_char / 255.0).min(1.0);
                embedding[1] = (word_count / 20.0).min(1.0);
                embedding[2] = (len / 100.0).min(1.0);

                // Fill rest with deterministic values based on text
                for (i, val) in embedding.iter_mut().enumerate().skip(3) {
                    *val = ((i as f32 * len) % 1.0) * 0.1;
                }

                // Normalize to unit vector (for cosine similarity)
                let mag: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                if mag > 0.0 {
                    embedding.iter_mut().for_each(|x| *x /= mag);
                }

                embedding
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
    println!("=== PostgreSQL pgvector Vector Store Example ===\n");
    println!("Note: This requires PostgreSQL with pgvector extension");
    println!("Start with: docker run --name postgres-pgvector -e POSTGRES_PASSWORD=postgres \\");
    println!("  -p 5432:5432 -d pgvector/pgvector:pg16\n");

    // Create embeddings and connect to PostgreSQL
    let embeddings: Arc<dyn Embeddings> = Arc::new(DemoEmbeddings);
    let connection_string = "postgresql://postgres:postgres@localhost:5432/postgres";
    let collection_name = "dashflow_demo";

    println!(
        "Connecting to PostgreSQL and creating collection '{}'...",
        collection_name
    );
    let mut store =
        PgVectorStore::new(connection_string, collection_name, Arc::clone(&embeddings)).await?;
    println!("Connected successfully!\n");

    // Clean up any existing data from previous runs
    println!("Clearing any existing documents...");
    store.delete(None).await?;
    println!("Ready for examples\n");

    // Example 1: Add simple texts
    println!("Example 1: Adding Simple Texts");
    let texts = vec![
        "The quick brown fox jumps over the lazy dog",
        "A journey of a thousand miles begins with a single step",
        "To be or not to be, that is the question",
        "In the beginning was the Word",
    ];
    println!("Adding {} documents...", texts.len());
    let ids = store.add_texts(&texts, None, None).await?;
    println!("Added documents with IDs: {:?}\n", ids);

    // Example 2: Similarity Search
    println!("Example 2: Similarity Search");
    let query = "fox and dog";
    println!("Query: '{}'", query);
    let results = store._similarity_search(query, 2, None).await?;
    println!("Found {} results:", results.len());
    for (i, doc) in results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
    }
    println!();

    // Example 3: Similarity Search with Scores
    println!("Example 3: Similarity Search with Scores");
    let query = "journey and step";
    println!("Query: '{}'", query);
    let results_with_scores = store.similarity_search_with_score(query, 3, None).await?;
    println!("Found {} results with scores:", results_with_scores.len());
    for (i, (doc, score)) in results_with_scores.iter().enumerate() {
        println!("  {}. Score: {:.4} - {}", i + 1, score, doc.page_content);
    }
    println!();

    // Example 4: Add documents with metadata
    println!("Example 4: Adding Documents with Metadata");
    let docs_with_metadata = [
        (
            "Rust is a systems programming language",
            json!({"language": "rust", "category": "programming"}),
        ),
        (
            "Python is great for data science",
            json!({"language": "python", "category": "data"}),
        ),
        (
            "JavaScript runs in browsers",
            json!({"language": "javascript", "category": "web"}),
        ),
        (
            "Go is designed for scalability",
            json!({"language": "go", "category": "backend"}),
        ),
    ];

    let texts: Vec<&str> = docs_with_metadata.iter().map(|(t, _)| *t).collect();
    let metadatas: Vec<HashMap<String, serde_json::Value>> = docs_with_metadata
        .iter()
        .map(|(_, m)| {
            let mut map = HashMap::new();
            if let serde_json::Value::Object(obj) = m {
                for (k, v) in obj {
                    map.insert(k.clone(), v.clone());
                }
            }
            map
        })
        .collect();

    println!("Adding {} documents with metadata...", texts.len());
    let meta_ids = store.add_texts(&texts, Some(&metadatas), None).await?;
    println!("Added documents with IDs: {:?}\n", meta_ids);

    // Example 5: Search with metadata filtering
    println!("Example 5: Search with Metadata Filtering");
    let query = "programming";
    let mut filter = HashMap::new();
    filter.insert("category".to_string(), json!("programming"));
    println!("Query: '{}', Filter: category=programming", query);
    let filtered_results = store._similarity_search(query, 5, Some(&filter)).await?;
    println!("Found {} results:", filtered_results.len());
    for (i, doc) in filtered_results.iter().enumerate() {
        println!(
            "  {}. {} (category: {})",
            i + 1,
            doc.page_content,
            doc.metadata
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("none")
        );
    }
    println!();

    // Example 6: Get documents by ID
    println!("Example 6: Get Documents by ID");
    println!("Fetching first 2 documents by ID...");
    let fetch_ids = vec![ids[0].clone(), ids[1].clone()];
    let fetched_docs = store.get_by_ids(&fetch_ids).await?;
    println!("Fetched {} documents:", fetched_docs.len());
    for (i, doc) in fetched_docs.iter().enumerate() {
        println!(
            "  {}. ID: {} - {}",
            i + 1,
            doc.id.as_ref().unwrap_or(&"none".to_string()),
            doc.page_content
        );
    }
    println!();

    // Example 7: Delete specific documents
    println!("Example 7: Delete Specific Documents");
    let delete_ids = vec![ids[0].clone()];
    println!("Deleting document with ID: {}", delete_ids[0]);
    store.delete(Some(&delete_ids)).await?;
    println!("Document deleted");

    // Verify deletion
    let remaining = store.get_by_ids(&ids[..2]).await?;
    println!("Remaining documents from first 2: {}\n", remaining.len());

    // Example 8: Search all documents
    println!("Example 8: Search All Remaining Documents");
    let all_results = store._similarity_search("text", 10, None).await?;
    println!("Total documents in store: {}", all_results.len());
    for (i, doc) in all_results.iter().enumerate() {
        println!(
            "  {}. {}",
            i + 1,
            &doc.page_content[..50.min(doc.page_content.len())]
        );
    }
    println!();

    // Cleanup
    println!("Cleaning up - deleting all documents...");
    store.delete(None).await?;
    println!("Cleanup complete");

    println!("\n=== Example Complete ===");
    Ok(())
}

use serde_json::json;
