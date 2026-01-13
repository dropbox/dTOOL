//! Basic Remote Node Example
//!
//! This example demonstrates how to:
//! 1. Create a gRPC server that hosts a remote node
//! 2. Connect to the server from a client and execute the node
//! 3. Use RemoteNode in a DashFlow workflow
//!
//! Run with: cargo run --example basic_remote_node

use dashflow::{node::Node, StateGraph};
use dashflow_remote_node::{NodeRegistry, RemoteNode, RemoteNodeServer};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

/// Example state for the workflow
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ComputeState {
    value: i32,
    message: String,
}

impl dashflow::MergeableState for ComputeState {
    fn merge(&mut self, other: &Self) {
        // Take max value from parallel branches
        self.value = self.value.max(other.value);
        // Concatenate messages
        if !other.message.is_empty() {
            if !self.message.is_empty() {
                self.message.push('\n');
            }
            self.message.push_str(&other.message);
        }
    }
}

// GraphState is already implemented via blanket impl

/// A simple computation node that doubles the value
struct DoubleNode;

#[async_trait::async_trait]
impl Node<ComputeState> for DoubleNode {
    async fn execute(&self, mut state: ComputeState) -> dashflow::error::Result<ComputeState> {
        println!("[DoubleNode] Received value: {}", state.value);
        state.value *= 2;
        state.message = format!("Value doubled to {}", state.value);
        println!("[DoubleNode] Returning value: {}", state.value);
        Ok(state)
    }

    fn name(&self) -> String {
        "DoubleNode".to_string()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// A simple node that adds 10 to the value
struct AddTenNode;

#[async_trait::async_trait]
impl Node<ComputeState> for AddTenNode {
    async fn execute(&self, mut state: ComputeState) -> dashflow::error::Result<ComputeState> {
        println!("[AddTenNode] Received value: {}", state.value);
        state.value += 10;
        state.message = format!("Added 10, result: {}", state.value);
        println!("[AddTenNode] Returning value: {}", state.value);
        Ok(state)
    }

    fn name(&self) -> String {
        "AddTenNode".to_string()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Start the gRPC server in the background
async fn start_server() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async {
        println!("Starting gRPC server on 127.0.0.1:50051...");

        // Create node registry
        let mut registry = NodeRegistry::new();

        // Register nodes that can be executed remotely
        registry.register("double", DoubleNode);
        registry.register("add_ten", AddTenNode);

        println!("Registered nodes: double, add_ten");

        // Create and start server
        let server = RemoteNodeServer::new(registry);
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 50051));

        println!("Server listening on {}", addr);

        if let Err(e) = server.serve(addr).await {
            eprintln!("Server error: {}", e);
        }
    })
}

/// Run a simple workflow with remote nodes
async fn run_workflow_example() -> dashflow::error::Result<()> {
    println!("\n=== Running workflow with remote nodes ===\n");

    // Create initial state
    let initial_state = ComputeState {
        value: 5,
        message: "Initial value".to_string(),
    };

    println!("Initial state: value={}", initial_state.value);

    // Create remote nodes
    // Uses default retry policy (exponential backoff with jitter, 3 retries)
    let remote_double = RemoteNode::<ComputeState>::new("double")
        .with_endpoint("http://127.0.0.1:50051")
        .with_timeout(Duration::from_secs(5));

    let remote_add_ten = RemoteNode::<ComputeState>::new("add_ten")
        .with_endpoint("http://127.0.0.1:50051")
        .with_timeout(Duration::from_secs(5));

    // Build graph
    let mut graph = StateGraph::new();
    graph.add_node("double", remote_double);
    graph.add_node("add_ten", remote_add_ten);
    graph.add_edge("double", "add_ten");
    graph.add_edge("add_ten", dashflow::edge::END);
    graph.set_entry_point("double");

    // Compile and execute
    println!("\nCompiling graph...");
    let app = graph.compile()?;

    println!("Executing graph...\n");
    let result = app.invoke(initial_state).await?;

    println!("\n=== Workflow completed ===");
    println!("Final value: {}", result.final_state.value);
    println!("Final message: {}", result.final_state.message);
    println!("Expected: 5 * 2 + 10 = {}", 5 * 2 + 10);

    Ok(())
}

/// Direct node execution (bypass graph)
async fn direct_execution_example() -> dashflow::error::Result<()> {
    println!("\n=== Direct remote node execution ===\n");

    let initial_state = ComputeState {
        value: 7,
        message: "Direct execution".to_string(),
    };

    println!("Initial state: value={}", initial_state.value);

    // Create remote node
    let remote_double = RemoteNode::<ComputeState>::new("double")
        .with_endpoint("http://127.0.0.1:50051")
        .with_timeout(Duration::from_secs(5));

    // Execute directly
    println!("Executing node...");
    let result = remote_double.execute(initial_state).await?;

    println!("\n=== Execution completed ===");
    println!("Final value: {}", result.value);
    println!("Final message: {}", result.message);
    println!("Expected: 7 * 2 = {}", 7 * 2);

    Ok(())
}

#[tokio::main]
async fn main() -> dashflow::error::Result<()> {
    // Start server in background
    let _server_handle = start_server().await;

    // Wait for server to start
    println!("Waiting for server to start...\n");
    sleep(Duration::from_millis(500)).await;

    // Run examples
    direct_execution_example().await?;
    run_workflow_example().await?;

    println!("\n=== All examples completed successfully ===\n");

    // Keep server running briefly
    sleep(Duration::from_secs(1)).await;

    Ok(())
}
