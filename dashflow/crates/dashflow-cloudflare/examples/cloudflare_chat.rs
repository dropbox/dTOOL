//! Multi-turn conversation example with Cloudflare Workers AI

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_cloudflare::ChatCloudflare;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the model
    let model = ChatCloudflare::new()
        .with_account_id(std::env::var("CLOUDFLARE_ACCOUNT_ID")?)
        .with_api_token(std::env::var("CLOUDFLARE_API_TOKEN")?)
        .with_model("@cf/meta/llama-3.1-8b-instruct")
        .with_temperature(0.7)
        .with_max_tokens(256);

    // Build a conversation
    let messages = vec![
        Message::system("You are a helpful assistant that explains technical concepts concisely."),
        Message::human("What is Cloudflare Workers AI?"),
    ];

    // First turn
    println!("User: What is Cloudflare Workers AI?");
    let response1 = model.generate(&messages, None, None, None, None).await?;
    let assistant_msg1 = if let Some(gen) = response1.generations.first() {
        let content = gen.message.as_text();
        println!("\nAssistant: {}\n", content);
        Message::ai(content)
    } else {
        return Err("No response generated".into());
    };

    // Second turn
    let mut messages2 = messages.clone();
    messages2.push(assistant_msg1);
    messages2.push(Message::human(
        "How does it compare to traditional cloud inference?",
    ));

    println!("User: How does it compare to traditional cloud inference?");
    let response2 = model.generate(&messages2, None, None, None, None).await?;
    if let Some(gen) = response2.generations.first() {
        println!("\nAssistant: {}", gen.message.as_text());
    }

    Ok(())
}
