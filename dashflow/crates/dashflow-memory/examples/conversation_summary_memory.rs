//! Example demonstrating ConversationSummaryMemory
//!
//! This example shows how to use ConversationSummaryMemory to maintain
//! a running summary of a conversation instead of storing all messages.
//!
//! # Running
//!
//! ```bash
//! export OPENAI_API_KEY=your-key-here
//! cargo run --example conversation_summary_memory
//! ```

use dashflow::core::chat_history::InMemoryChatMessageHistory;
use dashflow_memory::{BaseMemory, ConversationSummaryMemory};
use dashflow_openai::ChatOpenAI;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Conversation Summary Memory Example ===\n");

    // Create an LLM for summarization (using GPT-4 for better summaries)
    let llm = ChatOpenAI::default()
        .with_model("gpt-4o-mini")
        .with_temperature(0.0);

    // Create chat history and memory
    let chat_history = InMemoryChatMessageHistory::new();
    let mut memory = ConversationSummaryMemory::new(Box::new(llm), chat_history);

    println!("Starting conversation...\n");

    // Conversation turn 1: Introduction
    println!("User: Hi, my name is Alice and I'm a software engineer.");
    let mut inputs = HashMap::new();
    inputs.insert(
        "input".to_string(),
        "Hi, my name is Alice and I'm a software engineer.".to_string(),
    );
    let mut outputs = HashMap::new();
    let output_text =
        "Hello Alice! It's nice to meet you. How can I help you today?".to_string();
    outputs.insert(
        "output".to_string(),
        output_text.clone(),
    );
    memory.save_context(&inputs, &outputs).await?;
    println!("Assistant: {}\n", output_text);

    // Load and display summary
    let vars = memory.load_memory_variables(&HashMap::new()).await?;
    println!("Summary after turn 1:");
    match vars.get("history") {
        Some(history) => println!("{}\n", history),
        None => println!("<no history>\n"),
    }
    println!("---\n");

    // Conversation turn 2: More details
    println!("User: I work at a startup building AI tools. We use Rust and Python.");
    let mut inputs = HashMap::new();
    inputs.insert(
        "input".to_string(),
        "I work at a startup building AI tools. We use Rust and Python.".to_string(),
    );
    let mut outputs = HashMap::new();
    let output_text = "That sounds exciting! Rust and Python are great choices for AI tools. What kind of AI tools are you building?".to_string();
    outputs.insert(
        "output".to_string(),
        output_text.clone(),
    );
    memory.save_context(&inputs, &outputs).await?;
    println!("Assistant: {}\n", output_text);

    // Load and display updated summary
    let vars = memory.load_memory_variables(&HashMap::new()).await?;
    println!("Summary after turn 2:");
    match vars.get("history") {
        Some(history) => println!("{}\n", history),
        None => println!("<no history>\n"),
    }
    println!("---\n");

    // Conversation turn 3: More context
    println!(
        "User: We're focused on LLM orchestration frameworks, similar to DashFlow but in Rust."
    );
    let mut inputs = HashMap::new();
    inputs.insert(
        "input".to_string(),
        "We're focused on LLM orchestration frameworks, similar to DashFlow but in Rust."
            .to_string(),
    );
    let mut outputs = HashMap::new();
    let output_text = "That's a great niche! Having a Rust implementation of DashFlow would provide better performance and type safety. Are you building this from scratch?".to_string();
    outputs.insert(
        "output".to_string(),
        output_text.clone(),
    );
    memory.save_context(&inputs, &outputs).await?;
    println!("Assistant: {}\n", output_text);

    // Load and display final summary
    let vars = memory.load_memory_variables(&HashMap::new()).await?;
    println!("Final summary after turn 3:");
    match vars.get("history") {
        Some(history) => println!("{}\n", history),
        None => println!("<no history>\n"),
    }
    println!("---\n");

    println!("Notice how the summary progressively captures the key information");
    println!("from the conversation without storing all the messages verbatim.");
    println!("This keeps memory usage bounded regardless of conversation length!");

    Ok(())
}
