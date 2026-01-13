//! Example demonstrating the Cohere Rerank API for document reranking
//!
//! This example shows how to use CohereRerank to reorder documents by
//! relevance to a query. This is particularly useful in RAG (Retrieval
//! Augmented Generation) pipelines to improve the quality of retrieved
//! documents.
//!
//! To run this example:
//! ```bash
//! export COHERE_API_KEY="your-api-key"
//! cargo run --example cohere_rerank
//! ```

use dashflow::core::documents::{Document, DocumentCompressor};
use dashflow_cohere::CohereRerank;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a reranker that returns top 2 documents
    let reranker = CohereRerank::new()
        .with_model("rerank-english-v3.0")
        .with_top_n(Some(2));

    // Sample documents about different topics
    let documents = vec![
        Document::new("Carson City is the capital city of the American state of Nevada."),
        Document::new("The Commonwealth of the Northern Mariana Islands is a group of islands in the Pacific Ocean. Its capital is Saipan."),
        Document::new("Capitalization or capitalisation in English grammar is the use of a capital letter at the start of a word. English usage varies from capitalization in other languages."),
        Document::new("Washington, D.C. (also known as simply Washington or D.C., and officially as the District of Columbia) is the capital of the United States. It is a federal district."),
    ];

    println!("Original documents:");
    for (i, doc) in documents.iter().enumerate() {
        println!("  {}. {}", i + 1, doc.page_content);
    }
    println!();

    // Query to rerank by
    let query = "What is the capital of the United States?";
    println!("Query: {}\n", query);

    // Rerank documents
    let reranked = reranker.compress_documents(documents, query, None).await?;

    println!("Reranked documents (top {}):", reranked.len());
    for (i, doc) in reranked.iter().enumerate() {
        let score = doc
            .metadata
            .get("relevance_score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        println!("  {}. [Score: {:.4}] {}", i + 1, score, doc.page_content);
    }

    Ok(())
}
