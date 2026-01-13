//! Graph Execution Events Example
//!
//! This example demonstrates DashFlow's event system for monitoring and
//! debugging graph execution. Events provide visibility into:
//! - When nodes start and complete
//! - Edge traversals and routing decisions
//! - Parallel execution coordination
//! - Overall graph execution timing
//!
//! Run: cargo run --example graph_events

use dashflow::{
    CollectingCallback, EdgeType, GraphEvent, MergeableState, PrintCallback, StateGraph, END,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
struct ResearchState {
    query: String,
    research_notes: Vec<String>,
    draft: String,
    final_text: String,
    should_revise: bool,
    revision_count: i32,
}

impl MergeableState for ResearchState {
    fn merge(&mut self, other: &Self) {
        if !other.query.is_empty() {
            if self.query.is_empty() {
                self.query = other.query.clone();
            } else {
                self.query.push('\n');
                self.query.push_str(&other.query);
            }
        }
        self.research_notes.extend(other.research_notes.clone());
        if !other.draft.is_empty() {
            if self.draft.is_empty() {
                self.draft = other.draft.clone();
            } else {
                self.draft.push('\n');
                self.draft.push_str(&other.draft);
            }
        }
        if !other.final_text.is_empty() {
            if self.final_text.is_empty() {
                self.final_text = other.final_text.clone();
            } else {
                self.final_text.push('\n');
                self.final_text.push_str(&other.final_text);
            }
        }
        self.should_revise = self.should_revise || other.should_revise;
        self.revision_count = self.revision_count.max(other.revision_count);
    }
}

impl ResearchState {
    fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            research_notes: Vec::new(),
            draft: String::new(),
            final_text: String::new(),
            should_revise: true,
            revision_count: 0,
        }
    }
}

fn build_research_graph() -> StateGraph<ResearchState> {
    let mut graph = StateGraph::new();

    // Node 1: Research
    graph.add_node_from_fn("research", |mut state: ResearchState| {
        Box::pin(async move {
            println!("  📚 Researching topic: {}", state.query);
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            state
                .research_notes
                .push(format!("Fact 1 about {}", state.query));
            state
                .research_notes
                .push(format!("Fact 2 about {}", state.query));
            state
                .research_notes
                .push(format!("Fact 3 about {}", state.query));

            Ok(state)
        })
    });

    // Node 2: Write Draft
    graph.add_node_from_fn("write_draft", |mut state: ResearchState| {
        Box::pin(async move {
            println!("  ✍️  Writing draft...");
            tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;

            state.draft = format!(
                "Draft article about {}:\n{}",
                state.query,
                state.research_notes.join("\n")
            );

            Ok(state)
        })
    });

    // Node 3: Review
    graph.add_node_from_fn("review", |mut state: ResearchState| {
        Box::pin(async move {
            println!("  🔍 Reviewing draft...");
            tokio::time::sleep(tokio::time::Duration::from_millis(60)).await;

            state.revision_count += 1;

            // Only allow one revision for this demo
            if state.revision_count >= 2 {
                state.should_revise = false;
                state.final_text = format!("[FINAL] {}", state.draft);
                println!("  ✅ Draft approved!");
            } else {
                state.should_revise = true;
                println!("  ⚠️  Needs revision (attempt {})", state.revision_count);
            }

            Ok(state)
        })
    });

    // Set up the workflow
    graph.set_entry_point("research");
    graph.add_edge("research", "write_draft");

    // Conditional edge: review decides to revise or finish
    let mut routes = std::collections::HashMap::new();
    routes.insert("revise".to_string(), "write_draft".to_string());
    routes.insert("finish".to_string(), END.to_string());

    graph.add_conditional_edges(
        "review",
        |state: &ResearchState| {
            if state.should_revise {
                "revise".to_string()
            } else {
                "finish".to_string()
            }
        },
        routes,
    );

    graph.add_edge("write_draft", "review");

    graph
}

#[tokio::main]
async fn main() -> dashflow::error::Result<()> {
    println!("=== DashFlow Event System Example ===\n");

    // Part 1: Using PrintCallback
    println!("--- Part 1: Using PrintCallback ---\n");

    let graph = build_research_graph();

    let app = graph.compile()?.with_callback(PrintCallback);

    let initial_state = ResearchState::new("Rust programming");
    let result = app
        .invoke(initial_state)
        .await
        ?;

    println!("\n📊 Execution Summary:");
    println!("   Nodes executed: {}", result.execution_path().len());
    println!("   Path: {}", result.execution_path().join(" -> "));
    println!("   Revision count: {}", result.state().revision_count);

    // Part 2: Using CollectingCallback for analysis
    println!("\n\n--- Part 2: Using CollectingCallback for Analysis ---\n");

    let graph2 = build_research_graph();
    let callback = CollectingCallback::new();
    let callback_clone = callback.shared_clone();

    let app2 = graph2.compile()?.with_callback(callback);

    let initial_state2 = ResearchState::new("Graph databases");
    let _result2 = app2
        .invoke(initial_state2)
        .await
        ?;

    println!("\n📈 Event Analysis:");
    let events = callback_clone.events();
    println!("   Total events: {}", events.len());

    let mut total_node_time = std::time::Duration::ZERO;
    let mut node_times: Vec<(String, std::time::Duration)> = Vec::new();

    for event in &events {
        match event {
            GraphEvent::GraphStart { .. } => {
                println!("\n   Event sequence:");
                println!("   1. GraphStart");
            }
            GraphEvent::NodeEnd { node, duration, .. } => {
                total_node_time += *duration;
                node_times.push((node.clone(), *duration));
                println!("   - NodeEnd: {} ({:?})", node, duration);
            }
            GraphEvent::EdgeTraversal {
                from,
                to,
                edge_type,
                ..
            } => {
                let edge_desc = match edge_type {
                    EdgeType::Simple => "simple".to_string(),
                    EdgeType::Conditional { condition_result } => {
                        format!("conditional [{}]", condition_result)
                    }
                    EdgeType::Parallel => "parallel".to_string(),
                };
                println!(
                    "   - EdgeTraversal: {} -> {} ({})",
                    from,
                    to.join(", "),
                    edge_desc
                );
            }
            GraphEvent::GraphEnd { duration, .. } => {
                println!("   - GraphEnd (total: {:?})", duration);
            }
            _ => {}
        }
    }

    println!("\n   Node execution times:");
    for (node, duration) in node_times {
        let percentage = (duration.as_millis() as f64 / total_node_time.as_millis() as f64) * 100.0;
        println!("   - {}: {:?} ({:.1}%)", node, duration, percentage);
    }

    println!("\n✅ Example complete!");
    Ok(())
}
