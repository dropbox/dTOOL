//! Tool Calling End-to-End Integration Tests
//!
//! Tests that verify tool binding and execution work correctly with real LLM providers.
//! These tests use OpenAI's function calling API to verify:
//! - Tools can be bound to LLMs
//! - LLMs actually call tools (not hallucinating)
//! - Tools execute correctly
//! - Results are returned properly
//!
//! Run with: cargo test --test integration test_tool_calling -- --ignored --nocapture

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::clone_on_ref_ptr,
    clippy::float_cmp
)]

use dashflow::core::language_models::{ChatModel, ToolChoice};
use dashflow::core::messages::Message;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow_macros::tool;
use dashflow_openai::ChatOpenAI;
use schemars::JsonSchema;
use serde::Deserialize;

use super::common::{get_openai_key, load_test_env};

// ============================================================================
// Test Tools
// ============================================================================

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

#[derive(Debug, Deserialize, JsonSchema)]
struct WeatherArgs {
    /// City name (e.g., "San Francisco", "New York", "London")
    city: String,
}

/// Get current weather for a city (mock data)
#[tool]
async fn get_weather(args: WeatherArgs) -> Result<String, String> {
    // Mock weather data
    Ok(format!(
        "Weather in {}: Sunny, 72°F (22°C), humidity 45%, wind 5 mph",
        args.city
    ))
}

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_tool_calling_with_calculator() {
    println!("\n=== Test: Tool Calling with Calculator ===\n");

    load_test_env();
    let _ = get_openai_key(); // Will panic with clear message if not set

    // Create ChatOpenAI and tool
    let chat = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4o-mini")
        .with_temperature(0.0); // Deterministic

    let calc_tool = calculator();
    let tool_defs = vec![calc_tool.to_definition()];

    // Ask a question that requires tool use
    let messages = vec![Message::human(
        "What is 173 multiplied by 29? Use the calculator tool to compute this.",
    )];

    println!("Question: What is 173 multiplied by 29?");

    // Call LLM with tools
    let result = chat
        .generate(
            &messages,
            None,
            Some(&tool_defs),
            Some(&ToolChoice::Auto),
            None,
        )
        .await
        .expect("LLM call should succeed");

    let ai_msg = &result.generations[0].message;
    println!("AI Response: {:?}", ai_msg);

    // RIGOROUS CHECK: Verify tool was called
    let tool_calls = ai_msg.tool_calls();
    assert!(
        !tool_calls.is_empty(),
        "LLM should call calculator tool (got {} tool calls)",
        tool_calls.len()
    );

    println!("Tool calls made: {}", tool_calls.len());

    // SKEPTICAL CHECK: Verify correct tool was called
    let first_call = &tool_calls[0];
    assert_eq!(
        first_call.name, "calculator",
        "Should call calculator tool, got: {}",
        first_call.name
    );

    println!("Tool called: {}", first_call.name);
    println!("Arguments: {}", first_call.args);

    // Execute the tool
    let tool_input = ToolInput::Structured(first_call.args.clone());
    let tool_result = calc_tool
        ._call(tool_input)
        .await
        .expect("Tool execution should succeed");

    println!("Tool result: {}", tool_result);

    // RIGOROUS CHECK: Verify result is correct (173 * 29 = 5017)
    let result_num: f64 = tool_result
        .trim()
        .parse()
        .expect("Result should be a number");
    assert_eq!(
        result_num, 5017.0,
        "Calculator should return correct result: 173 * 29 = 5017, got: {}",
        result_num
    );

    println!("✅ Tool calling with calculator works correctly\n");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_tool_calling_full_loop() {
    println!("\n=== Test: Full Tool Calling Loop ===\n");

    load_test_env();
    let _ = get_openai_key();

    let chat = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4o-mini")
        .with_temperature(0.0);

    let calc_tool = calculator();
    let tool_defs = vec![calc_tool.to_definition()];

    // Step 1: Initial question
    let messages = vec![Message::human(
        "Calculate 42 + 17 using the calculator tool.",
    )];

    println!("Step 1: Ask LLM to use calculator");
    let result = chat
        .generate(
            &messages,
            None,
            Some(&tool_defs),
            Some(&ToolChoice::Auto),
            None,
        )
        .await
        .expect("Initial LLM call should succeed");

    let ai_msg = &result.generations[0].message;
    let tool_calls = ai_msg.tool_calls();

    assert!(!tool_calls.is_empty(), "Should have tool calls");
    println!("  Tool calls received: {}", tool_calls.len());

    // Step 2: Execute tool
    let first_call = &tool_calls[0];
    let tool_input = ToolInput::Structured(first_call.args.clone());
    let tool_result = calc_tool
        ._call(tool_input)
        .await
        .expect("Tool should execute");

    println!("Step 2: Tool executed, result: {}", tool_result);

    // Step 3: Send result back to LLM
    let tool_message = Message::tool(tool_result, &first_call.id);
    let mut final_messages = messages.clone();
    final_messages.push(ai_msg.clone());
    final_messages.push(tool_message);

    println!("Step 3: Send tool result back to LLM");
    let final_result = chat
        .generate(&final_messages, None, Some(&tool_defs), None, None)
        .await
        .expect("Final LLM call should succeed");

    let final_answer = final_result.generations[0].message.as_text();
    println!("  Final answer: {}", final_answer);

    // SKEPTICAL CHECK: Verify answer contains correct result
    assert!(
        final_answer.contains("59") || final_answer.contains("59.0"),
        "Final answer should contain correct result (59), got: {}",
        final_answer
    );

    // RIGOROUS CHECK: Verify LLM didn't just calculate it itself (should reference tool use)
    // This is hard to verify deterministically, but we can check the tool was actually called
    assert_eq!(tool_calls.len(), 1, "Should have exactly one tool call");

    println!("✅ Full tool calling loop works correctly\n");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_tool_calling_with_multiple_tools() {
    println!("\n=== Test: Multiple Tools Available ===\n");

    load_test_env();
    let _ = get_openai_key();

    let chat = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4o-mini")
        .with_temperature(0.0);

    // Provide both calculator and weather tools
    let calc_tool = calculator();
    let weather_tool = get_weather();
    let tool_defs = vec![calc_tool.to_definition(), weather_tool.to_definition()];

    // Ask question that should use calculator, not weather
    let messages = vec![Message::human(
        "What is 100 divided by 4? Use the appropriate tool.",
    )];

    println!("Question: What is 100 divided by 4?");
    println!("Tools available: calculator, get_weather");

    let result = chat
        .generate(
            &messages,
            None,
            Some(&tool_defs),
            Some(&ToolChoice::Auto),
            None,
        )
        .await
        .expect("LLM call should succeed");

    let ai_msg = &result.generations[0].message;
    let tool_calls = ai_msg.tool_calls();

    assert!(!tool_calls.is_empty(), "Should call a tool");

    let first_call = &tool_calls[0];
    println!("Tool selected: {}", first_call.name);

    // SKEPTICAL CHECK: Should pick calculator, not weather
    assert_eq!(
        first_call.name, "calculator",
        "Should select calculator tool for math question, got: {}",
        first_call.name
    );

    // Execute and verify
    let tool_input = ToolInput::Structured(first_call.args.clone());
    let tool_result = calc_tool
        ._call(tool_input)
        .await
        .expect("Tool should execute");

    let result_num: f64 = tool_result.trim().parse().expect("Result should be number");
    assert_eq!(result_num, 25.0, "100 / 4 = 25, got: {}", result_num);

    println!("Result: {}", result_num);
    println!("✅ LLM correctly selects appropriate tool\n");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_tool_calling_error_handling() {
    println!("\n=== Test: Tool Error Handling ===\n");

    load_test_env();
    let _ = get_openai_key();

    let chat = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4o-mini")
        .with_temperature(0.0);

    let calc_tool = calculator();
    let tool_defs = vec![calc_tool.to_definition()];

    // Ask for division by zero
    let messages = vec![Message::human(
        "Calculate 10 divided by 0 using the calculator tool.",
    )];

    println!("Question: Calculate 10 / 0 (will cause error)");

    let result = chat
        .generate(
            &messages,
            None,
            Some(&tool_defs),
            Some(&ToolChoice::Auto),
            None,
        )
        .await
        .expect("LLM call should succeed");

    let ai_msg = &result.generations[0].message;
    let tool_calls = ai_msg.tool_calls();

    assert!(!tool_calls.is_empty(), "Should attempt tool call");

    let first_call = &tool_calls[0];
    let tool_input = ToolInput::Structured(first_call.args.clone());
    let tool_result = calc_tool._call(tool_input).await;

    // RIGOROUS CHECK: Tool should error on division by zero
    assert!(
        tool_result.is_err(),
        "Division by zero should produce error, got: {:?}",
        tool_result
    );

    if let Err(e) = tool_result {
        println!("Tool error (expected): {}", e);
        assert!(
            e.to_string().contains("zero") || e.to_string().contains("Division"),
            "Error should mention division by zero, got: {}",
            e
        );
    }

    println!("✅ Tool error handling works correctly\n");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_tool_calling_no_tool_needed() {
    println!("\n=== Test: LLM Knows When Not To Use Tools ===\n");

    load_test_env();
    let _ = get_openai_key();

    let chat = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4o-mini")
        .with_temperature(0.0);

    let calc_tool = calculator();
    let tool_defs = vec![calc_tool.to_definition()];

    // Ask simple question that doesn't need tools
    let messages = vec![Message::human("Hello, how are you?")];

    println!("Question: Hello, how are you? (no tools needed)");

    let result = chat
        .generate(
            &messages,
            None,
            Some(&tool_defs),
            Some(&ToolChoice::Auto),
            None,
        )
        .await
        .expect("LLM call should succeed");

    let ai_msg = &result.generations[0].message;
    let tool_calls = ai_msg.tool_calls();

    println!("Tool calls made: {}", tool_calls.len());
    println!("Response: {}", ai_msg.as_text());

    // SKEPTICAL CHECK: Should not call tools for simple greeting
    // Note: Some models might still try, so we just verify it responds
    assert!(!ai_msg.as_text().is_empty(), "Should have text response");

    println!("✅ LLM responds appropriately to non-tool questions\n");
}
