//! Traced Agent Workflow Example
//!
//! This example demonstrates OpenTelemetry tracing integration with DashFlow:
//! 1. Initializing tracing with Jaeger backend
//! 2. Automatic span creation for graph execution
//! 3. State size tracking
//! 4. Checkpoint operation tracing
//! 5. Custom span attributes
//!
//! Prerequisites:
//! - Jaeger running on localhost:4317 (OTLP endpoint)
//!
//! Start Jaeger:
//! ```bash
//! docker run -d \
//!   -p 4317:4317 \
//!   -p 16686:16686 \
//!   -e COLLECTOR_OTLP_ENABLED=true \
//!   jaegertracing/all-in-one:latest
//! ```
//!
//! Run example:
//! ```bash
//! cargo run --example traced_agent --features observability
//! ```
//!
//! View traces:
//! Open http://localhost:16686 in your browser

use dashflow::checkpointer::MemoryCheckpointer;
use dashflow::schema::{NodeMetadata, NodeType};
use dashflow::{StateGraph, END};
use dashflow_observability::{init_tracing, TracingConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use tracing::{info, Span};

/// Research agent state
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct ResearchState {
    /// Research topic
    topic: String,
    /// Research findings (keyed by source)
    findings: HashMap<String, String>,
    /// Analysis results
    analysis: String,
    /// Final report
    report: String,
    /// Processing status
    status: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 Traced Research Agent Example\n");
    println!("=================================\n");

    // Initialize OpenTelemetry tracing with Jaeger
    let config = TracingConfig::new()
        .with_service_name("research-agent")
        .with_otlp_endpoint("http://localhost:4317")
        .with_sampling_rate(1.0) // Sample 100% for demo
        .with_stdout(true); // Enable stdout for debugging

    match init_tracing(config).await {
        Ok(_) => {
            println!("✅ Tracing initialized (Jaeger at http://localhost:16686)");
        }
        Err(e) => {
            println!("⚠️  Failed to initialize tracing: {}", e);
            println!("   Continuing without tracing...");
            println!("   Start Jaeger: docker run -d -p 4317:4317 -p 16686:16686 \\");
            println!("                  -e COLLECTOR_OTLP_ENABLED=true jaegertracing/all-in-one:latest\n");
        }
    }

    // Build research workflow graph
    let mut graph: StateGraph<ResearchState> = StateGraph::new();

    // Set graph description
    graph.with_description("Research agent that gathers, analyzes, and writes reports on topics");

    // Node 1: Research - Gather information
    graph.add_node_with_metadata(
        "researcher",
        NodeMetadata::new("Gathers research from Wikipedia, ArXiv, and news sources")
            .with_node_type(NodeType::Tool)
            .with_input_fields(vec!["topic"])
            .with_output_fields(vec!["findings", "status"]),
        |mut state| {
        Box::pin(async move {
            info!("Starting research on topic: {}", state.topic);
            Span::current().record("topic", state.topic.as_str());

            state.status = "researching".to_string();

            // Simulate API calls to research sources
            println!("📚 Gathering research from multiple sources...");

            sleep(Duration::from_millis(300)).await;
            state
                .findings
                .insert("wikipedia".to_string(), "Definition and background".to_string());

            sleep(Duration::from_millis(250)).await;
            state.findings.insert(
                "arxiv".to_string(),
                "Recent academic papers and citations".to_string(),
            );

            sleep(Duration::from_millis(200)).await;
            state.findings.insert(
                "news".to_string(),
                "Current events and trending discussions".to_string(),
            );

            Span::current().record("sources_found", state.findings.len() as i64);
            println!("   Found {} sources", state.findings.len());

            state.status = "researched".to_string();
            Ok(state)
        })
    });

    // Node 2: Analyzer - Process findings
    graph.add_node_with_metadata(
        "analyzer",
        NodeMetadata::new("Analyzes research findings and extracts key insights")
            .with_node_type(NodeType::Llm)
            .with_input_fields(vec!["findings"])
            .with_output_fields(vec!["analysis", "status"]),
        |mut state| {
            Box::pin(async move {
                info!("Analyzing research findings");
                state.status = "analyzing".to_string();

                println!("🔬 Analyzing findings...");

                sleep(Duration::from_millis(400)).await;

                // Simulate analysis
                let mut analysis_parts = Vec::new();
                for (source, content) in &state.findings {
                    analysis_parts.push(format!("[{}] {}", source, content));
                }

                state.analysis = analysis_parts.join("\n");
                Span::current().record("analysis_length", state.analysis.len() as i64);
                println!("   Generated analysis ({} characters)", state.analysis.len());

                state.status = "analyzed".to_string();
                Ok(state)
            })
        },
    );

    // Node 3: Writer - Generate final report
    graph.add_node_with_metadata(
        "writer",
        NodeMetadata::new("Generates final research report from analysis")
            .with_node_type(NodeType::Llm)
            .with_input_fields(vec!["topic", "findings", "analysis"])
            .with_output_fields(vec!["report", "status"]),
        |mut state| {
            Box::pin(async move {
                info!("Generating final report");
                state.status = "writing".to_string();

                println!("✍️  Writing final report...");

                sleep(Duration::from_millis(500)).await;

                // Generate report
                state.report = format!(
                    "Research Report: {}\n\n\
                     Sources Consulted: {}\n\n\
                     Analysis:\n{}\n\n\
                     Report generated successfully.",
                    state.topic,
                    state.findings.len(),
                    state.analysis
                );

                Span::current().record("report_length", state.report.len() as i64);
                println!("   Report completed ({} characters)", state.report.len());

                state.status = "completed".to_string();
                Ok(state)
            })
        },
    );

    // Define workflow edges
    graph.add_edge("researcher", "analyzer");
    graph.add_edge("analyzer", "writer");
    graph.add_edge("writer", END);
    graph.set_entry_point("researcher");

    // Export graph schema for visualization
    let schema = graph.export_schema("research-agent");
    println!("\n📊 Graph Schema exported:");
    println!("   Nodes: {:?}", schema.nodes.iter().map(|n| &n.name).collect::<Vec<_>>());
    println!("   Edges: {}", schema.edges.len());
    if let Ok(json) = schema.to_json_pretty() {
        println!("\n{}", json);
    }

    // Compile with checkpointing (enables checkpoint tracing)
    let app = graph
        .compile()?
        .with_checkpointer(MemoryCheckpointer::new())
        .with_thread_id("demo-session-001")
        .with_name("research-agent");

    println!("\n🚀 Executing research workflow (with tracing)...\n");

    // Execute workflow
    let initial_state = ResearchState {
        topic: "Rust async programming".to_string(),
        ..Default::default()
    };

    let result = app.invoke(initial_state).await?;

    // Display results
    println!("\n✅ Research workflow completed!\n");
    println!("📊 Results:");
    println!("   Topic: {}", result.topic);
    println!("   Sources: {}", result.findings.len());
    println!("   Analysis: {} chars", result.analysis.len());
    println!("   Report: {} chars", result.report.len());
    println!("   Status: {}", result.status);

    println!("\n🔍 View trace visualization:");
    println!("   1. Open http://localhost:16686 in your browser");
    println!("   2. Select service: 'research-agent'");
    println!("   3. Click 'Find Traces'");
    println!("   4. Click on the trace to see the span hierarchy\n");

    println!("📈 Trace details you'll see:");
    println!("   • graph.invoke - Overall workflow execution");
    println!("   • graph.execute_node (researcher) - Research phase with state size");
    println!("   • checkpoint.save - Checkpoint after researcher");
    println!("   • graph.execute_node (analyzer) - Analysis phase");
    println!("   • checkpoint.save - Checkpoint after analyzer");
    println!("   • graph.execute_node (writer) - Writing phase");
    println!("   • checkpoint.save - Final checkpoint\n");

    // Flush spans to ensure they're exported
    println!("🔄 Flushing spans to Jaeger...");
    opentelemetry::global::shutdown_tracer_provider();
    println!("✅ Spans exported. Check Jaeger UI for visualization.\n");

    Ok(())
}
