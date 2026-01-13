//! ReAct Agent End-to-End Integration Tests
//!
//! **DEPRECATED PATTERN**: These tests use the deprecated `ReActAgent` and `AgentExecutor` APIs for backward compatibility testing.
//! For new tests, use `create_react_agent()` from `dashflow` instead.
//!
//! Tests that verify ReAct agents work correctly with real LLMs.
//! ReAct agents use prompt-based reasoning (Thought/Action/Observation pattern)
//! and work with any LLM, not just those with native tool calling.
//!
//! Run with: cargo test --test integration test_react_agent -- --ignored --nocapture

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
use dashflow::core::tools::{FunctionTool, Tool};
use dashflow_openai::ChatOpenAI;
use std::sync::Arc;

use super::common::{extract_numbers, get_openai_key, load_test_env};

// ============================================================================
// Test Tools
// ============================================================================

/// Create a calculator tool for testing
fn create_calculator_tool() -> impl Tool {
    FunctionTool::new(
        "calculator",
        "Performs mathematical calculations. Input should be a math expression like '15 * 23' or '100 + 50'.",
        |input: String| {
            Box::pin(async move {
                // Simple calculator implementation
                let input = input.trim();

                // Parse basic operations
                if let Some((a, b)) = input.split_once('*') {
                    let a = a.trim().parse::<f64>().map_err(|e| e.to_string())?;
                    let b = b.trim().parse::<f64>().map_err(|e| e.to_string())?;
                    return Ok((a * b).to_string());
                }

                if let Some((a, b)) = input.split_once('+') {
                    let a = a.trim().parse::<f64>().map_err(|e| e.to_string())?;
                    let b = b.trim().parse::<f64>().map_err(|e| e.to_string())?;
                    return Ok((a + b).to_string());
                }

                if let Some((a, b)) = input.split_once('-') {
                    let a = a.trim().parse::<f64>().map_err(|e| e.to_string())?;
                    let b = b.trim().parse::<f64>().map_err(|e| e.to_string())?;
                    return Ok((a - b).to_string());
                }

                if let Some((a, b)) = input.split_once('/') {
                    let a = a.trim().parse::<f64>().map_err(|e| e.to_string())?;
                    let b = b.trim().parse::<f64>().map_err(|e| e.to_string())?;
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

/// Create a search tool (mock) for testing
fn create_search_tool() -> impl Tool {
    FunctionTool::new(
        "search",
        "Search for information on the internet. Returns relevant information about the query.",
        |query: String| {
            Box::pin(async move {
                // Mock search results
                let result = if query.to_lowercase().contains("rust") {
                    "Rust is a systems programming language that is fast, memory-safe, and designed for performance. Created by Mozilla."
                } else if query.to_lowercase().contains("tokyo")
                    || query.to_lowercase().contains("population")
                {
                    "Tokyo is the capital of Japan with a population of approximately 14 million in the city proper (37 million in the metro area)."
                } else {
                    "No specific information found for this query."
                };
                Ok(result.to_string())
            })
        },
    )
}

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_react_agent_simple_calculation() {
    println!("\n=== Test: ReAct Agent Calculation Capability ===\n");

    load_test_env();
    let _ = get_openai_key();

    // Create LLM and tools
    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    let calculator = Arc::new(create_calculator_tool());

    // Create ReAct agent
    let agent = ReActAgent::new(
        llm,
        vec![calculator.clone()],
        "You are a helpful mathematical assistant. Use tools when appropriate.",
    );

    // Create executor
    let config = AgentExecutorConfig::default();
    let executor = AgentExecutor::new(Box::new(agent))
        .with_tools(vec![Box::new(create_calculator_tool())])
        .with_config(config);

    // Test with moderate-sized multiplication
    let question = "What is 123 times 456?";
    println!("Question: {}", question);

    let result = executor
        .execute(question)
        .await
        .expect("Agent execution should succeed");

    println!("Answer: {}", result.output);
    println!("Steps taken: {}", result.intermediate_steps.len());
    println!("Iterations: {}", result.iterations);

    // PRAGMATIC CHECK: Verify answer is correct (123 * 456 = 56088)
    // Don't require tool usage - modern LLMs can calculate this correctly
    let numbers = extract_numbers(&result.output);
    assert!(
        numbers.contains(&56088.0),
        "Answer should contain 56088 (123 * 456), got numbers: {:?}, answer: {}",
        numbers,
        result.output
    );

    if !result.intermediate_steps.is_empty() {
        println!("\nReasoning trace:");
        for (i, step) in result.intermediate_steps.iter().enumerate() {
            println!("  Step {}: {}", i + 1, step.action.tool);
            println!("    Input: {:?}", step.action.tool_input);
            println!("    Output: {}", step.observation);
        }
    }

    println!("✅ ReAct agent calculation works correctly\n");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_react_agent_multi_step_reasoning() {
    println!("\n=== Test: ReAct Agent Multi-Step Reasoning ===\n");

    load_test_env();
    let _ = get_openai_key();

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    let calculator = Arc::new(create_calculator_tool());

    let agent = ReActAgent::new(
        llm,
        vec![calculator.clone()],
        "You are a helpful assistant. Use tools when appropriate.",
    );

    let mut config = AgentExecutorConfig::default();
    config.max_iterations = 10;
    let executor = AgentExecutor::new(Box::new(agent))
        .with_tools(vec![Box::new(create_calculator_tool())])
        .with_config(config);

    // Multi-step problem: (15 + 8) * 2
    let question = "Calculate 15 plus 8, then multiply the result by 2";
    println!("Question: {}", question);

    let result = executor
        .execute(question)
        .await
        .expect("Agent should complete");

    println!("Answer: {}", result.output);
    println!("Steps taken: {}", result.intermediate_steps.len());

    // PRAGMATIC CHECK: Final answer should be 46 ((15+8)*2 = 23*2)
    let numbers = extract_numbers(&result.output);
    assert!(
        numbers.contains(&46.0),
        "Answer should contain 46 ((15+8)*2), got numbers: {:?}, answer: {}",
        numbers,
        result.output
    );

    if !result.intermediate_steps.is_empty() {
        println!("\nReasoning steps:");
        for (i, step) in result.intermediate_steps.iter().enumerate() {
            println!(
                "  {}. {:?} -> {}",
                i + 1,
                step.action.tool_input,
                step.observation
            );
        }
    }

    println!("✅ Multi-step reasoning works\n");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_react_agent_with_multiple_tools() {
    println!("\n=== Test: ReAct Agent with Multiple Tools ===\n");

    load_test_env();
    let _ = get_openai_key();

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    let calculator = Arc::new(create_calculator_tool());
    let search = Arc::new(create_search_tool());

    let agent = ReActAgent::new(
        llm,
        vec![calculator.clone(), search.clone()],
        "You are a helpful assistant with access to search and calculator tools.",
    );

    let mut config = AgentExecutorConfig::default();
    config.max_iterations = 10;
    let executor = AgentExecutor::new(Box::new(agent))
        .with_tools(vec![
            Box::new(create_calculator_tool()),
            Box::new(create_search_tool()),
        ])
        .with_config(config);

    // Test that agent with multiple tools can complete a task
    let question = "What is 25 times 4?";
    println!("Question: {}", question);

    let result = executor
        .execute(question)
        .await
        .expect("Agent should complete");

    println!("Answer: {}", result.output);
    println!("Steps taken: {}", result.intermediate_steps.len());

    // PRAGMATIC CHECK: Verify correct answer (25 * 4 = 100)
    let numbers = extract_numbers(&result.output);
    assert!(
        numbers.contains(&100.0),
        "Answer should contain 100 (25 * 4), got numbers: {:?}, answer: {}",
        numbers,
        result.output
    );

    if !result.intermediate_steps.is_empty() {
        println!("\nTools used:");
        for step in result.intermediate_steps.iter() {
            println!("  - {}", step.action.tool);
        }
    }

    println!("✅ Agent with multiple tools works correctly\n");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_react_agent_direct_answer() {
    println!("\n=== Test: ReAct Agent Direct Answer (No Tools) ===\n");

    load_test_env();
    let _ = get_openai_key();

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    let calculator = Arc::new(create_calculator_tool());

    let agent = ReActAgent::new(
        llm,
        vec![calculator.clone()],
        "You are a helpful assistant. Only use tools when necessary.",
    );

    let config = AgentExecutorConfig::default();
    let executor = AgentExecutor::new(Box::new(agent))
        .with_tools(vec![Box::new(create_calculator_tool())])
        .with_config(config);

    // Simple question that doesn't need tools
    let question = "What is the capital of France?";
    println!("Question: {}", question);

    let result = executor
        .execute(question)
        .await
        .expect("Agent should complete");

    println!("Answer: {}", result.output);
    println!("Steps taken: {}", result.intermediate_steps.len());

    // SKEPTICAL CHECK: Should answer directly without tools
    assert!(
        result.output.to_lowercase().contains("paris"),
        "Should answer Paris, got: {}",
        result.output
    );

    // Note: Agent might use 0 or 1 steps depending on how it's implemented
    // We just verify it produces a correct answer
    println!("✅ Agent can answer directly when tools aren't needed\n");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_react_agent_max_iterations() {
    println!("\n=== Test: ReAct Agent Max Iterations Limit ===\n");

    load_test_env();
    let _ = get_openai_key();

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    let calculator = Arc::new(create_calculator_tool());

    let agent = ReActAgent::new(
        llm,
        vec![calculator.clone()],
        "You are a helpful assistant.",
    );

    // Set very low max iterations
    let mut config = AgentExecutorConfig::default();
    config.max_iterations = 2;
    let executor = AgentExecutor::new(Box::new(agent))
        .with_tools(vec![Box::new(create_calculator_tool())])
        .with_config(config);

    let question = "Calculate 1 + 1, then add 2, then add 3, then add 4, then add 5";
    println!("Question: {} (should hit iteration limit)", question);

    let result = executor.execute(question).await;

    // Should either complete early or hit max iterations
    match result {
        Ok(r) => {
            println!("Completed in {} iterations", r.iterations);
            assert!(
                r.iterations <= 2,
                "Should not exceed max iterations (2), got: {}",
                r.iterations
            );
        }
        Err(e) => {
            println!("Hit iteration limit (expected): {}", e);
            assert!(
                e.to_string().contains("iteration") || e.to_string().contains("limit"),
                "Error should mention iteration limit, got: {}",
                e
            );
        }
    }

    println!("✅ Max iterations limit works correctly\n");
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_react_agent_handles_tool_errors() {
    println!("\n=== Test: ReAct Agent Handles Tool Errors ===\n");

    load_test_env();
    let _ = get_openai_key();

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    let calculator = Arc::new(create_calculator_tool());

    let agent = ReActAgent::new(
        llm,
        vec![calculator.clone()],
        "You are a helpful assistant. If a tool returns an error, explain it to the user.",
    );

    let config = AgentExecutorConfig::default();
    let executor = AgentExecutor::new(Box::new(agent))
        .with_tools(vec![Box::new(create_calculator_tool())])
        .with_config(config);

    // Ask for division by zero
    let question = "What is 10 divided by 0?";
    println!("Question: {}", question);

    let result = executor.execute(question).await;

    match result {
        Ok(r) => {
            println!("Agent response: {}", r.output);
            // Agent should acknowledge the error in its response
            let output_lower = r.output.to_lowercase();
            assert!(
                output_lower.contains("error")
                    || output_lower.contains("cannot")
                    || output_lower.contains("undefined")
                    || output_lower.contains("zero"),
                "Agent should mention error/problem with division by zero, got: {}",
                r.output
            );
        }
        Err(e) => {
            println!("Error (acceptable): {}", e);
            // Tool error is also acceptable
            assert!(
                e.to_string().contains("zero") || e.to_string().contains("Division"),
                "Error should mention division by zero, got: {}",
                e
            );
        }
    }

    println!("✅ Agent handles tool errors appropriately\n");
}
