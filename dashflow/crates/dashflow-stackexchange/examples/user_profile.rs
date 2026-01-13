//! Example: Get Stack Exchange user profile information
//!
//! This example demonstrates how to retrieve user profile information
//! including reputation, badges, and other details.
//!
//! Usage:
//! ```bash
//! cargo run --example user_profile
//! ```

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_stackexchange::StackExchangeUserTool;
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Stack Exchange User Profile Example ===\n");

    // Create the user tool for Stack Overflow
    let tool = StackExchangeUserTool::new("stackoverflow".to_string());

    // Example 1: Get profile for user ID 1 (the first Stack Overflow user)
    println!("Fetching profile for Stack Overflow user #1...\n");

    let input = json!({
        "user_id": "1"
    });

    match tool._call(ToolInput::Structured(input)).await {
        Ok(result) => {
            println!("{}\n", result);
        }
        Err(e) => eprintln!("Error: {}\n", e),
    }

    // Example 2: Get profile for user ID 22656 (Jon Skeet - highest reputation user)
    println!("Fetching profile for Stack Overflow user #22656 (Jon Skeet)...\n");

    let input = json!({
        "user_id": "22656"
    });

    match tool._call(ToolInput::Structured(input)).await {
        Ok(result) => {
            println!("{}\n", result);
            println!("Note: Jon Skeet is one of the most prolific contributors to Stack Overflow!");
        }
        Err(e) => eprintln!("Error: {}\n", e),
    }

    println!("\n=== Example Complete ===");
    println!("\nTo look up other users:");
    println!("1. Visit a Stack Overflow user profile");
    println!("2. Copy the user ID from the URL (e.g., /users/12345/username)");
    println!("3. Update this example with that user ID");

    Ok(())
}
