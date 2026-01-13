//! # Supabase Vector Store Example
//!
//! This example demonstrates how to use SupabaseVectorStore for storing
//! and searching document embeddings in Supabase (PostgreSQL + pgvector).
//!
//! **Prerequisites:**
//! - Create a Supabase project at https://supabase.com
//! - Enable pgvector extension in your project (SQL Editor):
//!   ```sql
//!   CREATE EXTENSION IF NOT EXISTS vector;
//!   ```
//! - Get your connection string from Project Settings → Database → Connection string
//!
//! **Run this example:**
//! ```bash
//! # Set your connection details
//! export SUPABASE_CONNECTION_STRING="postgresql://postgres.[PROJECT_ID].supabase.co:5432/postgres"
//! export SUPABASE_PASSWORD="your_password"
//!
//! cargo run --package dashflow-supabase --example supabase_basic
//! ```
//!
//! Covers:
//! - Creating a Supabase vector store with collection
//! - Adding documents with metadata
//! - Similarity search with scores
//! - Metadata filtering
//! - CRUD operations

use async_trait::async_trait;
use dashflow::core::{
    documents::Document, embeddings::Embeddings, vector_stores::VectorStore, Error,
};
use dashflow_supabase::SupabaseVectorStore;
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
    println!("=== Supabase Vector Store Example ===\n");
    println!("Note: This requires a Supabase project with pgvector extension");
    println!("Visit https://supabase.com to create a project\n");

    // Get connection details from environment
    let connection_string = std::env::var("SUPABASE_CONNECTION_STRING").unwrap_or_else(|_| {
        "postgresql://postgres.[PROJECT_ID].supabase.co:5432/postgres".to_string()
    });
    let password =
        std::env::var("SUPABASE_PASSWORD").unwrap_or_else(|_| "your_password".to_string());

    if connection_string.contains("[PROJECT_ID]") || password == "your_password" {
        println!(
            "⚠️  Please set SUPABASE_CONNECTION_STRING and SUPABASE_PASSWORD environment variables"
        );
        println!("   Example:");
        println!("   export SUPABASE_CONNECTION_STRING=\"postgresql://postgres.abcdef123456.supabase.co:5432/postgres\"");
        println!("   export SUPABASE_PASSWORD=\"your_password\"");
        return Ok(());
    }

    // Create embeddings and connect to Supabase
    let embeddings: Arc<dyn Embeddings> = Arc::new(DemoEmbeddings);
    let collection_name = "dashflow_demo";

    println!(
        "Connecting to Supabase and creating collection '{}'...",
        collection_name
    );
    let mut store = SupabaseVectorStore::new(
        &connection_string,
        &password,
        collection_name,
        Arc::clone(&embeddings),
    )
    .await?;
    println!("Connected successfully!\n");

    // Clean up any existing data from previous runs
    println!("Cleaning up previous data...");
    // Note: delete_all not implemented in base trait, so we skip this
    println!("Not performed (would delete all documents)\n");

    // 1. Add documents with metadata
    println!("1. Adding documents with metadata...");
    let documents = [
        Document {
            page_content: "The quick brown fox jumps over the lazy dog".to_string(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("category".to_string(), serde_json::json!("animals"));
                m.insert("id".to_string(), serde_json::json!(1));
                m
            },
            id: None,
        },
        Document {
            page_content: "Machine learning is a subset of artificial intelligence".to_string(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("category".to_string(), serde_json::json!("technology"));
                m.insert("id".to_string(), serde_json::json!(2));
                m
            },
            id: None,
        },
        Document {
            page_content: "The Eiffel Tower is located in Paris, France".to_string(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("category".to_string(), serde_json::json!("geography"));
                m.insert("id".to_string(), serde_json::json!(3));
                m
            },
            id: None,
        },
    ];

    // Convert documents to texts and metadatas
    let texts: Vec<String> = documents.iter().map(|d| d.page_content.clone()).collect();
    let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
    let metadatas: Vec<HashMap<String, serde_json::Value>> =
        documents.iter().map(|d| d.metadata.clone()).collect();

    let ids = store.add_texts(&text_refs, Some(&metadatas), None).await?;
    println!("   Added {} documents with IDs: {:?}\n", ids.len(), ids);

    // 2. Similarity search
    println!("2. Similarity search for 'dogs and cats'...");
    let results = store._similarity_search("dogs and cats", 2, None).await?;
    for (i, doc) in results.iter().enumerate() {
        println!("   Result {}: {}", i + 1, doc.page_content);
        println!("   Metadata: {:?}\n", doc.metadata);
    }

    // 3. Similarity search with scores
    println!("3. Similarity search with scores for 'artificial intelligence'...");
    let results_with_scores = store
        .similarity_search_with_score("artificial intelligence", 2, None)
        .await?;
    for (i, (doc, score)) in results_with_scores.iter().enumerate() {
        println!("   Result {}: score={:.4}", i + 1, score);
        println!("   Text: {}", doc.page_content);
        println!("   Metadata: {:?}\n", doc.metadata);
    }

    // 4. Metadata filtering
    println!("4. Search with metadata filter (category=technology)...");
    let filter = {
        let mut f = HashMap::new();
        f.insert("category".to_string(), serde_json::json!("technology"));
        f
    };
    let filtered_results = store
        ._similarity_search("information", 5, Some(&filter))
        .await?;
    println!(
        "   Found {} documents in 'technology' category",
        filtered_results.len()
    );
    for (i, doc) in filtered_results.iter().enumerate() {
        println!("   Result {}: {}", i + 1, doc.page_content);
        println!("   Metadata: {:?}\n", doc.metadata);
    }

    // 5. Get documents by IDs
    println!("5. Get documents by IDs...");
    if !ids.is_empty() {
        let id_slice = vec![ids[0].clone()];
        let retrieved = store.get_by_ids(&id_slice).await?;
        println!("   Retrieved {} documents", retrieved.len());
        for doc in retrieved {
            println!("   {}", doc.page_content);
        }
        println!();
    }

    // 6. Delete documents
    println!("6. Deleting documents...");
    if !ids.is_empty() {
        let id_to_delete = &ids[0];
        store
            .delete(Some(std::slice::from_ref(id_to_delete)))
            .await?;
        println!("   Deleted document with ID: {}", id_to_delete);
    }

    println!("\n✅ Example completed successfully!");
    println!("\nNote: Data remains in your Supabase collection.");
    println!("Clean up in Supabase dashboard if needed.");

    Ok(())
}
