//! S3 Checkpointing Example
//!
//! Demonstrates using S3 as a checkpointer for DashFlow state persistence.
//! This example shows:
//! - Creating an S3 checkpointer with AWS credentials
//! - Building a simple stateful graph
//! - Saving and resuming execution from S3 checkpoints
//! - Using compression to reduce storage costs
//! - Applying retention policies for automatic cleanup
//!
//! # Prerequisites
//!
//! 1. AWS credentials configured (one of):
//!    - Environment variables: AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_REGION
//!    - AWS CLI profile: AWS_PROFILE=your-profile
//!    - IAM role (if running on EC2/Lambda)
//!
//! 2. S3 bucket created and accessible
//!
//! # Running
//!
//! ```bash
//! # Set environment variables
//! export AWS_REGION=us-east-1
//! export TEST_S3_BUCKET=my-dashflow-checkpoints
//!
//! # Run the example
//! cargo run --example s3_checkpointing --features dashflow-s3-checkpointer/compression
//! ```
//!
//! # S3 Storage Structure
//!
//! Checkpoints are stored with this key structure:
//! ```text
//! s3://bucket-name/
//!   dashflow/
//!     checkpoints/
//!       {checkpoint-id-1}  # Bincode-encoded checkpoint
//!       {checkpoint-id-2}
//!     threads/
//!       {thread-id}/
//!         index.json  # Metadata for efficient listing
//! ```

use dashflow::{
    Checkpoint, Checkpointer, StateGraph, GraphState, RetentionPolicy,
};
use dashflow_s3_checkpointer::S3Checkpointer;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Agent state for multi-step conversation
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct ConversationState {
    messages: Vec<String>,
    user_name: Option<String>,
    conversation_count: usize,
}

impl GraphState for ConversationState {
    fn merge(&mut self, other: Self) {
        // Append new messages
        self.messages.extend(other.messages);
        // Update user name if provided
        if other.user_name.is_some() {
            self.user_name = other.user_name;
        }
        // Update count
        self.conversation_count = other.conversation_count;
    }
}

/// Greeter node: Asks for user's name
fn greeter_node(state: ConversationState) -> ConversationState {
    let mut new_state = state.clone();
    new_state.messages.push("Hi! What's your name?".to_string());
    new_state
}

/// Name processor node: Saves the user's name
fn name_processor_node(mut state: ConversationState) -> ConversationState {
    // Simulate extracting name from last message
    if let Some(last_msg) = state.messages.last() {
        if last_msg.starts_with("My name is ") {
            state.user_name = Some(last_msg.replace("My name is ", ""));
            state.messages.push(format!(
                "Nice to meet you, {}!",
                state.user_name.as_ref().unwrap()
            ));
        }
    }
    state.conversation_count += 1;
    state
}

/// Conversation node: Continues the conversation
fn conversation_node(mut state: ConversationState) -> ConversationState {
    if let Some(name) = &state.user_name {
        state.messages.push(format!(
            "How can I help you today, {}?",
            name
        ));
    } else {
        state.messages.push("What can I help you with?".to_string());
    }
    state.conversation_count += 1;
    state
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== S3 Checkpointing Example ===\n");

    // 1. Configure S3 bucket
    let bucket = std::env::var("TEST_S3_BUCKET")
        .unwrap_or_else(|_| {
            eprintln!("ERROR: TEST_S3_BUCKET environment variable not set");
            eprintln!("Usage: export TEST_S3_BUCKET=my-bucket-name");
            std::process::exit(1);
        });

    println!("Using S3 bucket: {}", bucket);

    // 2. Create S3 checkpointer with compression and retention policy
    println!("\n[1] Creating S3 checkpointer with compression...");

    #[cfg(feature = "compression")]
    let checkpointer = {
        use dashflow_compression::CompressionType;
        S3Checkpointer::<ConversationState>::with_prefix(&bucket, "dashflow/examples")
            .await?
            .with_compression(CompressionType::Zstd(3))?
            .with_retention_policy(RetentionPolicy::builder()
                .keep_last_n(10)  // Keep last 10 checkpoints always
                .keep_daily_for(Duration::from_secs(30 * 86400))  // Keep daily for 30 days
                .delete_after(Duration::from_secs(90 * 86400))  // Delete after 90 days
                .build())
    };

    #[cfg(not(feature = "compression"))]
    let checkpointer = {
        S3Checkpointer::<ConversationState>::with_prefix(&bucket, "dashflow/examples")
            .await?
            .with_retention_policy(RetentionPolicy::builder()
                .keep_last_n(10)
                .keep_daily_for(Duration::from_secs(30 * 86400))
                .delete_after(Duration::from_secs(90 * 86400))
                .build())
    };

    println!("✓ S3 checkpointer created");
    #[cfg(feature = "compression")]
    println!("  - Compression: Zstd level 3 (5-10× reduction)");
    println!("  - Retention: Keep last 10, daily for 30d, max 90d");

    // 3. Build the conversation graph
    println!("\n[2] Building conversation graph...");
    let mut graph = StateGraph::new();

    // This would normally add nodes and edges
    // For this example, we'll just demonstrate checkpoint operations
    println!("✓ Graph built (simplified for example)");

    // 4. Demonstrate checkpoint save/load
    println!("\n[3] Demonstrating checkpoint operations...");

    let thread_id = "user-session-12345".to_string();

    // Create initial state
    let initial_state = ConversationState {
        messages: vec!["Hello!".to_string()],
        user_name: None,
        conversation_count: 0,
    };

    // Save first checkpoint
    let checkpoint1 = Checkpoint::new(
        thread_id.clone(),
        initial_state.clone(),
        "greeter".to_string(),
        None,
    );
    let checkpoint1_id = checkpoint1.id.clone();

    println!("  Saving checkpoint 1 (greeter node)...");
    checkpointer.save(checkpoint1).await?;
    println!("  ✓ Saved: {}", checkpoint1_id);

    // Small delay to ensure different timestamps
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Save second checkpoint (user provided name)
    let mut state_after_name = initial_state.clone();
    state_after_name.messages.push("My name is Alice".to_string());
    state_after_name.user_name = Some("Alice".to_string());
    state_after_name.conversation_count = 1;

    let checkpoint2 = Checkpoint::new(
        thread_id.clone(),
        state_after_name.clone(),
        "name_processor".to_string(),
        Some(checkpoint1_id.clone()),
    );
    let checkpoint2_id = checkpoint2.id.clone();

    println!("  Saving checkpoint 2 (after name)...");
    checkpointer.save(checkpoint2).await?;
    println!("  ✓ Saved: {}", checkpoint2_id);

    // 5. Load checkpoints
    println!("\n[4] Loading checkpoints from S3...");

    // Load specific checkpoint
    let loaded = checkpointer.load(&checkpoint1_id).await?;
    if let Some(cp) = loaded {
        println!("  ✓ Loaded checkpoint 1:");
        println!("    - Node: {}", cp.node);
        println!("    - Messages: {:?}", cp.state.messages);
        println!("    - User: {:?}", cp.state.user_name);
    }

    // Get latest checkpoint
    let latest = checkpointer.get_latest(&thread_id).await?;
    if let Some(cp) = latest {
        println!("  ✓ Latest checkpoint:");
        println!("    - Node: {}", cp.node);
        println!("    - Messages: {:?}", cp.state.messages);
        println!("    - User: {:?}", cp.state.user_name);
        println!("    - Count: {}", cp.state.conversation_count);
    }

    // 6. List all checkpoints for thread
    println!("\n[5] Listing all checkpoints for thread...");
    let checkpoints = checkpointer.list(&thread_id).await?;
    println!("  Found {} checkpoints:", checkpoints.len());
    for (i, meta) in checkpoints.iter().enumerate() {
        println!("  {}. ID: {}", i + 1, &meta.id[..8]);
        println!("     Node: {}", meta.node);
        println!("     Time: {:?}", meta.timestamp);
    }

    // 7. Apply retention policy
    println!("\n[6] Applying retention policy...");
    let deleted_count = checkpointer.apply_retention(&thread_id).await?;
    println!("  ✓ Deleted {} old checkpoints", deleted_count);

    // 8. Resume execution from checkpoint
    println!("\n[7] Resuming execution from checkpoint...");
    if let Some(checkpoint) = checkpointer.get_latest(&thread_id).await? {
        println!("  Resumed from node: {}", checkpoint.node);
        println!("  State: {:?}", checkpoint.state.messages);
        println!("  ✓ Ready to continue execution");
    }

    // 9. Cleanup: Delete all checkpoints for this thread
    println!("\n[8] Cleanup: Deleting all checkpoints...");
    checkpointer.delete_thread(&thread_id).await?;
    println!("  ✓ All checkpoints deleted");

    // Verify deletion
    let remaining = checkpointer.list(&thread_id).await?;
    println!("  Remaining checkpoints: {}", remaining.len());

    println!("\n=== Example Complete ===");
    println!("\n💡 Key Takeaways:");
    println!("  - S3 checkpointing enables serverless/Lambda deployments");
    println!("  - Compression reduces storage costs by 5-10×");
    println!("  - Retention policies prevent unbounded growth");
    println!("  - Thread index avoids expensive ListObjects calls");
    println!("  - Resume execution from any checkpoint for debugging");

    Ok(())
}
