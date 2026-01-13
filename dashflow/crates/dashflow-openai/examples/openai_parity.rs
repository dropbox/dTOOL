//! Test Rust OpenAI against Python baseline

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_openai::ChatOpenAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Rust ChatOpenAI ===");

    // Create ChatOpenAI instance (gets API key from env automatically)
    let chat = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-3.5-turbo")
        .with_temperature(0.0);

    // Same test as Python
    let messages = vec![Message::human("Say exactly: Hello from Rust")];

    match chat.generate(&messages, None, None, None, None).await {
        Ok(result) => {
            let generation = &result.generations[0];
            let response = &generation.message;
            println!("Response type: AIMessage");
            println!("Content: {}", response.as_text());
            println!("✅ Rust OpenAI works");

            println!("\nMessage fields:");
            println!("  - content: {}", response.as_text());
            if let Message::AI { usage_metadata, .. } = response {
                if let Some(usage) = usage_metadata {
                    println!(
                        "  - usage_metadata: input={}, output={}, total={}",
                        usage.input_tokens, usage.output_tokens, usage.total_tokens
                    );
                } else {
                    println!("  - usage_metadata: NOT POPULATED");
                }
            }

            println!("\nGeneration info (response_metadata):");
            if let Some(gen_info) = &generation.generation_info {
                println!("  - Keys: {:?}", gen_info.keys().collect::<Vec<_>>());
                for (key, value) in gen_info.iter() {
                    println!("    - {}: {}", key, value);
                }
            } else {
                println!("  - generation_info: EMPTY");
            }

            println!("\n✅ Compare to Python output above");
            Ok(())
        }
        Err(e) => {
            println!("❌ Error: {}", e);
            Err(e.into())
        }
    }
}
