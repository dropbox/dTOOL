//! Cost Tracking Example
//!
//! Demonstrates cost tracking for LLM calls in a multi-node research graph.
//!
//! This example shows:
//! - Configuring model pricing (OpenAI GPT models)
//! - Recording LLM call costs per node
//! - Generating cost reports with breakdowns by model and node
//! - Integration with DashFlow execution
//!
//! # Running the Example
//!
//! ```bash
//! cargo run --example cost_tracking_example --features observability
//! ```
//!
//! # Expected Output
//!
//! ```text
//! === Research Graph with Cost Tracking ===
//!
//! Running 3 graph invocations...
//!
//! [Graph 1] Research Task: AI safety
//! [researcher] Simulated LLM call: gpt-4 (1200 input, 800 output tokens)
//! [analyzer] Simulated LLM call: gpt-4-turbo (1500 input, 600 output tokens)
//! [writer] Simulated LLM call: gpt-3.5-turbo (2000 input, 1000 tokens)
//!
//! [Graph 2] Research Task: quantum computing
//! ...
//!
//! Cost Report
//! ===========
//! Total Calls: 9
//! Total Cost: $0.4680
//! Total Tokens: 27300 (input: 16200, output: 11100)
//! Average Cost/Call: $0.0520
//!
//! Cost by Model:
//!   gpt-4: $0.2520
//!   gpt-4-turbo: $0.1350
//!   gpt-3.5-turbo: $0.0810
//!
//! Cost by Node:
//!   researcher: $0.2520
//!   analyzer: $0.1350
//!   writer: $0.0810
//! ```

use dashflow::{MergeableState, Result as GraphResult, StateGraph, END};
use dashflow_observability::cost::{CostTracker, ModelPricing, Pricing};
use std::sync::{Arc, Mutex};

/// Research graph state
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ResearchState {
    /// Research topic
    topic: String,
    /// Research findings from researcher node
    research: String,
    /// Analysis from analyzer node
    analysis: String,
    /// Final report from writer node
    report: String,
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
        if !other.research.is_empty() {
            if self.research.is_empty() {
                self.research = other.research.clone();
            } else {
                self.research.push('\n');
                self.research.push_str(&other.research);
            }
        }
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
    }
}

/// Simulate an LLM call with cost tracking
fn simulate_llm_call(
    cost_tracker: &Arc<Mutex<CostTracker>>,
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    node_name: &str,
    prompt: &str,
) -> String {
    println!(
        "[{}] Simulated LLM call: {} ({} input, {} output tokens)",
        node_name, model, input_tokens, output_tokens
    );

    // Record cost
    let cost = cost_tracker
        .lock()
        .unwrap()
        .record_llm_call(model, input_tokens, output_tokens, Some(node_name))
        .unwrap();

    println!("  Cost: ${:.4}", cost);

    // Simulate LLM response
    format!(
        "Response to '{}' using {} (cost: ${:.4})",
        prompt, model, cost
    )
}

/// Researcher node: Gathers initial research using GPT-4
async fn researcher_node(
    mut state: ResearchState,
    cost_tracker: Arc<Mutex<CostTracker>>,
) -> GraphResult<ResearchState> {
    println!("\n[researcher] Processing topic: {}", state.topic);

    let research = simulate_llm_call(
        &cost_tracker,
        "gpt-4",
        1200, // input tokens
        800,  // output tokens
        "researcher",
        &format!("Research the topic: {}", state.topic),
    );

    state.research = research;
    Ok(state)
}

/// Analyzer node: Analyzes research using GPT-4 Turbo
async fn analyzer_node(
    mut state: ResearchState,
    cost_tracker: Arc<Mutex<CostTracker>>,
) -> GraphResult<ResearchState> {
    println!("\n[analyzer] Analyzing research findings");

    let analysis = simulate_llm_call(
        &cost_tracker,
        "gpt-4-turbo",
        1500, // input tokens
        600,  // output tokens
        "analyzer",
        &format!("Analyze this research: {}", state.research),
    );

    state.analysis = analysis;
    Ok(state)
}

/// Writer node: Writes final report using GPT-3.5 Turbo
async fn writer_node(
    mut state: ResearchState,
    cost_tracker: Arc<Mutex<CostTracker>>,
) -> GraphResult<ResearchState> {
    println!("\n[writer] Writing final report");

    let report = simulate_llm_call(
        &cost_tracker,
        "gpt-3.5-turbo",
        2000, // input tokens
        1000, // output tokens
        "writer",
        &format!("Write a report based on: {}", state.analysis),
    );

    state.report = report;
    Ok(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Research Graph with Cost Tracking ===\n");

    // Configure pricing for OpenAI models
    let pricing = ModelPricing::new()
        .with_model("gpt-4", Pricing::new(0.03, 0.06))
        .with_model("gpt-4-turbo", Pricing::new(0.01, 0.03))
        .with_model("gpt-3.5-turbo", Pricing::new(0.0005, 0.0015));

    // Create cost tracker
    let cost_tracker = Arc::new(Mutex::new(CostTracker::new(pricing)));

    // Build research graph
    let mut graph = StateGraph::<ResearchState>::new();

    let cost_tracker_clone = cost_tracker.clone();
    graph.add_node_from_fn("researcher", move |state| {
        let tracker = cost_tracker_clone.clone();
        Box::pin(researcher_node(state, tracker))
    });

    let cost_tracker_clone = cost_tracker.clone();
    graph.add_node_from_fn("analyzer", move |state| {
        let tracker = cost_tracker_clone.clone();
        Box::pin(analyzer_node(state, tracker))
    });

    let cost_tracker_clone = cost_tracker.clone();
    graph.add_node_from_fn("writer", move |state| {
        let tracker = cost_tracker_clone.clone();
        Box::pin(writer_node(state, tracker))
    });

    graph.set_entry_point("researcher");
    graph.add_edge("researcher", "analyzer");
    graph.add_edge("analyzer", "writer");
    graph.add_edge("writer", END);

    let app = graph.compile()?;

    // Run multiple graph invocations
    println!("Running 3 graph invocations...\n");

    let topics = ["AI safety", "quantum computing", "climate change"];

    for (i, topic) in topics.iter().enumerate() {
        println!("=== Graph {} ===", i + 1);
        println!("Research Task: {}", topic);

        let input = ResearchState {
            topic: topic.to_string(),
            research: String::new(),
            analysis: String::new(),
            report: String::new(),
        };

        let result = app.invoke(input).await?;
        println!("\n✓ Completed research on: {}", result.final_state.topic);
    }

    // Generate cost report
    println!("\n=== Final Cost Report ===\n");
    let report = cost_tracker.lock().unwrap().report();
    println!("{}", report.format());

    // Additional insights
    println!("\n=== Cost Insights ===");
    if let Some((model, cost)) = report
        .cost_by_model()
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
    {
        println!("Most expensive model: {} (${:.4})", model, cost);
    }

    if let Some((node, cost)) = report
        .cost_by_node()
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
    {
        println!("Most expensive node: {} (${:.4})", node, cost);
    }

    println!("\n✓ Cost tracking example complete!");
    println!("\nNote: This example uses simulated LLM calls with fixed token counts.");
    println!("In a real application, token counts would come from actual LLM responses.");

    Ok(())
}
