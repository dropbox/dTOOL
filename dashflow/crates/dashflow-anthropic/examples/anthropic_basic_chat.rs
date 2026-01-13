// Basic ChatAnthropic usage example
//
// Run with: cargo run -p dashflow-anthropic --example basic_chat
//
// Required environment variable:
// - ANTHROPIC_API_KEY: Your Anthropic API key

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_anthropic::ChatAnthropic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Basic ChatAnthropic Example ===\n");

    let chat = ChatAnthropic::try_new()?.with_model("claude-3-5-haiku-20241022");

    let messages = vec![
        Message::system("You are a helpful assistant. Be concise."),
        Message::human("What is the capital of France?"),
    ];

    let result = chat.generate(&messages, None, None, None, None).await?;
    let first_generation = &result.generations[0];

    println!("Question: What is the capital of France?");
    println!("Answer: {}\n", first_generation.message.as_text());

    println!("✅ Basic Anthropic chat example complete");
    Ok(())
}
