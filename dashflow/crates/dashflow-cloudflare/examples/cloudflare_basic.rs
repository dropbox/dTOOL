//! Basic Cloudflare Workers AI usage example

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_cloudflare::ChatCloudflare;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the model with environment variables
    let model = ChatCloudflare::new()
        .with_account_id(std::env::var("CLOUDFLARE_ACCOUNT_ID")?)
        .with_api_token(std::env::var("CLOUDFLARE_API_TOKEN")?)
        .with_model("@cf/meta/llama-3.1-8b-instruct")
        .with_temperature(0.7)
        .with_max_tokens(512);

    // Create a simple message
    let messages = vec![Message::human(
        "Explain the benefits of edge computing in 3 sentences.",
    )];

    // Generate a response
    println!("Sending request to Cloudflare Workers AI...");
    let response = model.generate(&messages, None, None, None, None).await?;

    // Print the response
    if let Some(generation) = response.generations.first() {
        println!("\nResponse:");
        println!("{}", generation.message.as_text());
    }

    Ok(())
}
