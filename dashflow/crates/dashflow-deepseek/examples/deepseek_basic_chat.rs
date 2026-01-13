// Basic ChatDeepSeek usage example
//
// Run with: cargo run -p dashflow-deepseek --example deepseek_basic_chat
//
// Required environment variable:
// - DEEPSEEK_API_KEY: Your DeepSeek API key

use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_deepseek::build_chat_model;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Basic ChatDeepSeek Example ===\n");

    let config = ChatModelConfig::DeepSeek {
        model: "deepseek-chat".to_string(),
        api_key: SecretReference::from_env("DEEPSEEK_API_KEY"),
        temperature: None,
    };
    let chat = build_chat_model(&config)?;

    let messages = vec![
        Message::system("You are a helpful assistant. Be concise."),
        Message::human("What is the capital of France?"),
    ];

    let result = chat.generate(&messages, None, None, None, None).await?;
    let first_generation = &result.generations[0];

    println!("Question: What is the capital of France?");
    println!("Answer: {}\n", first_generation.message.as_text());

    // Show usage metadata if available
    if let Message::AI {
        usage_metadata: Some(usage),
        ..
    } = &first_generation.message
    {
        println!("Usage:");
        println!("  Input tokens: {}", usage.input_tokens);
        println!("  Output tokens: {}", usage.output_tokens);
        println!("  Total tokens: {}", usage.total_tokens);
    }

    println!("\n✅ Basic DeepSeek chat example complete");
    Ok(())
}
