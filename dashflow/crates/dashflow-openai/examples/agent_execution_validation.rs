//! Agent Execution Validation - Rust Implementation
//!
//! Validates agent execution using create_react_agent (modern DashFlow API)
//!
//! Tests agent loop: reasoning -> tool selection -> execution -> answer synthesis
//!
//! Requirements:
//! - OPENAI_API_KEY environment variable
//!
//! Tool: Calculator (basic arithmetic evaluation)
//! LLM: OpenAI gpt-4o-mini (supports tool calling)
//!
//! Test queries:
//! 1. Simple math: "What is 234 multiplied by 567?"
//! 2. Multi-step: "Calculate 15 plus 23, then multiply the result by 3"
//! 3. No tool needed: "What is the capital of France?"
//!
//! Expected behavior:
//! - Query 1: Use calculator tool once, return correct answer (132,678)
//! - Query 2: May use calculator 1-2 times depending on reasoning, return 114
//! - Query 3: No tool call needed, direct answer from knowledge
//!
//! Validation criteria:
//! - Tool called when needed (queries 1, 2)
//! - Tool not called when unnecessary (query 3)
//! - Tool inputs are correct (proper expressions)
//! - Final answers are correct

use dashflow::core::language_models::bind_tools::ChatModelToolBindingExt;
use dashflow::core::messages::Message;
use dashflow::core::tools::builtin::calculator_tool;
use dashflow::error::Result;
use dashflow::executor::CompiledGraph;
use dashflow::prebuilt::{create_react_agent, AgentState};
use dashflow_openai::ChatOpenAI;
use std::sync::Arc;

/// Run an agent query and display results
async fn run_agent_query(
    agent: &CompiledGraph<AgentState>,
    query: &str,
    query_num: usize,
) -> Result<()> {
    println!("\n{}", "=".repeat(80));
    println!("Query {}: {}", query_num, query);
    println!("{}\n", "=".repeat(80));

    // Execute agent
    let state = AgentState::with_human_message(query);
    let result = agent.invoke(state).await?;

    // Display results
    println!("Final Answer:");
    if let Some(last_message) = result.final_state.messages.last() {
        println!("  {}\n", last_message.content().as_text());
    }

    // Count and display tool usage
    let tool_count = result
        .final_state
        .messages
        .iter()
        .filter(|m| matches!(m, Message::Tool { .. }))
        .count();

    if tool_count > 0 {
        println!("Tool Usage ({} calls):", tool_count);
        let mut step = 1;
        for msg in &result.final_state.messages {
            if let Message::Tool {
                content,
                tool_call_id,
                ..
            } = msg
            {
                println!("\n  Step {}:", step);
                println!("    Tool call ID: {}", tool_call_id);
                println!("    Output: {}", content.as_text());
                step += 1;
            }
        }
    } else {
        println!("Tool Usage: None (direct answer)");
    }

    println!();

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("{}", "=".repeat(80));
    println!("Agent Execution Validation - Rust Implementation");
    println!("{}", "=".repeat(80));

    // Check API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("ERROR: OPENAI_API_KEY environment variable not set");
        std::process::exit(1);
    }

    println!("\n1. Creating tools");

    // Create calculator tool
    let calculator = Arc::new(calculator_tool()) as Arc<dyn dashflow::core::tools::Tool>;
    let tools = vec![Arc::clone(&calculator)];

    println!("   Created {} tools: calculator", tools.len());

    println!("\n2. Creating LLM (OpenAI gpt-4o-mini)");

    let model = ChatOpenAI::with_config(Default::default())
        .with_model("gpt-4o-mini")
        .with_temperature(0.0);

    // Bind tools to model
    let model_with_tools = model.bind_tools(tools.clone(), None);

    println!("   LLM ready with tools bound");

    println!("\n3. Creating ReAct agent (using create_react_agent)");

    // Create agent using create_react_agent
    let agent = create_react_agent(model_with_tools, tools)?;

    println!("   Agent created");

    println!("\n4. Running test queries");

    let test_queries = [
        "What is 234 multiplied by 567?",
        "Calculate 15 plus 23, then multiply the result by 3",
        "What is the capital of France?",
    ];

    for (i, query) in test_queries.iter().enumerate() {
        run_agent_query(&agent, query, i + 1).await?;
    }

    println!("{}", "=".repeat(80));
    println!("Rust Validation Complete (using create_react_agent)");
    println!("{}", "=".repeat(80));

    Ok(())
}
