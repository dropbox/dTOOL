// Streaming response example
//
// Run with: cargo run -p dashflow-perplexity --example perplexity_streaming
//
// Required environment variable:
// - PPLX_API_KEY: Your Perplexity API key

use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_perplexity::build_chat_model;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Perplexity Streaming Example ===\n");

    let config = ChatModelConfig::Perplexity {
        model: "sonar".to_string(),
        api_key: SecretReference::from_env("PPLX_API_KEY"),
        temperature: None,
    };
    let chat = build_chat_model(&config)?;

    let messages = vec![Message::human(
        "Explain how async/await works in Rust in 3 paragraphs.",
    )];

    println!("Streaming response:");
    let mut stream = chat.stream(&messages, None, None, None, None).await?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        print!("{}", chunk.message.content);
        // Flush to ensure real-time display
        use std::io::Write;
        std::io::stdout().flush()?;
    }

    println!("\n\n✅ Streaming example complete");

    Ok(())
}
