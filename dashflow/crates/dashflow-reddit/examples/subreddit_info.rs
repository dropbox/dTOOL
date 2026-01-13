//! Example demonstrating Reddit subreddit information retrieval.
//!
//! This example shows how to get information about subreddits.
//!
//! Run with: cargo run --example subreddit_info

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_reddit::RedditSubredditTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Reddit Subreddit Info Tool Examples ===\n");

    let tool = RedditSubredditTool::new();

    // Example 1: Get info about r/rust
    println!("Example 1: r/rust information\n");
    let input1 = serde_json::json!({
        "subreddit": "rust"
    });

    match tool._call(ToolInput::Structured(input1)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 2: Get info about r/programming
    println!("Example 2: r/programming information\n");
    let input2 = serde_json::json!({
        "subreddit": "programming"
    });

    match tool._call(ToolInput::Structured(input2)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 3: Simple string input
    println!("Example 3: r/opensource information (string input)\n");
    match tool._call(ToolInput::String("opensource".to_string())).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    Ok(())
}
