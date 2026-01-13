//! Bedrock Claude Demo - Uses your AWS SSO credentials
//!
//! This example demonstrates using Claude Sonnet 4.5 via AWS Bedrock
//! with the same AWS profile used by Claude Code.
//!
//! # Setup
//!
//! Your AWS credentials are already configured in ~/.aws/config:
//! - Profile: claude
//! - Region: us-west-2
//! - Authentication: AWS SSO
//!
//! # Run
//!
//! ```bash
//! # Make sure you're logged in to AWS SSO
//! aws sso login --profile claude
//!
//! # Run the example
//! AWS_PROFILE=claude cargo run --example bedrock_demo
//! ```

use dashflow_bedrock::chat_models::models;
use dashflow_bedrock::ChatBedrock;
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════╗");
    println!("║   Bedrock Claude Demo - DashFlow          ║");
    println!("╚══════════════════════════════════════════════════╝\n");

    // Initialize ChatBedrock with the same region and model as Claude Code
    // This uses the AWS_PROFILE=claude environment variable for authentication
    let bedrock = ChatBedrock::new("us-west-2")
        .await?
        .with_model(models::CLAUDE_SONNET_4_5)
        .with_max_tokens(2048)
        .with_temperature(0.7);

    println!("✅ Connected to AWS Bedrock");
    println!("   Region: us-west-2");
    println!("   Model:  Claude Sonnet 4.5 ({})", models::CLAUDE_SONNET_4_5);
    println!("   Profile: claude (from ~/.aws/config)\n");

    // Example 1: Simple question
    println!("📝 Example 1: Simple Question");
    println!("─────────────────────────────────────────────────\n");

    let messages = vec![Message::human(
        "What are the three main benefits of using Rust for systems programming?",
    )];

    let result = bedrock.generate(&messages, None, None, None, None).await?;

    for generation in &result.generations {
        println!("{}\n", generation.message.as_text());

        if let Message::AI { usage_metadata, .. } = &generation.message {
            if let Some(usage) = usage_metadata {
                println!("📊 Tokens: {} input + {} output = {} total\n",
                    usage.input_tokens,
                    usage.output_tokens,
                    usage.total_tokens
                );
            }
        }
    }

    // Example 2: Chain of thought reasoning
    println!("📝 Example 2: Reasoning Task");
    println!("─────────────────────────────────────────────────\n");

    let messages = vec![Message::human(
        "If a train leaves Chicago at 3pm going 60 mph, and another leaves New York at 4pm going 70 mph, and they're 790 miles apart, when will they meet? Show your reasoning.",
    )];

    let result = bedrock.generate(&messages, None, None, None, None).await?;

    for generation in &result.generations {
        println!("{}\n", generation.message.as_text());

        if let Message::AI { usage_metadata, .. } = &generation.message {
            if let Some(usage) = usage_metadata {
                println!("📊 Tokens: {} input + {} output = {} total\n",
                    usage.input_tokens,
                    usage.output_tokens,
                    usage.total_tokens
                );
            }
        }
    }

    // Example 3: Multi-turn conversation
    println!("📝 Example 3: Multi-turn Conversation");
    println!("─────────────────────────────────────────────────\n");

    let messages = vec![
        Message::human("What's the capital of France?"),
        Message::ai("The capital of France is Paris."),
        Message::human("What's the population?"),
    ];

    let result = bedrock.generate(&messages, None, None, None, None).await?;

    for generation in &result.generations {
        println!("{}\n", generation.message.as_text());

        if let Message::AI { usage_metadata, .. } = &generation.message {
            if let Some(usage) = usage_metadata {
                println!("📊 Tokens: {} input + {} output = {} total\n",
                    usage.input_tokens,
                    usage.output_tokens,
                    usage.total_tokens
                );
            }
        }
    }

    println!("╔══════════════════════════════════════════════════╗");
    println!("║   ✅ Demo Complete                               ║");
    println!("╚══════════════════════════════════════════════════╝");

    Ok(())
}
