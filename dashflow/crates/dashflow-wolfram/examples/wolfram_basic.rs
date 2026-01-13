//! Basic WolframAlpha example
//!
//! This example demonstrates basic usage of the WolframAlpha tool.
//!
//! # Usage
//!
//! ```bash
//! export WOLFRAM_APP_ID=your_app_id_here
//! cargo run --example wolfram_basic
//! ```

use dashflow::core::tools::{Tool, ToolInput};
use dashflow_wolfram::WolframAlpha;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the App ID from environment
    let app_id = std::env::var("WOLFRAM_APP_ID").unwrap_or_else(|_| "demo".to_string());

    println!("WolframAlpha Basic Examples\n");
    println!("===========================\n");

    // Create the tool
    let tool = WolframAlpha::new(&app_id);

    // Example queries
    let queries = vec![
        "What is 2 + 2?",
        "What is the capital of France?",
        "How many days until Christmas?",
        "What is the speed of light?",
        "Convert 100 kilometers to miles",
        "What is the population of Tokyo?",
        "Solve x^2 - 5x + 6 = 0",
        "What is the square root of 144?",
    ];

    for query in queries {
        println!("Query: {}", query);

        // Use ToolInput::String for direct string queries
        let input = ToolInput::String(query.to_string());

        match tool._call(input).await {
            Ok(answer) => {
                println!("Answer: {}\n", answer);
            }
            Err(e) => {
                println!("Error: {}\n", e);
            }
        }
    }

    // Example with builder pattern
    println!("\n=== Builder Pattern Example ===\n");

    let tool = WolframAlpha::builder()
        .app_id(&app_id)
        .units("metric")
        .timeout(60)
        .build();

    println!("Query: What is 32 fahrenheit in celsius?");
    let input = ToolInput::String("32 fahrenheit in celsius".to_string());
    match tool._call(input).await {
        Ok(answer) => println!("Answer: {}", answer),
        Err(e) => println!("Error: {}", e),
    }

    Ok(())
}
