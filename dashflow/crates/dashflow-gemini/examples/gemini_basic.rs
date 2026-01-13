//! Basic example of using Google Gemini chat model
//!
//! This example demonstrates:
//! - Creating a ChatGemini instance
//! - Simple text generation
//! - Accessing response metadata
//!
//! # Setup
//!
//! Set your Gemini API key:
//! ```bash
//! export GEMINI_API_KEY=your-api-key-here
//! ```
//!
//! Get your API key from: https://aistudio.google.com/app/apikey
//!
//! # Run
//!
//! ```bash
//! cargo run --example gemini_basic --features dashflow-gemini
//! ```

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_gemini::ChatGemini;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check for API key
    if std::env::var("GEMINI_API_KEY").is_err() {
        eprintln!("Error: GEMINI_API_KEY environment variable not set");
        eprintln!("Get your API key from: https://aistudio.google.com/app/apikey");
        std::process::exit(1);
    }

    // Create a Gemini model instance
    println!("Creating ChatGemini model...");
    let model = ChatGemini::new()
        .with_model("gemini-2.0-flash-exp")
        .with_temperature(0.7)
        .with_max_tokens(1024);

    // Create a simple message
    let messages = vec![Message::human(
        "What is Rust programming language? Answer in 2-3 sentences.",
    )];

    // Generate response
    println!("\nSending message to Gemini...");
    println!("Question: What is Rust programming language?");
    println!("\nWaiting for response...\n");

    let result = model.generate(&messages, None, None, None, None).await?;

    // Extract the response
    if let Some(generation) = result.generations.first() {
        let response_text = generation.message.as_text();
        println!("Gemini: {}", response_text);

        // Print usage information if available from message fields
        let fields = generation.message.fields();
        if let Some(usage_val) = fields.response_metadata.get("usage") {
            println!("\n--- Usage Information ---");
            println!("{}", serde_json::to_string_pretty(usage_val)?);
        }

        // Print generation info
        if let Some(info) = &generation.generation_info {
            println!("\n--- Generation Metadata ---");
            println!("{}", serde_json::to_string_pretty(info)?);
        }
    } else {
        println!("No response generated");
    }

    Ok(())
}
