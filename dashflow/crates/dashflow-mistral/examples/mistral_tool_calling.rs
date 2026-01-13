use dashflow::core::config_loader::SecretReference;
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow_mistral::ChatMistralAI;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Mistral AI Tool Calling Example ===\n");

    // Define tools in OpenAI-compatible format
    let calculator_tool = json!({
        "type": "function",
        "function": {
            "name": "calculator",
            "description": "Performs basic arithmetic operations",
            "parameters": {
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "The operation to perform: add, subtract, multiply, or divide"
                    },
                    "a": {
                        "type": "string",
                        "description": "The first number"
                    },
                    "b": {
                        "type": "string",
                        "description": "The second number"
                    }
                },
                "required": ["operation", "a", "b"]
            }
        }
    });

    let weather_tool = json!({
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Get the current weather for a location",
            "parameters": {
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "The city name, e.g., 'San Francisco'"
                    },
                    "units": {
                        "type": "string",
                        "description": "Temperature units: 'celsius' or 'fahrenheit'"
                    }
                },
                "required": ["location"]
            }
        }
    });

    // Initialize the Mistral chat model with tools (using deprecated with_tools API)
    // Requires MISTRAL_API_KEY environment variable
    #[allow(deprecated)]
    let model = ChatMistralAI::with_api_key(SecretReference::from_env("MISTRAL_API_KEY").resolve()?)?
        .with_model("mistral-large-latest") // Use mistral-large for better tool calling
        .with_temperature(0.0) // Low temperature for more deterministic responses
        .with_tools(vec![calculator_tool, weather_tool])
        .with_tool_choice("auto"); // Let the model decide when to use tools

    println!("Available tools:");
    println!("  - calculator: Performs basic arithmetic");
    println!("  - get_weather: Gets weather for a location\n");

    // Example 1: Tool calling with calculator
    println!("Example 1: Math calculation");
    println!("User: What is 42 multiplied by 17?");

    let messages1 = vec![Message::human("What is 42 multiplied by 17?")];

    let result1 = model.generate(&messages1, None, None, None, None).await?;

    if let Some(generation) = result1.generations.first() {
        let ai_message = &generation.message;
        println!("Assistant: {}", ai_message.as_text());

        // Check if the model called any tools
        match ai_message {
            Message::AI { tool_calls, .. } if !tool_calls.is_empty() => {
                println!("\nTool calls made:");
                for (idx, tool_call) in tool_calls.iter().enumerate() {
                    println!("  {}. {} (id: {})", idx + 1, tool_call.name, tool_call.id);
                    println!(
                        "     Arguments: {}",
                        serde_json::to_string_pretty(&tool_call.args)?
                    );

                    // Execute the tool (mock execution)
                    if tool_call.name == "calculator" {
                        let operation = tool_call.args["operation"].as_str().unwrap_or("");
                        let a: f64 = tool_call.args["a"]
                            .as_str()
                            .unwrap_or("0")
                            .parse()
                            .unwrap_or(0.0);
                        let b: f64 = tool_call.args["b"]
                            .as_str()
                            .unwrap_or("0")
                            .parse()
                            .unwrap_or(0.0);

                        let result = match operation {
                            "add" => a + b,
                            "subtract" => a - b,
                            "multiply" => a * b,
                            "divide" if b != 0.0 => a / b,
                            _ => 0.0,
                        };

                        println!("     Result: {}", result);
                    }
                }
            }
            _ => {
                println!("\n(No tool calls made)");
            }
        }
    }

    println!("\n{}\n", "=".repeat(50));

    // Example 2: Tool calling with weather
    println!("Example 2: Weather query");
    println!("User: What's the weather like in Paris?");

    let messages2 = vec![Message::human("What's the weather like in Paris?")];

    let result2 = model.generate(&messages2, None, None, None, None).await?;

    if let Some(generation) = result2.generations.first() {
        let ai_message = &generation.message;
        println!("Assistant: {}", ai_message.as_text());

        match ai_message {
            Message::AI { tool_calls, .. } if !tool_calls.is_empty() => {
                println!("\nTool calls made:");
                for (idx, tool_call) in tool_calls.iter().enumerate() {
                    println!("  {}. {} (id: {})", idx + 1, tool_call.name, tool_call.id);
                    println!(
                        "     Arguments: {}",
                        serde_json::to_string_pretty(&tool_call.args)?
                    );

                    // Execute the tool (mock execution)
                    if tool_call.name == "get_weather" {
                        let location = tool_call.args["location"].as_str().unwrap_or("Unknown");
                        println!("     Mock Result: Sunny, 22°C in {}", location);
                    }
                }
            }
            _ => {
                println!("\n(No tool calls made)");
            }
        }
    }

    println!("\n{}\n", "=".repeat(50));

    // Example 3: Multi-turn conversation with tool results
    println!("Example 3: Multi-turn conversation");
    println!("User: Calculate 15 + 27");

    let messages3 = vec![Message::human("Calculate 15 + 27")];

    let result3 = model.generate(&messages3, None, None, None, None).await?;

    if let Some(generation) = result3.generations.first() {
        let ai_message = &generation.message;
        println!("Assistant: {}", ai_message.as_text());

        match ai_message {
            Message::AI { tool_calls, .. } if !tool_calls.is_empty() => {
                println!("\nTool calls:");
                for tool_call in tool_calls.iter() {
                    println!(
                        "  - {}: {}",
                        tool_call.name,
                        serde_json::to_string(&tool_call.args)?
                    );
                }

                // Simulate executing the tool and continuing the conversation
                println!("\n(In a real application, you would:");
                println!(" 1. Execute the tool with the provided arguments");
                println!(" 2. Add a Message::Tool with the result to the conversation");
                println!(" 3. Call the model again with the updated message history");
                println!(" 4. The model would then provide a natural language response)");
            }
            _ => {
                println!("\n(No tool calls made - model answered directly)");
            }
        }
    }

    println!("\n{}", "=".repeat(50));
    println!("\nNote: This example demonstrates tool calling configuration.");
    println!("In production, you would:");
    println!("  1. Execute the actual tool functions");
    println!("  2. Add tool results as Message::Tool to the conversation");
    println!("  3. Continue the conversation with the model");

    Ok(())
}
