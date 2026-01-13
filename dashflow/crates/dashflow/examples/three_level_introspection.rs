//! Three-Level Introspection Example
//!
//! Demonstrates DashFlow's complete self-awareness at three levels:
//!
//! 1. **Platform Introspection** - DashFlow framework capabilities
//! 2. **App Introspection** - Application-specific graph configuration
//! 3. **Live Introspection** - Runtime execution state
//!
//! Run with:
//! ```bash
//! cargo run --package dashflow --example three_level_introspection
//! ```

use dashflow::{ExecutionTracker, MergeableState, StateGraph};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AgentState {
    input: String,
    output: String,
    step_count: u32,
}

impl MergeableState for AgentState {
    fn merge(&mut self, other: &Self) {
        if !other.output.is_empty() {
            self.output = other.output.clone();
        }
        self.step_count = self.step_count.max(other.step_count);
    }
}

impl AgentState {
    fn new(input: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            output: String::new(),
            step_count: 0,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("DashFlow Three-Level Introspection Demo\n");
    println!("{}", "=".repeat(70));

    // Build a simple graph
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    graph.add_node_from_fn("processor", |mut state| {
        Box::pin(async move {
            state.step_count += 1;
            state.output = format!("Processed: {}", state.input);
            Ok(state)
        })
    });

    graph.add_node_from_fn("formatter", |mut state| {
        Box::pin(async move {
            state.step_count += 1;
            state.output = format!("[FORMATTED] {}", state.output);
            Ok(state)
        })
    });

    graph.set_entry_point("processor");
    graph.add_edge("processor", "formatter");
    graph.add_edge("formatter", "__end__");

    // Create execution tracker for live introspection
    let tracker = Arc::new(ExecutionTracker::new());

    // Compile with execution tracker attached
    let app = graph
        .compile()?
        .with_execution_tracker(Arc::clone(&tracker));

    // ============================================================
    // LEVEL 1: Platform Introspection
    // ============================================================
    println!("\n[LEVEL 1] PLATFORM INTROSPECTION");
    println!("{}", "-".repeat(70));
    println!("Platform introspection reveals DashFlow framework capabilities.\n");

    let platform = app.platform_introspection();

    println!("DashFlow Version: {}", platform.version_info().version);
    println!("Rust Version: {}", platform.version_info().rust_version);

    println!(
        "\nAvailable Features ({}):",
        platform.available_features().len()
    );
    for feature in platform.available_features().iter().take(5) {
        println!(
            "  - {} (default: {})",
            feature.name,
            if feature.default_enabled() {
                "enabled"
            } else {
                "disabled"
            }
        );
    }

    println!(
        "\nSupported Node Types ({}):",
        platform.supported_node_types().len()
    );
    for node_type in platform.supported_node_types().iter().take(3) {
        println!("  - {}: {}", node_type.name, node_type.description);
    }

    println!(
        "\nSupported Edge Types ({}):",
        platform.supported_edge_types().len()
    );
    for edge_type in platform.supported_edge_types() {
        println!("  - {}: {}", edge_type.name, edge_type.description);
    }

    println!(
        "\nBuilt-in Templates ({}):",
        platform.built_in_templates().len()
    );
    for template in platform.built_in_templates().iter().take(3) {
        println!("  - {}: {}", template.name, template.description);
    }

    // ============================================================
    // LEVEL 2: App Introspection
    // ============================================================
    println!("\n\n[LEVEL 2] APP INTROSPECTION");
    println!("{}", "-".repeat(70));
    println!("App introspection reveals this specific graph's configuration.\n");

    let app_info = app.introspect();
    let manifest = &app_info.manifest;
    let architecture = &app_info.architecture;
    let capabilities = &app_info.capabilities;

    println!(
        "Graph Name: {}",
        manifest.graph_name.as_deref().unwrap_or("(unnamed)")
    );
    println!("Entry Point: {}", manifest.entry_point);

    println!("\nNodes ({}):", manifest.nodes.len());
    for name in manifest.nodes.keys() {
        println!("  - {}", name);
    }

    println!("\nEdges ({}):", manifest.edges.len());
    for (source, edges) in &manifest.edges {
        for edge in edges {
            println!("  - {} -> {}", source, edge.to);
        }
    }

    println!("\nArchitecture:");
    println!(
        "  - Features Used: {}",
        architecture.dashflow_features_used.len()
    );
    println!("  - Dependencies: {}", architecture.dependencies.len());
    println!(
        "  - Custom Code Modules: {}",
        architecture.custom_code.len()
    );

    println!("\nCapabilities:");
    println!("  - Tools Available: {}", capabilities.tools.len());
    println!("  - Models Available: {}", capabilities.models.len());
    println!("  - Storage Backends: {}", capabilities.storage.len());

    // ============================================================
    // LEVEL 3: Live Introspection
    // ============================================================
    println!("\n\n[LEVEL 3] LIVE INTROSPECTION");
    println!("{}", "-".repeat(70));
    println!("Live introspection reveals runtime execution state.\n");

    // Before execution
    let executions_before = app.live_executions();
    println!("Active Executions (before): {}", executions_before.len());

    // Run the graph
    println!("\nExecuting graph...");
    let initial_state = AgentState::new("Hello, Introspection!");
    let result = app.invoke(initial_state).await?;

    println!("Execution complete!");
    println!("Output: {}", result.state().output);
    println!("Steps: {}", result.state().step_count);

    // After execution - check tracked executions
    let executions_after = app.live_executions();
    println!("\nTracked Executions (after): {}", executions_after.len());

    for exec in &executions_after {
        println!("\n  Execution: {}", exec.execution_id);
        println!("  Status: {:?}", exec.status);
        println!("  Started: {}", exec.started_at);
        println!("  Graph: {}", exec.graph_name);
        println!("  Current Node: {}", exec.current_node);
        println!("  Iterations: {}", exec.iteration);
    }

    // ============================================================
    // UNIFIED INTROSPECTION
    // ============================================================
    println!("\n\n[UNIFIED] ALL THREE LEVELS IN ONE CALL");
    println!("{}", "-".repeat(70));
    println!("UnifiedIntrospection combines all three levels.\n");

    let unified = app.unified_introspection();

    println!(
        "Platform Version: {}",
        unified.platform.version_info().version
    );
    println!("App Entry Point: {}", unified.app.manifest.entry_point);
    println!("Live Executions: {}", unified.live.len());
    println!(
        "Active Execution Count: {}",
        unified.active_execution_count()
    );
    println!("Has Active Executions: {}", unified.has_active_executions());

    // Serialize to JSON
    println!("\nJSON Output (truncated):");
    let json = unified.to_json()?;
    let truncated: String = json.chars().take(500).collect();
    println!("{}...", truncated);

    println!("\n\n{}", "=".repeat(70));
    println!("Three-Level Introspection Demo Complete!");
    println!("\nThis example demonstrated how AI agents can achieve complete");
    println!("self-awareness using DashFlow's introspection capabilities.");

    Ok(())
}
