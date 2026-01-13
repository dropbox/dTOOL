//! Comprehensive Chain Integration Tests with Real LLMs
//!
//! These tests verify real chain execution with real LLM providers.
//!
//! Prerequisites:
//! - OPENAI_API_KEY environment variable must be set
//!
//! Run with: cargo test --test chain_integration_tests --package dashflow-chains
//!
//! Note: Tests are ignored by default to avoid accidental API calls

use dashflow::core::documents::Document;
use dashflow::core::error::{Error, Result};
use dashflow::core::prompts::PromptTemplate;
use dashflow_chains::combine_documents::{
    MapReduceDocumentsChain, RefineDocumentsChain, StuffDocumentsChain,
};
use dashflow_chains::summarize::{load_summarize_chain_chat, SummarizeChain, SummarizeChainType};
use dashflow_chains::{HypotheticalDocumentEmbedder, TransformChain};
use dashflow_openai::{embeddings::OpenAIEmbeddings, ChatOpenAI};
use std::collections::HashMap;
use std::sync::Arc;

/// Helper to check if OpenAI API key is available
fn has_openai_key() -> bool {
    std::env::var("OPENAI_API_KEY").is_ok()
}

// =============================================================================
// DOCUMENT COMBINING CHAIN TESTS
// =============================================================================

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_stuff_documents_chain_basic() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create LLM
    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini") // Cost-efficient model
            .with_temperature(0.0), // Deterministic
    );

    // Create chain with custom prompt
    let prompt = PromptTemplate::from_template(
        "Write a brief summary (1-2 sentences) of the following documents:\n\n{context}",
    )?;

    let chain = StuffDocumentsChain::new_chat(llm)
        .with_prompt(prompt)
        .with_document_variable_name("context");

    // Create test documents
    let docs = vec![
        Document::new("Rust is a systems programming language focused on safety and performance."),
        Document::new("Rust's ownership system ensures memory safety without garbage collection."),
        Document::new("Rust is popular for building reliable and efficient software."),
    ];

    // Execute chain
    let (output, _extra_keys) = chain.combine_docs(&docs, None).await?;

    // Verify output
    assert!(!output.is_empty(), "Expected non-empty summary");
    assert!(
        output.to_lowercase().contains("rust")
            || output.to_lowercase().contains("programming")
            || output.to_lowercase().contains("safety"),
        "Expected summary to mention key topics, got: {}",
        output
    );
    assert!(
        output.len() < 500,
        "Expected brief summary, got {} characters",
        output.len()
    );

    println!("StuffDocumentsChain output: {}", output);
    Ok(())
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_map_reduce_documents_chain() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create LLM
    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    // Create prompts
    let map_prompt =
        PromptTemplate::from_template("Summarize this in one sentence:\n\n{page_content}")?;
    let reduce_prompt =
        PromptTemplate::from_template("Combine these summaries into one:\n\n{context}")?;

    // Create reduce chain
    let reduce_chain = StuffDocumentsChain::new_chat(Arc::<ChatOpenAI>::clone(&llm))
        .with_prompt(reduce_prompt)
        .with_document_variable_name("context");

    // Create map-reduce chain
    let chain = MapReduceDocumentsChain::new_chat(llm)
        .with_map_prompt(map_prompt)
        .with_reduce_chain(reduce_chain);

    // Create test documents (enough to require map-reduce)
    let docs = vec![
        Document::new("Python was created by Guido van Rossum in 1991. It emphasizes readability."),
        Document::new("Python is widely used in data science, web development, and automation."),
        Document::new("Python has a large ecosystem of packages available via pip."),
        Document::new("Python's simplicity makes it popular for beginners and experts alike."),
    ];

    // Execute chain
    let (output, _extra_keys) = chain.combine_docs(&docs, None).await?;

    // Verify output
    assert!(!output.is_empty(), "Expected non-empty summary");
    assert!(
        output.to_lowercase().contains("python"),
        "Expected summary to mention Python, got: {}",
        output
    );

    println!("MapReduceDocumentsChain output: {}", output);
    Ok(())
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_refine_documents_chain() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    // Create LLM
    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    // Create prompts
    let initial_prompt = PromptTemplate::from_template("Summarize this:\n\n{context}")?;
    let refine_prompt = PromptTemplate::from_template(
        "Here's an existing summary: {existing_answer}\n\n\
         Refine it with this additional context:\n{context}\n\n\
         Provide a refined summary:",
    )?;

    // Create chain
    let chain = RefineDocumentsChain::new_chat(llm)
        .with_initial_prompt(initial_prompt)
        .with_refine_prompt(refine_prompt);

    // Create test documents
    let docs = vec![
        Document::new("JavaScript was created in 1995 for web browsers."),
        Document::new("JavaScript is now used for both frontend and backend with Node.js."),
        Document::new("JavaScript has evolved significantly with ES6 and later versions."),
    ];

    // Execute chain
    let (output, _extra_keys) = chain.combine_docs(&docs, None).await?;

    // Verify output
    assert!(!output.is_empty(), "Expected non-empty refined summary");
    assert!(
        output.to_lowercase().contains("javascript") || output.to_lowercase().contains("js"),
        "Expected summary to mention JavaScript, got: {}",
        output
    );

    println!("RefineDocumentsChain output: {}", output);
    Ok(())
}

// =============================================================================
// SUMMARIZATION CHAIN TESTS
// =============================================================================

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_summarize_chain_stuff() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    let chain = load_summarize_chain_chat(llm, SummarizeChainType::Stuff);

    let docs = vec![
        Document::new("TypeScript adds static typing to JavaScript."),
        Document::new("TypeScript code compiles to plain JavaScript."),
    ];

    let SummarizeChain::Stuff(c) = chain else {
        return Err(Error::InvalidInput("Expected Stuff chain".to_string()));
    };
    let output = c.combine_docs(&docs, None).await?.0;

    assert!(!output.is_empty());
    assert!(
        output.to_lowercase().contains("typescript")
            || output.to_lowercase().contains("typing")
            || output.to_lowercase().contains("javascript"),
        "Expected TypeScript-related content, got: {}",
        output
    );

    println!("Summarize (Stuff) output: {}", output);
    Ok(())
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_summarize_chain_map_reduce() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    let chain = load_summarize_chain_chat(llm, SummarizeChainType::MapReduce);

    let docs = vec![
        Document::new("Go is a compiled language designed at Google."),
        Document::new("Go has built-in concurrency with goroutines."),
        Document::new("Go is popular for cloud-native applications."),
    ];

    let SummarizeChain::MapReduce(c) = chain else {
        return Err(Error::InvalidInput("Expected MapReduce chain".to_string()));
    };
    let output = c.combine_docs(&docs, None).await?.0;

    assert!(!output.is_empty());
    assert!(
        output.to_lowercase().contains("go") || output.to_lowercase().contains("golang"),
        "Expected Go-related content, got: {}",
        output
    );

    println!("Summarize (MapReduce) output: {}", output);
    Ok(())
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_summarize_chain_refine() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    let chain = load_summarize_chain_chat(llm, SummarizeChainType::Refine);

    let docs = vec![
        Document::new("Swift is Apple's modern programming language."),
        Document::new("Swift is used for iOS, macOS, and other Apple platforms."),
    ];

    let SummarizeChain::Refine(c) = chain else {
        return Err(Error::InvalidInput("Expected Refine chain".to_string()));
    };
    let output = c.combine_docs(&docs, None).await?.0;

    assert!(!output.is_empty());
    assert!(
        output.to_lowercase().contains("swift") || output.to_lowercase().contains("apple"),
        "Expected Swift-related content, got: {}",
        output
    );

    println!("Summarize (Refine) output: {}", output);
    Ok(())
}

// =============================================================================
// LLMMATH CHAIN TESTS
// =============================================================================
// Note: LLMMathChain requires LLM trait, not ChatModel.
// ChatModel-based tests would require a wrapper or separate implementation.
// Skipping LLMMathChain tests as they need LLM provider (OpenAI completion API)
// which is deprecated in favor of ChatModels.

// =============================================================================
// TRANSFORM CHAIN TESTS
// =============================================================================

#[test]
fn test_transform_chain_basic() -> Result<()> {
    // TransformChain doesn't need API keys - it's pure transformation

    let chain = TransformChain::new(
        vec!["input".to_string()],
        vec!["output".to_string()],
        Box::new(|inputs: &HashMap<String, String>| {
            let mut result = HashMap::new();
            if let Some(input) = inputs.get("input") {
                result.insert("output".to_string(), input.to_uppercase());
            }
            Ok(result)
        }),
    );

    let mut inputs = HashMap::new();
    inputs.insert("input".to_string(), "hello world".to_string());

    let output = chain.transform(&inputs)?;

    assert_eq!(
        output.get("output"),
        Some(&"HELLO WORLD".to_string()),
        "Expected uppercase transformation"
    );

    println!("TransformChain output: {:?}", output);
    Ok(())
}

#[test]
fn test_transform_chain_multiple_outputs() -> Result<()> {
    let chain = TransformChain::new(
        vec!["text".to_string()],
        vec![
            "length".to_string(),
            "words".to_string(),
            "uppercase".to_string(),
        ],
        Box::new(|inputs: &HashMap<String, String>| {
            let mut result = HashMap::new();
            if let Some(text) = inputs.get("text") {
                result.insert("length".to_string(), text.len().to_string());
                result.insert(
                    "words".to_string(),
                    text.split_whitespace().count().to_string(),
                );
                result.insert("uppercase".to_string(), text.to_uppercase());
            }
            Ok(result)
        }),
    );

    let mut inputs = HashMap::new();
    inputs.insert("text".to_string(), "The quick brown fox".to_string());

    let output = chain.transform(&inputs)?;

    assert_eq!(output.get("length"), Some(&"19".to_string()));
    assert_eq!(output.get("words"), Some(&"4".to_string()));
    assert_eq!(
        output.get("uppercase"),
        Some(&"THE QUICK BROWN FOX".to_string())
    );

    println!("TransformChain (multiple outputs): {:?}", output);
    Ok(())
}

// =============================================================================
// HYDE (HYPOTHETICAL DOCUMENT EMBEDDER) TESTS
// =============================================================================

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_hyde_basic() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.3), // Slight temperature for document generation
    );

    let embeddings = Arc::new(OpenAIEmbeddings::try_new()?.with_model("text-embedding-3-small"));

    let hyde = HypotheticalDocumentEmbedder::from_prompt_key(llm, embeddings, "web_search")
        .map_err(dashflow::core::error::Error::Other)?;

    // Generate hypothetical document and embedding
    let query = "What are the benefits of using Rust for systems programming?";
    let embedding_result = hyde.embed_query(query).await?;

    // Verify embedding
    assert_eq!(
        embedding_result.len(),
        1536,
        "Expected text-embedding-3-small to produce 1536-dimensional vector"
    );

    // Verify embedding is not all zeros
    let non_zero_count = embedding_result.iter().filter(|&&x| x != 0.0).count();
    assert!(
        non_zero_count > 100,
        "Expected meaningful embedding, got mostly zeros"
    );

    println!(
        "HyDE embedding vector length: {}, non-zero elements: {}",
        embedding_result.len(),
        non_zero_count
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_hyde_custom_prompt() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.3),
    );

    let embeddings = Arc::new(OpenAIEmbeddings::try_new()?.with_model("text-embedding-3-small"));

    // HyDE with custom prompt for a different domain
    let hyde = HypotheticalDocumentEmbedder::from_prompt_key(llm, embeddings, "fiqa")
        .map_err(dashflow::core::error::Error::Other)?;

    let query = "What are the best investment strategies for retirement?";
    let embedding_result = hyde.embed_query(query).await?;

    // Should generate embedding
    assert_eq!(embedding_result.len(), 1536);

    let non_zero_count = embedding_result.iter().filter(|&&x| x != 0.0).count();
    assert!(non_zero_count > 100);

    println!(
        "HyDE (fiqa prompt) embedding vector length: {}, non-zero elements: {}",
        embedding_result.len(),
        non_zero_count
    );

    Ok(())
}

// =============================================================================
// DOCUMENT COMBINING WITH CUSTOM SEPARATORS
// =============================================================================

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_stuff_documents_custom_separator() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    let prompt = PromptTemplate::from_template("Summarize these items:\n\n{context}")?;

    // Use custom separator
    let chain = StuffDocumentsChain::new_chat(llm)
        .with_prompt(prompt)
        .with_document_separator("\n---\n");

    let docs = vec![
        Document::new("Item 1: Red apples"),
        Document::new("Item 2: Green bananas"),
        Document::new("Item 3: Yellow lemons"),
    ];

    let (output, _) = chain.combine_docs(&docs, None).await?;

    assert!(!output.is_empty());
    println!("Custom separator output: {}", output);

    Ok(())
}

// =============================================================================
// ERROR HANDLING TESTS
// =============================================================================

#[test]
fn test_transform_chain_error_handling() -> Result<()> {
    let chain = TransformChain::new(
        vec!["input".to_string()],
        vec!["output".to_string()],
        Box::new(|_inputs: &HashMap<String, String>| {
            // Simulate error
            Err(dashflow::core::error::Error::InvalidInput(
                "Simulated error".to_string(),
            ))
        }),
    );

    let mut inputs = HashMap::new();
    inputs.insert("input".to_string(), "test".to_string());

    let result = chain.transform(&inputs);

    assert!(result.is_err(), "Expected error from transform chain");

    Ok(())
}

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY"]
async fn test_stuff_documents_empty_docs() -> Result<()> {
    assert!(has_openai_key(), "OPENAI_API_KEY must be set");

    let llm = Arc::new(
        ChatOpenAI::with_config(Default::default())
            .with_model("gpt-4o-mini")
            .with_temperature(0.0),
    );

    let chain = StuffDocumentsChain::new_chat(llm);

    // Empty documents
    let docs: Vec<Document> = vec![];

    let result = chain.combine_docs(&docs, None).await;

    // Should handle gracefully (may return empty or error)
    match result {
        Ok((output, _)) => {
            println!("Empty docs output: {}", output);
            Ok(())
        }
        Err(e) => {
            println!("Empty docs error (expected): {}", e);
            Ok(()) // Expected behavior
        }
    }
}
