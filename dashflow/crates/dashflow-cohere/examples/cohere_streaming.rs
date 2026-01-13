// Streaming response example
//
// Run with: cargo run -p dashflow-cohere --example cohere_streaming
//
// Required environment variable:
// - COHERE_API_KEY: Your Cohere API key

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_cohere::ChatCohere;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cohere Streaming Example ===\n");

    let chat = ChatCohere::new().with_model("command-r-plus");

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
