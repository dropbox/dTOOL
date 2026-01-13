use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_mistral::build_chat_model;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ChatModelConfig::Mistral {
        model: "mistral-small-latest".to_string(),
        api_key: SecretReference::from_env("MISTRAL_API_KEY"),
        temperature: Some(0.7),
    };
    let model = build_chat_model(&config)?;

    // Create a conversation
    let messages = vec![
        Message::system("You are a creative storyteller."),
        Message::human("Tell me a very short story about a robot learning to paint."),
    ];

    println!("Streaming response from Mistral AI...\n");

    // Stream the response
    let mut stream = model.stream(&messages, None, None, None, None).await?;

    print!("Assistant: ");
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) => {
                // Print each chunk as it arrives
                print!("{}", chunk.message.content);
                // Flush stdout to see the streaming effect
                use std::io::Write;
                std::io::stdout().flush()?;
            }
            Err(e) => {
                eprintln!("\nError: {}", e);
                break;
            }
        }
    }

    println!("\n");

    Ok(())
}
