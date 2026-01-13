// Basic ChatOllama usage example
//
// Run with: cargo run -p dashflow-ollama --example basic_chat
//
// Prerequisites:
// 1. Install Ollama: https://ollama.ai/download
// 2. Pull a model: ollama pull llama3.2
// 3. Start Ollama (it usually auto-starts)

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_ollama::ChatOllama;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Basic ChatOllama Example ===\n");
    println!("Make sure you have Ollama running with a model pulled\n");

    let chat = ChatOllama::with_base_url("http://localhost:11434").with_model("llama3.2");

    let messages = vec![
        Message::system("You are a helpful assistant. Be concise."),
        Message::human("What is Rust?"),
    ];

    let result = chat.generate(&messages, None, None, None, None).await?;
    let first_generation = &result.generations[0];

    println!("Question: What is Rust?");
    println!("Answer: {}\n", first_generation.message.as_text());

    println!("✅ Basic Ollama chat example complete");
    println!("💡 Run 'ollama list' to see available models");
    Ok(())
}
