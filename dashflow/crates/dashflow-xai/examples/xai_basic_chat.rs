//! Basic chat example with xAI AI
//!
//! Run with: cargo run --example xai_basic_chat --package dashflow-xai
//!
//! Requires XAI_API_KEY environment variable.

use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_xai::build_chat_model;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ChatModelConfig::XAI {
        model: "grok-beta".to_string(),
        api_key: SecretReference::from_env("XAI_API_KEY"),
        temperature: Some(0.7),
    };
    let model = build_chat_model(&config)?;

    // Create messages
    let messages = vec![
        Message::system("You are a helpful assistant that provides concise answers."),
        Message::human("What is the capital of France?"),
    ];

    // Generate response
    println!("Sending request to xAI AI...");
    let result = model.generate(&messages, None, None, None, None).await?;

    // Print response
    if let Some(generation) = result.generations.first() {
        println!("Response: {}", generation.message.content().as_text());
    }

    Ok(())
}
