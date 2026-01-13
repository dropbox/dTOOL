//! Comprehensive Agent Integration Tests for OpenAI
//!
//! **DEPRECATED PATTERN**: These tests use the deprecated `AgentExecutor` API for backward compatibility testing.
//! For new tests, use `create_react_agent()` from `dashflow` instead.
//!
//! These tests verify real agent execution with ChatOpenAI and tool calling.
//!
//! Prerequisites:
//! - OPENAI_API_KEY environment variable must be set
//!
//! Run with: cargo test --test agent_integration_tests -- --ignored

#![allow(deprecated)]

use dashflow::core::agents::{AgentExecutor, AgentExecutorConfig, ToolCallingAgent};
use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
use dashflow::core::error::Result;
use dashflow::core::tools::builtin::{calculator_tool, echo_tool};
use dashflow::core::tools::{FunctionTool, Tool};
use dashflow_openai::build_chat_model;
use std::sync::Arc;

/// Helper to check if OpenAI API key is available
fn has_openai_key() -> bool {
    std::env::var("OPENAI_API_KEY").is_ok()
}

/// Helper to create a ToolCallingAgent with OpenAI
fn create_test_agent(tools: Vec<Arc<dyn Tool>>, system_message: &str) -> Result<ToolCallingAgent> {
    let config = ChatModelConfig::OpenAI {
        model: "gpt-4o-mini".to_string(),
        api_key: SecretReference::from_env("OPENAI_API_KEY"),
        temperature: Some(0.0),
        max_tokens: None,
        base_url: None,
        organization: None,
    };
    let model = build_chat_model(&config)?;

    Ok(ToolCallingAgent::new(model, tools, system_message.to_string()))
}

/// Helper to create AgentExecutor with configuration
fn create_test_executor(agent: ToolCallingAgent, tools: Vec<Box<dyn Tool>>) -> AgentExecutor {
    let config = AgentExecutorConfig {
        max_iterations: 10,
        max_execution_time: Some(60.0),
        early_stopping_method: "force".to_string(),
        handle_parsing_errors: true,
        checkpoint_id: None,
    };

    AgentExecutor::new(Box::new(agent))
        .with_tools(tools)
        .with_config(config)
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_agent_simple_calculation() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create tools
    let calculator: Arc<dyn Tool> = Arc::new(calculator_tool());
    let agent = create_test_agent(
        vec![Arc::clone(&calculator)],
        "You are a helpful assistant that can use tools to answer questions.",
    )?;

    let executor = create_test_executor(agent, vec![Box::new(calculator_tool())]);

    // Execute: simple calculation
    let result = executor.execute("What is 234 * 567?").await?;

    // Verify
    assert!(
        result.output.contains("132,678") || result.output.contains("132678"),
        "Expected result to contain 132678, got: {}",
        result.output
    );
    assert!(result.iterations > 0, "Expected at least 1 iteration");
    assert!(
        !result.intermediate_steps.is_empty(),
        "Expected tool to be called"
    );

    // Verify calculator was used
    let calculator_used = result
        .intermediate_steps
        .iter()
        .any(|step| step.action.tool == "calculator");
    assert!(calculator_used, "Expected calculator tool to be used");

    Ok(())
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_agent_multi_step_reasoning() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let calculator: Arc<dyn Tool> = Arc::new(calculator_tool());
    let agent = create_test_agent(
        vec![Arc::clone(&calculator)],
        "You are a helpful assistant. Use tools to solve problems step by step.",
    )?;

    let executor = create_test_executor(agent, vec![Box::new(calculator_tool())]);

    // Multi-step problem
    let result = executor
        .execute("If I have 15 apples and buy 23 more, then eat 7, how many do I have left?")
        .await?;

    // Verify
    assert!(
        result.output.contains("31") || result.output.contains("thirty-one"),
        "Expected result to contain 31, got: {}",
        result.output
    );
    assert!(result.iterations >= 1, "Expected at least 1 iteration");

    // May use calculator once (15+23-7) or multiple times (15+23, then -7)
    assert!(
        !result.intermediate_steps.is_empty(),
        "Expected at least one tool call"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_agent_no_tool_needed() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let calculator: Arc<dyn Tool> = Arc::new(calculator_tool());
    let agent = create_test_agent(
        vec![Arc::clone(&calculator)],
        "You are a helpful assistant. Only use tools when necessary.",
    )?;

    let executor = create_test_executor(agent, vec![Box::new(calculator_tool())]);

    // Question that doesn't need tools
    let result = executor.execute("What is the capital of France?").await?;

    // Verify
    assert!(
        result.output.to_lowercase().contains("paris"),
        "Expected answer to contain 'Paris', got: {}",
        result.output
    );
    assert_eq!(
        result.iterations, 1,
        "Expected exactly 1 iteration (no tool use)"
    );
    assert!(
        result.intermediate_steps.is_empty(),
        "Expected no tool calls"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_agent_multiple_tools() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create multiple tools
    let calculator: Arc<dyn Tool> = Arc::new(calculator_tool());
    let echo: Arc<dyn Tool> = Arc::new(echo_tool());

    let agent = create_test_agent(
        vec![Arc::clone(&calculator), Arc::clone(&echo)],
        "You are a helpful assistant. Use the appropriate tool for each task.",
    )?;

    let executor = create_test_executor(
        agent,
        vec![Box::new(calculator_tool()), Box::new(echo_tool())],
    );

    // Test that requires echo tool
    let result = executor
        .execute("Please echo the text 'Hello, Agent!'")
        .await?;

    // Verify
    assert!(
        result.output.contains("Hello, Agent!"),
        "Expected echoed text in output, got: {}",
        result.output
    );

    // Verify echo was used
    let echo_used = result
        .intermediate_steps
        .iter()
        .any(|step| step.action.tool == "echo");
    assert!(echo_used, "Expected echo tool to be used");

    Ok(())
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_agent_custom_function_tool() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create a custom weather tool
    let weather_tool = FunctionTool::new(
        "get_weather",
        "Get the current weather for a city",
        |input: String| {
            Box::pin(async move {
                let city = input.trim();
                Ok(format!("The weather in {} is sunny and 72°F", city))
            })
        },
    );

    let weather_arc: Arc<dyn Tool> = Arc::new(weather_tool);
    let agent = create_test_agent(
        vec![Arc::clone(&weather_arc)],
        "You are a helpful weather assistant. Use the get_weather tool to answer weather questions.",
    )?;

    let weather_tool2 = FunctionTool::new(
        "get_weather",
        "Get the current weather for a city",
        |input: String| {
            Box::pin(async move {
                let city = input.trim();
                Ok(format!("The weather in {} is sunny and 72°F", city))
            })
        },
    );

    let executor = create_test_executor(agent, vec![Box::new(weather_tool2)]);

    // Ask about weather
    let result = executor
        .execute("What's the weather like in San Francisco?")
        .await?;

    // Verify
    assert!(
        result.output.to_lowercase().contains("san francisco"),
        "Expected city name in output, got: {}",
        result.output
    );
    assert!(
        result.output.contains("sunny") || result.output.contains("72"),
        "Expected weather info in output, got: {}",
        result.output
    );

    // Verify tool was called
    let tool_used = result
        .intermediate_steps
        .iter()
        .any(|step| step.action.tool == "get_weather");
    assert!(tool_used, "Expected get_weather tool to be used");

    Ok(())
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_agent_max_iterations_limit() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let calculator: Arc<dyn Tool> = Arc::new(calculator_tool());
    let agent = create_test_agent(vec![Arc::clone(&calculator)], "You are a helpful assistant.")?;

    // Create executor with low max_iterations
    let config = AgentExecutorConfig {
        max_iterations: 2,
        max_execution_time: Some(60.0),
        early_stopping_method: "force".to_string(),
        handle_parsing_errors: true,
        checkpoint_id: None,
    };

    let executor = AgentExecutor::new(Box::new(agent))
        .with_tools(vec![Box::new(calculator_tool())])
        .with_config(config);

    // Simple task that should complete within 2 iterations
    let result = executor.execute("What is 2 + 2?").await?;

    // Verify iterations constraint
    assert!(
        result.iterations <= 2,
        "Expected max 2 iterations, got {}",
        result.iterations
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_agent_tool_with_json_output() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Tool that returns structured data as JSON string
    let data_tool = FunctionTool::new(
        "get_user_data",
        "Get user data for a given user ID",
        |input: String| {
            Box::pin(async move {
                let user_id = input.trim();
                Ok(format!(
                    r#"{{"id": "{}", "name": "John Doe", "age": 30, "email": "john@example.com"}}"#,
                    user_id
                ))
            })
        },
    );

    let data_arc: Arc<dyn Tool> = Arc::new(data_tool);
    let agent = create_test_agent(
        vec![Arc::clone(&data_arc)],
        "You are a helpful assistant that can retrieve user data.",
    )?;

    let data_tool2 = FunctionTool::new(
        "get_user_data",
        "Get user data for a given user ID",
        |input: String| {
            Box::pin(async move {
                let user_id = input.trim();
                Ok(format!(
                    r#"{{"id": "{}", "name": "John Doe", "age": 30, "email": "john@example.com"}}"#,
                    user_id
                ))
            })
        },
    );

    let executor = create_test_executor(agent, vec![Box::new(data_tool2)]);

    // Ask for user data
    let result = executor.execute("What is the name of user 123?").await?;

    // Verify
    assert!(
        result.output.contains("John Doe"),
        "Expected user name in output, got: {}",
        result.output
    );

    Ok(())
}
