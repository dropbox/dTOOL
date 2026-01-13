//! Example demonstrating VectorStoreRetrieverMemory usage.
//!
//! VectorStoreRetrieverMemory stores conversation history in a vector store
//! and retrieves semantically relevant past conversations based on the current input.
//!
//! Run with:
//! ```bash
//! cargo run --example vectorstore_retriever_memory
//! ```

use dashflow::core::{embeddings::Embeddings, vector_stores::InMemoryVectorStore};
use dashflow_memory::{BaseMemory, MemoryResult, VectorStoreRetrieverMemory};
use std::collections::HashMap;
use std::sync::Arc;

/// Simple mock embeddings for demonstration.
/// In production, use OpenAI, Cohere, or other real embedding models.
struct MockEmbeddings;

#[async_trait::async_trait]
impl Embeddings for MockEmbeddings {
    async fn _embed_documents(
        &self,
        texts: &[String],
    ) -> dashflow::core::error::Result<Vec<Vec<f32>>> {
        // Simple embedding based on text characteristics
        Ok(texts
            .iter()
            .map(|text| {
                let len = text.len() as f32;
                let words = text.split_whitespace().count() as f32;
                let has_question = if text.contains('?') { 1.0 } else { 0.0 };
                vec![len / 100.0, words / 10.0, has_question, 0.5]
            })
            .collect())
    }

    async fn _embed_query(&self, text: &str) -> dashflow::core::error::Result<Vec<f32>> {
        let len = text.len() as f32;
        let words = text.split_whitespace().count() as f32;
        let has_question = if text.contains('?') { 1.0 } else { 0.0 };
        Ok(vec![len / 100.0, words / 10.0, has_question, 0.5])
    }
}

#[tokio::main]
async fn main() -> MemoryResult<()> {
    println!("=== VectorStoreRetrieverMemory Example ===\n");

    // Create embeddings and vector store
    let embeddings = Arc::new(MockEmbeddings);
    let vector_store = InMemoryVectorStore::new(embeddings);

    // Create memory with vector store backend
    let mut memory = VectorStoreRetrieverMemory::new(vector_store)
        .with_k(2) // Retrieve top 2 most relevant memories
        .with_memory_key("history");

    println!("Memory created with k=2 (retrieve 2 most relevant memories)\n");

    // Simulate a conversation about different programming topics
    let conversations = [
        (
            "What is Rust?",
            "Rust is a systems programming language focused on safety, speed, and concurrency.",
        ),
        (
            "Tell me about Python",
            "Python is a high-level, interpreted language known for readability and ease of use.",
        ),
        (
            "What about JavaScript?",
            "JavaScript is primarily used for web development, running in browsers and Node.js.",
        ),
        (
            "Explain Go",
            "Go is a statically typed language designed at Google for efficient concurrent programming.",
        ),
    ];

    // Save all conversation turns
    println!("Saving conversation history...");
    for (i, (input, output)) in conversations.iter().enumerate() {
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), input.to_string());

        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), output.to_string());

        memory.save_context(&inputs, &outputs).await?;
        println!("  Turn {}: {} -> {}", i + 1, input, output);
    }

    println!("\n--- Testing Semantic Retrieval ---\n");

    // Test 1: Query about systems programming (should retrieve Rust and Go)
    println!("Query: 'Tell me about systems programming languages'");
    let mut query1 = HashMap::new();
    query1.insert(
        "input".to_string(),
        "Tell me about systems programming languages".to_string(),
    );

    let vars1 = memory.load_memory_variables(&query1).await?;
    let history1 = vars1
        .get("history")
        .map(String::as_str)
        .unwrap_or("<no history returned>");
    println!("Retrieved memories (k=2):");
    println!("{}\n", history1);

    // Test 2: Query about web development (should retrieve JavaScript and Python)
    println!("Query: 'Which languages are good for web development?'");
    let mut query2 = HashMap::new();
    query2.insert(
        "input".to_string(),
        "Which languages are good for web development?".to_string(),
    );

    let vars2 = memory.load_memory_variables(&query2).await?;
    let history2 = vars2
        .get("history")
        .map(String::as_str)
        .unwrap_or("<no history returned>");
    println!("Retrieved memories (k=2):");
    println!("{}\n", history2);

    // Test 3: Query about concurrency (should retrieve Go and Rust)
    println!("Query: 'What languages support concurrent programming?'");
    let mut query3 = HashMap::new();
    query3.insert(
        "input".to_string(),
        "What languages support concurrent programming?".to_string(),
    );

    let vars3 = memory.load_memory_variables(&query3).await?;
    let history3 = vars3
        .get("history")
        .map(String::as_str)
        .unwrap_or("<no history returned>");
    println!("Retrieved memories (k=2):");
    println!("{}\n", history3);

    println!("--- Testing return_docs Option ---\n");

    // Create memory that returns Document objects instead of text
    let embeddings2 = Arc::new(MockEmbeddings);
    let vector_store2 = InMemoryVectorStore::new(embeddings2);
    let mut memory_docs = VectorStoreRetrieverMemory::new(vector_store2)
        .with_k(1)
        .with_return_docs(true); // Return Document objects

    // Save one conversation
    let mut inputs = HashMap::new();
    inputs.insert("input".to_string(), "Hello!".to_string());

    let mut outputs = HashMap::new();
    outputs.insert(
        "output".to_string(),
        "Hi there! How can I help?".to_string(),
    );

    memory_docs.save_context(&inputs, &outputs).await?;

    // Load with return_docs=true
    let mut query = HashMap::new();
    query.insert("input".to_string(), "Hey".to_string());

    let vars = memory_docs.load_memory_variables(&query).await?;
    let Some(history_json) = vars.get("history") else {
        eprintln!("No history returned for return_docs=true example; skipping.");
        return Ok(());
    };

    println!("return_docs=true returns Document objects (as JSON string):");
    // Parse the JSON string to pretty-print it
    let docs: Vec<serde_json::Value> = serde_json::from_str(history_json)?;
    println!("{}\n", serde_json::to_string_pretty(&docs)?);

    println!("--- Testing exclude_input_keys ---\n");

    // Create memory that excludes certain input keys from being stored
    let embeddings3 = Arc::new(MockEmbeddings);
    let vector_store3 = InMemoryVectorStore::new(embeddings3);
    let mut memory_exclude = VectorStoreRetrieverMemory::new(vector_store3)
        .with_k(1)
        .with_exclude_input_keys(vec!["system_prompt".to_string(), "temperature".to_string()]);

    // Save conversation with extra metadata that should be excluded
    let mut inputs = HashMap::new();
    inputs.insert("input".to_string(), "What is 2+2?".to_string());
    inputs.insert(
        "system_prompt".to_string(),
        "You are a helpful math tutor".to_string(),
    );
    inputs.insert("temperature".to_string(), "0.7".to_string());

    let mut outputs = HashMap::new();
    outputs.insert("output".to_string(), "2+2 equals 4".to_string());

    memory_exclude.save_context(&inputs, &outputs).await?;

    // Retrieve - excluded keys should not be in stored text
    let mut query = HashMap::new();
    query.insert("input".to_string(), "What is 3+3?".to_string());

    let vars = memory_exclude.load_memory_variables(&query).await?;
    let Some(history) = vars.get("history") else {
        eprintln!("No history returned for exclude_input_keys example; skipping.");
        return Ok(());
    };

    println!("Saved with exclude_input_keys=['system_prompt', 'temperature']:");
    println!("Retrieved: {}", history);
    println!(
        "Contains 'system_prompt': {}",
        history.contains("system_prompt")
    );
    println!(
        "Contains 'temperature': {}",
        history.contains("temperature")
    );
    println!("Contains 'input': {}", history.contains("input"));
    println!("Contains 'output': {}", history.contains("output"));

    println!("\n=== Example Complete ===");
    Ok(())
}
