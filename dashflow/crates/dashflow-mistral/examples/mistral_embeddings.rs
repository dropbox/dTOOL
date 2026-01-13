//! Example demonstrating Mistral AI embeddings
//!
//! This example shows how to use Mistral's embedding models to convert text into
//! vector embeddings that can be used for semantic search, clustering, and other
//! machine learning tasks.
//!
//! # Prerequisites
//!
//! Set your Mistral API key as an environment variable:
//!
//! ```bash
//! export MISTRAL_API_KEY=your-api-key-here
//! ```
//!
//! Get your API key from: https://console.mistral.ai/
//!
//! # Running the Example
//!
//! ```bash
//! cargo run --example mistral_embeddings
//! ```

use dashflow::{embed, embed_query};
use dashflow_mistral::MistralEmbeddings;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the embeddings provider
    // This reads the API key from the MISTRAL_API_KEY environment variable
    let embeddings = Arc::new(MistralEmbeddings::new());

    println!("=== Mistral AI Embeddings Example ===\n");

    // Example 1: Embed a single query
    println!("1. Embedding a single query:");
    let query = "What is the capital of France?";
    let query_embedding = embed_query(Arc::clone(&embeddings), query).await?;

    println!("   Query: {}", query);
    println!("   Embedding dimensions: {}", query_embedding.len());
    println!(
        "   First 5 values: {:?}",
        &query_embedding[..5.min(query_embedding.len())]
    );
    println!();

    // Example 2: Embed multiple documents
    println!("2. Embedding multiple documents:");
    let documents = vec![
        "Paris is the capital of France.".to_string(),
        "Berlin is the capital of Germany.".to_string(),
        "Madrid is the capital of Spain.".to_string(),
        "Rome is the capital of Italy.".to_string(),
    ];

    let doc_embeddings = embed(Arc::clone(&embeddings), &documents).await?;

    println!("   Number of documents: {}", documents.len());
    println!("   Number of embeddings: {}", doc_embeddings.len());
    println!("   Dimensions per embedding: {}", doc_embeddings[0].len());
    println!();

    // Example 3: Calculate similarity between query and documents
    println!("3. Calculating similarity between query and documents:");
    println!("   Query: {}", query);
    println!();

    for (i, (doc, doc_embedding)) in documents.iter().zip(doc_embeddings.iter()).enumerate() {
        let similarity = cosine_similarity(&query_embedding, doc_embedding);
        println!("   Document {}: \"{}\"", i + 1, doc);
        println!("   Similarity: {:.4}", similarity);
        println!();
    }

    // Example 4: Find most similar document
    println!("4. Finding most similar document:");
    let Some((most_similar_idx, max_similarity)) = doc_embeddings
        .iter()
        .enumerate()
        .map(|(i, emb)| (i, cosine_similarity(&query_embedding, emb)))
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    else {
        println!("   No documents to compare.");
        return Ok(());
    };

    println!("   Query: {}", query);
    println!(
        "   Most similar document: \"{}\"",
        documents[most_similar_idx]
    );
    println!("   Similarity score: {:.4}", max_similarity);
    println!();

    // Example 5: Semantic search with different query
    println!("5. Semantic search with different query:");
    let search_query = "European countries and their capitals";
    let search_embedding = embed_query(Arc::clone(&embeddings), search_query).await?;

    println!("   Search query: {}", search_query);
    println!();

    let mut similarities: Vec<_> = doc_embeddings
        .iter()
        .enumerate()
        .map(|(i, emb)| (i, cosine_similarity(&search_embedding, emb)))
        .collect();

    // Sort by similarity (highest first)
    similarities.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    println!("   Results (sorted by relevance):");
    for (rank, (idx, sim)) in similarities.iter().enumerate() {
        println!(
            "   {}. \"{}\" (score: {:.4})",
            rank + 1,
            documents[*idx],
            sim
        );
    }

    Ok(())
}

/// Calculate cosine similarity between two vectors
///
/// Cosine similarity ranges from -1 (completely dissimilar) to 1 (identical).
/// For normalized vectors, this is equivalent to the dot product.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    dot_product / (magnitude_a * magnitude_b)
}
