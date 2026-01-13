//! AWS Bedrock streaming example
//!
//! This example demonstrates streaming responses from Bedrock Claude models.
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
//! cargo run --example bedrock_streaming
//! ```

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_bedrock::ChatBedrock;
use futures::stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bedrock = ChatBedrock::new("us-west-2")
        .await?
        .with_model("anthropic.claude-3-5-sonnet-20241022-v2:0")
        .with_max_tokens(2048);

    println!("🚀 Bedrock ChatModel initialized (streaming mode)");
    println!("Model: anthropic.claude-3-5-sonnet-20241022-v2:0\n");

    let messages = vec![Message::human(
        "Write a haiku about Rust programming. Think step by step.",
    )];

    println!("📤 Streaming response:\n");

    let mut stream = bedrock.stream(&messages, None, None, None, None).await?;

    let mut accumulated_text = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        let text = &chunk.message.content;
        if !text.is_empty() {
            print!("{}", text);
            accumulated_text.push_str(text);
            // Flush stdout to show streaming effect
            use std::io::Write;
            std::io::stdout().flush()?;
        }

        // Check for usage metadata in final chunk
        if let Some(usage) = &chunk.message.usage_metadata {
            println!("\n\n📊 Usage:");
            println!("  Input tokens:  {}", usage.input_tokens);
            println!("  Output tokens: {}", usage.output_tokens);
            println!("  Total tokens:  {}", usage.total_tokens);
        }
    }

    println!("\n\n✅ Streaming complete");
    println!("📝 Total characters received: {}", accumulated_text.len());

    Ok(())
}
