//! Basic human input example.
//!
//! This example demonstrates how to use the HumanTool to request input from users.
//!
//! Run with: cargo run --example basic_input

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_human_tool::HumanTool;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Human Tool - Basic Input Example ===\n");

    let tool = HumanTool::new();

    // Example 1: Using string input
    println!("Example 1: String Input");
    println!("------------------------");
    let response = tool
        ._call(ToolInput::String("What is your name?".to_string()))
        .await?;
    println!("You said: {}\n", response);

    // Example 2: Using structured input
    println!("Example 2: Structured Input");
    println!("---------------------------");
    let input = json!({
        "prompt": "What is your favorite programming language?"
    });
    let response = tool._call(ToolInput::Structured(input)).await?;
    println!("You said: {}\n", response);

    // Example 3: Collecting multiple inputs
    println!("Example 3: Multiple Questions");
    println!("-----------------------------");

    let questions = [
        "What is your occupation?",
        "How many years of experience do you have?",
        "What are your technical strengths?",
    ];

    for (i, question) in questions.iter().enumerate() {
        let input = json!({"prompt": question});
        let response = tool._call(ToolInput::Structured(input)).await?;
        println!("Question {}: {}", i + 1, question);
        println!("Answer: {}\n", response);
    }

    // Example 4: Confirmation prompt
    println!("Example 4: Confirmation Prompt");
    println!("------------------------------");
    let response = tool
        ._call(ToolInput::String(
            "Do you want to continue? (yes/no)".to_string(),
        ))
        .await?;

    if response.to_lowercase().contains("yes") {
        println!("Continuing...\n");
    } else {
        println!("Operation cancelled.\n");
        return Ok(());
    }

    // Example 5: Collecting structured data
    println!("Example 5: Collecting Structured Data");
    println!("-------------------------------------");

    let mut user_data = std::collections::HashMap::new();

    let fields = vec![
        ("email", "What is your email address?"),
        ("phone", "What is your phone number?"),
        ("city", "What city do you live in?"),
    ];

    for (field, prompt) in fields {
        let response = tool._call(ToolInput::String(prompt.to_string())).await?;
        user_data.insert(field, response);
    }

    println!("\nCollected Data:");
    for (key, value) in &user_data {
        println!("  {}: {}", key, value);
    }

    println!("\n=== Example Complete ===");

    Ok(())
}
