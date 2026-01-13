//! Multi-Agent Research System Example
//!
//! This comprehensive example demonstrates DashFlow's key features:
//! - Conditional routing (supervisor decides which agent to call)
//! - Parallel execution (multiple researchers work concurrently)
//! - Event callbacks (monitor execution)
//! - Streaming (real-time progress updates)
//!
//! Architecture: Supervisor agent coordinates specialist agents
//! - Researcher agents gather information (parallel)
//! - Analyst synthesizes findings
//! - Writer produces final report
//! - Reviewer checks quality (conditional loop back if needed)
//!
//! Run: cargo run --example multi_agent_research

use dashflow::{CollectingCallback, MergeableState, StateGraph, StreamEvent, StreamMode, END};
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
struct ResearchState {
    query: String,
    research_plan: Vec<String>,
    research_results: HashMap<String, String>,
    analysis: String,
    draft: String,
    revision_notes: Vec<String>,
    final_report: String,
    next_action: String,
    iteration: u32,
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
        self.research_plan.extend(other.research_plan.clone());
        self.research_results.extend(other.research_results.clone());
        if !other.analysis.is_empty() {
            if self.analysis.is_empty() {
                self.analysis = other.analysis.clone();
            } else {
                self.analysis.push('\n');
                self.analysis.push_str(&other.analysis);
            }
        }
        if !other.draft.is_empty() {
            if self.draft.is_empty() {
                self.draft = other.draft.clone();
            } else {
                self.draft.push('\n');
                self.draft.push_str(&other.draft);
            }
        }
        self.revision_notes.extend(other.revision_notes.clone());
        if !other.final_report.is_empty() {
            if self.final_report.is_empty() {
                self.final_report = other.final_report.clone();
            } else {
                self.final_report.push('\n');
                self.final_report.push_str(&other.final_report);
            }
        }
        if !other.next_action.is_empty() {
            if self.next_action.is_empty() {
                self.next_action = other.next_action.clone();
            } else {
                self.next_action.push('\n');
                self.next_action.push_str(&other.next_action);
            }
        }
        self.iteration = self.iteration.max(other.iteration);
    }
}

impl ResearchState {
    fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            research_plan: Vec::new(),
            research_results: HashMap::new(),
            analysis: String::new(),
            draft: String::new(),
            revision_notes: Vec::new(),
            final_report: String::new(),
            next_action: String::new(),
            iteration: 0,
        }
    }
}

fn build_research_graph() -> StateGraph<ResearchState> {
    let mut graph = StateGraph::new();

    // Node 1: Supervisor - Creates research plan
    graph.add_node_from_fn("supervisor", |mut state: ResearchState| {
        Box::pin(async move {
            println!("  🎯 Supervisor: Planning research for '{}'", state.query);
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

            state.research_plan = vec![
                "technical_aspects".to_string(),
                "market_trends".to_string(),
                "user_feedback".to_string(),
            ];

            println!(
                "  🎯 Supervisor: Research plan created - {} topics",
                state.research_plan.len()
            );
            Ok(state)
        })
    });

    // Node 2-4: Parallel researchers
    graph.add_node_from_fn("researcher_technical", |mut state: ResearchState| {
        Box::pin(async move {
            println!("  🔬 Technical Researcher: Investigating...");
            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

            state.research_results.insert(
                "technical_aspects".to_string(),
                format!("Technical analysis of {}: High performance, modern architecture, growing adoption", state.query),
            );

            println!("  🔬 Technical Researcher: Complete");
            Ok(state)
        })
    });

    graph.add_node_from_fn("researcher_market", |mut state: ResearchState| {
        Box::pin(async move {
            println!("  📈 Market Researcher: Analyzing trends...");
            tokio::time::sleep(tokio::time::Duration::from_millis(700)).await;

            state.research_results.insert(
                "market_trends".to_string(),
                format!(
                    "Market trends for {}: Strong growth, high demand, competitive landscape",
                    state.query
                ),
            );

            println!("  📈 Market Researcher: Complete");
            Ok(state)
        })
    });

    graph.add_node_from_fn("researcher_user", |mut state: ResearchState| {
        Box::pin(async move {
            println!("  👥 User Researcher: Gathering feedback...");
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            state.research_results.insert(
                "user_feedback".to_string(),
                format!(
                    "User feedback on {}: Positive reception, requested features, pain points",
                    state.query
                ),
            );

            println!("  👥 User Researcher: Complete");
            Ok(state)
        })
    });

    // Node 5: Analyst - Synthesizes research
    graph.add_node_from_fn("analyst", |mut state: ResearchState| {
        Box::pin(async move {
            println!("  🧠 Analyst: Synthesizing findings...");
            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

            let mut analysis_points = Vec::new();
            for (topic, result) in &state.research_results {
                analysis_points.push(format!("- {}: {}", topic, result));
            }

            state.analysis = format!(
                "Analysis of {}:\n{}",
                state.query,
                analysis_points.join("\n")
            );

            println!(
                "  🧠 Analyst: Synthesized {} research findings",
                state.research_results.len()
            );
            Ok(state)
        })
    });

    // Node 6: Writer - Creates draft
    graph.add_node_from_fn("writer", |mut state: ResearchState| {
        Box::pin(async move {
            state.iteration += 1;
            println!("  ✍️  Writer: Drafting report (iteration {})...", state.iteration);
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            state.draft = if state.iteration == 1 {
                format!(
                    "DRAFT REPORT: {}\n\nExecutive Summary:\n{}\n\nConclusion: Needs review",
                    state.query, state.analysis
                )
            } else {
                format!(
                    "REVISED REPORT: {}\n\nExecutive Summary:\n{}\n\nRevisions:\n{}\n\nConclusion: Ready",
                    state.query,
                    state.analysis,
                    state.revision_notes.join("\n")
                )
            };

            println!("  ✍️  Writer: Draft complete");
            Ok(state)
        })
    });

    // Node 7: Reviewer - Quality check (conditional routing)
    graph.add_node_from_fn("reviewer", |mut state: ResearchState| {
        Box::pin(async move {
            println!("  🔍 Reviewer: Evaluating quality...");
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

            if state.iteration < 2 {
                state.next_action = "revise".to_string();
                state
                    .revision_notes
                    .push("Add more specific examples".to_string());
                state.revision_notes.push("Expand conclusion".to_string());
                println!(
                    "  🔍 Reviewer: Revision needed (iteration {})",
                    state.iteration
                );
            } else {
                state.next_action = "approve".to_string();
                state.final_report = state.draft.clone();
                println!("  🔍 Reviewer: Approved!");
            }

            Ok(state)
        })
    });

    // Build the workflow graph
    graph.set_entry_point("supervisor");

    // Supervisor → Parallel Researchers
    graph.add_parallel_edges(
        "supervisor",
        vec![
            "researcher_technical".to_string(),
            "researcher_market".to_string(),
            "researcher_user".to_string(),
        ],
    );

    // All researchers → Analyst (using last researcher's edge)
    graph.add_edge("researcher_user", "analyst");

    // Analyst → Writer
    graph.add_edge("analyst", "writer");

    // Writer → Reviewer
    graph.add_edge("writer", "reviewer");

    // Reviewer → Writer (revise) OR END (approve) - Conditional
    let mut routes = HashMap::new();
    routes.insert("revise".to_string(), "writer".to_string());
    routes.insert("approve".to_string(), END.to_string());

    graph.add_conditional_edges(
        "reviewer",
        |state: &ResearchState| state.next_action.clone(),
        routes,
    );

    graph
}

#[tokio::main]
async fn main() -> dashflow::error::Result<()> {
    println!("=== Multi-Agent Research System ===\n");
    println!("This example demonstrates:");
    println!("- Parallel execution (3 researchers)");
    println!("- Conditional routing (reviewer loop)");
    println!("- Event callbacks (execution monitoring)");
    println!("- Streaming (real-time updates)\n");

    let graph = build_research_graph();

    // Part 1: Use events callback for monitoring
    println!("--- Part 1: With Event Callbacks ---\n");

    let callback = CollectingCallback::new();
    let callback_clone = callback.shared_clone();

    let app = graph.compile()?.with_callback(callback);

    let initial_state = ResearchState::new("Rust async programming");

    println!("▶️  Starting research workflow...\n");

    let result = app.invoke(initial_state).await?;

    println!("\n📊 Execution Summary:");
    println!("   Total nodes executed: {}", result.execution_path().len());
    println!("   Execution path: {}", result.execution_path().join(" → "));
    println!("   Iterations: {}", result.state().iteration);

    let events = callback_clone.events();
    let parallel_events: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                dashflow::GraphEvent::ParallelStart { .. }
                    | dashflow::GraphEvent::ParallelEnd { .. }
            )
        })
        .collect();

    println!("   Parallel execution events: {}", parallel_events.len());

    // Part 2: Use streaming for real-time updates
    println!("\n\n--- Part 2: With Streaming ---\n");

    let graph2 = build_research_graph();
    let app2 = graph2.compile()?;

    let initial_state2 = ResearchState::new("Machine learning frameworks");
    let mut stream = Box::pin(app2.stream(initial_state2, StreamMode::Values));

    println!("▶️  Starting streaming workflow...\n");

    let mut nodes_completed = 0;

    while let Some(event_result) = stream.next().await {
        let event = event_result?;

        match event {
            StreamEvent::Values { node, state } => {
                nodes_completed += 1;
                println!("✓ Node {} completed ({} total)", node, nodes_completed);
                if !state.draft.is_empty() {
                    println!(
                        "  Draft preview: {}...",
                        state.draft.lines().next().unwrap_or("")
                    );
                }
            }
            StreamEvent::Done {
                state,
                execution_path,
            } => {
                println!("\n🎉 Research Complete!");
                println!("   Path: {}", execution_path.join(" → "));
                println!("   Total iterations: {}", state.iteration);
                println!("\n📄 Final Report:");
                println!("{}", "-".repeat(60));
                for line in state.final_report.lines().take(5) {
                    println!("{}", line);
                }
                if state.final_report.lines().count() > 5 {
                    println!("...");
                }
                println!("{}", "-".repeat(60));
            }
            _ => {}
        }
    }

    println!("\n=== Example Complete ===");
    println!("\nThis example combined:");
    println!("✓ Parallel researchers (concurrent execution)");
    println!("✓ Conditional routing (quality loop)");
    println!("✓ Event monitoring (observability)");
    println!("✓ Streaming (real-time updates)");
    println!("\nDashFlow enables complex multi-agent workflows in Rust!");
    Ok(())
}
