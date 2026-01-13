//! Example: Nomic AI Embeddings
//!
//! This example demonstrates how to use Nomic AI's embedding models
//! for semantic similarity search.
//!
//! # Setup
//!
//! Set your Nomic API key:
//! ```bash
//! export NOMIC_API_KEY="nk_your_api_key_here"
//! ```
//!
//! Get your API key from: https://atlas.nomic.ai/
//!
//! # Run
//!
//! ```bash
//! cargo run --example nomic_embeddings --package dashflow-nomic
//! ```

use dashflow::{embed, embed_query};
use dashflow_nomic::NomicEmbeddings;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Nomic embeddings with default model (nomic-embed-text-v1.5)
    let embedder = Arc::new(NomicEmbeddings::new());

    println!("🔮 Nomic AI Embeddings Example\n");
    println!("Model: nomic-embed-text-v1.5");
    println!("Dimensions: 768\n");

    // Example 1: Embed a single query
    println!("Example 1: Single Query Embedding");
    println!("=====================================");
    let query = "What is artificial intelligence?";
    println!("Query: {}", query);

    let query_embedding = embed_query(Arc::clone(&embedder), query).await?;
    println!("Embedding dimension: {}", query_embedding.len());
    println!("First 5 values: {:?}\n", &query_embedding[..5]);

    // Example 2: Embed multiple documents
    println!("Example 2: Multiple Document Embeddings");
    println!("==========================================");
    let documents = vec![
        "Artificial intelligence is the simulation of human intelligence by machines.".to_string(),
        "Machine learning is a subset of AI that focuses on learning from data.".to_string(),
        "Deep learning uses neural networks with multiple layers.".to_string(),
        "The weather is nice today.".to_string(),
    ];

    println!("Documents:");
    for (i, doc) in documents.iter().enumerate() {
        println!("  {}. {}", i + 1, doc);
    }

    let doc_embeddings = embed(Arc::clone(&embedder), &documents).await?;
    println!("\nGenerated {} embeddings", doc_embeddings.len());

    // Example 3: Semantic similarity search
    println!("\nExample 3: Semantic Similarity Search");
    println!("========================================");
    println!("Query: {}", query);
    println!("Finding most similar document...\n");

    // Calculate cosine similarity between query and each document
    let similarities: Vec<f32> = doc_embeddings
        .iter()
        .map(|doc_emb| cosine_similarity(&query_embedding, doc_emb))
        .collect();

    // Find the most similar document
    let mut similarities_with_idx: Vec<_> = similarities.iter().enumerate().collect();
    similarities_with_idx.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

    println!("Similarity scores:");
    for (idx, similarity) in &similarities_with_idx {
        println!(
            "  Document {}: {:.4} - {}",
            idx + 1,
            similarity,
            documents[*idx]
        );
    }

    let most_similar_idx = similarities_with_idx[0].0;
    println!("\n✨ Most similar document:");
    println!("  {}", documents[most_similar_idx]);

    // Example 4: Task-specific embeddings
    println!("\n\nExample 4: Task-Specific Embeddings");
    println!("======================================");
    println!("Note: Nomic automatically uses the correct task type:");
    println!("  - embed_documents() uses 'search_document'");
    println!("  - embed_query() uses 'search_query'");
    println!("\nThis improves retrieval quality compared to generic embeddings.");

    // Example 5: Dimensionality reduction
    println!("\n\nExample 5: Matryoshka Embeddings (Dimensionality Reduction)");
    println!("==============================================================");
    let embedder_512 = Arc::new(NomicEmbeddings::new().with_dimensionality(512));

    println!("Model: nomic-embed-text-v1.5 with dimensionality=512");
    let reduced_embedding = embed_query(embedder_512, "Test").await?;
    println!("Embedding dimension: {}", reduced_embedding.len());
    println!("This saves storage while maintaining most semantic information.");

    Ok(())
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if magnitude_a == 0.0 || magnitude_b == 0.0 {
        0.0
    } else {
        dot_product / (magnitude_a * magnitude_b)
    }
}
