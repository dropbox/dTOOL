//! Basic Bing Search example
//!
//! This example demonstrates basic usage of the Bing Search tool.
//!
//! To run this example:
//! ```bash
//! export BING_SUBSCRIPTION_KEY="your-subscription-key"
//! cargo run --example bing_search
//! ```

use dashflow::core::tools::Tool;
use dashflow_bing::BingSearchTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get subscription key from environment
    let subscription_key = match std::env::var("BING_SUBSCRIPTION_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("BING_SUBSCRIPTION_KEY environment variable not set.");
            println!("Run: export BING_SUBSCRIPTION_KEY=\"your-subscription-key\"");
            return Ok(());
        }
    };

    // Create the search tool
    let bing = BingSearchTool::builder()
        .subscription_key(&subscription_key)
        .count(5)
        .build()?;

    println!("=== Bing Search Tool Demo ===\n");
    println!("Tool name: {}", bing.name());
    println!("Tool description: {}\n", bing.description());

    // Example 1: General search
    println!("--- Example 1: General Search ---");
    let query1 = "What is machine learning?";
    println!("Query: {}\n", query1);

    let results1 = bing._call_str(query1.to_string()).await?;
    println!("{}\n", results1);

    // Example 2: Recent news
    println!("--- Example 2: Recent News ---");
    let bing_fresh = BingSearchTool::builder()
        .subscription_key(&subscription_key)
        .count(3)
        .freshness("Week")
        .build()?;

    let query2 = "latest technology news";
    println!("Query: {} (freshness: Week)\n", query2);

    let results2 = bing_fresh._call_str(query2.to_string()).await?;
    println!("{}\n", results2);

    // Example 3: Different market
    println!("--- Example 3: Spanish Market ---");
    let bing_es = BingSearchTool::builder()
        .subscription_key(&subscription_key)
        .count(3)
        .market("es-ES")
        .build()?;

    let query3 = "inteligencia artificial";
    println!("Query: {} (market: es-ES)\n", query3);

    let results3 = bing_es._call_str(query3.to_string()).await?;
    println!("{}\n", results3);

    Ok(())
}
