//! Example demonstrating S3 checkpointing with DashFlow
//!
//! This example shows how to use the S3 checkpointer to persist graph state
//! in Amazon S3 or S3-compatible storage.
//!
//! # Prerequisites
//!
//! 1. AWS credentials configured (via environment variables or ~/.aws/credentials)
//! 2. An S3 bucket created
//! 3. IAM permissions for s3:PutObject, s3:GetObject, s3:DeleteObject
//!
//! # Running
//!
//! ```bash
//! export AWS_PROFILE=your-profile
//! export S3_BUCKET=your-bucket-name
//! cargo run --example s3_checkpointing
//! ```

use dashflow::StateGraph;
use dashflow_s3_checkpointer::S3Checkpointer;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AgentState {
    messages: Vec<String>,
    counter: i32,
    user_input: String,
}

impl dashflow::MergeableState for AgentState {
    fn merge(&mut self, other: &Self) {
        // Merge messages from parallel branches
        self.messages.extend(other.messages.clone());
        // Take max counter
        self.counter = self.counter.max(other.counter);
    }
}

async fn step1(state: AgentState) -> dashflow::Result<AgentState> {
    let mut new_state = state;
    println!(
        "Step 1: Processing user input: {}",
        new_state.user_input
    );
    new_state
        .messages
        .push(format!("Received: {}", new_state.user_input));
    new_state.counter += 1;
    Ok(new_state)
}

async fn step2(state: AgentState) -> dashflow::Result<AgentState> {
    println!("Step 2: Analyzing input...");
    let mut new_state = state;
    new_state.messages.push("Analysis complete".to_string());
    new_state.counter += 1;
    Ok(new_state)
}

async fn step3(state: AgentState) -> dashflow::Result<AgentState> {
    let mut new_state = state;
    println!("Step 3: Generating response...");
    new_state.messages.push(format!(
        "Response: Processed '{}' successfully",
        new_state.user_input
    ));
    new_state.counter += 1;
    Ok(new_state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get S3 bucket from environment or use default
    let bucket = env::var("S3_BUCKET").unwrap_or_else(|_| "dashflow-checkpoints".to_string());

    println!("=== S3 Checkpointing Example ===");
    println!("Bucket: {}", bucket);
    println!();

    // Create S3 checkpointer
    println!("Initializing S3 checkpointer...");
    let checkpointer = S3Checkpointer::with_prefix(&bucket, "examples/agent").await?;
    println!("✓ S3 checkpointer ready");
    println!();

    // Build the graph
    println!("Building graph...");
    let mut graph = StateGraph::new();

    graph.add_node_from_fn("step1", |state| Box::pin(step1(state)));
    graph.add_node_from_fn("step2", |state| Box::pin(step2(state)));
    graph.add_node_from_fn("step3", |state| Box::pin(step3(state)));

    graph.add_edge("step1", "step2");
    graph.add_edge("step2", "step3");
    graph.add_edge("step3", "__end__");

    graph.set_entry_point("step1");

    let app = graph.compile()?.with_checkpointer(checkpointer);
    println!("✓ Graph compiled with S3 checkpointer");
    println!();

    // Run the graph
    println!("Running graph...");
    let initial_state = AgentState {
        messages: vec![],
        counter: 0,
        user_input: "Hello, DashFlow!".to_string(),
    };

    let thread_id = "example-thread-1";
    let execution_result = app
        .with_thread_id(thread_id)
        .invoke(initial_state.clone())
        .await?;
    let result = execution_result.final_state;

    println!();
    println!("=== Final State ===");
    println!("Counter: {}", result.counter);
    println!("Messages:");
    for (i, msg) in result.messages.iter().enumerate() {
        println!("  {}. {}", i + 1, msg);
    }
    println!();

    println!("=== Example Complete ===");
    println!("Checkpoints were successfully saved to S3!");
    println!(
        "Check your S3 bucket '{}' under 'examples/agent/' prefix",
        bucket
    );
    println!();
    println!("Note: To clean up test data, use AWS CLI:");
    println!("  aws s3 rm s3://{}/examples/agent/ --recursive", bucket);

    Ok(())
}
