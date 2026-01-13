//! Streaming response example with Cloudflare Workers AI

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_cloudflare::ChatCloudflare;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the model
    let model = ChatCloudflare::new()
        .with_account_id(std::env::var("CLOUDFLARE_ACCOUNT_ID")?)
        .with_api_token(std::env::var("CLOUDFLARE_API_TOKEN")?)
        .with_model("@cf/meta/llama-3.1-8b-instruct")
        .with_temperature(0.8);

    // Create a message
    let messages = vec![Message::human("Write a short poem about edge computing.")];

    // Stream the response
    println!("Streaming response from Cloudflare Workers AI...\n");
    let mut stream = model.stream(&messages, None, None, None, None).await?;

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(generation) => {
                print!("{}", generation.message.content);
                std::io::Write::flush(&mut std::io::stdout())?;
            }
            Err(e) => {
                eprintln!("\nError: {}", e);
                break;
            }
        }
    }

    println!("\n\nStreaming complete!");
    Ok(())
}
