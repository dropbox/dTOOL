//! Quickstart Research Workflow - Multi-Agent Example
//!
//! This example demonstrates:
//! - Building a StateGraph with 3 agents
//! - Sequential execution (planner → researcher → writer)
//! - State that flows between nodes
//! - Real-world application pattern
//!
//! Run: cargo run -p dashflow --example quickstart_research_workflow

use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow::{Error, MergeableState, Result, StateGraph, END};
use dashflow_openai::ChatOpenAI;
use serde::{Deserialize, Serialize};

// Define the state that flows through the workflow
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResearchState {
    topic: String,
    outline: String,
    research_notes: Vec<String>,
    final_report: String,
}

impl MergeableState for ResearchState {
    fn merge(&mut self, other: &Self) {
        // Keep topic unchanged
        if !other.outline.is_empty() {
            self.outline = other.outline.clone();
        }
        self.research_notes.extend(other.research_notes.clone());
        if !other.final_report.is_empty() {
            self.final_report = other.final_report.clone();
        }
    }
}

// Agent 1: Creates research outline
async fn planner_node(mut state: ResearchState) -> Result<ResearchState> {
    let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
    let prompt = format!("Create a 3-point outline for: {}", state.topic);
    let messages = vec![
        Message::system("You create research outlines."),
        Message::human(prompt),
    ];

    match llm.generate(&messages, None, None, None, None).await {
        Ok(result) => {
            state.outline = result.generations[0].message.as_text();
            println!("📝 Planner: Created outline");
            Ok(state)
        }
        Err(e) => {
            eprintln!("Error in planner: {}", e);
            Err(Error::Generic(format!("Planner failed: {}", e)))
        }
    }
}

// Agent 2: Gathers research for each point
async fn researcher_node(mut state: ResearchState) -> Result<ResearchState> {
    let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
    let prompt = format!(
        "Research these points:\n{}\n\nProvide 3 key insights.",
        state.outline
    );
    let messages = vec![
        Message::system("You gather key facts and insights."),
        Message::human(prompt),
    ];

    match llm.generate(&messages, None, None, None, None).await {
        Ok(result) => {
            state
                .research_notes
                .push(result.generations[0].message.as_text());
            println!("🔍 Researcher: Gathered insights");
            Ok(state)
        }
        Err(e) => {
            eprintln!("Error in researcher: {}", e);
            Err(Error::Generic(format!("Researcher failed: {}", e)))
        }
    }
}

// Agent 3: Writes final report
async fn writer_node(mut state: ResearchState) -> Result<ResearchState> {
    let llm = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");
    let prompt = format!(
        "Write a report on: {}\n\nOutline:\n{}\n\nResearch:\n{}",
        state.topic,
        state.outline,
        state.research_notes.join("\n\n")
    );
    let messages = vec![
        Message::system("You write concise, well-structured reports."),
        Message::human(prompt),
    ];

    match llm.generate(&messages, None, None, None, None).await {
        Ok(result) => {
            state.final_report = result.generations[0].message.as_text();
            println!("✍️  Writer: Completed report");
            Ok(state)
        }
        Err(e) => {
            eprintln!("Error in writer: {}", e);
            Err(Error::Generic(format!("Writer failed: {}", e)))
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting Research Workflow\n");

    // Build the workflow graph
    let mut graph = StateGraph::new();

    // Add three agents as nodes
    graph.add_node_from_fn("planner", |state| Box::pin(planner_node(state)));
    graph.add_node_from_fn("researcher", |state| Box::pin(researcher_node(state)));
    graph.add_node_from_fn("writer", |state| Box::pin(writer_node(state)));

    // Define the workflow: planner → researcher → writer → end
    graph.set_entry_point("planner");
    graph.add_edge("planner", "researcher");
    graph.add_edge("researcher", "writer");
    graph.add_edge("writer", END);

    // Compile the graph into an executable app
    let app = graph.compile()?;

    // Run the workflow
    let initial_state = ResearchState {
        topic: "Artificial Intelligence in Healthcare".to_string(),
        outline: String::new(),
        research_notes: vec![],
        final_report: String::new(),
    };

    let result = app.invoke(initial_state).await?;

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📄 FINAL REPORT\n");
    println!("{}", result.final_state.final_report);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    Ok(())
}
