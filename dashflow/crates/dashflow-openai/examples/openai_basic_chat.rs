// Basic ChatOpenAI usage example
//
// Run with: cargo run -p dashflow-openai --example basic_chat
//
// Required environment variable:
// - OPENAI_API_KEY: Your OpenAI API key

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_openai::ChatOpenAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Basic ChatOpenAI Example ===\n");

    // Create ChatOpenAI instance
    let chat = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4o-mini")
        .with_temperature(0.7);

    // Simple question
    let messages = vec![Message::human("What is the capital of France?")];
    let result = chat.generate(&messages, None, None, None, None).await?;

    let first_generation = &result.generations[0];
    println!("Question: What is the capital of France?");
    println!("Answer: {}\n", first_generation.message.as_text());

    // With system message
    let messages = vec![
        Message::system("You are a helpful assistant. Be concise."),
        Message::human("Explain Rust in one sentence."),
    ];
    let result = chat.generate(&messages, None, None, None, None).await?;
    let first_generation = &result.generations[0];
    println!("With system message:");
    println!("{}\n", first_generation.message.as_text());

    println!("✅ Basic chat example complete");
    Ok(())
}
