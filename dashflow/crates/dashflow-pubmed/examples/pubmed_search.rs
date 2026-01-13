//! Basic PubMed search example
//!
//! This example demonstrates how to search PubMed for medical and scientific literature.
//!
//! Run this example:
//! ```bash
//! cargo run --example pubmed_search
//! ```

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_pubmed::PubMedSearch;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== PubMed Search Tool Example ===\n");

    // Create a PubMed search tool
    let tool = PubMedSearch::new().max_results(3).build();

    // Example 1: Search for CRISPR articles
    println!("Example 1: Searching for 'CRISPR gene editing'...\n");
    match tool
        ._call(ToolInput::String("CRISPR gene editing".to_string()))
        .await
    {
        Ok(results) => println!("{}", results),
        Err(e) => eprintln!("Error: {}", e),
    }

    println!("\n{}\n", "=".repeat(80));

    // Example 2: Search for COVID-19 vaccine research
    println!("Example 2: Searching for 'COVID-19 vaccine'...\n");
    match tool
        ._call(ToolInput::String("COVID-19 vaccine".to_string()))
        .await
    {
        Ok(results) => println!("{}", results),
        Err(e) => eprintln!("Error: {}", e),
    }

    println!("\n{}\n", "=".repeat(80));

    // Example 3: Advanced search with Boolean operators
    let advanced_tool = PubMedSearch::builder()
        .max_results(2)
        .sort_by("pub_date")
        .build();

    println!("Example 3: Advanced search with Boolean operators...\n");
    match advanced_tool
        ._call(ToolInput::String(
            "(cancer OR tumor) AND immunotherapy".to_string(),
        ))
        .await
    {
        Ok(results) => println!("{}", results),
        Err(e) => eprintln!("Error: {}", e),
    }

    println!("\n{}\n", "=".repeat(80));

    // Example 4: Field-specific search
    println!("Example 4: Searching in specific fields...\n");
    match tool
        ._call(ToolInput::String(
            "stem cells[Title] AND 2023[Publication Date]".to_string(),
        ))
        .await
    {
        Ok(results) => println!("{}", results),
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}
