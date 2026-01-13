//! Tool Calling Example with #[tool] Macro
//!
//! This example demonstrates:
//! 1. Defining tools using the `#[tool]` macro
//! 2. Passing tools to OpenAI for function calling
//! 3. Executing tool calls from the model's response
//!
//! Prerequisites:
//! - Set OPENAI_API_KEY environment variable
//!
//! Run with: cargo run --package dashflow-openai --example tool_calling_with_macro

use dashflow::core::error::Result;
use dashflow::core::language_models::{ChatModel, ToolChoice, ToolDefinition};
use dashflow::core::messages::{Message, MessageContent};
use dashflow::core::tools::{Tool, ToolInput};
use dashflow_macros::tool;
use dashflow_openai::ChatOpenAI;
use schemars::JsonSchema;
use serde::Deserialize;

// ============================================================================
// Tool Definitions using #[tool] macro
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
struct WeatherArgs {
    /// City name (e.g., "San Francisco", "New York", "London")
    city: String,
}

/// Get current weather for a city
#[tool]
async fn get_weather(args: WeatherArgs) -> Result<String, String> {
    // Mock weather data
    Ok(format!(
        "Weather in {}: Sunny, 72°F (22°C), humidity 45%, wind 5 mph",
        args.city
    ))
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CalculatorArgs {
    /// First number
    a: f64,
    /// Second number
    b: f64,
    /// Operation: add, subtract, multiply, divide
    operation: String,
}

/// Performs basic arithmetic operations
#[tool]
fn calculator(args: CalculatorArgs) -> Result<String, String> {
    let result = match args.operation.as_str() {
        "add" => args.a + args.b,
        "subtract" => args.a - args.b,
        "multiply" => args.a * args.b,
        "divide" => {
            if args.b == 0.0 {
                return Err("Division by zero".to_string());
            }
            args.a / args.b
        }
        _ => return Err(format!("Unknown operation: {}", args.operation)),
    };
    Ok(result.to_string())
}

// ============================================================================
// Main Example
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Tool Calling with #[tool] Macro ===\n");

    // Check for API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("Error: OPENAI_API_KEY environment variable not set");
        eprintln!("Please set it and try again: export OPENAI_API_KEY=sk-...");
        std::process::exit(1);
    }

    // 1. Create tools using the #[tool] macro
    println!("Step 1: Creating tools using #[tool] macro");
    let weather = get_weather();
    let calc = calculator();

    println!("  - {}: {}", weather.name(), weather.description());
    println!("  - {}: {}", calc.name(), calc.description());
    println!();

    // 2. Convert tools to ToolDefinitions for OpenAI
    let tool_defs = vec![weather.to_definition(), calc.to_definition()];

    println!("Step 2: Tool schemas generated:");
    for def in &tool_defs {
        println!("  - {} parameters: {}", def.name, def.parameters);
    }
    println!();

    // 3. Create chat model
    println!("Step 3: Creating ChatOpenAI model");
    let model = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
    println!("Model: gpt-4o-mini\n");

    // 4. Example conversations
    println!("=== Example 1: Weather Query ===");
    run_weather_example(&model, &tool_defs, &weather).await?;

    println!("\n=== Example 2: Calculator Query ===");
    run_calculator_example(&model, &tool_defs, &calc).await?;

    println!("\n=== Complete! ===");
    Ok(())
}

// Example 1: Weather query
async fn run_weather_example(
    model: &ChatOpenAI,
    tool_defs: &[ToolDefinition],
    weather_tool: &impl Tool,
) -> Result<()> {
    let query = "What's the weather like in San Francisco?";
    println!("User: {}", query);

    let messages = vec![Message::human(query)];

    // Call model with tools
    let result = model
        ._generate(
            &messages,
            None,
            Some(tool_defs),
            Some(&ToolChoice::Auto),
            None,
        )
        .await?;

    let response = &result.generations[0].message;
    match response {
        Message::AI { tool_calls, .. } if !tool_calls.is_empty() => {
            println!("  Model wants to call: {}", tool_calls[0].name);
            println!("  With arguments: {}", tool_calls[0].args);

            // Execute the tool
            let tool_result = weather_tool
                ._call(ToolInput::Structured(tool_calls[0].args.clone()))
                .await?;

            println!("  Tool result: {}", tool_result);

            // Send result back to model for final answer
            let mut final_messages = messages.clone();
            final_messages.push(response.clone());
            final_messages.push(Message::Tool {
                content: MessageContent::Text(tool_result),
                tool_call_id: tool_calls[0].id.clone(),
                artifact: None,
                status: None,
                fields: Default::default(),
            });

            let final_result = model
                ._generate(&final_messages, None, Some(tool_defs), None, None)
                .await?;

            if let Message::AI { content, .. } = &final_result.generations[0].message {
                println!("  Final answer: {}", content.as_text());
            }
        }
        Message::AI { content, .. } => {
            println!("  Direct answer (no tool call): {}", content.as_text());
        }
        _ => {}
    }

    Ok(())
}

// Example 2: Calculator query
async fn run_calculator_example(
    model: &ChatOpenAI,
    tool_defs: &[ToolDefinition],
    calc_tool: &impl Tool,
) -> Result<()> {
    let query = "What is 234 multiplied by 567?";
    println!("User: {}", query);

    let messages = vec![Message::human(query)];

    // Call model with tools
    let result = model
        ._generate(
            &messages,
            None,
            Some(tool_defs),
            Some(&ToolChoice::Auto),
            None,
        )
        .await?;

    let response = &result.generations[0].message;
    match response {
        Message::AI { tool_calls, .. } if !tool_calls.is_empty() => {
            println!("  Model wants to call: {}", tool_calls[0].name);
            println!("  With arguments: {}", tool_calls[0].args);

            // Execute the tool
            let tool_result = calc_tool
                ._call(ToolInput::Structured(tool_calls[0].args.clone()))
                .await?;

            println!("  Tool result: {}", tool_result);

            // Send result back to model for final answer
            let mut final_messages = messages.clone();
            final_messages.push(response.clone());
            final_messages.push(Message::Tool {
                content: MessageContent::Text(tool_result),
                tool_call_id: tool_calls[0].id.clone(),
                artifact: None,
                status: None,
                fields: Default::default(),
            });

            let final_result = model
                ._generate(&final_messages, None, Some(tool_defs), None, None)
                .await?;

            if let Message::AI { content, .. } = &final_result.generations[0].message {
                println!("  Final answer: {}", content.as_text());
            }
        }
        Message::AI { content, .. } => {
            println!("  Direct answer (no tool call): {}", content.as_text());
        }
        _ => {}
    }

    Ok(())
}
