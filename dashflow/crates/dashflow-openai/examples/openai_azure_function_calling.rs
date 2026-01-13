// Azure OpenAI function calling example
//
// This example demonstrates:
// - Defining tools/functions for the model
// - Function calling with Azure OpenAI
// - Handling tool calls in responses
//
// Run with: cargo run -p dashflow-openai --example azure_openai_function_calling
//
// Required environment variables:
// - AZURE_OPENAI_API_KEY: Your Azure OpenAI API key
// - AZURE_OPENAI_ENDPOINT: Your Azure endpoint (e.g., https://your-resource.openai.azure.com)
// - AZURE_OPENAI_API_VERSION: API version (e.g., 2024-05-01-preview)

use dashflow::core::language_models::{ChatModel, ToolDefinition};
use dashflow::core::messages::Message;
use dashflow_openai::AzureChatOpenAI;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Azure OpenAI Function Calling Example ===\n");

    // Check for required environment variables
    if std::env::var("AZURE_OPENAI_API_KEY").is_err() {
        eprintln!("Error: AZURE_OPENAI_API_KEY environment variable not set");
        std::process::exit(1);
    }
    if std::env::var("AZURE_OPENAI_ENDPOINT").is_err() {
        eprintln!("Error: AZURE_OPENAI_ENDPOINT environment variable not set");
        std::process::exit(1);
    }
    if std::env::var("AZURE_OPENAI_API_VERSION").is_err() {
        eprintln!("Error: AZURE_OPENAI_API_VERSION environment variable not set");
        std::process::exit(1);
    }

    // Create AzureChatOpenAI instance
    println!("Creating AzureChatOpenAI model...");
    let chat = AzureChatOpenAI::with_config(Default::default())
        .with_model("gpt-4") // Underlying model name for tracing
        .with_deployment("gpt-4") // Your deployment name in Azure
        .with_temperature(0.0); // Use temperature 0 for deterministic tool calls

    // Define a tool for getting weather information
    let get_weather_tool = ToolDefinition {
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
                    "description": "The temperature unit"
                }
            },
            "required": ["location"]
        }),
    };

    // Define a tool for calculating
    let calculator_tool = ToolDefinition {
        name: "calculator".to_string(),
        description: "Perform basic arithmetic operations".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"],
                    "description": "The arithmetic operation to perform"
                },
                "a": {
                    "type": "number",
                    "description": "The first operand"
                },
                "b": {
                    "type": "number",
                    "description": "The second operand"
                }
            },
            "required": ["operation", "a", "b"]
        }),
    };

    let tools = vec![get_weather_tool, calculator_tool];

    // Create a message that should trigger tool usage
    let messages = vec![Message::human(
        "What's the weather like in Paris? Also, what's 42 times 17?",
    )];

    // Generate response with tools
    println!("\nSending message to Azure OpenAI with tools...");
    println!("Question: What's the weather like in Paris? Also, what's 42 times 17?");
    println!("\nWaiting for response...\n");

    let result = chat
        .generate(&messages, None, Some(&tools), None, None)
        .await?;

    // Check if the model wants to call tools
    if let Some(generation) = result.generations.first() {
        let message = &generation.message;

        // Check for tool calls
        if let Message::AI {
            tool_calls,
            content,
            ..
        } = message
        {
            if !tool_calls.is_empty() {
                println!("--- Model wants to call {} tool(s) ---\n", tool_calls.len());

                for (i, tool_call) in tool_calls.iter().enumerate() {
                    println!("Tool Call {}:", i + 1);
                    println!("  Name: {}", tool_call.name);
                    println!("  ID: {}", tool_call.id);
                    println!(
                        "  Arguments: {}",
                        serde_json::to_string_pretty(&tool_call.args)?
                    );

                    // Simulate tool execution
                    match tool_call.name.as_str() {
                        "get_weather" => {
                            println!("  Simulated Result: Cloudy, 18°C");
                        }
                        "calculator" => {
                            if let Some(op) =
                                tool_call.args.get("operation").and_then(|v| v.as_str())
                            {
                                if let (Some(a), Some(b)) = (
                                    tool_call.args.get("a").and_then(|v| v.as_f64()),
                                    tool_call.args.get("b").and_then(|v| v.as_f64()),
                                ) {
                                    let result = match op {
                                        "multiply" => a * b,
                                        "add" => a + b,
                                        "subtract" => a - b,
                                        "divide" => a / b,
                                        _ => 0.0,
                                    };
                                    println!("  Simulated Result: {}", result);
                                }
                            }
                        }
                        _ => {
                            println!("  Unknown tool");
                        }
                    }
                    println!();
                }
            } else {
                println!("No tool calls in response");
            }

            // Print any text content
            let text = content.as_text();
            if !text.is_empty() {
                println!("--- Response Text ---");
                println!("{}", text);
            }
        }

        // Print generation info
        if let Some(info) = &generation.generation_info {
            println!("\n--- Generation Metadata ---");
            println!("{}", serde_json::to_string_pretty(info)?);
        }
    } else {
        println!("No response generated");
    }

    println!("\n--- Note ---");
    println!("In a real application, you would:");
    println!("1. Execute the tool calls with the provided arguments");
    println!("2. Send the results back to the model as tool messages");
    println!("3. Get the final response synthesizing the tool results");

    println!("\n✅ Azure OpenAI function calling example complete");
    Ok(())
}
