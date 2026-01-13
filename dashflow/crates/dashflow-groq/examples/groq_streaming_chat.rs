//! Streaming chat example with Groq
//!
//! Run with: cargo run --example groq_streaming_chat --package dashflow-groq
//!
//! Requires GROQ_API_KEY environment variable.
//!
//! This example demonstrates the recommended pattern using `build_chat_model()`
//! which enables tracing and optimization integration. See DASHOPTIMIZE_GUIDE.md.

use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_groq::build_chat_model;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Groq model using config-based pattern (recommended for tracing/optimization)
    let config = ChatModelConfig::Groq {
        model: "llama-3.1-8b-instant".to_string(),
        api_key: SecretReference::from_env("GROQ_API_KEY"),
        temperature: Some(0.7),
    };
    let model = build_chat_model(&config)?;

    // Create messages
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::human("Write a haiku about Rust programming."),
    ];

    // Stream response
    println!("Streaming response from Groq:\n");
    let mut stream = model.stream(&messages, None, None, None, None).await?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                print!("{}", chunk.message.content);
                std::io::Write::flush(&mut std::io::stdout())?;
            }
            Err(e) => {
                eprintln!("\nError: {}", e);
                break;
            }
        }
    }

    println!("\n");
    Ok(())
}
