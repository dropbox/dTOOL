// Basic AzureChatOpenAI usage example
//
// Run with: cargo run -p dashflow-openai --example azure_openai_basic_chat
//
// Required environment variables:
// - AZURE_OPENAI_API_KEY: Your Azure OpenAI API key
// - AZURE_OPENAI_ENDPOINT: Your Azure endpoint (e.g., https://your-resource.openai.azure.com)
// - AZURE_OPENAI_API_VERSION: API version (e.g., 2024-05-01-preview)

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_openai::AzureChatOpenAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Basic AzureChatOpenAI Example ===\n");

    // Create AzureChatOpenAI instance
    // Note: Requires AZURE_OPENAI_API_KEY, AZURE_OPENAI_ENDPOINT, and AZURE_OPENAI_API_VERSION
    let chat = AzureChatOpenAI::with_config(Default::default())
        .with_model("gpt-4") // Underlying model name for tracing
        .with_deployment("gpt-4") // Your deployment name in Azure
        .with_temperature(0.7);

    println!("Using Azure OpenAI deployment...\n");

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

    println!("✅ Basic Azure chat example complete");
    Ok(())
}
