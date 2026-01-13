//! Streaming example of using Google Gemini chat model
//!
//! This example demonstrates:
//! - Creating a ChatGemini instance
//! - Streaming text generation
//! - Processing chunks as they arrive
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
//! cargo run --example gemini_streaming --features dashflow-gemini
//! ```

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_gemini::ChatGemini;
use futures::StreamExt;

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
        .with_temperature(0.8)
        .with_max_tokens(2048);

    // Create a message
    let messages = vec![Message::human(
        "Write a short poem about Rust programming language (4 lines).",
    )];

    // Stream response
    println!("\nSending message to Gemini (streaming)...");
    println!("Question: Write a short poem about Rust programming language");
    println!("\nGemini: ");

    let mut stream = model.stream(&messages, None, None, None, None).await?;

    let mut full_response = String::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                // AIMessageChunk has a `content` field, not a method
                let text = &chunk.message.content;
                print!("{}", text);
                full_response.push_str(text);
                // Flush stdout to show chunks immediately
                use std::io::Write;
                std::io::stdout().flush()?;
            }
            Err(e) => {
                eprintln!("\nError: {}", e);
                break;
            }
        }
    }

    println!("\n\n--- Streaming Complete ---");
    println!("Total characters received: {}", full_response.len());

    Ok(())
}
