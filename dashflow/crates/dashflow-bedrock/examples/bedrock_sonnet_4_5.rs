//! AWS Bedrock with Claude Sonnet 4.5 example
//!
//! This example demonstrates the latest Claude Sonnet 4.5 model on Bedrock.
//!
//! # Prerequisites
//!
//! 1. AWS CLI configured with SSO profile
//! 2. Login to AWS SSO: `aws sso login --profile claude`
//! 3. Set AWS_PROFILE: `export AWS_PROFILE=claude`
//!
//! # Run
//!
//! ```bash
//! cargo run --example bedrock_sonnet_4_5
//! ```

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_bedrock::chat_models::models;
use dashflow_bedrock::ChatBedrock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use the latest Claude Sonnet 4.5 model
    let bedrock = ChatBedrock::new("us-west-2")
        .await?
        .with_model(models::CLAUDE_SONNET_4_5)
        .with_max_tokens(4096);

    println!("🚀 Bedrock ChatModel initialized");
    println!("Model: Claude Sonnet 4.5 (us.anthropic.claude-sonnet-4-5-20250929-v1:0)");
    println!("Region: us-west-2\n");

    // Test with a complex reasoning task
    let messages = vec![Message::human(
        "Explain the concept of 'zero-copy' in Rust and why it matters for performance. Be concise.",
    )];

    println!("📤 Sending message...\n");
    let result = bedrock.generate(&messages, None, None, None, None).await?;

    println!("📥 Response:");
    for generation in result.generations {
        println!("{}", generation.message.as_text());

        // Extract usage metadata from AI message
        if let Message::AI {
            usage_metadata: Some(usage),
            ..
        } = &generation.message
        {
            println!("\n📊 Usage:");
            println!("  Input tokens:  {}", usage.input_tokens);
            println!("  Output tokens: {}", usage.output_tokens);
            println!("  Total tokens:  {}", usage.total_tokens);
        }
    }

    Ok(())
}
