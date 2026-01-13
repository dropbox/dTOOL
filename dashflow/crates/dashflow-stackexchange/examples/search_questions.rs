//! Example: Search for questions on Stack Overflow
//!
//! This example demonstrates how to search for questions on Stack Overflow
//! using the StackExchangeSearchTool.
//!
//! Usage:
//! ```bash
//! cargo run --example search_questions
//! ```

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_stackexchange::StackExchangeSearchTool;
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Stack Exchange Search Example ===\n");

    // Create the search tool for Stack Overflow
    let tool = StackExchangeSearchTool::new("stackoverflow".to_string());

    // Example 1: Search for Rust async questions
    println!("Example 1: Searching for 'rust async trait'...\n");
    let input = json!({
        "query": "rust async trait",
        "max_results": "3",
        "sort": "votes"
    });

    match tool._call(ToolInput::Structured(input)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 2: Search for error handling in Rust
    println!("Example 2: Searching for 'rust error handling'...\n");
    let input = json!({
        "query": "rust error handling",
        "max_results": "5",
        "sort": "relevance"
    });

    match tool._call(ToolInput::Structured(input)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 3: Search on a different Stack Exchange site
    println!("Example 3: Searching Server Fault for 'nginx configuration'...\n");
    let tool = StackExchangeSearchTool::new("serverfault".to_string());
    let input = json!({
        "query": "nginx configuration",
        "max_results": "3"
    });

    match tool._call(ToolInput::Structured(input)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    println!("=== Examples Complete ===");

    Ok(())
}
