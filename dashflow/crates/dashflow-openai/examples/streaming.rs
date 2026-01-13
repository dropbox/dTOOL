// Streaming response example
//
// Run with: cargo run -p dashflow-openai --example streaming
//
// Required environment variable:
// - OPENAI_API_KEY: Your OpenAI API key

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_openai::ChatOpenAI;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Streaming Example ===\n");

    let chat = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");

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
