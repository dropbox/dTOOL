//! Example demonstrating Reddit search functionality.
//!
//! This example shows how to search Reddit posts with various filters.
//!
//! Run with: cargo run --example search_posts

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_reddit::RedditSearchTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Reddit Search Tool Examples ===\n");

    let tool = RedditSearchTool::new();

    // Example 1: Basic search
    println!("Example 1: Basic search for 'rust programming'\n");
    let input1 = serde_json::json!({
        "query": "rust programming",
        "limit": 3
    });

    match tool._call(ToolInput::Structured(input1)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 2: Search within specific subreddit
    println!("Example 2: Search for 'async' in r/rust\n");
    let input2 = serde_json::json!({
        "query": "async",
        "subreddit": "rust",
        "limit": 3,
        "sort": "top"
    });

    match tool._call(ToolInput::Structured(input2)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 3: Search with time filter
    println!("Example 3: Top posts about 'tokio' this week\n");
    let input3 = serde_json::json!({
        "query": "tokio",
        "limit": 2,
        "sort": "top",
        "time": "week"
    });

    match tool._call(ToolInput::Structured(input3)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 4: Simple string input
    println!("Example 4: Simple string search\n");
    match tool._call(ToolInput::String("dashflow".to_string())).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    Ok(())
}
