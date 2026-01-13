//! Function calling example with Azure OpenAI.
//!
//! This example demonstrates how to use Azure OpenAI with tools/function calling.
//!
//! # Required Environment Variables
//!
//! - `AZURE_OPENAI_API_KEY`: Your Azure OpenAI API key
//! - `AZURE_OPENAI_ENDPOINT`: Your Azure OpenAI endpoint
//! - `AZURE_OPENAI_DEPLOYMENT_NAME`: Your deployment name (must support function calling)
//!
//! # Usage
//!
//! ```bash
//! export AZURE_OPENAI_API_KEY="your-api-key"
//! export AZURE_OPENAI_ENDPOINT="https://my-resource.openai.azure.com"
//! export AZURE_OPENAI_DEPLOYMENT_NAME="gpt-4"
//! cargo run --example azure_openai_tools
//! ```

use dashflow::core::language_models::{ChatModel, ToolDefinition};
use dashflow::core::messages::Message;
use dashflow_azure_openai::ChatAzureOpenAI;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = match std::env::var("AZURE_OPENAI_ENDPOINT") {
        Ok(endpoint) => endpoint,
        Err(_) => {
            println!("AZURE_OPENAI_ENDPOINT environment variable not set.");
            println!("Example: export AZURE_OPENAI_ENDPOINT=\"https://my-resource.openai.azure.com\"");
            return Ok(());
        }
    };

    // Define tools
    let tools = vec![
        ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get the current weather for a location".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "The city and state, e.g. San Francisco, CA"
                    },
                    "unit": {
                        "type": "string",
                        "enum": ["celsius", "fahrenheit"],
                        "description": "The unit of temperature"
                    }
                },
                "required": ["location"]
            }),
        },
        ToolDefinition {
            name: "calculate".to_string(),
            description: "Perform a mathematical calculation".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The mathematical expression to evaluate"
                    }
                },
                "required": ["expression"]
            }),
        },
    ];

    // Initialize Azure OpenAI with tools (using deprecated with_tools API)
    #[allow(deprecated)]
    let chat = ChatAzureOpenAI::new()
        .with_deployment_name(
            std::env::var("AZURE_OPENAI_DEPLOYMENT_NAME").unwrap_or_else(|_| "gpt-4".to_string()),
        )
        .with_endpoint(endpoint)
        .with_tools(tools);

    // Send message that should trigger tool use
    println!("Asking about weather...");
    let messages = vec![Message::human(
        "What's the weather like in Seattle, WA? Use Fahrenheit.",
    )];

    let result = chat.generate(&messages, None, None, None, None).await?;

    println!("\nResponse:");
    for generation in &result.generations {
        if let Message::AI {
            content,
            tool_calls,
            usage_metadata,
            ..
        } = &generation.message
        {
            let text = content.as_text();
            if !text.is_empty() {
                println!("Content: {}", text);
            }

            if !tool_calls.is_empty() {
                println!("\nTool Calls:");
                for tool_call in tool_calls {
                    println!("  - Function: {}", tool_call.name);
                    println!("    ID: {}", tool_call.id);
                    println!(
                        "    Arguments: {}",
                        serde_json::to_string_pretty(&tool_call.args)?
                    );
                }
            }

            // Print usage metadata if available
            if let Some(usage) = usage_metadata {
                println!("\nUsage:");
                println!("  Input tokens: {}", usage.input_tokens);
                println!("  Output tokens: {}", usage.output_tokens);
                println!("  Total tokens: {}", usage.total_tokens);
            }
        }
    }

    Ok(())
}
