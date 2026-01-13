// Azure OpenAI streaming response example
//
// Run with: cargo run -p dashflow-openai --example azure_openai_streaming
//
// Required environment variables:
// - AZURE_OPENAI_API_KEY: Your Azure OpenAI API key
// - AZURE_OPENAI_ENDPOINT: Your Azure endpoint (e.g., https://your-resource.openai.azure.com)
// - AZURE_OPENAI_API_VERSION: API version (e.g., 2024-05-01-preview)

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_openai::AzureChatOpenAI;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Azure OpenAI Streaming Example ===\n");

    // Create AzureChatOpenAI instance
    let chat = AzureChatOpenAI::with_config(Default::default())
        .with_model("gpt-4") // Underlying model name for tracing
        .with_deployment("gpt-4"); // Your deployment name in Azure

    println!("Using Azure OpenAI deployment for streaming...\n");

    // Create a prompt that generates a longer response
    let messages = vec![Message::human(
        "Write a haiku about Rust programming. Think step by step.",
    )];

    println!("Streaming response:\n");
    let mut stream = chat.stream(&messages, None, None, None, None).await?;

    // Stream chunks as they arrive
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        print!("{}", chunk.message.content);
        std::io::Write::flush(&mut std::io::stdout())?;
    }

    println!("\n\n✅ Azure OpenAI streaming example complete");
    Ok(())
}
