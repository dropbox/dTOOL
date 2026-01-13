// Ollama embeddings example
//
// Run with: cargo run -p dashflow-ollama --example ollama_embeddings
//
// Prerequisites:
// 1. Install Ollama: https://ollama.ai
// 2. Pull an embedding model: ollama pull nomic-embed-text
// 3. Ensure Ollama is running: ollama serve

use dashflow::{embed, embed_query};
use dashflow_ollama::OllamaEmbeddings;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Ollama Embeddings Example ===\n");

    // Create embeddings client with nomic-embed-text model
    // Other good options: mxbai-embed-large (1024-dim), all-minilm (384-dim)
    let embeddings = Arc::new(OllamaEmbeddings::new().with_model("nomic-embed-text"));

    // Single text embedding
    let text = "Rust is a systems programming language.";
    println!("Generating embedding for: {}", text);
    let embedding = embed_query(Arc::<OllamaEmbeddings>::clone(&embeddings), text).await?;

    println!("✓ Embedding dimensions: {}", embedding.len());
    println!("✓ First 5 values: {:?}\n", &embedding[..5]);

    // Batch embeddings
    let texts = vec![
        "Rust is a systems programming language.".to_string(),
        "Python is a high-level programming language.".to_string(),
        "JavaScript is used for web development.".to_string(),
        "Go is designed for concurrent programming.".to_string(),
    ];

    println!("Generating batch embeddings for {} texts...", texts.len());
    let batch_embeddings = embed(Arc::<OllamaEmbeddings>::clone(&embeddings), &texts).await?;
    println!("✓ Batch embedded {} texts\n", batch_embeddings.len());

    // Compute similarity between different language descriptions
    println!("Computing semantic similarities:");
    let rust_python_sim = cosine_similarity(&batch_embeddings[0], &batch_embeddings[1]);
    let rust_go_sim = cosine_similarity(&batch_embeddings[0], &batch_embeddings[3]);
    let python_js_sim = cosine_similarity(&batch_embeddings[1], &batch_embeddings[2]);

    println!("  • Rust ↔ Python: {:.4}", rust_python_sim);
    println!("  • Rust ↔ Go:     {:.4}", rust_go_sim);
    println!("  • Python ↔ JS:   {:.4}", python_js_sim);

    // Semantic search example
    println!("\n=== Semantic Search Example ===\n");
    let query = "concurrent programming";
    let query_embedding = embed_query(Arc::<OllamaEmbeddings>::clone(&embeddings), query).await?;

    println!("Query: '{}'", query);
    println!("\nRanking documents by relevance:");

    let mut similarities: Vec<(usize, f32)> = batch_embeddings
        .iter()
        .enumerate()
        .map(|(i, doc_embedding)| (i, cosine_similarity(&query_embedding, doc_embedding)))
        .collect();

    // Sort by similarity (highest first)
    similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    for (rank, (idx, sim)) in similarities.iter().enumerate() {
        println!("  {}. [Score: {:.4}] {}", rank + 1, sim, texts[*idx]);
    }

    println!("\n✅ Ollama embeddings example complete");
    println!("\n💡 Tip: Ollama runs locally, so your data never leaves your machine!");
    Ok(())
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (mag_a * mag_b)
}
