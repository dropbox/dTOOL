//! Work-stealing scheduler for distributed parallel execution
//!
//! This example demonstrates how to use the work-stealing scheduler to
//! distribute parallel node execution across remote workers. The scheduler
//! automatically load-balances tasks using work-stealing algorithms.
//!
//! # Features
//! - Local execution when no workers available
//! - Automatic fallback to local execution if distribution fails
//! - Configurable worker selection strategies (RoundRobin, LeastLoaded, Random)
//! - Performance metrics tracking
//!
//! # Architecture
//! - **Local Queue Threshold**: When queue < threshold, execute locally
//! - **Distribution**: When queue >= threshold, distribute to workers
//! - **Fault Tolerance**: Fallback to local execution if workers unavailable
//!
//! Note: This example shows local fallback mode since no actual remote workers
//! are configured. In production, you would configure gRPC worker endpoints.

use dashflow::{scheduler::WorkStealingScheduler, MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AnalysisState {
    input: String,
    analyses: Vec<String>,
    summary: String,
}

impl MergeableState for AnalysisState {
    fn merge(&mut self, other: &Self) {
        if !other.input.is_empty() {
            if self.input.is_empty() {
                self.input = other.input.clone();
            } else {
                self.input.push('\n');
                self.input.push_str(&other.input);
            }
        }
        self.analyses.extend(other.analyses.clone());
        if !other.summary.is_empty() {
            if self.summary.is_empty() {
                self.summary = other.summary.clone();
            } else {
                self.summary.push('\n');
                self.summary.push_str(&other.summary);
            }
        }
    }
}

impl AnalysisState {
    fn new(input: String) -> Self {
        Self {
            input,
            analyses: Vec::new(),
            summary: String::new(),
        }
    }

    fn add_analysis(&mut self, analysis: String) {
        self.analyses.push(analysis);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Work-Stealing Scheduler Example\n");

    // Create the graph
    let mut graph: StateGraph<AnalysisState> = StateGraph::new();

    // Input node - prepares data
    graph.add_node_from_fn("input", |state| {
        Box::pin(async move {
            println!("📥 Input: Processing '{}'", state.input);
            Ok(state)
        })
    });

    // Analysis node 1 - Sentiment analysis
    graph.add_node_from_fn("sentiment_analysis", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let sentiment = if state.input.contains("good") || state.input.contains("great") {
                "Positive"
            } else if state.input.contains("bad") || state.input.contains("poor") {
                "Negative"
            } else {
                "Neutral"
            };
            let analysis = format!("Sentiment: {}", sentiment);
            println!("  🔍 Sentiment Analysis: {}", analysis);
            state.add_analysis(analysis);
            Ok(state)
        })
    });

    // Analysis node 2 - Keyword extraction
    graph.add_node_from_fn("keyword_extraction", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let keywords: Vec<&str> = state
                .input
                .split_whitespace()
                .filter(|w| w.len() > 5)
                .take(3)
                .collect();
            let analysis = format!("Keywords: {}", keywords.join(", "));
            println!("  🔍 Keyword Extraction: {}", analysis);
            state.add_analysis(analysis);
            Ok(state)
        })
    });

    // Analysis node 3 - Language detection
    graph.add_node_from_fn("language_detection", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let language = "English"; // Simplified
            let analysis = format!("Language: {}", language);
            println!("  🔍 Language Detection: {}", analysis);
            state.add_analysis(analysis);
            Ok(state)
        })
    });

    // Analysis node 4 - Entity extraction
    graph.add_node_from_fn("entity_extraction", |mut state| {
        Box::pin(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let entities: Vec<&str> = state
                .input
                .split_whitespace()
                .filter(|w| w.chars().next().is_some_and(|c| c.is_uppercase()))
                .collect();
            let analysis = format!("Entities: {}", entities.join(", "));
            println!("  🔍 Entity Extraction: {}", analysis);
            state.add_analysis(analysis);
            Ok(state)
        })
    });

    // Summary node - aggregates results
    graph.add_node_from_fn("summary", |mut state| {
        Box::pin(async move {
            println!("\n🔀 Summary: Aggregating analysis results...");
            state.summary = state.analyses.join(" | ");
            println!("✅ Summary complete: {}\n", state.summary);
            Ok(state)
        })
    });

    // Build the graph
    graph.set_entry_point("input");

    // Fan out to parallel analysis nodes
    graph.add_parallel_edges(
        "input",
        vec![
            "sentiment_analysis".to_string(),
            "keyword_extraction".to_string(),
            "language_detection".to_string(),
            "entity_extraction".to_string(),
        ],
    );

    // All analysis nodes converge to summary
    graph.add_edge("entity_extraction", "summary");
    graph.add_edge("summary", END);

    // Create work-stealing scheduler
    // Note: No workers configured = local execution fallback
    // In production: .with_workers(vec!["worker1:50051", "worker2:50051"])
    let scheduler = WorkStealingScheduler::new()
        .with_threshold(2) // Distribute when queue has 2+ tasks
        .with_strategy(dashflow::scheduler::SelectionStrategy::LeastLoaded);

    println!("⚙️  Scheduler Configuration:");
    println!("  - Strategy: LeastLoaded");
    println!("  - Threshold: 2 tasks");
    println!("  - Workers: 0 (local fallback mode)\n");

    // Compile with scheduler
    let app = graph.compile()?.with_scheduler(scheduler.clone());

    let initial_state = AnalysisState::new(
        "The DashFlow scheduler provides great distributed execution with excellent performance."
            .to_string(),
    );

    println!("🚀 Starting analysis workflow with scheduler...\n");
    let start = std::time::Instant::now();
    let result = app.invoke(initial_state).await?;
    let duration = start.elapsed();

    println!("📊 Execution Summary:");
    println!("  Duration: {:?}", duration);
    println!("  Nodes executed: {:?}", result.nodes_executed);
    println!(
        "  Analyses performed: {}",
        result.final_state.analyses.len()
    );
    println!("  Summary: {}\n", result.final_state.summary);

    // Show scheduler metrics
    let metrics = scheduler.metrics().await;
    println!("📈 Scheduler Metrics:");
    println!("  Tasks submitted: {}", metrics.tasks_submitted);
    println!("  Tasks executed locally: {}", metrics.tasks_executed_local);
    println!(
        "  Tasks executed remotely: {}",
        metrics.tasks_executed_remote
    );
    println!("  Local execution time: {:?}", metrics.execution_time_local);
    println!(
        "  Remote execution time: {:?}",
        metrics.execution_time_remote
    );

    println!("\n✅ Work-stealing scheduler example complete!");
    println!("\n💡 Next steps:");
    println!(
        "  1. Configure remote workers: .with_workers(vec![\"worker1:50051\", \"worker2:50051\"])"
    );
    println!("  2. Deploy worker services via gRPC");
    println!("  3. Adjust threshold and strategy for your workload");
    println!("  4. Monitor metrics for performance optimization");

    Ok(())
}
