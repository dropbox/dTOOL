//! DashFlow Streaming Integration Example
//!
//! This example demonstrates how to use the DashStream callback to stream
//! graph execution telemetry to Kafka.
//!
//! # Prerequisites
//!
//! Start Kafka using Docker Compose:
//! ```bash
//! docker-compose -f docker-compose-kafka.yml up -d
//! ```
//!
//! # Running
//!
//! ```bash
//! cargo run --example dashstream_integration --features dashstream
//! ```
//!
//! # What it demonstrates
//!
//! - Creating a DashStream callback with custom configuration
//! - Integrating the callback with a StateGraph
//! - Streaming graph execution events to Kafka
//! - Automatic state diffing for incremental updates
//! - Multi-node graph execution with telemetry
//!
//! # Monitoring
//!
//! You can monitor the Kafka topic to see the events:
//! ```bash
//! docker-compose -f docker-compose-kafka.yml exec kafka \
//!     kafka-console-consumer --bootstrap-server localhost:9092 \
//!     --topic dashstream-demo --from-beginning
//! ```

use dashflow::schema::{NodeMetadata, NodeType};
use dashflow::{DashStreamCallback, DashStreamConfig, Error, MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct AgentState {
    messages: Vec<String>,
    step_count: i32,
    current_value: i32,
}

impl MergeableState for AgentState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.clone());
        self.step_count = self.step_count.max(other.step_count);
        self.current_value = self.current_value.max(other.current_value);
    }
}

async fn analyze_node(mut state: AgentState) -> Result<AgentState, Error> {
    println!("📊 Analyzing: current value = {}", state.current_value);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    state.step_count += 1;
    state.messages.push(format!(
        "Analysis complete for value {}",
        state.current_value
    ));
    state.current_value *= 2;
    Ok(state)
}

async fn process_node(mut state: AgentState) -> Result<AgentState, Error> {
    println!("⚙️ Processing: current value = {}", state.current_value);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    state.step_count += 1;
    state.messages.push(format!(
        "Processed value {} -> {}",
        state.current_value,
        state.current_value + 10
    ));
    state.current_value += 10;
    Ok(state)
}

async fn finalize_node(mut state: AgentState) -> Result<AgentState, Error> {
    println!("✅ Finalizing: current value = {}", state.current_value);
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    state.step_count += 1;
    state.messages.push(format!(
        "Finalization complete with result: {}",
        state.current_value
    ));
    Ok(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 DashFlow Streaming Integration Example");
    println!("==================================\n");

    // Create DashStream configuration
    // Use dashstream-quality topic to match the WebSocket server subscription
    let config = DashStreamConfig {
        bootstrap_servers: std::env::var("KAFKA_BROKERS")
            .unwrap_or_else(|_| "localhost:9092".to_string()),
        topic: std::env::var("DASHSTREAM_TOPIC")
            .unwrap_or_else(|_| "dashstream-quality".to_string()),
        tenant_id: "demo-tenant".to_string(),
        thread_id: format!("demo-session-{}", uuid::Uuid::new_v4()),
        enable_state_diff: true,
        compression_threshold: 512,
        max_state_diff_size: 10 * 1024 * 1024, // 10MB limit for state diffs
        ..Default::default()
    };

    println!("📡 Connecting to Kafka at {}", config.bootstrap_servers);
    println!("📬 Topic: {}", config.topic);
    println!("🆔 Thread ID: {}", config.thread_id);
    println!();

    // Create DashStream callback
    let callback = DashStreamCallback::<AgentState>::with_config(config.clone()).await?;

    println!("✅ DashStream callback created successfully\n");

    // Clone callback to keep a reference for flushing
    let callback_for_flush = callback.clone();

    // Build the graph with metadata for visualization
    println!("🔧 Building StateGraph...");
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Set graph description for visualization
    graph.with_description("Demo pipeline that analyzes, processes, and finalizes values");

    // Add nodes with metadata for dashboard visualization
    graph.add_node_with_metadata(
        "analyze",
        NodeMetadata::new("Analyzes the current value and doubles it")
            .with_node_type(NodeType::Transform)
            .with_input_fields(vec!["current_value".to_string()])
            .with_output_fields(vec!["current_value".to_string(), "messages".to_string()]),
        |state: AgentState| Box::pin(analyze_node(state)),
    );

    graph.add_node_with_metadata(
        "process",
        NodeMetadata::new("Processes the value by adding 10")
            .with_node_type(NodeType::Transform)
            .with_input_fields(vec!["current_value".to_string()])
            .with_output_fields(vec!["current_value".to_string(), "messages".to_string()]),
        |state: AgentState| Box::pin(process_node(state)),
    );

    graph.add_node_with_metadata(
        "finalize",
        NodeMetadata::new("Finalizes the computation and generates summary")
            .with_node_type(NodeType::Transform)
            .with_input_fields(vec!["current_value".to_string()])
            .with_output_fields(vec!["messages".to_string()]),
        |state: AgentState| Box::pin(finalize_node(state)),
    );

    graph.add_edge("analyze", "process");
    graph.add_edge("process", "finalize");
    graph.add_edge("finalize", END);
    graph.set_entry_point("analyze");

    // Export schema for verification
    let schema = graph.export_schema("demo-pipeline");
    println!(
        "📊 Graph Schema: {} nodes, {} edges",
        schema.nodes.len(),
        schema.edges.len()
    );
    println!("✅ Graph built with 3 nodes: analyze -> process -> finalize\n");

    // Compile with callback and name for telemetry
    let compiled = graph
        .compile()?
        .with_name("demo-pipeline")
        .with_callback(callback);

    println!("✅ Graph compiled with DashStream callback\n");

    // Execute graph
    println!("▶️  Starting graph execution...\n");

    let initial_state = AgentState {
        messages: vec![],
        step_count: 0,
        current_value: 5,
    };

    let result = compiled
        .invoke(initial_state)
        .await
        ?
        .final_state;

    println!("\n📊 Execution Complete!");
    println!("======================");
    println!("Steps executed: {}", result.step_count);
    println!("Final value: {}", result.current_value);
    println!("\n📝 Messages:");
    for (i, msg) in result.messages.iter().enumerate() {
        println!("  {}. {}", i + 1, msg);
    }

    // Flush Kafka messages
    println!("\n🔄 Flushing DashStream messages to Kafka...");
    callback_for_flush
        .flush()
        .await
        ?;

    println!("✅ All messages flushed successfully!\n");

    println!("📊 Telemetry Data:");
    println!("==================");
    println!("- Graph execution events sent to Kafka");
    println!("- State diffs captured for each node");
    println!("- All events available in topic: {}", config.topic);
    println!("\n💡 Tip: Use Kafka consumer to view the telemetry:");
    println!("   docker-compose -f docker-compose-kafka.yml exec kafka \\");
    println!("       kafka-console-consumer --bootstrap-server localhost:9092 \\");
    println!("       --topic {} --from-beginning", config.topic);

    Ok(())
}
