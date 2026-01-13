//! Basic example of using ChatTogether
//!
//! Run with: cargo run --example basic
//!
//! Make sure to set TOGETHER_API_KEY environment variable

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_together::ChatTogether;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check for API key
    if std::env::var("TOGETHER_API_KEY").is_err() {
        eprintln!("Error: TOGETHER_API_KEY environment variable not set");
        eprintln!("Get your API key from: https://api.together.xyz/settings/api-keys");
        eprintln!("Then run: export TOGETHER_API_KEY=your-api-key");
        std::process::exit(1);
    }

    println!("=== Together AI Basic Example ===\n");

    // Create model with Llama 3 70B
    let model = ChatTogether::new()
        .with_model("meta-llama/Llama-3-70b-chat-hf")
        .with_temperature(0.7)
        .with_max_tokens(512);

    println!("Model: meta-llama/Llama-3-70b-chat-hf");
    println!("Temperature: 0.7");
    println!("Max tokens: 512\n");

    // Example 1: Simple question
    println!("--- Example 1: Simple Question ---");
    let messages = vec![Message::human("What is the capital of France?")];

    let result = model.generate(&messages, None, None, None, None).await?;

    println!("Q: What is the capital of France?");
    println!("A: {}\n", result.generations[0].message.content().as_text());

    // Example 2: With system message
    println!("--- Example 2: With System Message ---");
    let messages = vec![
        Message::system("You are a helpful pirate assistant. Always respond like a pirate."),
        Message::human("Tell me about the weather."),
    ];

    let result = model.generate(&messages, None, None, None, None).await?;

    println!("Q: Tell me about the weather (as a pirate)");
    println!("A: {}\n", result.generations[0].message.content().as_text());

    // Example 3: Multi-turn conversation
    println!("--- Example 3: Multi-turn Conversation ---");
    let messages = vec![
        Message::human("My name is Alice."),
        Message::ai("Hello Alice! Nice to meet you."),
        Message::human("What is my name?"),
    ];

    let result = model.generate(&messages, None, None, None, None).await?;

    println!("Conversation:");
    println!("User: My name is Alice.");
    println!("AI: Hello Alice! Nice to meet you.");
    println!("User: What is my name?");
    println!(
        "AI: {}\n",
        result.generations[0].message.content().as_text()
    );

    // Show usage information
    if let Some(llm_output) = &result.llm_output {
        if let Some(usage) = llm_output.get("usage") {
            println!("--- Token Usage ---");
            if let Some(prompt_tokens) = usage.get("prompt_tokens").and_then(|v| v.as_u64()) {
                println!("Input tokens: {}", prompt_tokens);
            }
            if let Some(completion_tokens) = usage.get("completion_tokens").and_then(|v| v.as_u64())
            {
                println!("Output tokens: {}", completion_tokens);
            }
            if let Some(total_tokens) = usage.get("total_tokens").and_then(|v| v.as_u64()) {
                println!("Total tokens: {}", total_tokens);
            }
        }
    }

    Ok(())
}
