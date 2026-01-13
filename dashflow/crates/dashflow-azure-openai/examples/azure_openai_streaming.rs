//! Streaming example with Azure OpenAI.
//!
//! This example demonstrates how to use Azure OpenAI's streaming
//! capabilities to get incremental responses.
//!
//! # Required Environment Variables
//!
//! - `AZURE_OPENAI_API_KEY`: Your Azure OpenAI API key
//! - `AZURE_OPENAI_ENDPOINT`: Your Azure OpenAI endpoint
//! - `AZURE_OPENAI_DEPLOYMENT_NAME`: Your deployment name
//!
//! # Usage
//!
//! ```bash
//! export AZURE_OPENAI_API_KEY="your-api-key"
//! export AZURE_OPENAI_ENDPOINT="https://my-resource.openai.azure.com"
//! export AZURE_OPENAI_DEPLOYMENT_NAME="gpt-4"
//! cargo run --example azure_openai_streaming
//! ```

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_azure_openai::ChatAzureOpenAI;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = match std::env::var("AZURE_OPENAI_ENDPOINT") {
        Ok(endpoint) => endpoint,
        Err(_) => {
            println!("AZURE_OPENAI_ENDPOINT environment variable not set.");
            println!("Example: export AZURE_OPENAI_ENDPOINT=\"https://my-resource.openai.azure.com\"");
            return Ok(());
        }
    };

    // Initialize Azure OpenAI chat model
    let chat = ChatAzureOpenAI::new()
        .with_deployment_name(
            std::env::var("AZURE_OPENAI_DEPLOYMENT_NAME")
                .unwrap_or_else(|_| "gpt-35-turbo".to_string()),
        )
        .with_endpoint(endpoint)
        .with_temperature(0.7);

    // Create a streaming conversation
    println!("Streaming response from Azure OpenAI...\n");
    let messages = vec![Message::human(
        "Tell me a short story about a cloud that lived in Azure.",
    )];

    let mut stream = chat.stream(&messages, None, None, None, None).await?;

    // Process the stream
    print!("Response: ");
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                print!("{}", chunk.message.content);
                std::io::Write::flush(&mut std::io::stdout())?;
            }
            Err(e) => {
                eprintln!("\nError in stream: {}", e);
                break;
            }
        }
    }
    println!("\n");

    Ok(())
}
