use dashflow::{MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct AgentState {
    query: String,
    results: Vec<String>,
    score: f64,
}

impl MergeableState for AgentState {
    fn merge(&mut self, other: &Self) {
        if !other.query.is_empty() {
            if self.query.is_empty() {
                self.query = other.query.clone();
            } else {
                self.query.push('\n');
                self.query.push_str(&other.query);
            }
        }
        self.results.extend(other.results.clone());
        self.score = self.score.max(other.score);
    }
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            score: 0.0,
        }
    }
}

fn quality_check(state: &AgentState) -> String {
    if state.score > 0.8 {
        "done".to_string()
    } else {
        "revise".to_string()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DashFlow Mermaid Visualization Example ===\n");

    // Build a complex graph with multiple edge types
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Add nodes (inline for simplicity)
    graph.add_node_from_fn("research", |mut state| {
        Box::pin(async move {
            state.results.push("Research completed".to_string());
            Ok(state)
        })
    });

    graph.add_node_from_fn("analysis", |mut state| {
        Box::pin(async move {
            state.score = 0.85;
            Ok(state)
        })
    });

    graph.add_node_from_fn("write", |mut state| {
        Box::pin(async move {
            state.results.push("Report written".to_string());
            Ok(state)
        })
    });

    graph.add_node_from_fn("critique", |state| Box::pin(async move { Ok(state) }));

    // Add simple edge
    graph.add_edge("research", "analysis");

    // Add parallel edges (fan-out)
    graph.add_parallel_edges(
        "analysis",
        vec!["write".to_string(), "critique".to_string()],
    );

    // Add conditional edges (routing based on quality)
    graph.add_conditional_edges(
        "write",
        quality_check,
        vec![
            ("done".to_string(), END.to_string()),
            ("revise".to_string(), "research".to_string()),
        ]
        .into_iter()
        .collect(),
    );

    graph.add_edge("critique", END);

    // Set entry point
    graph.set_entry_point("research");

    // Generate Mermaid diagram
    let diagram = graph.to_mermaid();

    println!("Mermaid Diagram:");
    println!("{}", "=".repeat(80));
    println!("{}", diagram);
    println!("{}", "=".repeat(80));

    println!("\nYou can visualize this diagram at:");
    println!("  1. https://mermaid.live - Paste the diagram above");
    println!("  2. GitHub README (use ```mermaid code block)");
    println!("  3. VS Code with Mermaid extension");

    println!("\nDiagram Features:");
    println!("  - --> : Simple edge");
    println!("  - -->|label| : Conditional edge (with condition label)");
    println!("  - ==> : Parallel edge (thick arrow for fan-out)");
    println!("  - ([shape]) : Start/End nodes");
    println!("  - [shape] : Regular nodes");

    // Optionally save to file
    std::fs::write("graph.mmd", &diagram)?;
    println!("\n✓ Diagram saved to graph.mmd");

    Ok(())
}
