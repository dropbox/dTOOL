//! State Diff Example
//!
//! This example demonstrates how to use the state diffing functionality
//! to efficiently track state changes in DashFlow execution.
//!
//! Run with: cargo run --example state_diff

use dashflow_streaming::diff::protobuf::{apply_state_diff, create_state_diff};
use dashflow_streaming::diff::{apply_patch, diff_states};
use dashflow_streaming::{Header, MessageType};
use anyhow::{Context, Result};
use serde_json::json;

fn main() -> Result<()> {
    println!("=== DashStream State Diff Example ===\n");

    // Example 1: Basic state diff
    example_basic_diff()?;

    println!("\n{}\n", "=".repeat(80));

    // Example 2: Message thread state evolution
    example_message_thread()?;

    println!("\n{}\n", "=".repeat(80));

    // Example 3: Large state optimization
    example_large_state()?;

    println!("\n{}\n", "=".repeat(80));

    // Example 4: Protobuf integration
    example_protobuf_integration()?;

    Ok(())
}

fn example_basic_diff() -> Result<()> {
    println!("Example 1: Basic State Diff\n");

    let old_state = json!({
        "counter": 5,
        "user": "Alice"
    });

    let new_state = json!({
        "counter": 6,
        "user": "Alice"
    });

    println!(
        "Old state: {}",
        serde_json::to_string_pretty(&old_state)?
    );
    println!(
        "New state: {}",
        serde_json::to_string_pretty(&new_state)?
    );

    let result = diff_states(&old_state, &new_state)?;
    println!("\nDiff result: {}", result);
    println!("Number of operations: {}", result.patch.0.len());
    println!(
        "Patch JSON: {}",
        serde_json::to_string_pretty(&result.patch)?
    );

    // Apply patch to verify
    let reconstructed = apply_patch(&old_state, &result.patch)?;
    println!(
        "\nReconstructed state matches: {}",
        reconstructed == new_state
    );
    Ok(())
}

fn example_message_thread() -> Result<()> {
    println!("Example 2: Message Thread Evolution\n");

    let mut state = json!({
        "messages": [],
        "counter": 0,
        "metadata": {
            "user": "Bob",
            "session_id": "abc123"
        }
    });

    println!("Initial state:");
    println!("{}\n", serde_json::to_string_pretty(&state)?);

    // Simulate adding messages to a conversation
    let messages = [
        "Hello, how can I help you?",
        "I need information about Rust",
        "Rust is a systems programming language...",
        "Thanks for the information!",
    ];

    for (i, message) in messages.iter().enumerate() {
        let old_state = state.clone();

        // Update state
        state["messages"]
            .as_array_mut()
            .context("expected state.messages to be an array")?
            .push(json!(message));
        state["counter"] = json!(i + 1);

        // Compute diff
        let result = diff_states(&old_state, &state)?;

        println!("Step {}: Added message", i + 1);
        println!("  Message: \"{}\"", message);
        println!("  Diff: {}", result);
        println!(
            "  Savings: {:.1}%\n",
            (1.0 - (result.patch_size as f64 / result.full_state_size as f64)) * 100.0
        );
    }

    println!("Final state:");
    println!("{}", serde_json::to_string_pretty(&state)?);
    Ok(())
}

fn example_large_state() -> Result<()> {
    println!("Example 3: Large State Optimization\n");

    // Create a large state with many messages
    let old_state = json!({
        "messages": (0..50).map(|i| format!("Message {}", i)).collect::<Vec<_>>(),
        "counter": 50,
        "metadata": {
            "user": "Charlie",
            "attributes": (0..20).map(|i| (format!("attr{}", i), json!(i))).collect::<serde_json::Map<_, _>>()
        }
    });

    // Small change: add one message
    let mut new_state = old_state.clone();
    new_state["messages"]
        .as_array_mut()
        .context("expected old_state.messages to be an array")?
        .push(json!("Message 50"));
    new_state["counter"] = json!(51);

    let result = diff_states(&old_state, &new_state)?;

    println!("Large state diff:");
    println!("  Old state size: {} bytes", result.full_state_size);
    println!("  Patch size: {} bytes", result.patch_size);
    println!(
        "  Compression ratio: {:.2}x",
        result.full_state_size as f64 / result.patch_size as f64
    );
    println!(
        "  Using: {}",
        if result.use_full_state {
            "full state"
        } else {
            "patch"
        }
    );
    println!(
        "  Savings: {:.1}%",
        (1.0 - (result.patch_size as f64 / result.full_state_size as f64)) * 100.0
    );

    // Test the opposite: completely different small state
    let tiny_new_state = json!({"x": "tiny"});
    let result2 = diff_states(&old_state, &tiny_new_state)?;

    println!("\nComplete state replacement:");
    println!("  Old state size: {} bytes", result2.full_state_size);
    println!("  Patch size: {} bytes", result2.patch_size);
    println!(
        "  Using: {}",
        if result2.use_full_state {
            "full state"
        } else {
            "patch"
        }
    );
    println!("  (Optimization kicks in: patch would be larger than full state)");
    Ok(())
}

fn example_protobuf_integration() -> Result<()> {
    println!("Example 4: Protobuf Integration\n");

    let old_state = json!({
        "messages": ["Hello"],
        "counter": 1
    });

    let new_state = json!({
        "messages": ["Hello", "World"],
        "counter": 2
    });

    // Create a protobuf Header
    let header = Header {
        message_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
        timestamp_us: chrono::Utc::now().timestamp_micros(),
        tenant_id: "example-tenant".to_string(),
        thread_id: "thread-123".to_string(),
        sequence: 1,
        r#type: MessageType::StateDiff as i32,
        parent_id: vec![],
        compression: 0,
        schema_version: 1,
    };

    // Create StateDiff message
    let state_diff = create_state_diff(
        header,
        vec![], // No base checkpoint
        &old_state,
        &new_state,
    )
    ?;

    println!("Created StateDiff protobuf message:");
    let header = state_diff
        .header
        .as_ref()
        .context("expected StateDiff header to be present")?;
    println!("  Thread ID: {}", header.thread_id);
    println!("  Sequence: {}", header.sequence);
    println!("  Operations: {}", state_diff.operations.len());
    println!("  State hash: {}", hex::encode(&state_diff.state_hash));
    println!("  Using full state: {}", !state_diff.full_state.is_empty());

    // Apply the diff
    let reconstructed = apply_state_diff(&old_state, &state_diff)?;
    println!("\nApplied diff successfully!");
    println!(
        "  Reconstructed matches new state: {}",
        reconstructed == new_state
    );

    // Show the reconstructed state
    println!("\nReconstructed state:");
    println!("{}", serde_json::to_string_pretty(&reconstructed)?);
    Ok(())
}
