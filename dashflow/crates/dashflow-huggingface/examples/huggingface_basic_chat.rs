use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_huggingface::build_chat_model;

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
        Message::system("You are a helpful assistant that provides concise answers."),
        Message::human("What is the capital of France?"),
    ];

    // Generate response
    println!("\nGenerating response...");
    let result = model.generate(&messages, None, None, None, None).await?;

    // Print the response
    if let Some(generation) = result.generations.first() {
        println!("\nResponse: {}", generation.message.as_text());
    }

    Ok(())
}
