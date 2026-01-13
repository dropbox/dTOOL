//! Example demonstrating OpenTelemetry tracing integration with DashFlow
//!
//! This example shows how to enable distributed tracing for graph execution,
//! exporting spans to an OTLP endpoint (Jaeger, Zipkin, etc.).
//!
//! # Prerequisites
//!
//! Run Jaeger locally for viewing traces:
//! ```bash
//! docker run -d --name jaeger \
//!   -p 16686:16686 \
//!   -p 4317:4317 \
//!   jaegertracing/all-in-one:latest
//! ```
//!
//! Then view traces at: http://localhost:16686
//!
//! # Running
//!
//! ```bash
//! # Tracing only (Jaeger)
//! cargo run -p dashflow --example traced_agent --features observability
//!
//! # Tracing + live observability UI (Kafka → websocket-server → observability-ui)
//! cargo run -p dashflow --example traced_agent --features observability,dashstream
//! ```

use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};

#[cfg(feature = "dashstream")]
use dashflow::{DashStreamCallback, DashStreamConfig};

#[cfg(feature = "observability")]
use dashflow_observability::{init_tracing, TracingConfig};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AgentState {
    messages: Vec<String>,
    step_count: usize,
}

impl MergeableState for AgentState {
    fn merge(&mut self, other: &Self) {
        self.messages.extend(other.messages.clone());
        self.step_count = self.step_count.max(other.step_count);
    }
}

/// Research node - simulates fetching information
async fn research_node(mut state: AgentState) -> dashflow::Result<AgentState> {
    tracing::info!("Researching topic...");

    // Simulate some work
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    state
        .messages
        .push("Research complete: Found 42 sources".to_string());
    state.step_count += 1;

    Ok(state)
}

/// Analysis node - simulates analyzing data
async fn analyze_node(mut state: AgentState) -> dashflow::Result<AgentState> {
    tracing::info!("Analyzing data...");

    // Simulate analysis work
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    state
        .messages
        .push("Analysis complete: High confidence results".to_string());
    state.step_count += 1;

    Ok(state)
}

/// Writer node - simulates generating output
async fn writer_node(mut state: AgentState) -> dashflow::Result<AgentState> {
    tracing::info!("Writing report...");

    // Simulate writing work
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    state
        .messages
        .push("Report generated successfully".to_string());
    state.step_count += 1;

    Ok(state)
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing with OpenTelemetry (if feature enabled)
    #[cfg(feature = "observability")]
    {
        let config = TracingConfig::new()
            .with_service_name("traced-agent-example")
            .with_otlp_endpoint("http://localhost:4317")
            .with_sampling_rate(1.0); // Sample 100% of traces

        if let Err(e) = init_tracing(config).await {
            eprintln!("Warning: Failed to initialize tracing: {}", e);
            eprintln!("  Make sure Jaeger is running (see example documentation)");
        } else {
            println!("✓ OpenTelemetry tracing initialized");
            println!("  View traces at: http://localhost:16686");
        }
    }

    #[cfg(not(feature = "observability"))]
    {
        // Fall back to basic console logging
        tracing_subscriber::fmt::init();
        println!("Running without OpenTelemetry (use --features observability to enable)");
    }

    println!("\n=== Building Graph ===");

    // Build the agent graph
    let mut graph = StateGraph::<AgentState>::new();

    graph.add_node_from_fn("researcher", |state| Box::pin(research_node(state)));

    graph.add_node_from_fn("analyzer", |state| Box::pin(analyze_node(state)));

    graph.add_node_from_fn("writer", |state| Box::pin(writer_node(state)));

    // Define the workflow
    graph.add_edge("researcher", "analyzer");
    graph.add_edge("analyzer", "writer");
    graph.add_edge("writer", END);

    graph.set_entry_point("researcher");

    println!("✓ Graph structure defined");

    // Compile the graph with a name (used in tracing spans)
    let app = graph.compile()?.with_name("research-agent");

    #[cfg(feature = "dashstream")]
    let (app, callback_for_flush) = {
        let config = DashStreamConfig {
            bootstrap_servers: std::env::var("KAFKA_BROKERS")
                .unwrap_or_else(|_| "localhost:9092".to_string()),
            topic: std::env::var("DASHSTREAM_TOPIC")
                .unwrap_or_else(|_| "dashstream-quality".to_string()),
            tenant_id: std::env::var("DASHSTREAM_TENANT_ID")
                .unwrap_or_else(|_| "demo-tenant".to_string()),
            thread_id: std::env::var("DASHSTREAM_THREAD_ID")
                .unwrap_or_else(|_| format!("traced-agent-{}", uuid::Uuid::new_v4())),
            enable_state_diff: true,
            compression_threshold: 512,
            max_state_diff_size: 10 * 1024 * 1024,
            ..Default::default()
        };

        println!(
            "✓ DashStream enabled (kafka={} topic={} thread_id={})",
            config.bootstrap_servers, config.topic, config.thread_id
        );

        let callback = DashStreamCallback::<AgentState>::with_config(config).await?;
        let callback_for_flush = callback.clone();
        (app.with_callback(callback), callback_for_flush)
    };

    println!("✓ Graph compiled");

    println!("\n=== Executing Graph ===");

    // Create initial state
    let initial_state = AgentState {
        messages: vec!["Task: Research and write report on Rust async".to_string()],
        step_count: 0,
    };

    // Execute the graph (spans will be automatically created)
    let result = app.invoke(initial_state).await?;

    println!("\n=== Execution Complete ===");
    println!("Steps executed: {}", result.final_state.step_count);
    println!("\nMessages:");
    for (i, msg) in result.final_state.messages.iter().enumerate() {
        println!("  {}. {}", i + 1, msg);
    }

    println!("\nExecution path: {:?}", result.nodes_executed);

    // Display metrics
    let metrics = app.metrics();
    println!("\n=== Execution Metrics ===");
    println!("Total duration: {:?}", metrics.total_duration);

    if let Some((node, duration)) = metrics.slowest_node() {
        println!("Slowest node: {} ({:?})", node, duration);
    }

    #[cfg(feature = "observability")]
    {
        println!("\n✓ Traces exported to OpenTelemetry endpoint");
        println!("  View at: http://localhost:16686");
        println!("  Service name: traced-agent-example");

        // Give time for traces to be exported
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }

    #[cfg(feature = "dashstream")]
    {
        println!("\n🔄 Flushing DashStream messages to Kafka...");
        callback_for_flush.flush().await?;
        println!("✓ DashStream flushed");
    }

    Ok(())
}
