// Ollama streaming response example
//
// Run with: cargo run -p dashflow-ollama --example ollama_streaming
//
// Prerequisites:
// 1. Install Ollama: https://ollama.ai
// 2. Pull a model: ollama pull llama3.2
// 3. Ensure Ollama is running: ollama serve

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_ollama::ChatOllama;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Ollama Streaming Example ===\n");

    // Create chat model with llama3.2 (fast, 3B params)
    // Other options: llama3.1 (8B), mistral (7B), codellama (7B-34B)
    let chat = ChatOllama::with_base_url("http://localhost:11434").with_model("llama3.2");

    println!("Model: llama3.2");
    println!("Prompt: Tell me a short story about a brave robot.\n");
    println!("Streaming response:\n---");

    let messages = vec![Message::human(
        "Tell me a short story about a brave robot. Keep it under 100 words.",
    )];

    let mut stream = chat.stream(&messages, None, None, None, None).await?;

    // Stream tokens as they arrive
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(generation) => {
                // Print each token immediately
                print!("{}", generation.message.content);
                // Flush stdout to ensure immediate display
                use std::io::Write;
                std::io::stdout().flush()?;
            }
            Err(e) => {
                eprintln!("\n❌ Error: {}", e);
                break;
            }
        }
    }

    println!("\n---\n");
    println!("✅ Streaming example complete");
    println!("\n💡 Benefits of streaming:");
    println!("  • Reduced latency - see responses immediately");
    println!("  • Better UX - users don't wait for full response");
    println!("  • Memory efficient - process tokens as they arrive");
    Ok(())
}
