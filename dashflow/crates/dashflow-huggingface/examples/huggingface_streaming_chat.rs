use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_huggingface::build_chat_model;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a ChatHuggingFace instance using config-driven instantiation
    // You need to set HUGGINGFACEHUB_API_TOKEN or HF_TOKEN environment variable
    let config = ChatModelConfig::HuggingFace {
        model: "meta-llama/Llama-2-7b-chat-hf".to_string(),
        api_key: SecretReference::from_env("HF_TOKEN"),
        temperature: Some(0.7),
    };
    let model = build_chat_model(&config)?;

    println!("Model: {}", model.llm_type());

    // Create messages
    let messages = vec![
        Message::system("You are a helpful assistant that writes creative stories."),
        Message::human("Write a very short story about a robot learning to paint."),
    ];

    // Stream response
    println!("\nStreaming response:");
    println!("---");

    let mut stream = model.stream(&messages, None, None, None, None).await?;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                print!("{}", chunk.message.content);
                std::io::Write::flush(&mut std::io::stdout())?;
            }
            Err(e) => {
                eprintln!("\nError: {}", e);
                break;
            }
        }
    }

    println!("\n---");
    println!("Done!");

    Ok(())
}
