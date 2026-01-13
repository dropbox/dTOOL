// OpenAI embeddings example
//
// Run with: cargo run -p dashflow-openai --example embeddings
//
// Required environment variable:
// - OPENAI_API_KEY: Your OpenAI API key

use dashflow::{embed, embed_query};
use dashflow::core::config_loader::{EmbeddingConfig, SecretReference};
use dashflow_openai::build_embeddings;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpenAI Embeddings Example ===\n");

    let config = EmbeddingConfig::OpenAI {
        model: "text-embedding-3-small".to_string(),
        api_key: SecretReference::from_env("OPENAI_API_KEY"),
        batch_size: 32,
    };
    let embeddings = build_embeddings(&config)?;

    // Single text embedding
    let text = "Rust is a systems programming language.";
    let embedding = embed_query(std::sync::Arc::clone(&embeddings), text).await?;

    println!("Text: {}", text);
    println!("Embedding dimensions: {}", embedding.len());
    println!("First 5 values: {:?}\n", &embedding[..5]);

    // Batch embeddings
    let texts = vec![
        "Rust is a systems programming language.".to_string(),
        "Python is a high-level programming language.".to_string(),
        "JavaScript is used for web development.".to_string(),
    ];

    let batch_embeddings = embed(std::sync::Arc::clone(&embeddings), &texts).await?;
    println!("Batch embedded {} texts", batch_embeddings.len());

    // Compute similarity
    let similarity = cosine_similarity(&batch_embeddings[0], &batch_embeddings[1]);
    println!(
        "Similarity between Rust and Python texts: {:.4}",
        similarity
    );

    println!("\n✅ Embeddings example complete");
    Ok(())
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (mag_a * mag_b)
}
