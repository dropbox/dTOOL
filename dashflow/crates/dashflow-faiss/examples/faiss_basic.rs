//! DashFlow.
//!
//! This example demonstrates:
//! - Creating a FAISS vector store
//! - Adding documents
//! - Performing similarity search
//! - Using metadata filtering
//!
//! # Prerequisites
//!
//! Install FAISS library:
//! - macOS: `brew install faiss`
//! - Ubuntu: `apt-get install libfaiss-dev`
//!
//! # Running
//!
//! ```bash
//! cargo run --example faiss_basic
//! ```

use dashflow::core::documents::Document;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::vector_stores::VectorStore;
use dashflow_faiss::FaissVectorStore;
use std::collections::HashMap;
use std::sync::Arc;

// Simple mock embeddings for demonstration
struct MockEmbeddings {
    dimension: usize,
}

#[async_trait::async_trait]
impl Embeddings for MockEmbeddings {
    async fn _embed_documents(
        &self,
        texts: &[String],
    ) -> dashflow::core::Result<Vec<Vec<f32>>> {
        // Generate simple embeddings based on text length and content
        Ok(texts
            .iter()
            .map(|text| {
                let s = text.as_str();
                let mut vec = vec![0.0; self.dimension];

                // Simple hash-based embedding (not production quality!)
                for (i, c) in s.chars().enumerate() {
                    vec[i % self.dimension] += (c as u32 as f32) / 1000.0;
                }

                // Normalize
                let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 1e-10 {
                    vec.iter_mut().for_each(|x| *x /= norm);
                }

                vec
            })
            .collect())
    }

    async fn _embed_query(&self, text: &str) -> dashflow::core::Result<Vec<f32>> {
        let results = self._embed_documents(&[text.to_string()]).await?;
        Ok(results.into_iter().next().unwrap())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== FAISS Vector Store Example ===\n");

    // Create embeddings (384-dimensional)
    let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings { dimension: 384 });

    // Create FAISS vector store with a flat index
    println!("Creating FAISS vector store with Flat index...");
    let mut store = FaissVectorStore::new(
        embeddings.clone(),
        384,
        "Flat", // Use exact search for this example
    )
    .await?;

    // Add some sample documents
    println!("\nAdding documents...");
    let texts = vec![
        "The quick brown fox jumps over the lazy dog",
        "FAISS is a library for efficient similarity search",
        "Rust is a systems programming language",
        "Vector databases are useful for semantic search",
        "Machine learning models can generate embeddings",
    ];

    let mut metadatas = Vec::new();
    for (i, _) in texts.iter().enumerate() {
        let mut meta = HashMap::new();
        meta.insert("index".to_string(), serde_json::json!(i));
        meta.insert(
            "category".to_string(),
            if i < 2 {
                serde_json::json!("example")
            } else {
                serde_json::json!("tech")
            },
        );
        metadatas.push(meta);
    }

    let ids = store.add_texts(&texts, Some(&metadatas), None).await?;
    println!("Added {} documents with IDs: {:?}", ids.len(), ids);

    // Perform similarity search
    println!("\n--- Similarity Search ---");
    let query = "information about programming languages";
    println!("Query: '{}'", query);

    let results = store._similarity_search(query, 3, None).await?;
    println!("\nTop 3 results:");
    for (i, doc) in results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
        if let Some(category) = doc.metadata.get("category") {
            println!("     Category: {}", category);
        }
    }

    // Search with scores
    println!("\n--- Search with Scores ---");
    let results_with_scores = store
        .similarity_search_with_score(query, 3, None)
        .await?;
    for (i, (doc, score)) in results_with_scores.iter().enumerate() {
        println!("  {}. [Score: {:.4}] {}", i + 1, score, doc.page_content);
    }

    // Search with metadata filtering
    println!("\n--- Search with Metadata Filter ---");
    let mut filter = HashMap::new();
    filter.insert("category".to_string(), serde_json::json!("tech"));

    let filtered_results = store._similarity_search(query, 3, Some(&filter)).await?;
    println!("Results (category='tech' only):");
    for (i, doc) in filtered_results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
    }

    // Get documents by ID
    println!("\n--- Get by IDs ---");
    let retrieved = store.get_by_ids(&ids[0..2]).await?;
    println!("Retrieved {} documents by ID", retrieved.len());
    for doc in retrieved {
        println!("  - {}", doc.page_content);
    }

    // Maximum Marginal Relevance search
    println!("\n--- MMR Search (diverse results) ---");
    let mmr_results = store
        .max_marginal_relevance_search(
            "technology and computing",
            3,    // k: number of results
            5,    // fetch_k: candidates to consider
            0.5,  // lambda: balance relevance vs diversity (0.5 = equal weight)
            None,
        )
        .await?;
    println!("MMR results (more diverse):");
    for (i, doc) in mmr_results.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
    }

    // Delete a document
    println!("\n--- Delete Operation ---");
    let delete_id = &ids[0];
    println!("Deleting document with ID: {}", delete_id);
    let deleted = store.delete(Some(&[delete_id.clone()])).await?;
    println!("Deletion successful: {}", deleted);

    // Verify deletion
    let remaining = store.get_by_ids(&[delete_id.clone()]).await?;
    println!("Documents remaining with that ID: {}", remaining.len());

    println!("\n=== Example Complete ===");
    Ok(())
}
