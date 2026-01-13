//! Basic example of using TimescaleVectorStore with DiskANN indexing.
//!
//! This example demonstrates:
//! - Creating a TimescaleVectorStore connection
//! - Adding documents with embeddings
//! - Performing similarity search with DiskANN
//! - Searching with metadata filters
//!
//! Prerequisites:
//! - PostgreSQL with pgvector and pgvectorscale extensions installed
//! - Set DATABASE_URL environment variable or use default connection string
//!
//! Run with:
//! ```bash
//! cargo run --example timescale_basic
//! ```

use async_trait::async_trait;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::VectorStore;
use dashflow::core::{Error, Result};
use dashflow_timescale::TimescaleVectorStore;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;

/// Mock embeddings for demonstration purposes.
/// In production, use a real embeddings model like OpenAI, Ollama, or HuggingFace.
struct MockEmbeddings;

#[async_trait]
impl Embeddings for MockEmbeddings {
    async fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // Generate deterministic mock embeddings based on text length and content
        // In production, this would call a real embeddings API
        Ok(texts
            .iter()
            .map(|text| {
                let len = text.len() as f32;
                let hash = text.bytes().map(|b| b as f32).sum::<f32>();
                // Generate 1536-dimensional vector (OpenAI embedding size)
                (0..1536)
                    .map(|i| {
                        let val = (len * 0.1 + hash * 0.01 + i as f32 * 0.001).sin();
                        val / 1536.0_f32.sqrt() // Normalize for cosine similarity
                    })
                    .collect()
            })
            .collect())
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        let results = self.embed_documents(&[text.to_string()]).await?;
        results.into_iter().next().ok_or_else(|| {
            Error::other("Failed to generate embedding for query".to_string())
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("TimescaleVector (pgvectorscale) Basic Example");
    println!("==============================================\n");

    // Get database connection string from environment or use default
    let connection_string = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/vectordb".to_string());

    println!("Connecting to TimescaleDB...");
    println!("Connection: {}\n", connection_string);

    // Create embeddings model
    let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings);

    // Create vector store with DiskANN indexing
    let mut store = TimescaleVectorStore::new(
        &connection_string,
        "demo_documents",
        embeddings,
    )
    .await
    .map_err(|e| {
        eprintln!("Failed to connect to TimescaleDB: {}", e);
        eprintln!("\nMake sure:");
        eprintln!("1. PostgreSQL is running");
        eprintln!("2. pgvector extension is installed");
        eprintln!("3. pgvectorscale extension is installed");
        eprintln!("4. DATABASE_URL is set correctly");
        e
    })?;

    println!("✓ Connected to TimescaleDB");
    println!("✓ Extensions verified (pgvector + pgvectorscale)");
    println!("✓ Table created with DiskANN index\n");

    // Sample documents about programming and technology
    let documents = vec![
        "Rust is a systems programming language focused on safety and performance",
        "Python is great for data science and machine learning applications",
        "JavaScript runs in the browser and is essential for web development",
        "PostgreSQL is a powerful open-source relational database",
        "TimescaleDB extends PostgreSQL with time-series capabilities",
        "Vector databases enable semantic search and AI applications",
        "Machine learning models can be deployed using Docker containers",
        "Kubernetes orchestrates containerized applications at scale",
    ];

    // Add metadata for filtering
    let metadatas: Vec<HashMap<String, serde_json::Value>> = vec![
        [("category", "programming"), ("language", "rust")]
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect(),
        [("category", "programming"), ("language", "python")]
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect(),
        [("category", "programming"), ("language", "javascript")]
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect(),
        [("category", "database"), ("type", "sql")]
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect(),
        [("category", "database"), ("type", "timeseries")]
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect(),
        [("category", "database"), ("type", "vector")]
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect(),
        [("category", "infrastructure"), ("type", "container")]
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect(),
        [("category", "infrastructure"), ("type", "orchestration")]
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect(),
    ];

    println!("Adding {} documents to vector store...", documents.len());
    let ids = store
        .add_texts(&documents, Some(&metadatas), None)
        .await?;

    println!("✓ Added {} documents", ids.len());
    println!("✓ DiskANN index automatically optimized\n");

    // Example 1: Basic similarity search with DiskANN
    println!("Example 1: Similarity Search (DiskANN)");
    println!("---------------------------------------");
    println!("Query: 'programming languages for web development'\n");

    let results = store
        .similarity_search("programming languages for web development", 3)
        .await?;

    println!("Top 3 results:");
    for (i, doc) in results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
        if let Some(category) = doc.metadata.get("category") {
            println!("     Category: {}", category);
        }
    }
    println!();

    // Example 2: Search with scores
    println!("Example 2: Search with Similarity Scores");
    println!("-----------------------------------------");
    println!("Query: 'database systems'\n");

    let results_with_scores = store
        .similarity_search_with_score("database systems", 3)
        .await?;

    println!("Top 3 results with scores:");
    for (i, (doc, score)) in results_with_scores.iter().enumerate() {
        println!("  {}. {:.3} - {}", i + 1, score, doc.page_content);
    }
    println!();

    // Example 3: Filtered search
    println!("Example 3: Filtered Search (Metadata)");
    println!("--------------------------------------");
    println!("Query: 'programming' filtered by category='programming'\n");

    let mut filter = HashMap::new();
    filter.insert("category".to_string(), json!("programming"));

    let filtered_results = store
        .similarity_search("programming", 3, Some(&filter))
        .await?;

    println!("Filtered results:");
    for (i, doc) in filtered_results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
        if let Some(language) = doc.metadata.get("language") {
            println!("     Language: {}", language);
        }
    }
    println!();

    // Example 4: Get documents by ID
    println!("Example 4: Retrieve by ID");
    println!("-------------------------");

    if let Some(first_id) = ids.first() {
        println!("Fetching document with ID: {}\n", first_id);
        let docs = store.get_by_ids(&[first_id.clone()]).await?;
        if let Some(doc) = docs.first() {
            println!("Retrieved: {}", doc.page_content);
        }
    }
    println!();

    // Example 5: Delete documents
    println!("Example 5: Delete Documents");
    println!("---------------------------");

    if ids.len() >= 2 {
        let delete_ids = &ids[0..2];
        println!("Deleting {} documents...", delete_ids.len());
        store.delete(Some(delete_ids)).await?;
        println!("✓ Deleted successfully\n");
    }

    // Clean up: delete all remaining documents
    println!("Cleaning up: deleting all documents...");
    store.delete(None).await?;
    println!("✓ All documents deleted\n");

    println!("Example completed successfully!");
    println!("\nPerformance Note:");
    println!("With pgvectorscale's DiskANN index, you get:");
    println!("  • 28x lower p95 latency vs standard pgvector");
    println!("  • 16x higher query throughput");
    println!("  • 75% lower cost vs managed services");

    Ok(())
}
