//! Basic example of using HuggingFace embeddings.
//!
//! This example demonstrates how to use the HuggingFaceEmbeddings struct
//! to generate embeddings for text using HuggingFace Hub's Inference API.
//!
//! # Setup
//!
//! Set your HuggingFace API token:
//! ```bash
//! export HUGGINGFACEHUB_API_TOKEN=your_token_here
//! ```
//!
//! Or alternatively:
//! ```bash
//! export HF_TOKEN=your_token_here
//! ```
//!
//! You can get a token from https://huggingface.co/settings/tokens
//!
//! # Run
//! ```bash
//! cargo run --example embeddings_basic
//! ```

use dashflow::{embed, embed_query};
use dashflow_huggingface::HuggingFaceEmbeddings;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("HuggingFace Embeddings Example");
    println!("================================\n");

    // Create embeddings instance with default model (sentence-transformers/all-mpnet-base-v2)
    let embedder = Arc::new(HuggingFaceEmbeddings::new());

    // Embed a single query
    println!("Embedding a single query...");
    let query = "What is the meaning of life?";
    let query_embedding = embed_query(Arc::<HuggingFaceEmbeddings>::clone(&embedder), query).await?;
    println!("Query: {}", query);
    println!(
        "Embedding dimension: {} (first 5 values: {:?})",
        query_embedding.len(),
        &query_embedding[..5]
    );
    println!();

    // Embed multiple documents
    println!("Embedding multiple documents...");
    let documents = vec![
        "The quick brown fox jumps over the lazy dog.".to_string(),
        "Artificial intelligence is transforming our world.".to_string(),
        "Rust is a systems programming language.".to_string(),
    ];

    let doc_embeddings = embed(Arc::<HuggingFaceEmbeddings>::clone(&embedder), &documents).await?;
    println!("Embedded {} documents", doc_embeddings.len());
    for (i, (doc, embedding)) in documents.iter().zip(doc_embeddings.iter()).enumerate() {
        println!(
            "  Document {}: \"{}\" -> {} dimensions",
            i + 1,
            doc,
            embedding.len()
        );
    }
    println!();

    // Using a different model
    println!("Using a different model (all-MiniLM-L6-v2)...");
    let small_embedder =
        Arc::new(HuggingFaceEmbeddings::new().with_model("sentence-transformers/all-MiniLM-L6-v2"));

    let small_embedding = embed_query(small_embedder, query).await?;
    println!("Query: {}", query);
    println!(
        "Embedding dimension: {} (first 5 values: {:?})",
        small_embedding.len(),
        &small_embedding[..5]
    );
    println!("\nNote: all-MiniLM-L6-v2 produces 384-dimensional embeddings (vs 768 for all-mpnet-base-v2)");

    Ok(())
}
