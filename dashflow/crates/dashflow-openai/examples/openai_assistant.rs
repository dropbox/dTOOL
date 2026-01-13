//! Example demonstrating OpenAI Assistant API integration
//!
//! This example shows how to create and use an OpenAI Assistant with built-in tools
//! like code_interpreter.
//!
//! Run with:
//! ```bash
//! export OPENAI_API_KEY=your_api_key_here
//! cargo run --example openai_assistant --features async-openai
//! ```

use dashflow::core::runnable::Runnable;
use dashflow_openai::OpenAIAssistantRunnable;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an assistant with code interpreter tool
    println!("Creating OpenAI Assistant with code interpreter...");

    let assistant = OpenAIAssistantRunnable::create_assistant(
        "Rust Math Tutor",
        "You are a helpful math tutor. Write and execute code to solve math problems.",
        vec![serde_json::json!({"type": "code_interpreter"})],
        "gpt-4-turbo-preview",
        None,
    )
    .await?;

    println!("Assistant created successfully!");

    // Example 1: Basic math calculation
    println!("\n=== Example 1: Basic Calculation ===");
    let mut input = HashMap::new();
    input.insert(
        "content".to_string(),
        serde_json::json!("What is 25 * 4 raised to the 2.7 power?"),
    );

    let result = assistant.invoke(input, None).await?;
    println!("Result: {:?}", result);

    // Example 2: Using agent mode (compatible with AgentExecutor)
    println!("\n=== Example 2: Agent Mode ===");
    let agent = OpenAIAssistantRunnable::create_assistant(
        "Rust Agent",
        "You are a helpful agent.",
        vec![serde_json::json!({"type": "code_interpreter"})],
        "gpt-4-turbo-preview",
        None,
    )
    .await?
    .with_as_agent(true);

    let mut agent_input = HashMap::new();
    agent_input.insert(
        "content".to_string(),
        serde_json::json!("Calculate fibonacci(10)"),
    );

    let agent_result = agent.invoke(agent_input, None).await?;
    println!("Agent result: {:?}", agent_result);

    // Example 3: Using file_search tool
    println!("\n=== Example 3: File Search ===");
    let search_assistant = OpenAIAssistantRunnable::create_assistant(
        "Search Assistant",
        "You are a helpful assistant that can search through documents.",
        vec![serde_json::json!({"type": "file_search"})],
        "gpt-4-turbo-preview",
        None,
    )
    .await?;

    let mut search_input = HashMap::new();
    search_input.insert(
        "content".to_string(),
        serde_json::json!("What are the key points about Rust ownership?"),
    );

    let search_result = search_assistant.invoke(search_input, None).await?;
    println!("Search result: {:?}", search_result);

    println!("\n✓ All examples completed successfully!");

    Ok(())
}
