//! Basic DashFlow Example - Multi-Agent Research Workflow
//!
//! This example demonstrates a simple multi-agent graph where agents
//! research a topic, write content, and review it for quality.
//!
//! Run with:
//! ```bash
//! cargo run --package dashflow --example basic_graph
//! ```

use dashflow::MergeableState;
use dashflow::StateGraph;
use serde::{Deserialize, Serialize};

const END: &str = "__end__";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResearchState {
    topic: String,
    research_notes: String,
    draft: String,
    review_passed: bool,
    iteration: u32,
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
        if !other.research_notes.is_empty() {
            if self.research_notes.is_empty() {
                self.research_notes = other.research_notes.clone();
            } else {
                self.research_notes.push('\n');
                self.research_notes.push_str(&other.research_notes);
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
        self.review_passed = self.review_passed || other.review_passed;
        self.iteration = self.iteration.max(other.iteration);
    }
}

impl ResearchState {
    fn new(topic: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            research_notes: String::new(),
            draft: String::new(),
            review_passed: false,
            iteration: 0,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("DashFlow - Multi-Agent Research Workflow\n");
    println!("{}", "=".repeat(60));

    // Build the graph
    let mut graph: StateGraph<ResearchState> = StateGraph::new();

    // Node 1: Research Agent
    graph.add_node_from_fn("researcher", |mut state| {
        Box::pin(async move {
            println!("\n[RESEARCHER] Researching topic: {}", state.topic);

            // Simulate research
            state.research_notes = format!(
                "Research notes on '{}':\n\
                 - Key concept A: ...\n\
                 - Key concept B: ...\n\
                 - Reference sources: ...",
                state.topic
            );

            println!("[RESEARCHER] Research complete!");
            println!("{}", state.research_notes);

            Ok(state)
        })
    });

    // Node 2: Writer Agent
    graph.add_node_from_fn("writer", |mut state| {
        Box::pin(async move {
            println!("\n[WRITER] Writing draft...");
            state.iteration += 1;

            // Simulate writing
            state.draft = format!(
                "Draft {} - Article on '{}':\n\n\
                 Introduction: This article explores {}.\n\n\
                 Main Content: Based on the research notes, we can conclude...\n\n\
                 Conclusion: In summary, {} is an important topic.",
                state.iteration, state.topic, state.topic, state.topic
            );

            println!("[WRITER] Draft {} complete!", state.iteration);
            println!("{}", state.draft);

            Ok(state)
        })
    });

    // Node 3: Reviewer Agent
    graph.add_node_from_fn("reviewer", |mut state| {
        Box::pin(async move {
            println!("\n[REVIEWER] Reviewing draft {}...", state.iteration);

            // Simulate review (pass after 2 iterations)
            if state.iteration >= 2 {
                state.review_passed = true;
                println!("[REVIEWER] ✓ Review passed! Draft is ready for publication.");
            } else {
                state.review_passed = false;
                println!("[REVIEWER] ✗ Review failed. Needs revision.");
                println!("[REVIEWER] Feedback: Needs more detail and better structure.");
            }

            Ok(state)
        })
    });

    // Define edges
    graph.add_edge("researcher", "writer");

    // Conditional edge: reviewer decides if we're done or need revision
    let mut review_routes = std::collections::HashMap::new();
    review_routes.insert("continue".to_string(), "writer".to_string());
    review_routes.insert("end".to_string(), END.to_string());

    graph.add_conditional_edges(
        "reviewer",
        |state: &ResearchState| {
            if state.review_passed {
                "end".to_string()
            } else {
                "continue".to_string()
            }
        },
        review_routes,
    );

    graph.add_edge("writer", "reviewer");

    // Set entry point
    graph.set_entry_point("researcher");

    // Compile the graph
    println!("\n[GRAPH] Compiling graph...");
    let app = graph.compile()?;
    println!("[GRAPH] Graph compiled successfully!");
    println!("[GRAPH] - Entry point: {}", app.entry_point());
    println!("[GRAPH] - Nodes: {}", app.node_count());
    println!("[GRAPH] - Edges: {}", app.edge_count());

    // Run the graph
    println!("\n[GRAPH] Starting execution...\n");
    println!("{}", "=".repeat(60));

    let initial_state = ResearchState::new("Rust Programming Language");
    let result = app.invoke(initial_state).await?;

    // Print results
    println!("\n");
    println!("{}", "=".repeat(60));
    println!("\n[GRAPH] Execution complete!\n");
    println!("Execution path: {:?}", result.execution_path());
    println!("Total iterations: {}", result.state().iteration);
    println!("\n[FINAL STATE]");
    println!("Topic: {}", result.state().topic);
    println!("Review passed: {}", result.state().review_passed);
    println!("\n[FINAL DRAFT]");
    println!("{}", result.state().draft);

    Ok(())
}
