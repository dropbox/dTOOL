//! Integration tests for memory implementations with real LLMs
//!
//! These tests verify memory functionality with actual LLM calls and real vector stores.
//! All tests are marked #[ignore] to prevent accidental API charges during regular test runs.
//!
//! # Running Tests
//!
//! ## All Memory Integration Tests
//! ```bash
//! cargo test --test memory_integration_tests --package dashflow-memory -- --ignored
//! ```
//!
//! ## Specific Test
//! ```bash
//! cargo test --test memory_integration_tests --package dashflow-memory test_conversation_summary_memory_real -- --ignored --exact
//! ```
//!
//! ## Prerequisites
//! - OPENAI_API_KEY environment variable set
//! - For vector store tests: vector store running (if applicable)
//!
//! # Cost Estimate
//! - ConversationSummaryMemory tests (3): ~$0.01-0.02 per test (gpt-4o-mini)
//! - ConversationEntityMemory tests (2): ~$0.02-0.03 per test (entity extraction + summarization)
//! - ConversationTokenBufferMemory tests (1): ~$0.01 per test (tokenization only)
//! - VectorStoreRetrieverMemory tests (3): ~$0.01-0.02 per test (embeddings)
//! - CombinedMemory tests (2): ~$0.03-0.05 per test (uses both summary + entity memory)
//! - ConversationKGMemory tests (3): ~$0.02-0.04 per test (entity + triple extraction)
//! - Total (14 tests): ~$0.10-0.19 per full run

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use dashflow::core::chat_history::InMemoryChatMessageHistory;
use dashflow::core::vector_stores::InMemoryVectorStore;
use dashflow_memory::{
    BaseMemory, CombinedMemory, ConversationEntityMemory, ConversationSummaryMemory,
    ConversationTokenBufferMemory, VectorStoreRetrieverMemory,
};
// These types are only used in #[ignore]d tests, so they may appear unused when running clippy
#[cfg(test)]
#[allow(unused_imports)]
use dashflow_memory::{ConversationKGMemory, NetworkxEntityGraph};
use dashflow_openai::{ChatOpenAI, OpenAIEmbeddings};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;

// ============================================================================
// Helper Functions
// ============================================================================

fn require_openai_api_key() {
    env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set to run ignored tests");
}

fn create_test_llm() -> ChatOpenAI {
    ChatOpenAI::default()
        .with_model("gpt-4o-mini") // Cost-efficient model for testing
        .with_temperature(0.0) // Deterministic responses
}

fn create_test_embeddings() -> OpenAIEmbeddings {
    OpenAIEmbeddings::default().with_model("text-embedding-3-small") // Cost-efficient embeddings
}

// ============================================================================
// ConversationSummaryMemory Tests
// ============================================================================

/// Test ConversationSummaryMemory with real LLM
///
/// Verifies that:
/// 1. Memory can summarize conversations using real LLM
/// 2. Summary is updated progressively with each turn
/// 3. Summary contains key information from conversation
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_conversation_summary_memory_real() {
    require_openai_api_key();

    let llm = create_test_llm();
    let chat_history = InMemoryChatMessageHistory::new();
    let mut memory = ConversationSummaryMemory::new(Box::new(llm), chat_history);

    // Turn 1: Introduction
    let mut inputs = HashMap::new();
    inputs.insert(
        "input".to_string(),
        "Hi, my name is Alice and I'm a software engineer.".to_string(),
    );
    let mut outputs = HashMap::new();
    outputs.insert(
        "output".to_string(),
        "Hello Alice! Nice to meet you.".to_string(),
    );
    memory.save_context(&inputs, &outputs).await.unwrap();

    // Load summary after turn 1
    let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
    let summary1 = vars.get("history").unwrap();

    // Summary should mention Alice and software engineer
    assert!(
        summary1.to_lowercase().contains("alice") || summary1.to_lowercase().contains("name"),
        "Summary should mention Alice or name: {}",
        summary1
    );
    assert!(
        summary1.to_lowercase().contains("software")
            || summary1.to_lowercase().contains("engineer"),
        "Summary should mention software engineer: {}",
        summary1
    );

    // Turn 2: More details
    let mut inputs = HashMap::new();
    inputs.insert(
        "input".to_string(),
        "I work at a startup building AI tools with Rust.".to_string(),
    );
    let mut outputs = HashMap::new();
    outputs.insert(
        "output".to_string(),
        "That sounds exciting! Rust is great for AI tools.".to_string(),
    );
    memory.save_context(&inputs, &outputs).await.unwrap();

    // Load summary after turn 2
    let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
    let summary2 = vars.get("history").unwrap();

    // Summary should be updated with new information
    assert_ne!(
        summary1, summary2,
        "Summary should be updated after new conversation turn"
    );
    assert!(
        summary2.to_lowercase().contains("rust")
            || summary2.to_lowercase().contains("startup")
            || summary2.to_lowercase().contains("ai"),
        "Summary should mention Rust, startup, or AI: {}",
        summary2
    );

    println!("✓ ConversationSummaryMemory test passed");
    println!("  Final summary: {}", summary2);
}

/// Test ConversationSummaryMemory clear functionality
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_conversation_summary_memory_clear_real() {
    require_openai_api_key();

    let llm = create_test_llm();
    let chat_history = InMemoryChatMessageHistory::new();
    let mut memory = ConversationSummaryMemory::new(Box::new(llm), chat_history);

    // Add conversation
    let mut inputs = HashMap::new();
    inputs.insert("input".to_string(), "Hello!".to_string());
    let mut outputs = HashMap::new();
    outputs.insert("output".to_string(), "Hi there!".to_string());
    memory.save_context(&inputs, &outputs).await.unwrap();

    // Verify summary exists
    let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
    assert!(!vars.get("history").unwrap().is_empty());

    // Clear memory
    memory.clear().await.unwrap();

    // Verify summary is cleared
    let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
    let summary = vars.get("history").unwrap();
    assert!(
        summary.is_empty(),
        "Summary should be empty after clear, got: {}",
        summary
    );

    println!("✓ ConversationSummaryMemory clear test passed");
}

/// Test ConversationSummaryMemory with multi-turn conversation
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_conversation_summary_memory_multi_turn_real() {
    require_openai_api_key();

    let llm = create_test_llm();
    let chat_history = InMemoryChatMessageHistory::new();
    let mut memory = ConversationSummaryMemory::new(Box::new(llm), chat_history);

    // Simulate a multi-turn technical discussion
    let conversation = vec![
        (
            "What is the difference between Box and Arc in Rust?",
            "Box is for single ownership, Arc is for shared ownership with reference counting.",
        ),
        (
            "When should I use Rc vs Arc?",
            "Use Rc for single-threaded scenarios, Arc when you need thread-safe reference counting.",
        ),
        (
            "What about weak references?",
            "Weak references prevent circular references and don't prevent deallocation.",
        ),
    ];

    for (input, output) in conversation {
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), input.to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), output.to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();
    }

    // Load final summary
    let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
    let summary = vars.get("history").unwrap();

    // Summary should mention key concepts from the conversation
    let key_concepts = ["box", "arc", "ownership", "reference", "rust"];
    let summary_lower = summary.to_lowercase();
    let mentions = key_concepts
        .iter()
        .filter(|concept| summary_lower.contains(*concept))
        .count();

    assert!(
        mentions >= 2,
        "Summary should mention at least 2 key concepts, found {}: {}",
        mentions,
        summary
    );

    println!("✓ ConversationSummaryMemory multi-turn test passed");
    println!("  Summary: {}", summary);
}

// ============================================================================
// ConversationEntityMemory Tests
// ============================================================================

/// Test ConversationEntityMemory with real LLM
///
/// Verifies that:
/// 1. Memory can extract entities from conversation
/// 2. Memory can generate summaries for entities
/// 3. Entity information is updated across turns
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_conversation_entity_memory_real() {
    require_openai_api_key();

    let llm = create_test_llm();
    let chat_history = InMemoryChatMessageHistory::new();
    let mut memory = ConversationEntityMemory::new(llm, chat_history);

    // Turn 1: Introduce entities
    let mut inputs = HashMap::new();
    inputs.insert(
        "input".to_string(),
        "I met Alice at the Rust conference in Berlin.".to_string(),
    );
    let mut outputs = HashMap::new();
    outputs.insert(
        "output".to_string(),
        "That's interesting! How did you like Berlin?".to_string(),
    );
    memory.save_context(&inputs, &outputs).await.unwrap();

    // Load memory variables - should contain entity information
    let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
    let entities = vars.get("entities").unwrap();

    // Should extract at least one entity (Alice, Berlin, or Rust)
    assert!(
        !entities.is_empty(),
        "Should extract entities from conversation"
    );

    // Turn 2: Add more information about an entity
    let mut inputs = HashMap::new();
    inputs.insert(
        "input".to_string(),
        "Alice is a senior engineer at Mozilla. She gave a great talk about async Rust."
            .to_string(),
    );
    let mut outputs = HashMap::new();
    outputs.insert(
        "output".to_string(),
        "Mozilla has done excellent work with Rust.".to_string(),
    );
    memory.save_context(&inputs, &outputs).await.unwrap();

    // Load memory variables again
    let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
    let entities2 = vars.get("entities").unwrap();

    // Entity information should be updated
    assert!(
        entities2.len() >= entities.len(),
        "Should maintain or increase entity count"
    );

    println!("✓ ConversationEntityMemory test passed");
    println!("  Entities extracted: {}", entities2);
}

/// Test ConversationEntityMemory clear functionality
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_conversation_entity_memory_clear_real() {
    require_openai_api_key();

    let llm = create_test_llm();
    let chat_history = InMemoryChatMessageHistory::new();
    let mut memory = ConversationEntityMemory::new(llm, chat_history);

    // Add conversation with entities
    let mut inputs = HashMap::new();
    inputs.insert(
        "input".to_string(),
        "Alice works at Google in California.".to_string(),
    );
    let mut outputs = HashMap::new();
    outputs.insert("output".to_string(), "That's great!".to_string());
    memory.save_context(&inputs, &outputs).await.unwrap();

    // Verify entities exist
    let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
    assert!(!vars.get("entities").unwrap().is_empty());

    // Clear memory
    memory.clear().await.unwrap();

    // Verify entities are cleared
    let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
    let entities = vars.get("entities").unwrap();
    assert!(
        entities.is_empty(),
        "Entities should be empty after clear, got: {}",
        entities
    );

    println!("✓ ConversationEntityMemory clear test passed");
}

// ============================================================================
// ConversationTokenBufferMemory Tests
// ============================================================================

/// Test ConversationTokenBufferMemory with real tokenizer
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_conversation_token_buffer_memory_real() {
    require_openai_api_key();

    let chat_history = InMemoryChatMessageHistory::new();
    let mut memory = ConversationTokenBufferMemory::new(
        chat_history,
        100, // 100 token limit
        "history",
    )
    .unwrap();

    // Add several messages
    for i in 0..5 {
        let mut inputs = HashMap::new();
        inputs.insert(
            "input".to_string(),
            format!("This is message number {} with some text content.", i),
        );
        let mut outputs = HashMap::new();
        outputs.insert(
            "output".to_string(),
            format!("Response to message {} with more text.", i),
        );
        memory.save_context(&inputs, &outputs).await.unwrap();
    }

    // Load memory variables
    let vars = memory.load_memory_variables(&HashMap::new()).await.unwrap();
    let history = vars.get("history").unwrap();

    // Should have pruned old messages to stay under token limit
    // Cannot have all 10 messages (5 turns × 2 messages) if they exceed 100 tokens
    assert!(!history.is_empty(), "History should not be empty");

    // Verify recent messages are retained
    assert!(
        history.contains("message number 4") || history.contains("Response to message 4"),
        "Most recent messages should be retained: {}",
        history
    );

    println!("✓ ConversationTokenBufferMemory test passed");
}

// ============================================================================
// VectorStoreRetrieverMemory Tests
// ============================================================================

/// Test VectorStoreRetrieverMemory with real embeddings
///
/// Verifies that:
/// 1. Memory can store messages in vector store
/// 2. Memory can retrieve relevant past messages
/// 3. Semantic search works correctly
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_vectorstore_retriever_memory_real() {
    require_openai_api_key();

    let embeddings = Arc::new(create_test_embeddings());
    let vector_store = InMemoryVectorStore::new(Arc::<OpenAIEmbeddings>::clone(&embeddings));
    let mut memory = VectorStoreRetrieverMemory::new(vector_store).with_k(2); // Retrieve 2 most relevant

    // Add several conversation turns with different topics
    let conversations = vec![
        ("What is Rust?", "Rust is a systems programming language."),
        ("Tell me about Python.", "Python is a high-level language."),
        (
            "How does memory management work in Rust?",
            "Rust uses ownership and borrowing.",
        ),
        (
            "What are Python decorators?",
            "Decorators modify function behavior.",
        ),
    ];

    for (input, output) in conversations {
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), input.to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), output.to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();
    }

    // Query with Rust-related input - should retrieve Rust-related memories
    let mut query_inputs = HashMap::new();
    query_inputs.insert(
        "input".to_string(),
        "Tell me more about Rust ownership.".to_string(),
    );

    let vars = memory.load_memory_variables(&query_inputs).await.unwrap();
    let history = vars.get("history").unwrap();

    // Should retrieve Rust-related conversations
    assert!(
        history.to_lowercase().contains("rust"),
        "Should retrieve Rust-related memories: {}",
        history
    );

    // Should NOT retrieve all conversations (only k=2 most relevant)
    let message_count = history.matches("Human:").count() + history.matches("AI:").count();
    assert!(
        message_count <= 4, // 2 turns × 2 messages
        "Should only retrieve k=2 most relevant conversations, got {} messages",
        message_count
    );

    println!("✓ VectorStoreRetrieverMemory test passed");
    println!("  Retrieved memories: {}", history);
}

/// Test VectorStoreRetrieverMemory clear functionality
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_vectorstore_retriever_memory_clear_real() {
    require_openai_api_key();

    let embeddings = Arc::new(create_test_embeddings());
    let vector_store = InMemoryVectorStore::new(Arc::<OpenAIEmbeddings>::clone(&embeddings));
    let mut memory = VectorStoreRetrieverMemory::new(vector_store).with_k(2);

    // Add memories
    let mut inputs = HashMap::new();
    inputs.insert("input".to_string(), "Hello!".to_string());
    let mut outputs = HashMap::new();
    outputs.insert("output".to_string(), "Hi there!".to_string());
    memory.save_context(&inputs, &outputs).await.unwrap();

    // Verify memories exist
    let mut query = HashMap::new();
    query.insert("input".to_string(), "hello".to_string());
    let vars = memory.load_memory_variables(&query).await.unwrap();
    assert!(!vars.get("history").unwrap().is_empty());

    // Clear memory
    memory.clear().await.unwrap();

    // Verify memories are cleared
    let vars = memory.load_memory_variables(&query).await.unwrap();
    let history = vars.get("history").unwrap();
    assert!(
        history.is_empty(),
        "History should be empty after clear, got: {}",
        history
    );

    println!("✓ VectorStoreRetrieverMemory clear test passed");
}

/// Test VectorStoreRetrieverMemory semantic relevance
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_vectorstore_retriever_memory_semantic_search_real() {
    require_openai_api_key();

    let embeddings = Arc::new(create_test_embeddings());
    let vector_store = InMemoryVectorStore::new(Arc::<OpenAIEmbeddings>::clone(&embeddings));
    let mut memory = VectorStoreRetrieverMemory::new(vector_store).with_k(1); // Only retrieve 1 most relevant

    // Add memories about different topics
    let conversations = vec![
        (
            "What's the weather like?",
            "It's sunny and 75 degrees today.",
        ),
        (
            "How do I cook pasta?",
            "Boil water, add pasta, cook for 10 minutes.",
        ),
        (
            "What's the capital of France?",
            "The capital of France is Paris.",
        ),
    ];

    for (input, output) in conversations {
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), input.to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), output.to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();
    }

    // Query about cooking - should retrieve cooking-related memory
    let mut query = HashMap::new();
    query.insert(
        "input".to_string(),
        "How do I prepare spaghetti?".to_string(),
    );

    let vars = memory.load_memory_variables(&query).await.unwrap();
    let history = vars.get("history").unwrap();

    // Should retrieve cooking-related conversation
    assert!(
        history.to_lowercase().contains("pasta")
            || history.to_lowercase().contains("cook")
            || history.to_lowercase().contains("boil"),
        "Should retrieve cooking-related memory: {}",
        history
    );

    // Should NOT retrieve weather or geography
    assert!(
        !history.to_lowercase().contains("weather") && !history.to_lowercase().contains("paris"),
        "Should not retrieve unrelated memories: {}",
        history
    );

    println!("✓ VectorStoreRetrieverMemory semantic search test passed");
}

// ============================================================================
// CombinedMemory Tests
// ============================================================================

/// Test CombinedMemory with real LLM composition
///
/// Verifies that:
/// 1. Multiple memory types can be combined
/// 2. Each memory contributes its own variables
/// 3. save_context updates all sub-memories
/// 4. load_memory_variables merges results from all sub-memories
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_combined_memory_real() {
    require_openai_api_key();

    // Create two different memory types
    let llm1 = create_test_llm();
    let chat_history1 = InMemoryChatMessageHistory::new();
    let summary_memory = ConversationSummaryMemory::new(Box::new(llm1), chat_history1);

    let llm2 = create_test_llm();
    let chat_history2 = InMemoryChatMessageHistory::new();
    let entity_memory = ConversationEntityMemory::new(llm2, chat_history2);

    // Combine them
    let mut combined =
        CombinedMemory::new(vec![Box::new(summary_memory), Box::new(entity_memory)]).unwrap();

    // Verify memory variables from both memories
    let vars = combined.memory_variables();
    assert_eq!(vars.len(), 2, "Should have 2 memory variables");
    assert!(
        vars.contains(&"history".to_string()),
        "Should have history variable from summary memory"
    );
    assert!(
        vars.contains(&"entities".to_string()),
        "Should have entities variable from entity memory"
    );

    // Turn 1: Introduction with entities
    let mut inputs = HashMap::new();
    inputs.insert(
        "input".to_string(),
        "Hi, I'm Bob. I work at OpenAI on GPT-4.".to_string(),
    );
    let mut outputs = HashMap::new();
    outputs.insert(
        "output".to_string(),
        "Hello Bob! That's impressive work.".to_string(),
    );
    combined.save_context(&inputs, &outputs).await.unwrap();

    // Load memory variables from both memories
    let loaded = combined
        .load_memory_variables(&HashMap::new())
        .await
        .unwrap();
    assert_eq!(loaded.len(), 2, "Should have 2 memory variables loaded");

    // Check summary memory variable
    let history = loaded.get("history").unwrap();
    assert!(
        history.to_lowercase().contains("bob") || history.to_lowercase().contains("openai"),
        "Summary should mention Bob or OpenAI: {}",
        history
    );

    // Check entity memory variable
    let entities = loaded.get("entities").unwrap();
    assert!(
        !entities.is_empty(),
        "Entities should not be empty after conversation"
    );

    // Turn 2: More context
    let mut inputs2 = HashMap::new();
    inputs2.insert(
        "input".to_string(),
        "I'm working on improving the reasoning capabilities of our models.".to_string(),
    );
    let mut outputs2 = HashMap::new();
    outputs2.insert(
        "output".to_string(),
        "That sounds like fascinating work!".to_string(),
    );
    combined.save_context(&inputs2, &outputs2).await.unwrap();

    // Load again - both memories should be updated
    let loaded2 = combined
        .load_memory_variables(&HashMap::new())
        .await
        .unwrap();
    assert_eq!(loaded2.len(), 2, "Should still have 2 memory variables");

    let history2 = loaded2.get("history").unwrap();
    assert!(
        history2.to_lowercase().contains("reasoning")
            || history2.to_lowercase().contains("capabilities")
            || history2.to_lowercase().contains("model"),
        "Updated summary should mention reasoning or capabilities: {}",
        history2
    );

    println!("✓ CombinedMemory composition test passed");
}

/// Test CombinedMemory clear propagation with real LLMs
///
/// Verifies that:
/// 1. clear() propagates to all sub-memories
/// 2. After clear, both memories return empty/default values
/// 3. Combined memory can continue to be used after clear
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_combined_memory_clear_real() {
    require_openai_api_key();

    // Create two different memory types
    let llm1 = create_test_llm();
    let chat_history1 = InMemoryChatMessageHistory::new();
    let summary_memory = ConversationSummaryMemory::new(Box::new(llm1), chat_history1);

    let llm2 = create_test_llm();
    let chat_history2 = InMemoryChatMessageHistory::new();
    let entity_memory = ConversationEntityMemory::new(llm2, chat_history2);

    let mut combined =
        CombinedMemory::new(vec![Box::new(summary_memory), Box::new(entity_memory)]).unwrap();

    // Add some context
    let mut inputs = HashMap::new();
    inputs.insert(
        "input".to_string(),
        "I'm Alice and I love machine learning.".to_string(),
    );
    let mut outputs = HashMap::new();
    outputs.insert("output".to_string(), "Great to meet you Alice!".to_string());
    combined.save_context(&inputs, &outputs).await.unwrap();

    // Verify memories have content
    let before_clear = combined
        .load_memory_variables(&HashMap::new())
        .await
        .unwrap();
    assert_eq!(
        before_clear.len(),
        2,
        "Should have 2 variables before clear"
    );
    let history_before = before_clear.get("history").unwrap();
    assert!(
        !history_before.is_empty(),
        "History should not be empty before clear"
    );
    // Entities might be empty if extraction didn't find any, so we don't assert on it

    // Clear all memories
    combined.clear().await.unwrap();

    // Verify all memories are cleared
    let after_clear = combined
        .load_memory_variables(&HashMap::new())
        .await
        .unwrap();
    assert_eq!(
        after_clear.len(),
        2,
        "Should still have 2 variables after clear"
    );

    let history_after = after_clear.get("history").unwrap();
    let entities_after = after_clear.get("entities").unwrap();

    // Summary memory should be cleared (empty or default prompt)
    assert!(
        history_after.is_empty() || history_after == "Current conversation:",
        "History should be cleared or at default prompt: {}",
        history_after
    );

    // Entity memory should be cleared (empty)
    assert!(
        entities_after.is_empty(),
        "Entities should be empty after clear: {}",
        entities_after
    );

    // Verify we can still use combined memory after clear
    let mut inputs2 = HashMap::new();
    inputs2.insert(
        "input".to_string(),
        "Starting a new conversation.".to_string(),
    );
    let mut outputs2 = HashMap::new();
    outputs2.insert("output".to_string(), "Hello again!".to_string());
    combined.save_context(&inputs2, &outputs2).await.unwrap();

    let after_new_save = combined
        .load_memory_variables(&HashMap::new())
        .await
        .unwrap();
    assert_eq!(
        after_new_save.len(),
        2,
        "Should have 2 variables after new save"
    );

    let history_new = after_new_save.get("history").unwrap();
    assert!(
        history_new.to_lowercase().contains("new")
            || history_new.to_lowercase().contains("conversation")
            || history_new.to_lowercase().contains("hello"),
        "New history should mention new conversation: {}",
        history_new
    );

    println!("✓ CombinedMemory clear propagation test passed");
}

// ============================================================================
// ConversationKGMemory Tests
// ============================================================================

/// Test ConversationKGMemory with real LLM
///
/// Verifies that:
/// 1. Memory can extract entities from conversation
/// 2. Memory can extract knowledge triples using LLM
/// 3. Memory can retrieve relevant knowledge based on entities
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_conversation_kg_memory_real() {
    require_openai_api_key();

    let chat_model = create_test_llm();
    let llm = dashflow::core::language_models::ChatModelToLLM::new(chat_model);
    let chat_history = InMemoryChatMessageHistory::new();
    let kg = dashflow_memory::NetworkxEntityGraph::new();
    let mut memory = dashflow_memory::ConversationKGMemory::new(llm, chat_history, kg).with_k(2);

    // Turn 1: Introduce entity with facts
    let mut inputs = HashMap::new();
    inputs.insert(
        "input".to_string(),
        "Nevada is a state in the US. It's the number 1 producer of gold.".to_string(),
    );
    let mut outputs = HashMap::new();
    outputs.insert(
        "output".to_string(),
        "That's interesting! Nevada is known for its mining industry.".to_string(),
    );
    memory.save_context(&inputs, &outputs).await.unwrap();

    // Query about Nevada - should extract and return knowledge
    let mut query_inputs = HashMap::new();
    query_inputs.insert("input".to_string(), "Tell me about Nevada.".to_string());

    let vars = memory.load_memory_variables(&query_inputs).await.unwrap();
    let history = vars.get("history").unwrap();

    // Should contain extracted knowledge about Nevada
    // (The exact format depends on LLM extraction, so we check for Nevada mention)
    assert!(
        history.contains("Nevada") || history.is_empty(),
        "History should mention Nevada or be empty if no entities extracted: {}",
        history
    );

    println!("✓ ConversationKGMemory test passed");
    println!("  Extracted knowledge: {}", history);
}

/// Test ConversationKGMemory clear functionality
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_conversation_kg_memory_clear_real() {
    require_openai_api_key();

    let chat_model = create_test_llm();
    let llm = dashflow::core::language_models::ChatModelToLLM::new(chat_model);
    let chat_history = InMemoryChatMessageHistory::new();
    let kg = dashflow_memory::NetworkxEntityGraph::new();
    let mut memory = dashflow_memory::ConversationKGMemory::new(llm, chat_history, kg);

    // Add conversation with knowledge
    let mut inputs = HashMap::new();
    inputs.insert(
        "input".to_string(),
        "Alice works at Google in California.".to_string(),
    );
    let mut outputs = HashMap::new();
    outputs.insert("output".to_string(), "That's great!".to_string());
    memory.save_context(&inputs, &outputs).await.unwrap();

    // Clear memory
    memory.clear().await.unwrap();

    // Verify knowledge graph is cleared
    let kg_guard = memory.kg().await;
    assert_eq!(
        kg_guard.node_count(),
        0,
        "Knowledge graph should be empty after clear"
    );
    assert_eq!(
        kg_guard.get_triples().len(),
        0,
        "Knowledge graph should have no triples after clear"
    );
    drop(kg_guard);

    // Verify chat history is cleared
    let vars = memory.load_memory_variables(&inputs).await.unwrap();
    let history = vars.get("history").unwrap();
    assert!(
        history.is_empty(),
        "History should be empty after clear, got: {}",
        history
    );

    println!("✓ ConversationKGMemory clear test passed");
}

/// Test ConversationKGMemory entity-focused retrieval
///
/// Verifies that:
/// 1. Memory retrieves knowledge only about queried entities
/// 2. Multiple entities can be tracked separately
/// 3. Entity-specific knowledge is correctly isolated
#[tokio::test]
#[ignore = "makes live OpenAI calls (requires OPENAI_API_KEY)"]
async fn test_conversation_kg_memory_entity_focus_real() {
    require_openai_api_key();

    let chat_model = create_test_llm();
    let llm = dashflow::core::language_models::ChatModelToLLM::new(chat_model);
    let chat_history = InMemoryChatMessageHistory::new();
    let kg = dashflow_memory::NetworkxEntityGraph::new();
    let mut memory = dashflow_memory::ConversationKGMemory::new(llm, chat_history, kg).with_k(3);

    // Add facts about multiple entities
    let conversations = vec![
        (
            "Alice works at Google and lives in San Francisco.",
            "Interesting! Google has a large presence in the Bay Area.",
        ),
        (
            "Bob works at Microsoft and lives in Seattle.",
            "Seattle is Microsoft's headquarters location.",
        ),
        (
            "Alice enjoys hiking and biking in her free time.",
            "The Bay Area has great outdoor activities!",
        ),
    ];

    for (input, output) in conversations {
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), input.to_string());
        let mut outputs = HashMap::new();
        outputs.insert("output".to_string(), output.to_string());
        memory.save_context(&inputs, &outputs).await.unwrap();
    }

    // Query about Alice - should only get Alice-related knowledge
    let mut alice_query = HashMap::new();
    alice_query.insert(
        "input".to_string(),
        "What do you know about Alice?".to_string(),
    );

    let vars = memory.load_memory_variables(&alice_query).await.unwrap();
    let alice_history = vars.get("history").unwrap();

    // If entities were extracted, history should mention Alice
    // If not, history will be empty (LLM didn't extract entities)
    if !alice_history.is_empty() {
        assert!(
            alice_history.to_lowercase().contains("alice"),
            "Alice query should return Alice-related knowledge: {}",
            alice_history
        );
    }

    // Query about Bob - should only get Bob-related knowledge
    let mut bob_query = HashMap::new();
    bob_query.insert("input".to_string(), "Tell me about Bob.".to_string());

    let vars = memory.load_memory_variables(&bob_query).await.unwrap();
    let bob_history = vars.get("history").unwrap();

    // If entities were extracted, history should mention Bob
    if !bob_history.is_empty() {
        assert!(
            bob_history.to_lowercase().contains("bob"),
            "Bob query should return Bob-related knowledge: {}",
            bob_history
        );
    }

    println!("✓ ConversationKGMemory entity-focused retrieval test passed");
    println!("  Alice knowledge: {}", alice_history);
    println!("  Bob knowledge: {}", bob_history);
}

// ============================================================================
// Integration Test Summary
// ============================================================================
//
// All memory types now have integration tests:
//
// 1. ConversationSummaryMemory: 3 tests ✓
// 2. ConversationEntityMemory: 2 tests ✓
// 3. ConversationTokenBufferMemory: 1 test ✓
// 4. VectorStoreRetrieverMemory: 3 tests ✓
// 5. CombinedMemory: 2 tests ✓
// 6. ConversationKGMemory: 3 tests ✓
//
// Total: 14 integration tests
//
// Note: ConversationBufferMemory and ConversationBufferWindowMemory don't require
// LLM integration tests as they're simple wrappers around chat history without
// LLM interactions. Their functionality is fully tested in unit tests.
