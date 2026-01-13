//! Basic example demonstrating ClickHouse vector store usage.
//!
//! This example shows how to:
//! - Connect to ClickHouse
//! - Add documents with embeddings
//! - Perform similarity search
//! - Use metadata filtering
//! - Delete documents
//!
//! # Prerequisites
//!
//! 1. Start ClickHouse server:
//! ```bash
//! docker run -d --name clickhouse-server \
//!   -p 8123:8123 -p 9000:9000 \
//!   --ulimit nofile=262144:262144 \
//!   clickhouse/clickhouse-server
//! ```
//!
//! 2. Set your OpenAI API key:
//! ```bash
//! export OPENAI_API_KEY="your-api-key"
//! ```
//!
//! # Run
//!
//! ```bash
//! cargo run --package dashflow-clickhouse --example clickhouse_basic
//! ```

use dashflow::core::vector_stores::VectorStore;
use dashflow::core::config_loader::{EmbeddingConfig, SecretReference};
use dashflow_clickhouse::ClickHouseVectorStore;
use dashflow_openai::build_embeddings;
use serde_json::json;
use std::collections::HashMap;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment
    if env::var("OPENAI_API_KEY").is_err() {
        println!("OPENAI_API_KEY environment variable must be set.");
        println!("Run: export OPENAI_API_KEY=\"your-api-key\"");
        return Ok(());
    }

    println!("🚀 ClickHouse Vector Store Example\n");

    // Create embeddings model (uses OPENAI_API_KEY from environment)
    let embedding_config = EmbeddingConfig::OpenAI {
        model: "text-embedding-3-small".to_string(),
        api_key: SecretReference::from_env("OPENAI_API_KEY"),
        batch_size: 32,
    };
    let embeddings = build_embeddings(&embedding_config)?;

    // Connect to ClickHouse
    println!("📊 Connecting to ClickHouse...");
    let mut store = ClickHouseVectorStore::new(
        "http://localhost:8123",
        "default",
        "dashflow_test_vectors",
        std::sync::Arc::clone(&embeddings),
    )
    .await?;
    println!("✅ Connected successfully\n");

    // Example 1: Add documents with embeddings
    println!("📝 Example 1: Adding documents with embeddings");
    let texts = vec![
        "The quick brown fox jumps over the lazy dog",
        "A journey of a thousand miles begins with a single step",
        "To be or not to be, that is the question",
        "All that glitters is not gold",
        "Where there is a will, there is a way",
    ];

    let mut metadatas = Vec::new();
    for (i, _) in texts.iter().enumerate() {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), json!(format!("doc_{}", i)));
        metadata.insert("category".to_string(), json!("quotes"));
        metadata.insert("index".to_string(), json!(i));
        metadatas.push(metadata);
    }

    let ids = store.add_texts(&texts, Some(&metadatas), None).await?;
    println!("✅ Added {} documents with IDs:", ids.len());
    for (i, id) in ids.iter().enumerate() {
        println!("   {} -> {}", texts[i], id);
    }
    println!();

    // Example 2: Similarity search
    println!("🔍 Example 2: Similarity search for 'fox'");
    let results = store._similarity_search("fox", 3, None).await?;
    println!("✅ Found {} results:", results.len());
    for (i, doc) in results.iter().enumerate() {
        println!("   {}. {}", i + 1, doc.page_content);
        if let Some(source) = doc.metadata.get("source") {
            println!("      Source: {}", source);
        }
    }
    println!();

    // Example 3: Similarity search with scores
    println!("🔍 Example 3: Similarity search with scores for 'journey'");
    let results_with_scores = store
        .similarity_search_with_score("journey", 3, None)
        .await?;
    println!("✅ Found {} results:", results_with_scores.len());
    for (i, (doc, score)) in results_with_scores.iter().enumerate() {
        println!("   {}. {} (score: {:.4})", i + 1, doc.page_content, score);
    }
    println!();

    // Example 4: Metadata filtering
    println!("🔍 Example 4: Search with metadata filter");
    let mut filter = HashMap::new();
    filter.insert("category".to_string(), json!("quotes"));

    let filtered_results = store._similarity_search("gold", 2, Some(&filter)).await?;
    println!(
        "✅ Found {} results with category='quotes':",
        filtered_results.len()
    );
    for (i, doc) in filtered_results.iter().enumerate() {
        println!("   {}. {}", i + 1, doc.page_content);
    }
    println!();

    // Example 5: Get documents by IDs
    println!("📄 Example 5: Get documents by IDs");
    let first_two_ids: Vec<String> = ids.iter().take(2).cloned().collect();
    let fetched_docs = store.get_by_ids(&first_two_ids).await?;
    println!("✅ Fetched {} documents:", fetched_docs.len());
    for doc in fetched_docs.iter() {
        println!("   {}", doc.page_content);
    }
    println!();

    // Example 6: Maximum Marginal Relevance search
    println!("🔍 Example 6: Maximum Marginal Relevance search");
    let mmr_results = store
        .max_marginal_relevance_search("wisdom", 3, 5, 0.5, None)
        .await?;
    println!("✅ Found {} diverse results:", mmr_results.len());
    for (i, doc) in mmr_results.iter().enumerate() {
        println!("   {}. {}", i + 1, doc.page_content);
    }
    println!();

    // Example 7: Delete specific documents
    println!("🗑️  Example 7: Delete specific documents");
    let ids_to_delete: Vec<String> = ids.iter().take(2).cloned().collect();
    store.delete(Some(&ids_to_delete)).await?;
    println!("✅ Deleted {} documents", ids_to_delete.len());

    // Verify deletion
    let remaining = store._similarity_search("", 10, None).await?;
    println!("   Remaining documents: {}", remaining.len());
    println!();

    // Example 8: Clean up - delete all documents
    println!("🗑️  Example 8: Delete all documents");
    store.delete(None).await?;
    println!("✅ All documents deleted");

    println!("\n✨ Example completed successfully!");

    Ok(())
}
