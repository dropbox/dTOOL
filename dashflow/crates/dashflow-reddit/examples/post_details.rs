//! Example demonstrating Reddit post detail retrieval.
//!
//! This example shows how to get full post content and comments.
//!
//! Run with: cargo run --example post_details

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_reddit::RedditPostTool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Reddit Post Detail Tool Example ===\n");

    let tool = RedditPostTool::new();

    // Note: Replace with actual post IDs from your search results
    // Format: post_id is the alphanumeric string after /comments/ in Reddit URLs
    // Example URL: https://reddit.com/r/rust/comments/abc123/post_title
    // post_id = "abc123"

    println!("Example: Get post details with comments\n");
    let input = serde_json::json!({
        "post_id": "1234abc",  // Replace with real post ID
        "subreddit": "rust",
        "num_comments": 5
    });

    match tool._call(ToolInput::Structured(input)).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => eprintln!("Error: {}\n", e),
    }

    println!("\nNote: To use this example with real data:");
    println!("1. Run the search_posts example first");
    println!("2. Copy a post URL from the results");
    println!("3. Extract the post_id and subreddit from the URL");
    println!("4. Update the input JSON with those values");

    Ok(())
}
