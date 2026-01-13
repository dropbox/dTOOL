//! Distributed Checkpointing Example
//!
//! This example demonstrates the DistributedCheckpointCoordinator which ensures
//! checkpoint consistency when using distributed execution with work-stealing schedulers.
//!
//! # Scenario
//!
//! A multi-agent system with parallel document analysis:
//! - 4 parallel analysis nodes (sentiment, keywords, summary, entities)
//! - Each node may execute on different workers
//! - Coordinator ensures checkpoints are consistent despite concurrent execution
//!
//! # Usage
//!
//! ```bash
//! cargo run --example distributed_checkpointing
//! ```

use dashflow::{
    Checkpointer, DistributedCheckpointCoordinator, MemoryCheckpointer, MergeableState, StateGraph,
    WorkStealingScheduler, END,
};
use serde::{Deserialize, Serialize};

/// Document analysis state
#[derive(Clone, Debug, Serialize, Deserialize)]
struct DocumentState {
    /// Input document text
    document: String,
    /// Sentiment analysis result
    sentiment: Option<String>,
    /// Extracted keywords
    keywords: Option<Vec<String>>,
    /// Document summary
    summary: Option<String>,
    /// Named entities
    entities: Option<Vec<String>>,
}

impl MergeableState for DocumentState {
    fn merge(&mut self, other: &Self) {
        if !other.document.is_empty() {
            if self.document.is_empty() {
                self.document = other.document.clone();
            } else {
                self.document.push('\n');
                self.document.push_str(&other.document);
            }
        }
        if other.sentiment.is_some() {
            self.sentiment = other.sentiment.clone();
        }
        if other.keywords.is_some() {
            self.keywords = other.keywords.clone();
        }
        if other.summary.is_some() {
            self.summary = other.summary.clone();
        }
        if other.entities.is_some() {
            self.entities = other.entities.clone();
        }
    }
}

// GraphState is automatically implemented for types that meet the trait bounds:
// Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static

/// Sentiment analysis node
async fn sentiment_node(mut state: DocumentState) -> Result<DocumentState, dashflow::Error> {
    println!("  [Sentiment] Analyzing document sentiment...");
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Simple sentiment analysis (mock)
    let sentiment = if state.document.contains("great") || state.document.contains("excellent") {
        "Positive"
    } else if state.document.contains("bad") || state.document.contains("poor") {
        "Negative"
    } else {
        "Neutral"
    };

    state.sentiment = Some(sentiment.to_string());
    println!("  [Sentiment] Result: {}", sentiment);
    Ok(state)
}

/// Keyword extraction node
async fn keywords_node(mut state: DocumentState) -> Result<DocumentState, dashflow::Error> {
    println!("  [Keywords] Extracting keywords...");
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    // Simple keyword extraction (mock)
    let keywords: Vec<String> = state
        .document
        .split_whitespace()
        .filter(|w| w.len() > 5)
        .take(5)
        .map(|s| s.to_string())
        .collect();

    state.keywords = Some(keywords.clone());
    println!("  [Keywords] Extracted: {:?}", keywords);
    Ok(state)
}

/// Summary generation node
async fn summary_node(mut state: DocumentState) -> Result<DocumentState, dashflow::Error> {
    println!("  [Summary] Generating summary...");
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Simple summary (mock)
    let summary = if state.document.len() > 50 {
        format!("{}...", &state.document[..50])
    } else {
        state.document.clone()
    };

    state.summary = Some(summary.clone());
    println!("  [Summary] Generated: {}", summary);
    Ok(state)
}

/// Entity extraction node
async fn entities_node(mut state: DocumentState) -> Result<DocumentState, dashflow::Error> {
    println!("  [Entities] Extracting named entities...");
    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;

    // Simple entity extraction (mock - find capitalized words)
    let entities: Vec<String> = state
        .document
        .split_whitespace()
        .filter(|w| w.chars().next().is_some_and(|c| c.is_uppercase()))
        .map(|s| s.to_string())
        .collect();

    state.entities = Some(entities.clone());
    println!("  [Entities] Found: {:?}", entities);
    Ok(state)
}

/// Aggregation node - combines results
async fn aggregation_node(state: DocumentState) -> Result<DocumentState, dashflow::Error> {
    println!("\n[Aggregation] Combining analysis results...");
    println!("  Sentiment: {:?}", state.sentiment);
    println!("  Keywords: {:?}", state.keywords);
    println!("  Summary: {:?}", state.summary);
    println!("  Entities: {:?}", state.entities);
    Ok(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Distributed Checkpointing Example ===\n");
    println!("Scenario: Parallel document analysis with distributed checkpointing");
    println!("Features: Concurrent checkpoint coordination, sequence tracking\n");

    // Create distributed checkpoint coordinator
    // Wraps a MemoryCheckpointer with distributed coordination logic
    let inner_checkpointer = MemoryCheckpointer::new();
    let coordinator = DistributedCheckpointCoordinator::new(inner_checkpointer);

    // Create work-stealing scheduler (no workers = local execution with distributed logic)
    let scheduler = WorkStealingScheduler::new()
        .with_threshold(3) // Distribute if queue > 3 tasks
        .with_strategy(dashflow::SelectionStrategy::LeastLoaded);

    // Build the graph
    let mut graph: StateGraph<DocumentState> = StateGraph::new();

    // Add start node
    graph.add_node_from_fn("start", |state| Box::pin(async move { Ok(state) }));

    // Add parallel analysis nodes
    graph.add_node_from_fn("sentiment", |state| Box::pin(sentiment_node(state)));
    graph.add_node_from_fn("keywords", |state| Box::pin(keywords_node(state)));
    graph.add_node_from_fn("summary", |state| Box::pin(summary_node(state)));
    graph.add_node_from_fn("entities", |state| Box::pin(entities_node(state)));

    // Add aggregation node
    graph.add_node_from_fn("aggregation", |state| Box::pin(aggregation_node(state)));

    // Set entry point
    graph.set_entry_point("start");

    // Parallel edges to analysis nodes
    graph.add_parallel_edges(
        "start",
        vec![
            "sentiment".to_string(),
            "keywords".to_string(),
            "summary".to_string(),
            "entities".to_string(),
        ],
    );

    // All analysis nodes converge to aggregation
    graph.add_edge("sentiment", "aggregation");
    graph.add_edge("keywords", "aggregation");
    graph.add_edge("summary", "aggregation");
    graph.add_edge("entities", "aggregation");

    graph.add_edge("aggregation", END);

    // Compile with distributed checkpointing and scheduler
    let app = graph
        .compile()?
        .with_checkpointer(coordinator.clone())
        .with_thread_id("doc-analysis-session-1")
        .with_scheduler(scheduler);

    // Run analysis on sample document
    let document = "The Anthropic team released Claude, an excellent AI assistant. \
                   The technology shows great promise for enterprise applications. \
                   San Francisco based company continues to innovate in safety research.";

    let initial_state = DocumentState {
        document: document.to_string(),
        sentiment: None,
        keywords: None,
        summary: None,
        entities: None,
    };

    println!("Document: {}\n", document);
    println!("Running parallel analysis...\n");

    let result = app.invoke(initial_state).await?;

    println!("\n=== Final Results ===");
    println!("Sentiment: {:?}", result.final_state.sentiment);
    println!("Keywords: {:?}", result.final_state.keywords);
    println!("Summary: {:?}", result.final_state.summary);
    println!("Entities: {:?}", result.final_state.entities);

    // Show checkpoint information
    println!("\n=== Checkpoint Information ===");
    let checkpoint_count = coordinator
        .checkpoint_count("doc-analysis-session-1")
        .await?;
    println!("Checkpoints created: {}", checkpoint_count);

    let checkpoints = coordinator.list("doc-analysis-session-1").await?;
    println!("Checkpoint sequence:");
    for (i, cp) in checkpoints.iter().enumerate() {
        let seq = cp
            .metadata
            .get("sequence")
            .map(|s| s.as_str())
            .unwrap_or("?");
        let distributed = cp
            .metadata
            .get("distributed")
            .map(|s| s.as_str())
            .unwrap_or("false");
        println!(
            "  {}. Node: {}, Sequence: {}, Distributed: {}",
            i + 1,
            cp.node,
            seq,
            distributed
        );
    }

    // Demonstrate checkpoint consistency
    println!("\n=== Consistency Verification ===");
    let latest = coordinator.get_latest("doc-analysis-session-1").await?;
    let Some(latest) = latest else {
        println!("No checkpoints found for doc-analysis-session-1");
        return Ok(());
    };
    println!("Latest checkpoint ID: {}", latest.id);
    println!("Latest checkpoint has all results:");
    println!("  Sentiment present: {}", latest.state.sentiment.is_some());
    println!("  Keywords present: {}", latest.state.keywords.is_some());
    println!("  Summary present: {}", latest.state.summary.is_some());
    println!("  Entities present: {}", latest.state.entities.is_some());

    // Demonstrate sequence is monotonic
    println!("\n=== Sequence Tracking Verification ===");
    let sequences: Vec<u64> = checkpoints
        .iter()
        .filter_map(|cp| cp.metadata.get("sequence")?.parse().ok())
        .collect();
    println!("Sequences are monotonic: {:?}", sequences);
    let is_monotonic = sequences.windows(2).all(|w| w[0] < w[1]);
    println!(
        "Monotonicity check: {}",
        if is_monotonic { "PASS ✓" } else { "FAIL ✗" }
    );

    println!("\n=== Distributed Checkpointing Features ===");
    println!("✓ Sequence tracking: Each checkpoint has monotonic sequence number");
    println!("✓ Thread isolation: Multiple sessions maintain separate sequences");
    println!("✓ Concurrent safety: Parallel nodes checkpoint without conflicts");
    println!("✓ Consistency: All results captured in final checkpoint");
    println!("✓ Distributed metadata: Checkpoints tagged for distributed execution");

    println!("\n=== Example Complete ===");
    println!("The distributed checkpoint coordinator ensures consistency when");
    println!("nodes execute across multiple workers in parallel. Sequence numbers");
    println!("enable deterministic ordering even with concurrent execution.");

    Ok(())
}
