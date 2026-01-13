//! Example: Get detailed information about a Stack Overflow question
//!
//! This example demonstrates how to retrieve full details about a specific
//! Stack Overflow question by its ID.
//!
//! Usage:
//! ```bash
//! cargo run --example get_question
//! ```

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_stackexchange::StackExchangeQuestionTool;
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Stack Exchange Question Details Example ===\n");

    // Create the question tool for Stack Overflow
    let tool = StackExchangeQuestionTool::new("stackoverflow".to_string());

    // Example: Get details for question ID 1 (the first Stack Overflow question)
    // This is a famous question: "How do I calculate someone's age in C#?"
    println!("Fetching details for Stack Overflow question #1...\n");

    let input = json!({
        "question_id": "1",
        "include_answers": "true"
    });

    match tool._call(ToolInput::Structured(input)).await {
        Ok(result) => {
            println!("{}\n", result);
            println!("Note: This is the very first question posted to Stack Overflow!");
        }
        Err(e) => eprintln!("Error: {}\n", e),
    }

    println!("\n=== Example Complete ===");
    println!("\nTo try a different question:");
    println!("1. Search for questions using the search_questions example");
    println!("2. Copy a question ID from the search results");
    println!("3. Update this example with that question ID");

    Ok(())
}
