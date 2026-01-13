//! Basic chat example with Groq
//!
//! Run with: cargo run --example groq_basic_chat --package dashflow-groq
//!
//! Requires GROQ_API_KEY environment variable.
//!
//! This example demonstrates the recommended pattern using `build_chat_model()`
//! which enables tracing and optimization integration. See DASHOPTIMIZE_GUIDE.md.

use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_groq::build_chat_model;

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
        Message::system("You are a helpful assistant that provides concise answers."),
        Message::human("What is the capital of France?"),
    ];

    // Generate response
    println!("Sending request to Groq...");
    let result = model.generate(&messages, None, None, None, None).await?;

    // Print response
    if let Some(generation) = result.generations.first() {
        println!("Response: {}", generation.message.content().as_text());
    }

    Ok(())
}
