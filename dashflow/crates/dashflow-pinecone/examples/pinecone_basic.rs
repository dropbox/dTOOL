//! DashFlow.
//!
//! This example demonstrates:
//! - Connecting to a Pinecone index
//! - Adding documents to the vector store
//! - Performing similarity search
//! - Searching with metadata filters
//! - Using different namespaces
//!
//! # Prerequisites
//!
//! 1. Create a Pinecone account at https://www.pinecone.io/
//! 2. Create an index in the Pinecone console (serverless or pod-based)
//! 3. Set environment variables:
//!    - PINECONE_API_KEY: Your Pinecone API key
//!    - PINECONE_INDEX_HOST: Your index host (e.g., "my-index-abc123.svc.aped-0000-1111.pinecone.io")
//!    - OPENAI_API_KEY: Your OpenAI API key (for embeddings)
//!
//! # Running
//!
//! ```bash
//! export PINECONE_API_KEY="your-api-key"
//! export PINECONE_INDEX_HOST="your-index-host"
//! export OPENAI_API_KEY="your-openai-key"
//! cargo run --package dashflow-pinecone --example pinecone_basic
//! ```

use dashflow::core::vector_stores::VectorStore;
use dashflow::prelude::Embeddings;
use dashflow_openai::embeddings::OpenAIEmbeddings;
use dashflow_pinecone::PineconeVectorStore;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Pinecone Vector Store Example ===\n");

    // Get configuration from environment
    let api_key = env::var("PINECONE_API_KEY").ok();
    let index_host = match env::var("PINECONE_INDEX_HOST") {
        Ok(host) => host,
        Err(_) => {
            println!("PINECONE_INDEX_HOST environment variable required.");
            println!("Example: export PINECONE_INDEX_HOST=\"my-index-abc123.svc.aped-0000-1111.pinecone.io\"");
            return Ok(());
        }
    };

    if api_key.is_none() {
        println!("Warning: PINECONE_API_KEY not set, will attempt to use SDK default");
    }

    // Create embeddings (OpenAI for this example)
    let embeddings: Arc<dyn Embeddings> = Arc::new(OpenAIEmbeddings::default());

    // Create Pinecone vector store
    println!("Connecting to Pinecone index: {}", index_host);
    let mut store = PineconeVectorStore::new(
        &index_host,
        Arc::clone(&embeddings),
        api_key.as_deref(),
        Some("example-namespace"),
    )
    .await?;
    println!("Connected successfully!\n");

    // Example 1: Add documents
    println!("--- Example 1: Adding documents ---");
    let texts = vec![
        "The quick brown fox jumps over the lazy dog",
        "Machine learning is a subset of artificial intelligence",
        "Rust is a systems programming language",
        "Vector databases enable semantic search",
    ];

    let ids = store.add_texts(&texts, None, None).await?;
    println!("Added {} documents with IDs:", ids.len());
    for (i, id) in ids.iter().enumerate() {
        println!("  {}: {}", i + 1, id);
    }
    println!();

    // Example 2: Similarity search
    println!("--- Example 2: Similarity search ---");
    let query = "Tell me about AI";
    println!("Query: \"{}\"", query);
    let results = store._similarity_search(query, 2, None).await?;
    println!("Found {} similar documents:", results.len());
    for (i, doc) in results.iter().enumerate() {
        println!(
            "  {}: {} (score: {})",
            i + 1,
            doc.page_content,
            doc.metadata.get("score").unwrap_or(&serde_json::json!(0.0))
        );
    }
    println!();

    // Example 3: Add documents with metadata
    println!("--- Example 3: Adding documents with metadata ---");
    let texts_with_meta = vec![
        "Python is great for data science",
        "JavaScript runs in the browser",
        "Go is good for microservices",
    ];

    let metadatas: Vec<HashMap<String, serde_json::Value>> = vec![
        [
            ("language".to_string(), "python".into()),
            ("category".to_string(), "programming".into()),
        ]
        .iter()
        .cloned()
        .collect(),
        [
            ("language".to_string(), "javascript".into()),
            ("category".to_string(), "programming".into()),
        ]
        .iter()
        .cloned()
        .collect(),
        [
            ("language".to_string(), "go".into()),
            ("category".to_string(), "programming".into()),
        ]
        .iter()
        .cloned()
        .collect(),
    ];

    let ids_with_meta = store
        .add_texts(&texts_with_meta, Some(&metadatas), None)
        .await?;
    println!("Added {} documents with metadata", ids_with_meta.len());
    println!();

    // Example 4: Search with metadata filter
    println!("--- Example 4: Search with metadata filter ---");
    let query = "programming languages";
    println!("Query: \"{}\"", query);
    println!("Filter: language = 'python'");
    let mut filter = HashMap::new();
    filter.insert("language".to_string(), "python".into());
    let filtered_results = store._similarity_search(query, 3, Some(&filter)).await?;
    println!("Found {} filtered results:", filtered_results.len());
    for (i, doc) in filtered_results.iter().enumerate() {
        println!(
            "  {}: {} (language: {})",
            i + 1,
            doc.page_content,
            doc.metadata
                .get("language")
                .unwrap_or(&serde_json::json!("unknown"))
        );
    }
    println!();

    // Example 5: Similarity search with scores
    println!("--- Example 5: Similarity search with scores ---");
    let query = "databases and search";
    println!("Query: \"{}\"", query);
    let results_with_scores = store.similarity_search_with_score(query, 3, None).await?;
    println!("Found {} results with scores:", results_with_scores.len());
    for (i, (doc, score)) in results_with_scores.iter().enumerate() {
        println!("  {}: {} (score: {:.4})", i + 1, doc.page_content, score);
    }
    println!();

    // Example 6: Delete documents
    println!("--- Example 6: Deleting documents ---");
    println!("Deleting first 2 documents: {:?}", &ids[0..2]);
    let deleted = store.delete(Some(&ids[0..2])).await?;
    println!("Delete successful: {}", deleted);
    println!();

    println!("=== Example complete! ===");
    println!("\nNote: Documents were added to namespace 'example-namespace'");
    println!("You can view them in the Pinecone console.");

    Ok(())
}
