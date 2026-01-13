// Basic ChatPerplexity usage example
//
// Run with: cargo run -p dashflow-perplexity --example perplexity_basic_chat
//
// Required environment variable:
// - PPLX_API_KEY: Your Perplexity API key

use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_perplexity::build_chat_model;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Basic ChatPerplexity Example ===\n");

    let config = ChatModelConfig::Perplexity {
        model: "sonar".to_string(),
        api_key: SecretReference::from_env("PPLX_API_KEY"),
        temperature: None,
    };
    let chat = build_chat_model(&config)?;

    let messages = vec![
        Message::system("You are a helpful assistant with access to real-time search."),
        Message::human("What is Rust programming language?"),
    ];

    let result = chat.generate(&messages, None, None, None, None).await?;
    let first_generation = &result.generations[0];

    println!("Question: What is Rust programming language?");
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

    // Example 2: Different model (sonar-pro)
    println!("\n=== Using sonar-pro model ===\n");

    let config_pro = ChatModelConfig::Perplexity {
        model: "sonar-pro".to_string(),
        api_key: SecretReference::from_env("PPLX_API_KEY"),
        temperature: Some(0.3),
    };
    let chat_pro = build_chat_model(&config_pro)?;

    let messages = vec![Message::human("What are the latest developments in AI?")];

    let result = chat_pro.generate(&messages, None, None, None, None).await?;
    let first_generation = &result.generations[0];

    println!("Question: What are the latest developments in AI?");
    println!("Answer: {}\n", first_generation.message.as_text());

    println!("\n✅ Basic Perplexity chat example complete");
    Ok(())
}
