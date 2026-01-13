//! RemoteRunnable Client Example
//!
//! This example demonstrates how to use the RemoteRunnable client to call
//! a remote LangServe server.
//!
//! # Prerequisites
//!
//! Start the basic_skeleton server first:
//! ```bash
//! cargo run --example basic_skeleton
//! ```
//!
//! Then run this client:
//! ```bash
//! cargo run --example client_example
//! ```

use dashflow_langserve::RemoteRunnable;
use futures::StreamExt;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== RemoteRunnable Client Example ===\n");

    // Create a RemoteRunnable client pointing to our local server
    let remote = RemoteRunnable::new("http://localhost:8000/dummy")?;
    println!("Created RemoteRunnable client for http://localhost:8000/dummy\n");

    // Example 1: Single invoke
    println!("--- Example 1: Single Invoke ---");
    let input = json!({
        "text": "Hello from RemoteRunnable client!"
    });
    println!("Sending: {}", serde_json::to_string_pretty(&input)?);

    match remote.invoke(input.clone(), None).await {
        Ok(output) => {
            println!("Received: {}", serde_json::to_string_pretty(&output)?);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("\nMake sure the server is running:");
            eprintln!("  cargo run --example basic_skeleton");
            return Ok(());
        }
    }
    println!();

    // Example 2: Batch invoke
    println!("--- Example 2: Batch Invoke ---");
    let batch_inputs = vec![
        json!({"text": "First input"}),
        json!({"text": "Second input"}),
        json!({"text": "Third input"}),
    ];
    println!("Sending batch of {} inputs", batch_inputs.len());

    match remote.batch(batch_inputs, None).await {
        Ok(outputs) => {
            println!("Received {} outputs:", outputs.len());
            for (i, output) in outputs.iter().enumerate() {
                println!(
                    "  Output {}: {}",
                    i + 1,
                    serde_json::to_string_pretty(output)?
                );
            }
        }
        Err(e) => {
            eprintln!("Batch error: {}", e);
        }
    }
    println!();

    // Example 3: Streaming
    println!("--- Example 3: Streaming ---");
    println!(
        "Streaming response for input: {}",
        serde_json::to_string_pretty(&input)?
    );

    match remote.stream(input, None).await {
        Ok(mut stream) => {
            println!("Streaming chunks:");
            let mut chunk_count = 0;
            while let Some(result) = stream.next().await {
                match result {
                    Ok(chunk) => {
                        chunk_count += 1;
                        println!(
                            "  Chunk {}: {}",
                            chunk_count,
                            serde_json::to_string_pretty(&chunk)?
                        );
                    }
                    Err(e) => {
                        eprintln!("  Stream error: {}", e);
                        break;
                    }
                }
            }
            println!("Stream complete ({} chunks received)", chunk_count);
        }
        Err(e) => {
            eprintln!("Stream error: {}", e);
        }
    }
    println!();

    // Example 4: Schema endpoints
    println!("--- Example 4: Schema Endpoints ---");

    match remote.input_schema().await {
        Ok(schema) => {
            println!("Input Schema:");
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
        Err(e) => {
            eprintln!("Error fetching input schema: {}", e);
        }
    }
    println!();

    match remote.output_schema().await {
        Ok(schema) => {
            println!("Output Schema:");
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
        Err(e) => {
            eprintln!("Error fetching output schema: {}", e);
        }
    }
    println!();

    match remote.config_schema().await {
        Ok(schema) => {
            println!("Config Schema:");
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
        Err(e) => {
            eprintln!("Error fetching config schema: {}", e);
        }
    }
    println!();

    // Example 5: With timeout
    println!("--- Example 5: RemoteRunnable with Timeout ---");
    let remote_with_timeout = RemoteRunnable::with_timeout("http://localhost:8000/dummy", 30)?;
    println!("Created RemoteRunnable with 30-second timeout");

    let result = remote_with_timeout
        .invoke(json!({"text": "Request with timeout"}), None)
        .await;

    match result {
        Ok(output) => {
            println!("Received: {}", serde_json::to_string_pretty(&output)?);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
    println!();

    println!("=== Client Example Complete ===");

    Ok(())
}
