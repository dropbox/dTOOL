// Basic ChatCohere usage example
//
// Run with: cargo run -p dashflow-cohere --example cohere_basic_chat
//
// Required environment variable:
// - COHERE_API_KEY: Your Cohere API key

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_cohere::ChatCohere;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Basic ChatCohere Example ===\n");

    let chat = ChatCohere::new().with_model("command-r-plus");

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

    println!("\n✅ Basic Cohere chat example complete");
    Ok(())
}
