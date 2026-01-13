//! DashFlow.
//!
//! This example demonstrates how to configure and use Azure OpenAI
//! with Azure-specific endpoints and authentication.
//!
//! # Required Environment Variables
//!
//! - `AZURE_OPENAI_API_KEY`: Your Azure OpenAI API key
//! - `AZURE_OPENAI_ENDPOINT`: Your Azure OpenAI endpoint (e.g., "https://my-resource.openai.azure.com")
//! - `AZURE_OPENAI_DEPLOYMENT_NAME`: Your deployment name (e.g., "gpt-4")
//!
//! # Usage
//!
//! ```bash
//! export AZURE_OPENAI_API_KEY="your-api-key"
//! export AZURE_OPENAI_ENDPOINT="https://my-resource.openai.azure.com"
//! export AZURE_OPENAI_DEPLOYMENT_NAME="gpt-4"
//! cargo run --example azure_openai_basic
//! ```

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_azure_openai::ChatAzureOpenAI;

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

    // Simple conversation
    println!("Sending message to Azure OpenAI...");
    let messages = vec![Message::human("Hello! What can you tell me about Azure?")];

    let result = chat.generate(&messages, None, None, None, None).await?;

    println!("\nResponse:");
    for generation in &result.generations {
        if let Message::AI {
            content,
            usage_metadata,
            ..
        } = &generation.message
        {
            println!("{}", content.as_text());

            // Print usage metadata if available
            if let Some(usage) = usage_metadata {
                println!("\nUsage:");
                println!("  Input tokens: {}", usage.input_tokens);
                println!("  Output tokens: {}", usage.output_tokens);
                println!("  Total tokens: {}", usage.total_tokens);
            }
        }
    }

    Ok(())
}
