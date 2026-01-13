//! Basic AWS Bedrock example with Claude models
//!
//! This example demonstrates how to use the ChatBedrock integration
//! with AWS SSO authentication.
//!
//! # Prerequisites
//!
//! 1. AWS CLI configured with SSO profile:
//!    ```bash
//!    aws configure sso
//!    ```
//!
//! 2. Login to AWS SSO:
//!    ```bash
//!    aws sso login --profile claude
//!    ```
//!
//! 3. Set AWS_PROFILE environment variable:
//!    ```bash
//!    export AWS_PROFILE=claude
//!    ```
//!
//! # Run
//!
//! ```bash
//! cargo run --example bedrock_basic
//! ```

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_bedrock::ChatBedrock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize ChatBedrock with region
    // Authentication uses AWS SDK default chain:
    // 1. Environment variables (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY)
    // 2. AWS_PROFILE environment variable
    // 3. ~/.aws/credentials file
    // 4. AWS SSO
    // 5. IAM instance profile (for EC2/ECS)
    let bedrock = ChatBedrock::new("us-west-2")
        .await?
        .with_model("anthropic.claude-3-5-sonnet-20241022-v2:0")
        .with_max_tokens(1024);

    println!("🚀 Bedrock ChatModel initialized");
    println!("Model: anthropic.claude-3-5-sonnet-20241022-v2:0");
    println!("Region: us-west-2\n");

    // Simple message
    let messages = vec![Message::human(
        "What is AWS Bedrock? Answer in one sentence.",
    )];

    println!("📤 Sending message...\n");
    let result = bedrock.generate(&messages, None, None, None, None).await?;

    println!("📥 Response:");
    for generation in result.generations {
        println!("{}", generation.message.as_text());

        // Print usage metadata if available
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
