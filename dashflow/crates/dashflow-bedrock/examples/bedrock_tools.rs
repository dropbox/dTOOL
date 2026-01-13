//! AWS Bedrock tool calling example
//!
//! This example demonstrates how to use Claude models on Bedrock with function calling.
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
//! cargo run --example bedrock_tools
//! ```

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_bedrock::ChatBedrock;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bedrock = ChatBedrock::new("us-west-2")
        .await?
        .with_model("anthropic.claude-3-5-sonnet-20241022-v2:0")
        .with_max_tokens(2048);

    println!("🚀 Bedrock ChatModel initialized with tools");
    println!("Model: anthropic.claude-3-5-sonnet-20241022-v2:0\n");

    // Define a simple weather tool
    let weather_tool = json!({
        "name": "get_weather",
        "description": "Get the current weather for a location",
        "input_schema": {
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city and state, e.g. San Francisco, CA"
                },
                "unit": {
                    "type": "string",
                    "enum": ["celsius", "fahrenheit"],
                    "description": "The unit for temperature"
                }
            },
            "required": ["location"]
        }
    });

    // Bind the tool to the model
    let bedrock_with_tools = bedrock.bind_tools(vec![weather_tool]);

    // Ask a question that should trigger tool use
    let messages = vec![Message::human("What's the weather like in Seattle, WA?")];

    println!("📤 Sending message with tool binding...\n");
    let result = bedrock_with_tools
        .generate(&messages, None, None, None, None)
        .await?;

    println!("📥 Response:");
    for generation in result.generations {
        let msg = &generation.message;

        // Print text content if any
        let content = msg.as_text();
        if !content.is_empty() {
            println!("Text: {}", content);
        }

        // Print tool calls and usage if this is an AI message
        if let Message::AI {
            tool_calls,
            usage_metadata,
            ..
        } = msg
        {
            if !tool_calls.is_empty() {
                println!("\n🔧 Tool Calls:");
                for tool_call in tool_calls {
                    println!("  Name: {}", tool_call.name);
                    println!("  ID: {}", tool_call.id);
                    println!("  Args: {}", serde_json::to_string_pretty(&tool_call.args)?);
                }
            }

            // Print usage
            if let Some(usage) = usage_metadata {
                println!("\n📊 Usage:");
                println!("  Input tokens:  {}", usage.input_tokens);
                println!("  Output tokens: {}", usage.output_tokens);
                println!("  Total tokens:  {}", usage.total_tokens);
            }
        }
    }

    // In a real application, you would:
    // 1. Execute the tool with the provided arguments
    // 2. Send the tool result back to the model with a Message::tool()
    // 3. Get the final response

    println!("\n💡 In a real application, you would execute the tool and send the result back.");

    Ok(())
}
