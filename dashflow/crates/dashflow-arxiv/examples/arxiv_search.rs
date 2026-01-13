//! Example: Basic arXiv search
//!
//! This example demonstrates how to use the ArxivSearchTool to search for
//! research papers on arXiv.
//!
//! Run with:
//! ```bash
//! cargo run --example arxiv_search
//! ```

use dashflow::core::tools::Tool;
use dashflow_arxiv::{ArxivSearchTool, SortBy};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== arXiv Search Tool Example ===\n");

    // Example 1: Basic search with default settings
    println!("Example 1: Search for 'quantum computing' papers");
    println!("-------------------------------------------------");
    let arxiv = ArxivSearchTool::new();
    match arxiv._call_str("quantum computing".to_string()).await {
        Ok(results) => println!("{}\n", results),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 2: Search with custom configuration
    println!("\nExample 2: Search for 'machine learning' (5 results, sorted by date)");
    println!("--------------------------------------------------------------------");
    let arxiv = ArxivSearchTool::builder()
        .max_results(5)
        .sort_by(SortBy::SubmittedDate)
        .build();

    match arxiv._call_str("machine learning".to_string()).await {
        Ok(results) => println!("{}\n", results),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 3: Search for a specific topic
    println!("\nExample 3: Search for 'neural architecture search'");
    println!("--------------------------------------------------");
    let arxiv = ArxivSearchTool::builder().max_results(2).build();

    match arxiv
        ._call_str("neural architecture search".to_string())
        .await
    {
        Ok(results) => println!("{}\n", results),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 4: Search for papers by a concept
    println!("\nExample 4: Search for 'transformer attention mechanism'");
    println!("-------------------------------------------------------");
    let arxiv = ArxivSearchTool::builder().max_results(3).build();

    match arxiv
        ._call_str("transformer attention mechanism".to_string())
        .await
    {
        Ok(results) => println!("{}\n", results),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    Ok(())
}
