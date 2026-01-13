//! Prometheus Metrics Example
//!
//! This example demonstrates how to collect and export Prometheus metrics
//! from a DashFlow application with an HTTP server for scraping.
//!
//! # Running this example
//!
//! 1. Start the example:
//!    ```bash
//!    cargo run --example metrics_example --package dashflow --features observability,dashflow-observability/metrics-server
//!    ```
//!
//! 2. In another terminal, scrape metrics:
//!    ```bash
//!    curl http://localhost:9091/metrics
//!    ```
//!
//! 3. Or run Prometheus locally:
//!    ```bash
//!    docker run -d -p 9090:9090 \
//!      -v $(pwd)/prometheus.yml:/etc/prometheus/prometheus.yml \
//!      prom/prometheus
//!    ```
//!
//!    With prometheus.yml:
//!    ```yaml
//!    scrape_configs:
//!      - job_name: 'dashflow'
//!        static_configs:
//!          - targets: ['host.docker.internal:9091']
//!    ```
//!
//! 4. View metrics in Prometheus UI: http://localhost:9090
//!
//! # Metrics Exposed
//!
//! - `graph_invocations_total{graph_name, status}` - Total graph invocations
//! - `graph_duration_seconds{graph_name}` - Graph execution duration histogram
//! - `graph_active_executions{graph_name}` - Active graph executions gauge
//! - `node_executions_total{graph_name, node_name, status}` - Node execution counter
//! - `node_duration_seconds{graph_name, node_name}` - Node execution duration histogram

use dashflow::{MergeableState, Result, StateGraph, END};
use dashflow_observability::metrics::init_default_recorder;
use dashflow_observability::metrics_server::serve_metrics;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// State for our research workflow
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResearchState {
    topic: String,
    sources: Vec<String>,
    analysis: String,
    report: String,
    iterations: u32,
}

impl MergeableState for ResearchState {
    fn merge(&mut self, other: &Self) {
        if !other.topic.is_empty() {
            if self.topic.is_empty() {
                self.topic = other.topic.clone();
            } else {
                self.topic.push('\n');
                self.topic.push_str(&other.topic);
            }
        }
        self.sources.extend(other.sources.clone());
        if !other.analysis.is_empty() {
            if self.analysis.is_empty() {
                self.analysis = other.analysis.clone();
            } else {
                self.analysis.push('\n');
                self.analysis.push_str(&other.analysis);
            }
        }
        if !other.report.is_empty() {
            if self.report.is_empty() {
                self.report = other.report.clone();
            } else {
                self.report.push('\n');
                self.report.push_str(&other.report);
            }
        }
        self.iterations = self.iterations.max(other.iterations);
    }
}

/// Simulated researcher node - finds sources
async fn researcher(mut state: ResearchState) -> Result<ResearchState> {
    println!("🔍 Researcher: Finding sources for '{}'", state.topic);

    // Simulate research work
    sleep(Duration::from_millis(100)).await;

    state.sources = vec![
        format!("https://arxiv.org/search?query={}", state.topic),
        format!("https://scholar.google.com/scholar?q={}", state.topic),
        format!("https://www.semanticscholar.org/search?q={}", state.topic),
    ];

    Ok(state)
}

/// Simulated analyzer node - analyzes sources
async fn analyzer(mut state: ResearchState) -> Result<ResearchState> {
    println!("📊 Analyzer: Processing {} sources", state.sources.len());

    // Simulate analysis work
    sleep(Duration::from_millis(150)).await;

    state.analysis = format!(
        "Analyzed {} sources on '{}'. Key findings: \
         1) Growing research interest, \
         2) Multiple practical applications, \
         3) Active development community.",
        state.sources.len(),
        state.topic
    );

    Ok(state)
}

/// Simulated writer node - generates report
async fn writer(mut state: ResearchState) -> Result<ResearchState> {
    println!("✍️  Writer: Generating report");

    // Simulate writing work
    sleep(Duration::from_millis(200)).await;

    state.report = format!(
        "# Research Report: {}\n\n\
         ## Sources\n{}\n\n\
         ## Analysis\n{}\n\n\
         ## Conclusion\nResearch complete after {} iterations.",
        state.topic,
        state.sources.join("\n"),
        state.analysis,
        state.iterations + 1
    );

    state.iterations += 1;

    Ok(state)
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting Prometheus Metrics Example\n");

    // Initialize metrics recorder (registers metrics with Prometheus)
    init_default_recorder()
        .map_err(|e| dashflow::Error::Validation(format!("Failed to initialize metrics: {}", e)))?;
    println!("✅ Metrics recorder initialized\n");

    // Build the research workflow graph
    let mut graph = StateGraph::<ResearchState>::new();

    graph.add_node_from_fn("researcher", |state| Box::pin(researcher(state)));

    graph.add_node_from_fn("analyzer", |state| Box::pin(analyzer(state)));

    graph.add_node_from_fn("writer", |state| Box::pin(writer(state)));

    // Define edges
    graph.set_entry_point("researcher");
    graph.add_edge("researcher", "analyzer");
    graph.add_edge("analyzer", "writer");
    graph.add_edge("writer", END);

    // Compile graph with a name for better metrics labels
    let app = Arc::new(graph.compile()?.with_name("research_workflow"));
    println!("✅ Research workflow compiled\n");

    // Spawn metrics server in background
    let metrics_handle = tokio::spawn(async move {
        println!("📊 Starting metrics server on http://localhost:9091/metrics");
        println!("   Run: curl http://localhost:9091/metrics\n");

        if let Err(e) = serve_metrics(9091).await {
            eprintln!("❌ Metrics server error: {}", e);
        }
    });

    // Give server time to start
    sleep(Duration::from_millis(100)).await;

    println!("🔄 Running research workflows to generate metrics...\n");

    // Run multiple workflow invocations to generate metrics
    for i in 1..=5 {
        let topic = match i % 3 {
            0 => "Rust async programming",
            1 => "DashFlow architecture",
            _ => "Distributed tracing",
        };

        println!("--- Workflow {}/5: {} ---", i, topic);

        let state = ResearchState {
            topic: topic.to_string(),
            sources: vec![],
            analysis: String::new(),
            report: String::new(),
            iterations: 0,
        };

        // Execute workflow (metrics are recorded automatically)
        match app.invoke(state).await {
            Ok(result) => {
                println!("✅ Workflow completed successfully");
                println!(
                    "   Report length: {} chars\n",
                    result.final_state.report.len()
                );
            }
            Err(e) => {
                eprintln!("❌ Workflow failed: {}\n", e);
            }
        }

        // Small delay between runs
        sleep(Duration::from_millis(100)).await;
    }

    println!("✅ All workflows completed!\n");
    println!("📊 Metrics available at: http://localhost:9091/metrics");
    println!("💡 Try these queries:");
    println!("   - curl http://localhost:9091/metrics | grep graph_invocations");
    println!("   - curl http://localhost:9091/metrics | grep graph_duration");
    println!("   - curl http://localhost:9091/metrics | grep node_executions");
    println!("\n⏸️  Press Ctrl+C to stop the metrics server...");

    // Keep running to allow metrics scraping
    let _ = metrics_handle.await;

    Ok(())
}
