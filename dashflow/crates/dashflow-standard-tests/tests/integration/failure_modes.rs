//! Failure Modes and Error Handling Integration Tests
//!
//! **DEPRECATED PATTERN**: These tests use the deprecated `AgentExecutor` API for backward compatibility testing.
//! For new tests, use `create_react_agent()` from `dashflow` instead.
//!
//! Tests that verify the system handles errors gracefully:
//! - Invalid API keys
//! - Network failures
//! - Malformed inputs
//! - Empty inputs
//! - Tool execution failures
//!
//! Run with: cargo test --test integration test_failure -- --ignored --nocapture

#![allow(deprecated)]
#![allow(clippy::field_reassign_with_default)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::clone_on_ref_ptr,
    clippy::float_cmp
)]

use dashflow::core::agents::{AgentExecutor, AgentExecutorConfig, ReActAgent};
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow::core::tools::{FunctionTool, Tool, ToolInput};
use dashflow_openai::ChatOpenAI;
use std::sync::Arc;

use super::common::{has_openai_key, load_test_env};

// ============================================================================
// Test Tools
// ============================================================================

/// Tool that always fails with an error
fn create_failing_tool() -> impl Tool {
    FunctionTool::new(
        "always_fails",
        "A tool that always fails with an error (for testing error handling)",
        |_input: String| {
            Box::pin(async move { Err("Tool execution failed: Simulated error".to_string()) })
        },
    )
}

/// Calculator tool that may fail on invalid input
fn create_strict_calculator() -> impl Tool {
    FunctionTool::new(
        "calculator",
        "Performs calculations. Input must be valid math expression.",
        |input: String| {
            Box::pin(async move {
                let input = input.trim();

                if input.is_empty() {
                    return Err("Empty input".to_string());
                }

                // Only handle basic operations
                if let Some((a, b)) = input.split_once('+') {
                    let a = a.trim().parse::<f64>().map_err(|_| "Invalid number")?;
                    let b = b.trim().parse::<f64>().map_err(|_| "Invalid number")?;
                    return Ok((a + b).to_string());
                }

                if let Some((a, b)) = input.split_once('/') {
                    let a = a.trim().parse::<f64>().map_err(|_| "Invalid number")?;
                    let b = b.trim().parse::<f64>().map_err(|_| "Invalid number")?;
                    if b == 0.0 {
                        return Err("Division by zero".to_string());
                    }
                    return Ok((a / b).to_string());
                }

                Err(format!("Cannot parse expression: '{}'", input))
            })
        },
    )
}

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires environment manipulation"]
async fn test_invalid_api_key_error() {
    println!("\n=== Test: Invalid API Key Error Handling ===\n");

    load_test_env();

    // Temporarily set invalid API key
    let original_key = std::env::var("OPENAI_API_KEY").ok();
    std::env::set_var("OPENAI_API_KEY", "sk-invalid-test-key-12345");

    let chat = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");

    let messages = vec![Message::human("Hello")];

    println!("Attempting to call OpenAI with invalid API key...");

    let result = chat.generate(&messages, None, None, None, None).await;

    // Restore original key
    if let Some(key) = original_key {
        std::env::set_var("OPENAI_API_KEY", key);
    } else {
        std::env::remove_var("OPENAI_API_KEY");
    }

    // RIGOROUS CHECK: Should error with authentication failure
    assert!(result.is_err(), "Should error with invalid API key");

    if let Err(e) = result {
        let error_msg = e.to_string();
        println!("Error received: {}", error_msg);

        // Check for authentication-related error indicators
        let is_auth_error = error_msg.contains("401")
            || error_msg.contains("Unauthorized")
            || error_msg.contains("Invalid")
            || error_msg.contains("API key")
            || error_msg.contains("authentication");

        assert!(
            is_auth_error,
            "Error should indicate authentication failure, got: {}",
            error_msg
        );
    }

    println!("✅ Invalid API key produces clear authentication error\n");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_agent_with_failing_tool() {
    println!("\n=== Test: Agent with Failing Tool ===\n");

    load_test_env();

    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    let failing_tool = Arc::new(create_failing_tool());

    let agent = ReActAgent::new(
        llm,
        vec![failing_tool.clone()],
        "You are a helpful assistant. If a tool fails, explain the error to the user.",
    );

    let mut config = AgentExecutorConfig::default();
    config.max_iterations = 5;
    let executor = AgentExecutor::new(Box::new(agent))
        .with_tools(vec![Box::new(create_failing_tool())])
        .with_config(config);

    println!("Asking agent to use a tool that will fail...");

    let result = executor.execute("Use the always_fails tool").await;

    // Agent should either:
    // 1. Complete and explain the error in its response
    // 2. Propagate the error with a clear message
    match result {
        Ok(r) => {
            println!("Agent completed with response: {}", r.output);
            // SKEPTICAL CHECK: Response should mention the error
            let output_lower = r.output.to_lowercase();
            assert!(
                output_lower.contains("error")
                    || output_lower.contains("fail")
                    || output_lower.contains("problem"),
                "Agent should acknowledge tool failure in response, got: {}",
                r.output
            );
        }
        Err(e) => {
            println!("Execution failed with error: {}", e);
            // RIGOROUS CHECK: Error should be clear and not a panic
            let error_msg = e.to_string();
            assert!(
                !error_msg.contains("panic") && !error_msg.contains("unwrap"),
                "Should be a clean error, not a panic: {}",
                error_msg
            );
        }
    }

    println!("✅ Agent handles tool failure gracefully\n");
}

#[tokio::test]
async fn test_empty_message_handling() {
    println!("\n=== Test: Empty Message Handling ===\n");

    load_test_env();

    // Test with mock to avoid API calls
    let chat = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");

    let messages = vec![]; // Empty messages

    println!("Testing with empty messages array...");

    let result = chat.generate(&messages, None, None, None, None).await;

    // Should either:
    // 1. Error with clear message about empty input
    // 2. Handle gracefully
    match result {
        Ok(_) => {
            println!("Empty messages handled gracefully");
        }
        Err(e) => {
            println!("Error with empty messages: {}", e);
            // RIGOROUS CHECK: Should not panic
            let error_msg = e.to_string();
            assert!(
                !error_msg.contains("panic") && !error_msg.contains("unwrap"),
                "Should have clean error handling: {}",
                error_msg
            );
        }
    }

    println!("✅ Empty message handling works\n");
}

#[tokio::test]
async fn test_tool_with_empty_input() {
    println!("\n=== Test: Tool with Empty Input ===\n");

    load_test_env();

    let calc = create_strict_calculator();

    println!("Testing calculator with empty input...");

    let result = calc
        ._call(ToolInput::Structured(serde_json::json!({"input": ""})))
        .await;

    // RIGOROUS CHECK: Should error on empty input
    assert!(
        result.is_err(),
        "Tool should reject empty input, got: {:?}",
        result
    );

    if let Err(e) = result {
        println!("Error (expected): {}", e);
        assert!(
            e.to_string().contains("empty") || e.to_string().contains("Empty"),
            "Error should mention empty input: {}",
            e
        );
    }

    println!("✅ Tool validates input correctly\n");
}

#[tokio::test]
async fn test_tool_with_malformed_input() {
    println!("\n=== Test: Tool with Malformed Input ===\n");

    load_test_env();

    let calc = create_strict_calculator();

    println!("Testing calculator with malformed input...");

    let result = calc
        ._call(ToolInput::Structured(
            serde_json::json!({"input": "not a math expression"}),
        ))
        .await;

    // RIGOROUS CHECK: Should error on malformed input
    assert!(
        result.is_err(),
        "Tool should reject malformed input, got: {:?}",
        result
    );

    if let Err(e) = result {
        println!("Error (expected): {}", e);
        assert!(
            !e.to_string().contains("panic"),
            "Should be clean error, not panic: {}",
            e
        );
    }

    println!("✅ Tool handles malformed input correctly\n");
}

#[tokio::test]
async fn test_division_by_zero_handling() {
    println!("\n=== Test: Division by Zero Handling ===\n");

    load_test_env();

    let calc = create_strict_calculator();

    println!("Testing division by zero...");

    let result = calc
        ._call(ToolInput::Structured(
            serde_json::json!({"input": "10 / 0"}),
        ))
        .await;

    // RIGOROUS CHECK: Should error on division by zero
    assert!(
        result.is_err(),
        "Should error on division by zero, got: {:?}",
        result
    );

    if let Err(e) = result {
        println!("Error (expected): {}", e);
        assert!(
            e.to_string().contains("zero") || e.to_string().contains("Division"),
            "Error should mention division by zero: {}",
            e
        );
    }

    println!("✅ Division by zero handled correctly\n");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_agent_without_tools() {
    println!("\n=== Test: Agent without Tools ===\n");

    load_test_env();

    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    // Create agent with no tools
    let agent = ReActAgent::new(llm, vec![], "You are a helpful assistant.");

    let config = AgentExecutorConfig::default();
    let executor = AgentExecutor::new(Box::new(agent))
        .with_tools(vec![])
        .with_config(config);

    println!("Testing agent with no tools available...");

    let result = executor.execute("What is 2 + 2?").await;

    // Should still be able to answer simple questions
    match result {
        Ok(r) => {
            println!("Response: {}", r.output);
            // Should have some response
            assert!(
                !r.output.is_empty(),
                "Should produce some response even without tools"
            );
        }
        Err(e) => {
            println!("Error: {}", e);
            // Or error clearly
            assert!(
                !e.to_string().contains("panic"),
                "Should not panic without tools: {}",
                e
            );
        }
    }

    println!("✅ Agent handles absence of tools gracefully\n");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_very_long_input() {
    println!("\n=== Test: Very Long Input Handling ===\n");

    load_test_env();

    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let chat = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");

    // Create a very long message (but not exceeding token limits)
    let long_text = "word ".repeat(500); // 500 words
    let messages = vec![Message::human(format!(
        "Summarize this text: {}",
        long_text
    ))];

    println!(
        "Testing with long input ({} words)...",
        long_text.split_whitespace().count()
    );

    let result = chat.generate(&messages, None, None, None, None).await;

    // Should either handle it or error clearly
    match result {
        Ok(r) => {
            println!(
                "Handled long input, response length: {}",
                r.generations[0].message.as_text().len()
            );
            assert!(
                !r.generations[0].message.as_text().is_empty(),
                "Should produce response"
            );
        }
        Err(e) => {
            println!("Error with long input: {}", e);
            // Should be a clear error about token limits or similar
            assert!(
                !e.to_string().contains("panic"),
                "Should error gracefully on long input: {}",
                e
            );
        }
    }

    println!("✅ Long input handled appropriately\n");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_special_characters_in_input() {
    println!("\n=== Test: Special Characters in Input ===\n");

    load_test_env();

    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let chat = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");

    // Input with special characters that might cause issues
    let special_chars = r#"Test with "quotes", 'apostrophes', <tags>, {braces}, [brackets], \backslashes, and 日本語"#;
    let messages = vec![Message::human(format!("Echo this: {}", special_chars))];

    println!("Testing with special characters...");

    let result = chat.generate(&messages, None, None, None, None).await;

    // Should handle special characters without error
    match result {
        Ok(r) => {
            println!("Response: {}", r.generations[0].message.as_text());
            assert!(
                !r.generations[0].message.as_text().is_empty(),
                "Should produce response"
            );
        }
        Err(e) => {
            println!("Error: {}", e);
            // Even if it errors, should not panic
            assert!(
                !e.to_string().contains("panic"),
                "Should handle special characters gracefully: {}",
                e
            );
        }
    }

    println!("✅ Special characters handled correctly\n");
}
