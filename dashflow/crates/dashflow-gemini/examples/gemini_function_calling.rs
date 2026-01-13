//! Function calling example with Google Gemini
//!
//! This example demonstrates:
//! - Defining tools/functions for the model
//! - Function calling with Gemini
//! - Handling tool calls in responses
//!
//! # Setup
//!
//! Set your Gemini API key:
//! ```bash
//! export GEMINI_API_KEY=your-api-key-here
//! ```
//!
//! Get your API key from: https://aistudio.google.com/app/apikey
//!
//! # Run
//!
//! ```bash
//! cargo run --example gemini_function_calling --features dashflow-gemini
//! ```

use dashflow::core::language_models::{ChatModel, ToolDefinition};
use dashflow::core::messages::Message;
use dashflow_gemini::ChatGemini;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check for API key
    if std::env::var("GEMINI_API_KEY").is_err() {
        eprintln!("Error: GEMINI_API_KEY environment variable not set");
        eprintln!("Get your API key from: https://aistudio.google.com/app/apikey");
        std::process::exit(1);
    }

    // Create a Gemini model instance
    println!("Creating ChatGemini model...");
    let model = ChatGemini::new()
        .with_model("gemini-2.0-flash-exp")
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
        "What's the weather like in San Francisco? Also, what's 15 times 23?",
    )];

    // Generate response with tools
    println!("\nSending message to Gemini with tools...");
    println!("Question: What's the weather like in San Francisco? Also, what's 15 times 23?");
    println!("\nWaiting for response...\n");

    let result = model
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
                            println!("  Simulated Result: Sunny, 72°F");
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

    Ok(())
}
