//! Agent Integration Example with OpenAI
//!
//! Demonstrates how to use create_react_agent with ChatOpenAI for real agentic workflows.
//!
//! This example shows:
//! - Creating a ReAct agent with ChatOpenAI using create_react_agent()
//! - Using built-in tools (calculator, echo)
//! - Executing an agent loop with real LLM calls
//! - Handling tool calls and observations with DashFlow
//!
//! Prerequisites:
//! - Set OPENAI_API_KEY environment variable
//!
//! Run with: cargo run --example agent_with_openai

use dashflow::core::language_models::bind_tools::ChatModelToolBindingExt;
use dashflow::core::messages::Message;
use dashflow::core::tools::builtin::{calculator_tool, echo_tool};
use dashflow::error::Result;
use dashflow::prebuilt::{create_react_agent, AgentState};
use dashflow_openai::ChatOpenAI;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Agent Integration with OpenAI Example ===\n");

    // Check for API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("❌ Error: OPENAI_API_KEY environment variable not set");
        eprintln!("Please set your OpenAI API key to run this example:");
        eprintln!("  export OPENAI_API_KEY=sk-...");
        std::process::exit(1);
    }

    // Create tools
    let calculator = Arc::new(calculator_tool()) as Arc<dyn dashflow::core::tools::Tool>;
    let echo = Arc::new(echo_tool()) as Arc<dyn dashflow::core::tools::Tool>;
    let tools = vec![Arc::clone(&calculator), Arc::clone(&echo)];

    println!("Tools available to agent:");
    println!("1. Calculator: Evaluate mathematical expressions");
    println!("2. Echo: Echo back any text\n");

    // Create ChatOpenAI model
    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4")
        .with_temperature(0.0);

    // Bind tools to model
    let model_with_tools = model.bind_tools(tools.clone(), None);

    println!("Model configured: gpt-4 with tool calling enabled\n");
    println!("{}", "=".repeat(60));

    // Create ReAct agent using create_react_agent
    let agent = create_react_agent(model_with_tools, tools)?;

    // Example 1: Math calculation requiring tool use
    println!("\n📝 Example 1: Math Calculation");
    println!("User: What is 234 * 567?");

    let state1 = AgentState::with_human_message("What is 234 * 567?");
    match agent.invoke(state1).await {
        Ok(result) => {
            // Find the final AI message
            if let Some(last_message) = result.final_state.messages.last() {
                println!("\n🤖 Agent: {}", last_message.content().as_text());
            }

            println!("\n📊 Execution Stats:");
            println!(
                "  - Messages in conversation: {}",
                result.final_state.messages.len()
            );

            // Count tool messages
            let tool_count = result
                .final_state
                .messages
                .iter()
                .filter(|m| matches!(m, Message::Tool { .. }))
                .count();
            println!("  - Tool calls: {}", tool_count);

            if tool_count > 0 {
                println!("\n🔧 Tool Usage:");
                let mut step = 1;
                for msg in &result.final_state.messages {
                    if let Message::Tool {
                        content,
                        tool_call_id,
                        ..
                    } = msg
                    {
                        println!("  Step {}:", step);
                        println!("    Tool call ID: {}", tool_call_id);
                        println!("    Result: {}", content.as_text());
                        step += 1;
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Error executing agent: {}", e);
        }
    }

    println!("\n{}", "=".repeat(60));

    // Example 2: Multiple calculations
    println!("\n📝 Example 2: Multi-step Reasoning");
    println!("User: If I have 15 apples and buy 23 more, then eat 7, how many do I have?");

    let state2 = AgentState::with_human_message(
        "If I have 15 apples and buy 23 more, then eat 7, how many do I have?",
    );
    match agent.invoke(state2).await {
        Ok(result) => {
            if let Some(last_message) = result.final_state.messages.last() {
                println!("\n🤖 Agent: {}", last_message.content().as_text());
            }

            println!("\n📊 Execution Stats:");
            println!(
                "  - Messages in conversation: {}",
                result.final_state.messages.len()
            );

            let tool_count = result
                .final_state
                .messages
                .iter()
                .filter(|m| matches!(m, Message::Tool { .. }))
                .count();
            println!("  - Tool calls: {}", tool_count);

            if tool_count > 0 {
                println!("\n🔧 Tool Usage:");
                let mut step = 1;
                for msg in &result.final_state.messages {
                    if let Message::Tool {
                        content,
                        tool_call_id,
                        ..
                    } = msg
                    {
                        println!("  Step {}:", step);
                        println!("    Tool call ID: {}", tool_call_id);
                        println!("    Result: {}", content.as_text());
                        step += 1;
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Error executing agent: {}", e);
        }
    }

    println!("\n{}", "=".repeat(60));

    // Example 3: No tool needed
    println!("\n📝 Example 3: Direct Answer (No Tool Needed)");
    println!("User: What is the capital of France?");

    let state3 = AgentState::with_human_message("What is the capital of France?");
    match agent.invoke(state3).await {
        Ok(result) => {
            if let Some(last_message) = result.final_state.messages.last() {
                println!("\n🤖 Agent: {}", last_message.content().as_text());
            }

            println!("\n📊 Execution Stats:");
            println!(
                "  - Messages in conversation: {}",
                result.final_state.messages.len()
            );

            let tool_count = result
                .final_state
                .messages
                .iter()
                .filter(|m| matches!(m, Message::Tool { .. }))
                .count();
            println!(
                "  - Tool calls: {} (agent decided no tools needed)",
                tool_count
            );
        }
        Err(e) => {
            eprintln!("❌ Error executing agent: {}", e);
        }
    }

    println!("\n{}", "=".repeat(60));
    println!("\n✅ Example Complete!");
    println!("\n💡 Key Points:");
    println!("  - Agent automatically decides when to use tools");
    println!("  - Multiple tool calls can be made in sequence");
    println!("  - Agent provides final answer after gathering information");
    println!("  - Uses create_react_agent() from DashFlow (Python-compatible API)");
    println!("  - Tool-calling happens transparently via OpenAI's function calling");

    Ok(())
}
