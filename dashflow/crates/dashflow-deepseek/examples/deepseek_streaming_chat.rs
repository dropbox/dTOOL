//! Streaming chat example with DeepSeek AI
//!
//! Run with: cargo run --example deepseek_streaming_chat --package dashflow-deepseek
//!
//! Requires DEEPSEEK_API_KEY environment variable.

use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_deepseek::build_chat_model;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ChatModelConfig::DeepSeek {
        model: "deepseek-chat".to_string(),
        api_key: SecretReference::from_env("DEEPSEEK_API_KEY"),
        temperature: Some(0.7),
    };
    let model = build_chat_model(&config)?;

    // Create messages
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::human("Write a haiku about Rust programming."),
    ];

    // Stream response
    println!("Streaming response from DeepSeek AI:\n");
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
