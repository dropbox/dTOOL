use dashflow::core::documents::{Document, DocumentCompressor};
use dashflow_jina::rerank::JinaRerank;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Note: Set JINA_API_KEY environment variable before running
    // Get your API key from: https://jina.ai/

    println!("Jina Rerank Example");
    println!("===================\n");

    // Create a set of documents about European capitals
    let documents = vec![
        Document::new("Paris is the capital and largest city of France."),
        Document::new("Berlin is the capital and largest city of Germany."),
        Document::new("The Eiffel Tower is a famous landmark in Paris, France."),
        Document::new("Madrid is the capital of Spain."),
        Document::new("Rome is the capital city of Italy."),
        Document::new("London is the capital of the United Kingdom."),
        Document::new("The weather is usually sunny in summer."),
        Document::new("Programming in Rust is memory-safe."),
    ];

    println!("Original documents:");
    for (i, doc) in documents.iter().enumerate() {
        println!("{}. {}", i + 1, doc.page_content);
    }
    println!();

    // Create Jina reranker with default settings (top_n=3)
    let reranker = JinaRerank::builder()
        .model("jina-reranker-v1-base-en".to_string())
        .top_n(Some(3)) // Return top 3 most relevant documents
        .build()?;

    // Query about France's capital
    let query = "What is the capital city of France?";
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
        println!("{}. [score: {:.4}] {}", i + 1, score, doc.page_content);
    }

    println!("\nExpected: Documents about Paris/France should be ranked highest");

    Ok(())
}
