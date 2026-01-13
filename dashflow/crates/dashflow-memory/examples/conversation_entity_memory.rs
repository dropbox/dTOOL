//! Example demonstrating ConversationEntityMemory
//!
//! This example shows how to use entity memory to track people, places, and
//! concepts mentioned in a conversation.
//!
//! Run with:
//! ```bash
//! cargo run --package dashflow-memory --example conversation_entity_memory
//! ```

use dashflow::core::chat_history::InMemoryChatMessageHistory;
use dashflow_memory::{BaseMemory, ConversationEntityMemory};
use dashflow_openai::ChatOpenAI;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up OpenAI API key
    // Make sure OPENAI_API_KEY environment variable is set
    let llm = ChatOpenAI::default();

    // Create memory with entity tracking
    let chat_memory = InMemoryChatMessageHistory::new();
    let mut memory = ConversationEntityMemory::new(llm, chat_memory);

    println!("=== Entity Memory Example ===\n");
    println!("This example demonstrates automatic entity extraction and summarization.\n");

    // First conversation turn
    println!("Turn 1:");
    println!("Human: I'm planning to visit my friend Alice in Seattle next week.");
    let mut inputs = HashMap::new();
    inputs.insert(
        "input".to_string(),
        "I'm planning to visit my friend Alice in Seattle next week.".to_string(),
    );
    let mut outputs = HashMap::new();
    outputs.insert(
        "output".to_string(),
        "That sounds great! Seattle is a beautiful city. I hope you have a wonderful time with Alice."
            .to_string(),
    );
    memory.save_context(&inputs, &outputs).await?;
    println!(
        "AI: That sounds great! Seattle is a beautiful city. I hope you have a wonderful time with Alice.\n"
    );

    // Second turn - load memory to extract entities
    println!("Turn 2:");
    println!("Human: What should I know about the city?");
    inputs.clear();
    inputs.insert(
        "input".to_string(),
        "What should I know about the city?".to_string(),
    );

    // Load memory variables - this extracts entities
    let vars = memory.load_memory_variables(&inputs).await?;
    println!("Memory variables:");
    for (key, value) in &vars {
        println!("  {}: {:?}", key, value);
    }
    println!();

    outputs.clear();
    outputs.insert(
        "output".to_string(),
        "Seattle is known for its coffee culture, tech industry, and the Space Needle. Alice can probably show you around!"
            .to_string(),
    );
    memory.save_context(&inputs, &outputs).await?;
    println!("AI: Seattle is known for its coffee culture, tech industry, and the Space Needle. Alice can probably show you around!\n");

    // Third turn - entities are now tracked
    println!("Turn 3:");
    println!("Human: Yes, Alice works at Microsoft and knows the city well.");
    inputs.clear();
    inputs.insert(
        "input".to_string(),
        "Yes, Alice works at Microsoft and knows the city well.".to_string(),
    );

    let vars = memory.load_memory_variables(&inputs).await?;
    println!("Memory variables after entity updates:");
    for (key, value) in &vars {
        println!("  {}: {:?}", key, value);
    }
    println!();

    outputs.clear();
    outputs.insert(
        "output".to_string(),
        "That's perfect! Microsoft is a major employer in Seattle. She'll be a great guide!"
            .to_string(),
    );
    memory.save_context(&inputs, &outputs).await?;
    println!(
        "AI: That's perfect! Microsoft is a major employer in Seattle. She'll be a great guide!\n"
    );

    // Fourth turn - demonstrate entity recall
    println!("Turn 4:");
    println!("Human: I'm excited to see her again!");
    inputs.clear();
    inputs.insert(
        "input".to_string(),
        "I'm excited to see her again!".to_string(),
    );

    let vars = memory.load_memory_variables(&inputs).await?;
    println!("Memory variables with entity context:");
    for (key, value) in &vars {
        println!("  {}: {:?}", key, value);
    }
    println!();

    println!("=== Summary ===");
    println!("Entity memory automatically:");
    println!("  1. Extracts named entities (Alice, Seattle, Microsoft)");
    println!("  2. Creates and updates summaries for each entity");
    println!("  3. Provides entity context for future interactions");
    println!("  4. Helps maintain personalized, coherent conversations");

    Ok(())
}
