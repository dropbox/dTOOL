//! DashFlow Integration Example
//!
//! Demonstrates how to use DashFlow Agents and Chains as DashFlow nodes.
//! This enables combining the power of DashFlow's agent system with
//! DashFlow's graph-based workflow orchestration.

use dashflow::core::config::RunnableConfig;
use dashflow::core::runnable::Runnable;
use dashflow::{AgentNode, MergeableState, StateGraph, END};
use serde::{Deserialize, Serialize};

/// State for our multi-step workflow
#[derive(Clone, Debug, Serialize, Deserialize)]
struct WorkflowState {
    input: String,
    research_notes: Vec<String>,
    draft: String,
    feedback: String,
    final_output: String,
}

impl MergeableState for WorkflowState {
    fn merge(&mut self, other: &Self) {
        if !other.input.is_empty() {
            self.input = other.input.clone();
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
        if !other.feedback.is_empty() {
            if self.feedback.is_empty() {
                self.feedback = other.feedback.clone();
            } else {
                self.feedback.push('\n');
                self.feedback.push_str(&other.feedback);
            }
        }
        if !other.final_output.is_empty() {
            if self.final_output.is_empty() {
                self.final_output = other.final_output.clone();
            } else {
                self.final_output.push('\n');
                self.final_output.push_str(&other.final_output);
            }
        }
    }
}

/// Mock research agent that gathers information
struct ResearchAgent;

#[async_trait::async_trait]
impl Runnable for ResearchAgent {
    type Input = String;
    type Output = Vec<String>;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> dashflow::core::Result<Self::Output> {
        println!("🔍 Research Agent: Researching '{}'...", input);

        // Simulate research
        let notes = vec![
            format!("Key concept: {} is important", input),
            format!("History: {} has evolved over time", input),
            format!("Current state: {} is widely used", input),
        ];

        Ok(notes)
    }
}

/// Mock writing chain that creates content
struct WritingChain;

#[async_trait::async_trait]
impl Runnable for WritingChain {
    type Input = (String, Vec<String>);
    type Output = String;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> dashflow::core::Result<Self::Output> {
        let (topic, notes) = input;
        println!("✍️  Writing Chain: Creating draft about '{}'...", topic);

        let draft = format!("Draft about {}:\n\n{}", topic, notes.join("\n"));

        Ok(draft)
    }
}

/// Mock review agent that provides feedback
struct ReviewAgent;

#[async_trait::async_trait]
impl Runnable for ReviewAgent {
    type Input = String;
    type Output = String;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> dashflow::core::Result<Self::Output> {
        println!("👀 Review Agent: Reviewing draft...");

        let feedback = if input.len() > 100 {
            "Draft looks good! Ready to finalize."
        } else {
            "Draft needs more detail. Consider expanding."
        };

        Ok(feedback.to_string())
    }
}

/// Mock finalization chain
struct FinalizeChain;

#[async_trait::async_trait]
impl Runnable for FinalizeChain {
    type Input = (String, String);
    type Output = String;

    async fn invoke(
        &self,
        input: Self::Input,
        _config: Option<RunnableConfig>,
    ) -> dashflow::core::Result<Self::Output> {
        let (draft, feedback) = input;
        println!("✅ Finalize Chain: Creating final version...");

        let final_output = format!(
            "{}\n\nReviewer Feedback: {}\n\n[FINALIZED]",
            draft, feedback
        );

        Ok(final_output)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== DashFlow Integration with DashFlow ===\n");

    // Create DashFlow agents and chains
    let research_agent = ResearchAgent;
    let writing_chain = WritingChain;
    let review_agent = ReviewAgent;
    let finalize_chain = FinalizeChain;

    // Wrap agents/chains as DashFlow nodes using AgentNode
    let research_node = AgentNode::new(
        "research",
        research_agent,
        |state: WorkflowState| state.input,
        |mut state, notes| {
            state.research_notes = notes;
            state
        },
    );

    let writing_node = AgentNode::new(
        "write",
        writing_chain,
        |state: WorkflowState| (state.input, state.research_notes),
        |mut state, draft| {
            state.draft = draft;
            state
        },
    );

    let review_node = AgentNode::new(
        "review",
        review_agent,
        |state: WorkflowState| state.draft,
        |mut state, feedback| {
            state.feedback = feedback;
            state
        },
    );

    let finalize_node = AgentNode::new(
        "finalize",
        finalize_chain,
        |state: WorkflowState| (state.draft, state.feedback),
        |mut state, final_output| {
            state.final_output = final_output;
            state
        },
    );

    // Build the workflow graph
    println!("Building workflow graph...\n");
    let mut graph = StateGraph::new();

    // Add nodes
    graph.add_node("research", research_node);
    graph.add_node("write", writing_node);
    graph.add_node("review", review_node);
    graph.add_node("finalize", finalize_node);

    // Define workflow edges
    graph.add_edge("research", "write");
    graph.add_edge("write", "review");
    graph.add_edge("review", "finalize");
    graph.add_edge("finalize", END);
    graph.set_entry_point("research");

    // Compile the graph
    let app = graph.compile()?;

    // Execute workflow
    println!("Executing workflow...\n");
    let initial_state = WorkflowState {
        input: "Rust Programming".to_string(),
        research_notes: vec![],
        draft: String::new(),
        feedback: String::new(),
        final_output: String::new(),
    };

    let result = app.invoke(initial_state).await?;

    // Display results
    println!("\n=== Workflow Complete ===\n");
    println!("Topic: {}", result.final_state.input);
    println!("\nResearch Notes:");
    for note in &result.final_state.research_notes {
        println!("  - {}", note);
    }
    println!("\nFeedback: {}", result.final_state.feedback);
    println!("\nFinal Output:\n{}", result.final_state.final_output);

    Ok(())
}
