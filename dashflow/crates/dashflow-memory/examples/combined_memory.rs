//! Example demonstrating CombinedMemory usage
//!
//! This example shows how to combine multiple memory types into a single
//! unified memory. We'll combine ConversationSummaryMemory and
//! ConversationEntityMemory to get both conversation summaries and entity tracking.

use dashflow::core::chat_history::InMemoryChatMessageHistory;
use dashflow_memory::{
    BaseMemory, CombinedMemory, ConversationEntityMemory, ConversationSummaryMemory,
};
use dashflow_openai::ChatOpenAI;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CombinedMemory Example ===\n");

    // Create first memory: ConversationSummaryMemory
    println!("Creating ConversationSummaryMemory...");
    let llm1 = Box::new(ChatOpenAI::default());
    let chat_history1 = InMemoryChatMessageHistory::new();
    let summary_memory = ConversationSummaryMemory::new(llm1, chat_history1);

    // Create second memory: ConversationEntityMemory
    println!("Creating ConversationEntityMemory...");
    let llm2 = ChatOpenAI::default();
    let chat_history2 = InMemoryChatMessageHistory::new();
    let entity_memory = ConversationEntityMemory::new(llm2, chat_history2);

    // Combine them
    println!("Combining memories...\n");
    let mut combined =
        CombinedMemory::new(vec![Box::new(summary_memory), Box::new(entity_memory)])?;

    // Show memory variables
    println!("Memory variables: {:?}\n", combined.memory_variables());

    // Simulate a conversation
    println!("=== Conversation ===\n");

    // Turn 1
    let mut inputs = HashMap::new();
    let input1 = "Hi, I'm Alice and I work at Anthropic.";
    inputs.insert(
        "input".to_string(),
        input1.to_string(),
    );
    let mut outputs = HashMap::new();
    let output1 = "Hello Alice! Nice to meet you. How are things at Anthropic?";
    outputs.insert(
        "output".to_string(),
        output1.to_string(),
    );

    println!("Human: {}", input1);
    println!("AI: {}", output1);

    combined.save_context(&inputs, &outputs).await?;

    // Turn 2
    inputs.clear();
    let input2 = "I'm working on a new LLM safety feature.";
    inputs.insert(
        "input".to_string(),
        input2.to_string(),
    );
    outputs.clear();
    let output2 = "That sounds fascinating! What aspect of safety are you focusing on?";
    outputs.insert(
        "output".to_string(),
        output2.to_string(),
    );

    println!("\nHuman: {}", input2);
    println!("AI: {}", output2);

    combined.save_context(&inputs, &outputs).await?;

    // Turn 3
    inputs.clear();
    let input3 = "I'm also learning Rust in my free time.";
    inputs.insert(
        "input".to_string(),
        input3.to_string(),
    );
    outputs.clear();
    let output3 = "Great choice! Rust is excellent for systems programming.";
    outputs.insert(
        "output".to_string(),
        output3.to_string(),
    );

    println!("\nHuman: {}", input3);
    println!("AI: {}", output3);

    combined.save_context(&inputs, &outputs).await?;

    // Load memory variables to see what both memories provide
    println!("\n=== Memory Contents ===\n");
    inputs.clear();
    inputs.insert(
        "input".to_string(),
        "What do you know about me?".to_string(),
    );

    let memory_vars = combined.load_memory_variables(&inputs).await?;

    for (key, value) in &memory_vars {
        println!("{}: {}\n", key, value);
    }

    // Clear all memories
    println!("=== Clearing Memories ===\n");
    combined.clear().await?;

    let empty_vars = combined.load_memory_variables(&HashMap::new()).await?;
    println!("Memory variables after clear: {:?}", empty_vars);

    Ok(())
}
