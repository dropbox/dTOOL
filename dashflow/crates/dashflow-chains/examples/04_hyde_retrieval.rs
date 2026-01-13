//! HyDE (Hypothetical Document Embeddings) Example
//!
//! Demonstrates the HyDE technique for improving retrieval quality.
//! Instead of embedding the query directly, HyDE:
//! 1. Uses an LLM to generate a hypothetical document that would answer the query
//! 2. Embeds that hypothetical document
//! 3. Uses it for similarity search
//!
//! This can improve retrieval because the hypothetical document is closer
//! to real documents in embedding space than the query is.
//!
//! Based on: https://arxiv.org/abs/2212.10496
//!
//! Run with:
//! ```bash
//! export OPENAI_API_KEY="your-key"
//! cargo run --package dashflow-chains --example 04_hyde_retrieval
//! ```

use dashflow_chains::HypotheticalDocumentEmbedder;
use dashflow_openai::{ChatOpenAI, OpenAIEmbeddings};
use std::error::Error;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("=== HyDE (Hypothetical Document Embeddings) Example ===\n");

    // Check for API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("Error: OPENAI_API_KEY environment variable not set");
        eprintln!("Please set your OpenAI API key:");
        eprintln!("  export OPENAI_API_KEY='your-key-here'");
        std::process::exit(1);
    }

    // 1. Create base components
    let chat_model = Arc::new(
        ChatOpenAI::default()
            .with_model("gpt-4o-mini")
            .with_temperature(0.7),
    );

    let embeddings = Arc::new(OpenAIEmbeddings::default());

    println!("Components initialized:");
    println!("  • LLM: gpt-4o-mini (for hypothetical doc generation)");
    println!("  • Embeddings: OpenAI text-embedding-ada-002");
    println!();

    // 2. Create HyDE embedder for web search
    println!("Creating HyDE embedder with 'web_search' prompt template...");
    let hyde_embedder = HypotheticalDocumentEmbedder::from_prompt_key(
        Arc::<ChatOpenAI>::clone(&chat_model),
        Arc::<OpenAIEmbeddings>::clone(&embeddings),
        "web_search",
    )?;

    println!("Prompt template: \"Please write a passage to answer the question\"");
    println!();

    // 3. Example queries
    let queries = [
        "What are the benefits of Rust's ownership system?",
        "How does async/await work in Rust?",
    ];

    for (i, query) in queries.iter().enumerate() {
        println!("=== Query {} ===", i + 1);
        println!("User query: {}", query);
        println!();

        // 3a. Traditional approach: embed query directly
        println!("Traditional approach:");
        println!("  Embedding query directly...");
        let query_embedding = dashflow::embed_query(Arc::clone(&embeddings), query).await?;
        println!("  ✓ Query embedding: {} dimensions", query_embedding.len());
        println!();

        // 3b. HyDE approach: generate hypothetical doc, then embed
        println!("HyDE approach:");
        println!("  Step 1: Generating hypothetical document with LLM...");
        let hyde_embedding = hyde_embedder.embed_query(query).await?;
        println!("  Step 2: Embedding hypothetical document...");
        println!("  ✓ HyDE embedding: {} dimensions", hyde_embedding.len());
        println!();

        // Compare embeddings (show that they're different)
        let cosine_similarity = compute_cosine_similarity(&query_embedding, &hyde_embedding);
        println!("Similarity between approaches: {:.4}", cosine_similarity);
        println!("(< 1.0 means embeddings differ, which is expected)\n");
    }

    // 4. Demonstrate different prompt templates
    println!("=== Different Use Cases ===\n");
    println!("HyDE provides specialized prompts for different domains:");
    println!();

    let use_cases = vec![
        ("web_search", "General web search queries"),
        ("sci_fact", "Scientific fact verification"),
        ("fiqa", "Financial question answering"),
        ("trec_news", "News topic exploration"),
    ];

    for (prompt_key, description) in use_cases {
        println!("• {}: {}", prompt_key, description);
    }

    println!();
    println!("=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!("  • HyDE generates hypothetical documents that answer queries");
    println!("  • Hypothetical docs are closer to real docs in embedding space");
    println!("  • Can improve retrieval quality, especially for complex queries");
    println!("  • Trades off: extra LLM call for potentially better retrieval");
    println!("  • Best for: semantic search, question answering, fact verification");

    Ok(())
}

/// Compute cosine similarity between two vectors
fn compute_cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if magnitude_a == 0.0 || magnitude_b == 0.0 {
        return 0.0;
    }

    dot_product / (magnitude_a * magnitude_b)
}
