//! Basic Serper Search example
//!
//! This example demonstrates basic usage of the Google Serper search tool.
//!
//! To run this example:
//! ```bash
//! export SERPER_API_KEY="your-api-key"
//! cargo run --example serper_search
//! ```

use dashflow::core::tools::Tool;
use dashflow_serper::SerperTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment
    let api_key = match std::env::var("SERPER_API_KEY") {
        Ok(api_key) => api_key,
        Err(_) => {
            println!("SERPER_API_KEY environment variable not set.");
            println!("Run: export SERPER_API_KEY=\"your-api-key\"");
            return Ok(());
        }
    };

    // Create the search tool
    let serper = SerperTool::builder()
        .api_key(api_key)
        .num_results(5)
        .build()?;

    println!("=== Google Serper Search Tool Demo ===\n");
    println!("Tool name: {}", serper.name());
    println!("Tool description: {}\n", serper.description());

    // Example 1: General search
    println!("--- Example 1: General Search ---");
    let query1 = "What is Rust programming language?";
    println!("Query: {}\n", query1);

    let results1 = serper._call_str(query1.to_string()).await?;
    println!("{}\n", results1);

    // Example 2: Tech search with knowledge graph
    println!("--- Example 2: Search with Knowledge Graph ---");
    let query2 = "Elon Musk";
    println!("Query: {}\n", query2);

    let results2 = serper._call_str(query2.to_string()).await?;
    println!("{}\n", results2);

    // Example 3: Location-specific search
    println!("--- Example 3: Location-Specific Search ---");
    let serper_uk = SerperTool::builder()
        .api_key(std::env::var("SERPER_API_KEY")?)
        .num_results(3)
        .location("United Kingdom")
        .language("en")
        .build()?;

    let query3 = "best restaurants near me";
    println!("Query: {} (location: United Kingdom)\n", query3);

    let results3 = serper_uk._call_str(query3.to_string()).await?;
    println!("{}\n", results3);

    Ok(())
}
