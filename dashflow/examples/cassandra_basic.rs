//! Basic Cassandra vector store example
//!
//! This example demonstrates how to use the Cassandra vector store for semantic search.
//!
//! ## Prerequisites
//!
//! 1. Apache Cassandra 5.0+ running on localhost:9042 OR DataStax Astra DB
//! 2. Keyspace and table created (see commands below)
//!
//! ## Setup
//!
//! ```bash
//! # Start Cassandra (if running locally)
//! cassandra -f
//! ```
//!
//! ```cql
//! -- Connect to Cassandra
//! cqlsh
//!
//! -- Create keyspace
//! CREATE KEYSPACE IF NOT EXISTS dashflow
//! WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};
//!
//! -- Create table with vector column
//! CREATE TABLE IF NOT EXISTS dashflow.vector_store (
//!     id UUID PRIMARY KEY,
//!     content TEXT,
//!     metadata TEXT,
//!     vector VECTOR<FLOAT, 3>
//! );
//!
//! -- Create vector index
//! CREATE INDEX IF NOT EXISTS idx_vector
//! ON dashflow.vector_store (vector)
//! WITH OPTIONS = {'similarity_function': 'cosine'};
//! ```
//!
//! ## Run
//!
//! ```bash
//! cargo run --example cassandra_basic
//! ```

use dashflow_cassandra::{CassandraVectorStore, SimilarityFunction};
use std::collections::HashMap;

/// Mock embeddings for demonstration (in production, use real embeddings)
struct MockEmbeddings;

impl MockEmbeddings {
    fn embed_texts(&self, texts: &[String]) -> Vec<Vec<f32>> {
        // Generate simple mock embeddings based on text length and content
        texts
            .iter()
            .map(|text| {
                let len = text.len() as f32;
                let char_sum = text.chars().map(|c| c as u32 as f32).sum::<f32>();
                // Normalize to unit vector
                let raw = vec![len / 100.0, char_sum / 1000.0, 0.5];
                let magnitude = (raw.iter().map(|x| x * x).sum::<f32>()).sqrt();
                raw.iter().map(|x| x / magnitude).collect()
            })
            .collect()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Cassandra Vector Store Example\n");

    // 1. Create vector store connection
    println!("📡 Connecting to Cassandra...");
    let store = CassandraVectorStore::builder()
        .contact_points(vec!["127.0.0.1:9042"])
        .keyspace("dashflow")
        .table("vector_store")
        .vector_dimension(3) // Using 3D vectors for demo
        .similarity_function(SimilarityFunction::Cosine)
        .build()
        .await?;

    println!("✅ Connected to Cassandra\n");

    // 2. Prepare sample documents
    let documents = vec![
        "The quick brown fox jumps over the lazy dog".to_string(),
        "A journey of a thousand miles begins with a single step".to_string(),
        "To be or not to be, that is the question".to_string(),
        "All that glitters is not gold".to_string(),
        "Where there is a will, there is a way".to_string(),
    ];

    let metadatas = Some(vec![
        HashMap::from([
            ("source".to_string(), serde_json::json!("proverb")),
            ("category".to_string(), serde_json::json!("animals")),
        ]),
        HashMap::from([
            ("source".to_string(), serde_json::json!("chinese_proverb")),
            ("category".to_string(), serde_json::json!("wisdom")),
        ]),
        HashMap::from([
            ("source".to_string(), serde_json::json!("shakespeare")),
            ("category".to_string(), serde_json::json!("literature")),
        ]),
        HashMap::from([
            ("source".to_string(), serde_json::json!("proverb")),
            ("category".to_string(), serde_json::json!("wisdom")),
        ]),
        HashMap::from([
            ("source".to_string(), serde_json::json!("proverb")),
            ("category".to_string(), serde_json::json!("motivation")),
        ]),
    ]);

    // 3. Generate embeddings (mock for demo)
    println!("🔢 Generating embeddings...");
    let embeddings_model = MockEmbeddings;
    let embeddings = embeddings_model.embed_texts(&documents);
    println!("✅ Generated {} embeddings\n", embeddings.len());

    // 4. Add documents to vector store
    println!("💾 Adding documents to Cassandra...");
    let ids = store
        .add_documents_with_embeddings(documents.clone(), embeddings.clone(), metadatas.clone())
        .await?;
    println!("✅ Added {} documents", ids.len());
    for (i, id) in ids.iter().enumerate() {
        println!("   [{}] ID: {}", i + 1, id);
    }
    println!();

    // 5. Perform similarity search
    println!("🔍 Searching for similar documents...");
    let query = "What is the meaning of existence?";
    println!("   Query: \"{}\"\n", query);

    let query_embedding = embeddings_model.embed_texts(&[query.to_string()]);
    let results = store
        .similarity_search_by_vector_with_score(&query_embedding[0], 3)
        .await?;

    println!("📊 Top {} results:", results.len());
    for (i, (doc, score)) in results.iter().enumerate() {
        println!("\n   [{}] Score: {:.4}", i + 1, score);
        println!("       Content: {}", doc.page_content);
        if let Some(source) = doc.metadata.get("source") {
            println!("       Source: {}", source);
        }
        if let Some(category) = doc.metadata.get("category") {
            println!("       Category: {}", category);
        }
    }
    println!();

    // 6. Get documents by IDs
    println!("🔎 Retrieving documents by ID...");
    let retrieved_docs = store.get_by_ids(vec![ids[0].clone()]).await?;
    println!("✅ Retrieved {} document(s)", retrieved_docs.len());
    for doc in &retrieved_docs {
        println!("   Content: {}", doc.page_content);
    }
    println!();

    // 7. Delete documents
    println!("🗑️  Deleting documents...");
    store.delete(ids.clone()).await?;
    println!("✅ Deleted {} documents\n", ids.len());

    println!("🎉 Example completed successfully!");

    Ok(())
}
