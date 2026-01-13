//! Basic Brave Search example
//!
//! This example demonstrates basic usage of the Brave Search tool.
//!
//! To run this example:
//! ```bash
//! export BRAVE_API_KEY="your-api-key"
//! cargo run --example brave_search
//! ```

use dashflow::core::tools::Tool;
use dashflow_brave::BraveSearchTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get API key from environment
    let api_key = match std::env::var("BRAVE_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("BRAVE_API_KEY environment variable not set.");
            println!("Run: export BRAVE_API_KEY=\"your-api-key\"");
            return Ok(());
        }
    };

    // Create the search tool
    let brave = BraveSearchTool::builder()
        .api_key(&api_key)
        .count(5)
        .build()?;

    println!("=== Brave Search Tool Demo ===\n");
    println!("Tool name: {}", brave.name());
    println!("Tool description: {}\n", brave.description());

    // Example 1: General search
    println!("--- Example 1: General Search ---");
    let query1 = "What is Rust programming language?";
    println!("Query: {}\n", query1);

    let results1 = brave._call_str(query1.to_string()).await?;
    println!("{}\n", results1);

    // Example 2: Recent news
    println!("--- Example 2: Recent Technology News ---");
    let brave_fresh = BraveSearchTool::builder()
        .api_key(&api_key)
        .count(3)
        .freshness("pw") // Past week
        .build()?;

    let query2 = "latest AI developments";
    println!("Query: {} (freshness: past week)\n", query2);

    let results2 = brave_fresh._call_str(query2.to_string()).await?;
    println!("{}\n", results2);

    // Example 3: Structured input
    println!("--- Example 3: Structured Input ---");
    let structured_input = serde_json::json!({
        "query": "quantum computing applications"
    });

    let results3 = brave
        ._call(dashflow::core::tools::ToolInput::Structured(
            structured_input,
        ))
        .await?;
    println!("{}\n", results3);

    Ok(())
}
