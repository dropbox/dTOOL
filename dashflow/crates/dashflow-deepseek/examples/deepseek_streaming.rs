// Streaming response example
//
// Run with: cargo run -p dashflow-deepseek --example deepseek_streaming
//
// Required environment variable:
// - DEEPSEEK_API_KEY: Your DeepSeek API key

use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_deepseek::build_chat_model;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DeepSeek Streaming Example ===\n");

    let config = ChatModelConfig::DeepSeek {
        model: "deepseek-chat".to_string(),
        api_key: SecretReference::from_env("DEEPSEEK_API_KEY"),
        temperature: None,
    };
    let chat = build_chat_model(&config)?;

    let messages = vec![Message::human("Count from 1 to 5, one number at a time.")];

    println!("Streaming response:");
    let mut stream = chat.stream(&messages, None, None, None, None).await?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        print!("{}", chunk.message.content);
    }
    println!("\n\n✅ Streaming example complete");

    Ok(())
}
