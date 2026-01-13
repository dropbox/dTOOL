use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_mistral::build_chat_model;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ChatModelConfig::Mistral {
        model: "mistral-small-latest".to_string(),
        api_key: SecretReference::from_env("MISTRAL_API_KEY"),
        temperature: Some(0.7),
    };
    let model = build_chat_model(&config)?;

    // Create a simple conversation
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::human("What is the capital of France?"),
    ];

    println!("Sending message to Mistral AI...");

    // Generate response
    let result = model.generate(&messages, None, None, None, None).await?;

    // Print the response
    if let Some(generation) = result.generations.first() {
        println!("\nAssistant: {}", generation.message.as_text());

        // Print usage stats if available
        if let Some(llm_output) = &result.llm_output {
            if let Some(usage) = llm_output.get("usage") {
                println!("\nUsage: {}", serde_json::to_string_pretty(usage)?);
            }
        }
    }

    Ok(())
}
