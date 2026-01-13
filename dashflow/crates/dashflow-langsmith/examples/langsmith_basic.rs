//! # LangSmith Client Example
//!
//! This example demonstrates how to use the LangSmith client to create and manage runs.
//!
//! **Prerequisites:**
//! - LangSmith account at https://smith.dashflow.com
//! - API key from your LangSmith account settings
//!
//! **Environment Variables:**
//! ```bash
//! export LANGSMITH_API_KEY="lsv2_pt_..."
//! export LANGSMITH_PROJECT="my-project"  # Optional
//! ```
//!
//! **Run this example:**
//! ```bash
//! cargo run --package dashflow-langsmith --example langsmith_basic
//! ```
//!
//! Covers:
//! - Creating a LangSmith client
//! - Creating individual runs
//! - Updating runs with outputs
//! - Batch ingestion of runs
//! - Parent-child run hierarchies
//! - Tags and metadata

use chrono::Utc;
use dashflow_langsmith::{Client, RunCreate, RunType, RunUpdate};
use std::collections::HashMap;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== LangSmith Client Example ===\n");

    // Check for API key
    if std::env::var("LANGSMITH_API_KEY").is_err() {
        println!("ERROR: LANGSMITH_API_KEY environment variable is not set");
        println!("Please set it to your LangSmith API key from https://smith.dashflow.com");
        println!("\nExample:");
        println!("  export LANGSMITH_API_KEY=\"lsv2_pt_...\"");
        println!("  export LANGSMITH_PROJECT=\"my-project\"  # Optional\n");
        return Ok(());
    }

    // Example 1: Create client from environment variables
    println!("Example 1: Creating Client from Environment");
    let client = Client::from_env()?;
    println!("Client created successfully!");
    println!("API URL: {}", client.api_url());
    if let Some(project) = client.project_name() {
        println!("Project: {}", project);
    }
    println!();

    // Example 2: Create a simple LLM run
    println!("Example 2: Creating a Simple LLM Run");
    let run_id = Uuid::new_v4();
    let run_create = RunCreate {
        id: run_id,
        name: "ChatOpenAI".to_string(),
        run_type: RunType::Llm,
        start_time: Utc::now(),
        parent_run_id: None,
        inputs: Some(serde_json::json!({
            "messages": [
                {"role": "user", "content": "What is the capital of France?"}
            ]
        })),
        serialized: None,
        tags: Some(vec!["example".to_string(), "llm".to_string()]),
        metadata: None,
        session_name: client.project_name().map(|s| s.to_string()),
        reference_example_id: None,
        extra: None,
    };

    client.create_run(&run_create).await?;
    println!("Created LLM run with ID: {}", run_id);
    println!();

    // Example 3: Update the run with output
    println!("Example 3: Updating Run with Output");
    let run_update = RunUpdate {
        end_time: Some(Utc::now()),
        outputs: Some(serde_json::json!({
            "content": "The capital of France is Paris."
        })),
        error: None,
        metadata: None,
        events: None,
        extra: None,
    };

    client.update_run(run_id, &run_update).await?;
    println!("Updated run {} with output", run_id);
    println!();

    // Example 4: Parent-child run hierarchy
    println!("Example 4: Creating Parent-Child Run Hierarchy");

    // Parent chain run
    let parent_id = Uuid::new_v4();
    let parent_run = RunCreate {
        id: parent_id,
        name: "QuestionAnswerChain".to_string(),
        run_type: RunType::Chain,
        start_time: Utc::now(),
        parent_run_id: None,
        inputs: Some(serde_json::json!({
            "question": "Explain quantum computing"
        })),
        serialized: None,
        tags: Some(vec!["chain".to_string(), "qa".to_string()]),
        metadata: None,
        session_name: client.project_name().map(|s| s.to_string()),
        reference_example_id: None,
        extra: None,
    };

    client.create_run(&parent_run).await?;
    println!("Created parent chain run: {}", parent_id);

    // Child retriever run
    let retriever_id = Uuid::new_v4();
    let retriever_run = RunCreate {
        id: retriever_id,
        name: "VectorStoreRetriever".to_string(),
        run_type: RunType::Retriever,
        start_time: Utc::now(),
        parent_run_id: Some(parent_id),
        inputs: Some(serde_json::json!({
            "query": "quantum computing"
        })),
        serialized: None,
        tags: Some(vec!["retrieval".to_string()]),
        metadata: None,
        session_name: client.project_name().map(|s| s.to_string()),
        reference_example_id: None,
        extra: None,
    };

    client.create_run(&retriever_run).await?;
    println!("Created child retriever run: {}", retriever_id);

    // Update retriever
    let retriever_update = RunUpdate {
        end_time: Some(Utc::now()),
        outputs: Some(serde_json::json!({
            "documents": [
                {"content": "Quantum computing uses qubits..."},
                {"content": "Superposition allows qubits..."}
            ]
        })),
        error: None,
        metadata: None,
        events: None,
        extra: None,
    };
    client.update_run(retriever_id, &retriever_update).await?;
    println!("Updated retriever run");

    // Child LLM run
    let llm_id = Uuid::new_v4();
    let llm_run = RunCreate {
        id: llm_id,
        name: "ChatOpenAI".to_string(),
        run_type: RunType::Llm,
        start_time: Utc::now(),
        parent_run_id: Some(parent_id),
        inputs: Some(serde_json::json!({
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "Explain quantum computing"}
            ]
        })),
        serialized: None,
        tags: Some(vec!["llm".to_string()]),
        metadata: None,
        session_name: client.project_name().map(|s| s.to_string()),
        reference_example_id: None,
        extra: None,
    };

    client.create_run(&llm_run).await?;
    println!("Created child LLM run: {}", llm_id);

    // Update LLM
    let llm_update = RunUpdate {
        end_time: Some(Utc::now()),
        outputs: Some(serde_json::json!({
            "content": "Quantum computing is a revolutionary approach..."
        })),
        error: None,
        metadata: None,
        events: None,
        extra: None,
    };
    client.update_run(llm_id, &llm_update).await?;
    println!("Updated LLM run");

    // Update parent
    let parent_update = RunUpdate {
        end_time: Some(Utc::now()),
        outputs: Some(serde_json::json!({
            "answer": "Quantum computing is a revolutionary approach..."
        })),
        error: None,
        metadata: None,
        events: None,
        extra: None,
    };
    client.update_run(parent_id, &parent_update).await?;
    println!("Updated parent chain run");
    println!();

    // Example 5: Run with metadata and custom tags
    println!("Example 5: Run with Metadata and Custom Tags");

    let mut metadata = HashMap::new();
    metadata.insert("user_id".to_string(), serde_json::json!("user123"));
    metadata.insert("session_id".to_string(), serde_json::json!("sess456"));
    metadata.insert("environment".to_string(), serde_json::json!("production"));

    let metadata_run_id = Uuid::new_v4();
    let metadata_run = RunCreate {
        id: metadata_run_id,
        name: "CustomTool".to_string(),
        run_type: RunType::Tool,
        start_time: Utc::now(),
        parent_run_id: None,
        inputs: Some(serde_json::json!({
            "action": "calculate",
            "params": {"a": 5, "b": 3}
        })),
        serialized: None,
        tags: Some(vec![
            "tool".to_string(),
            "calculator".to_string(),
            "production".to_string(),
        ]),
        metadata: Some(metadata),
        session_name: client.project_name().map(|s| s.to_string()),
        reference_example_id: None,
        extra: None,
    };

    client.create_run(&metadata_run).await?;
    println!("Created tool run with metadata: {}", metadata_run_id);

    let metadata_update = RunUpdate {
        end_time: Some(Utc::now()),
        outputs: Some(serde_json::json!({"result": 8})),
        error: None,
        metadata: None,
        events: None,
        extra: None,
    };
    client.update_run(metadata_run_id, &metadata_update).await?;
    println!("Updated tool run with output");
    println!();

    // Example 6: Error tracking
    println!("Example 6: Error Tracking");
    let error_run_id = Uuid::new_v4();
    let error_run = RunCreate {
        id: error_run_id,
        name: "FailingChain".to_string(),
        run_type: RunType::Chain,
        start_time: Utc::now(),
        parent_run_id: None,
        inputs: Some(serde_json::json!({
            "input": "trigger error"
        })),
        serialized: None,
        tags: Some(vec!["error-example".to_string()]),
        metadata: None,
        session_name: client.project_name().map(|s| s.to_string()),
        reference_example_id: None,
        extra: None,
    };

    client.create_run(&error_run).await?;
    println!("Created run that will error: {}", error_run_id);

    // Update with error
    let error_update = RunUpdate {
        end_time: Some(Utc::now()),
        outputs: None,
        error: Some("ValueError: Invalid input provided".to_string()),
        metadata: None,
        events: None,
        extra: None,
    };
    client.update_run(error_run_id, &error_update).await?;
    println!("Updated run with error message");
    println!();

    // Final summary
    println!("=== Summary ===");
    println!("Successfully demonstrated LangSmith client operations:");
    println!("- Created {} runs", 6);
    println!("- Updated runs with outputs and errors");
    println!("- Demonstrated parent-child hierarchies");
    println!("- Added tags and metadata");
    println!("\nView your traces at: https://smith.dashflow.com");

    if let Some(project) = client.project_name() {
        println!("Project: {}", project);
    }

    println!("\nExample complete!");

    Ok(())
}
