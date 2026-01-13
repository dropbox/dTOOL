//! DynamoDB checkpointing example
//!
//! Demonstrates how to use DynamoDB for persistent graph state storage.
//!
//! # Prerequisites
//!
//! 1. AWS credentials configured (via environment variables or ~/.aws/credentials)
//! 2. DynamoDB table created (see below)
//! 3. Or use LocalStack for local testing:
//!    ```bash
//!    docker run -d -p 4566:4566 localstack/localstack
//!    export AWS_ENDPOINT_URL=http://localhost:4566
//!    ```
//!
//! # Creating the DynamoDB Table
//!
//! ```bash
//! aws dynamodb create-table \
//!   --table-name dashflow-checkpoints \
//!   --attribute-definitions \
//!     AttributeName=thread_id,AttributeType=S \
//!     AttributeName=checkpoint_id,AttributeType=S \
//!   --key-schema \
//!     AttributeName=thread_id,KeyType=HASH \
//!     AttributeName=checkpoint_id,KeyType=RANGE \
//!   --billing-mode PAY_PER_REQUEST
//! ```
//!
//! For LocalStack:
//! ```bash
//! aws dynamodb create-table --endpoint-url http://localhost:4566 \
//!   --table-name dashflow-checkpoints \
//!   --attribute-definitions \
//!     AttributeName=thread_id,AttributeType=S \
//!     AttributeName=checkpoint_id,AttributeType=S \
//!   --key-schema \
//!     AttributeName=thread_id,KeyType=HASH \
//!     AttributeName=checkpoint_id,KeyType=RANGE \
//!   --billing-mode PAY_PER_REQUEST
//! ```
//!
//! # Running the Example
//!
//! ```bash
//! cargo run --example dynamodb_checkpointing
//! ```

use dashflow::{Checkpointer, StateGraph};
use dashflow_dynamodb_checkpointer::DynamoDBCheckpointer;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct AgentState {
    messages: Vec<String>,
    current_step: usize,
    total_steps: usize,
}

impl dashflow::MergeableState for AgentState {
    fn merge(&mut self, other: &Self) {
        // Merge messages from parallel branches
        self.messages.extend(other.messages.clone());
        // Take max for step counters
        self.current_step = self.current_step.max(other.current_step);
        self.total_steps = self.total_steps.max(other.total_steps);
    }
}

/// Researcher node: gather information
async fn researcher(mut state: AgentState) -> dashflow::Result<AgentState> {
    println!("\n[Researcher] Gathering information...");
    let message = "Research: Climate patterns analyzed".to_string();
    state.messages.push(message.clone());
    state.current_step += 1;
    println!(
        "  Step {}/{}: {}",
        state.current_step,
        state.total_steps,
        message
    );
    Ok(state)
}

/// Analyzer node: process information
async fn analyzer(mut state: AgentState) -> dashflow::Result<AgentState> {
    println!("\n[Analyzer] Processing information...");
    let message = "Analysis: 3 key trends identified".to_string();
    state.messages.push(message.clone());
    state.current_step += 1;
    println!(
        "  Step {}/{}: {}",
        state.current_step,
        state.total_steps,
        message
    );
    Ok(state)
}

/// Writer node: create summary
async fn writer(mut state: AgentState) -> dashflow::Result<AgentState> {
    println!("\n[Writer] Creating summary...");
    let message = "Summary: Report completed".to_string();
    state.messages.push(message.clone());
    state.current_step += 1;
    println!(
        "  Step {}/{}: {}",
        state.current_step,
        state.total_steps,
        message
    );
    Ok(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== DynamoDB Checkpointing Example ===\n");

    // Configure AWS SDK
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_dynamodb::Client::new(&config);

    // Create DynamoDB checkpointer
    let checkpointer = DynamoDBCheckpointer::new()
        .with_table_name("dashflow-checkpoints")
        .with_dynamodb_client(client);

    println!("✓ Connected to DynamoDB");
    println!("  Table: dashflow-checkpoints\n");

    // Build research workflow graph
    let mut graph = StateGraph::new();
    graph.add_node_from_fn("researcher", |state| Box::pin(researcher(state)));
    graph.add_node_from_fn("analyzer", |state| Box::pin(analyzer(state)));
    graph.add_node_from_fn("writer", |state| Box::pin(writer(state)));

    graph.add_edge("researcher", "analyzer");
    graph.add_edge("analyzer", "writer");
    graph.add_edge("writer", "__end__");

    graph.set_entry_point("researcher");

    // Compile graph with checkpointer
    let app = graph.compile()?.with_checkpointer(checkpointer);

    println!("✓ Graph compiled with 3 nodes: researcher → analyzer → writer\n");

    // Run workflow with checkpointing
    let thread_id = "research_workflow_1";

    println!("--- Running Workflow (Thread: {}) ---", thread_id);

    let initial_state = AgentState {
        messages: vec![],
        current_step: 0,
        total_steps: 3,
    };

    let execution_result = app.with_thread_id(thread_id).invoke(initial_state).await?;
    let result = execution_result.final_state;

    println!("\n=== Workflow Complete ===");
    println!("Total messages: {}", result.messages.len());
    for (i, msg) in result.messages.iter().enumerate() {
        println!("  {}. {}", i + 1, msg);
    }

    // Demonstrate checkpoint history
    println!("\n--- Checkpoint History ---");

    // Create a second checkpointer for querying (since first was moved to graph)
    let config2 = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client2 = aws_sdk_dynamodb::Client::new(&config2);
    let checkpointer2 = DynamoDBCheckpointer::<AgentState>::new()
        .with_table_name("dashflow-checkpoints")
        .with_dynamodb_client(client2);

    let checkpoints = checkpointer2.list(thread_id).await?;
    println!("Total checkpoints saved: {}", checkpoints.len());

    for (i, checkpoint) in checkpoints.iter().enumerate() {
        println!("\n{}. Checkpoint: {}", i + 1, checkpoint.id);
        println!("   Node: {}", checkpoint.node);
        println!("   Timestamp: {:?}", checkpoint.timestamp);
        if let Some(parent_id) = &checkpoint.parent_id {
            println!("   Parent: {}", parent_id);
        }
    }

    // Load latest checkpoint
    println!("\n--- Latest Checkpoint ---");
    if let Some(latest) = checkpointer2.get_latest(thread_id).await? {
        println!("Checkpoint ID: {}", latest.id);
        println!("Node: {}", latest.node);
        println!("State:");
        println!(
            "  Current step: {}/{}",
            latest.state.current_step, latest.state.total_steps
        );
        println!("  Messages: {}", latest.state.messages.len());
    }

    // Cleanup: Delete the thread's checkpoints
    println!("\n--- Cleanup ---");
    println!("Deleting checkpoints for thread: {}", thread_id);
    checkpointer2.delete_thread(thread_id).await?;
    println!("✓ Checkpoints deleted");

    // Verify deletion
    let remaining = checkpointer2.list(thread_id).await?;
    println!(
        "Remaining checkpoints for thread '{}': {}",
        thread_id,
        remaining.len()
    );

    println!("\n=== Example Complete ===");
    println!("\nKey Takeaways:");
    println!(
        "1. DynamoDB stores checkpoints with partition key (thread_id) + sort key (checkpoint_id)"
    );
    println!("2. Latest checkpoint retrieved in O(1) time using query operation");
    println!("3. Full checkpoint history maintained for debugging and time-travel");
    println!("4. Serverless scaling - no infrastructure management required");
    println!("5. Batch operations for efficient cleanup");

    Ok(())
}
