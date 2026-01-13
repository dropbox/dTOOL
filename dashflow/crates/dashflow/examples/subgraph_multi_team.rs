//! Multi-Team Collaboration with Subgraphs
//!
//! This example demonstrates how to use subgraphs to build modular, reusable workflows.
//! We model a project management system where different specialized teams (Research, Engineering, QA)
//! work independently, each with their own workflow and state, then integrate back into the main project.
//!
//! # Key Concepts
//!
//! - **Subgraphs**: Each team is a separate graph with its own state type
//! - **State Mapping**: Convert between parent project state and team-specific state
//! - **Modularity**: Teams can be tested and developed independently
//! - **Composition**: Multiple subgraphs coordinate in a larger workflow
//!
//! # Usage
//!
//! ```bash
//! cargo run --example subgraph_multi_team
//! ```

use dashflow::{MergeableState, Result, StateGraph, END};
use serde::{Deserialize, Serialize};

// ============================================================================
// Main Project State
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProjectState {
    /// Project requirements
    requirements: String,
    /// Research team findings
    research_output: Option<ResearchOutput>,
    /// Engineering team implementation
    engineering_output: Option<EngineeringOutput>,
    /// QA team test results
    qa_output: Option<QAOutput>,
    /// Final project status
    status: String,
}

impl MergeableState for ProjectState {
    fn merge(&mut self, other: &Self) {
        if !other.requirements.is_empty() {
            if self.requirements.is_empty() {
                self.requirements = other.requirements.clone();
            } else {
                self.requirements.push('\n');
                self.requirements.push_str(&other.requirements);
            }
        }
        if other.research_output.is_some() {
            self.research_output = other.research_output.clone();
        }
        if other.engineering_output.is_some() {
            self.engineering_output = other.engineering_output.clone();
        }
        if other.qa_output.is_some() {
            self.qa_output = other.qa_output.clone();
        }
        if !other.status.is_empty() {
            if self.status.is_empty() {
                self.status = other.status.clone();
            } else {
                self.status.push('\n');
                self.status.push_str(&other.status);
            }
        }
    }
}

// ============================================================================
// Research Team Subgraph
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResearchState {
    query: String,
    findings: Vec<String>,
    summary: String,
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
        self.findings.extend(other.findings.clone());
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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResearchOutput {
    findings: Vec<String>,
    summary: String,
}

async fn research_gather_data(state: ResearchState) -> Result<ResearchState> {
    println!("📚 Research Team: Gathering data for '{}'", state.query);
    let mut state = state;
    state.findings.push("Market analysis complete".to_string());
    state.findings.push("User research conducted".to_string());
    state.findings.push("Competitive analysis done".to_string());
    Ok(state)
}

async fn research_analyze(state: ResearchState) -> Result<ResearchState> {
    println!("📚 Research Team: Analyzing findings");
    let mut state = state;
    state.summary = format!(
        "Research complete: {} insights gathered",
        state.findings.len()
    );
    Ok(state)
}

fn create_research_subgraph() -> StateGraph<ResearchState> {
    let mut graph = StateGraph::new();

    graph.add_node_from_fn("gather", |state| Box::pin(research_gather_data(state)));
    graph.add_node_from_fn("analyze", |state| Box::pin(research_analyze(state)));

    graph.add_edge("gather", "analyze");
    graph.add_edge("analyze", END);
    graph.set_entry_point("gather");

    graph
}

// ============================================================================
// Engineering Team Subgraph
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EngineeringState {
    requirements: String,
    design: String,
    implementation_status: String,
}

impl MergeableState for EngineeringState {
    fn merge(&mut self, other: &Self) {
        if !other.requirements.is_empty() {
            if self.requirements.is_empty() {
                self.requirements = other.requirements.clone();
            } else {
                self.requirements.push('\n');
                self.requirements.push_str(&other.requirements);
            }
        }
        if !other.design.is_empty() {
            if self.design.is_empty() {
                self.design = other.design.clone();
            } else {
                self.design.push('\n');
                self.design.push_str(&other.design);
            }
        }
        if !other.implementation_status.is_empty() {
            if self.implementation_status.is_empty() {
                self.implementation_status = other.implementation_status.clone();
            } else {
                self.implementation_status.push('\n');
                self.implementation_status
                    .push_str(&other.implementation_status);
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EngineeringOutput {
    design: String,
    implementation_status: String,
}

async fn engineering_design(state: EngineeringState) -> Result<EngineeringState> {
    println!(
        "🛠️  Engineering Team: Designing solution for '{}'",
        state.requirements
    );
    let mut state = state;
    state.design =
        "Architecture designed: Microservices with event-driven communication".to_string();
    Ok(state)
}

async fn engineering_implement(state: EngineeringState) -> Result<EngineeringState> {
    println!("🛠️  Engineering Team: Implementing solution");
    let mut state = state;
    state.implementation_status = "Implementation complete: All features working".to_string();
    Ok(state)
}

fn create_engineering_subgraph() -> StateGraph<EngineeringState> {
    let mut graph = StateGraph::new();

    graph.add_node_from_fn("design", |state| Box::pin(engineering_design(state)));
    graph.add_node_from_fn("implement", |state| Box::pin(engineering_implement(state)));

    graph.add_edge("design", "implement");
    graph.add_edge("implement", END);
    graph.set_entry_point("design");

    graph
}

// ============================================================================
// QA Team Subgraph
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
struct QAState {
    features_to_test: String,
    test_results: Vec<String>,
    quality_score: i32,
}

impl MergeableState for QAState {
    fn merge(&mut self, other: &Self) {
        if !other.features_to_test.is_empty() {
            if self.features_to_test.is_empty() {
                self.features_to_test = other.features_to_test.clone();
            } else {
                self.features_to_test.push('\n');
                self.features_to_test.push_str(&other.features_to_test);
            }
        }
        self.test_results.extend(other.test_results.clone());
        self.quality_score = self.quality_score.max(other.quality_score);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct QAOutput {
    test_results: Vec<String>,
    quality_score: i32,
}

async fn qa_test_features(state: QAState) -> Result<QAState> {
    println!("🧪 QA Team: Testing features");
    let mut state = state;
    state
        .test_results
        .push("Unit tests: 100% passing".to_string());
    state
        .test_results
        .push("Integration tests: 95% passing".to_string());
    state
        .test_results
        .push("Performance tests: Passed".to_string());
    Ok(state)
}

async fn qa_evaluate(state: QAState) -> Result<QAState> {
    println!("🧪 QA Team: Evaluating quality");
    let mut state = state;
    state.quality_score = 92; // Based on test results
    Ok(state)
}

fn create_qa_subgraph() -> StateGraph<QAState> {
    let mut graph = StateGraph::new();

    graph.add_node_from_fn("test", |state| Box::pin(qa_test_features(state)));
    graph.add_node_from_fn("evaluate", |state| Box::pin(qa_evaluate(state)));

    graph.add_edge("test", "evaluate");
    graph.add_edge("evaluate", END);
    graph.set_entry_point("test");

    graph
}

// ============================================================================
// Main Project Orchestration
// ============================================================================

async fn initialize_project(state: ProjectState) -> Result<ProjectState> {
    println!("\n🚀 Initializing project: {}\n", state.requirements);
    let mut state = state;
    state.status = "Initialized".to_string();
    Ok(state)
}

async fn finalize_project(state: ProjectState) -> Result<ProjectState> {
    println!("\n✅ Project Complete!\n");
    println!("Research: {:?}", state.research_output);
    println!("Engineering: {:?}", state.engineering_output);
    println!("QA Score: {:?}\n", state.qa_output);

    let mut state = state;
    state.status = format!(
        "Complete - Quality Score: {}",
        state
            .qa_output
            .as_ref()
            .map(|qa| qa.quality_score)
            .unwrap_or(0)
    );
    Ok(state)
}

fn create_main_graph() -> Result<StateGraph<ProjectState>> {
    let mut graph = StateGraph::new();

    // Add initialization node
    graph.add_node_from_fn("init", |state| Box::pin(initialize_project(state)));

    // Add research subgraph
    graph.add_subgraph_with_mapping(
        "research_team",
        create_research_subgraph(),
        |project: &ProjectState| ResearchState {
            query: project.requirements.clone(),
            findings: Vec::new(),
            summary: String::new(),
        },
        |project: ProjectState, research: ResearchState| ProjectState {
            research_output: Some(ResearchOutput {
                findings: research.findings,
                summary: research.summary,
            }),
            ..project
        },
    )?;

    // Add engineering subgraph
    graph.add_subgraph_with_mapping(
        "engineering_team",
        create_engineering_subgraph(),
        |project: &ProjectState| EngineeringState {
            requirements: project.requirements.clone(),
            design: String::new(),
            implementation_status: String::new(),
        },
        |project: ProjectState, eng: EngineeringState| ProjectState {
            engineering_output: Some(EngineeringOutput {
                design: eng.design,
                implementation_status: eng.implementation_status,
            }),
            ..project
        },
    )?;

    // Add QA subgraph
    graph.add_subgraph_with_mapping(
        "qa_team",
        create_qa_subgraph(),
        |project: &ProjectState| QAState {
            features_to_test: project
                .engineering_output
                .as_ref()
                .map(|e| e.implementation_status.clone())
                .unwrap_or_default(),
            test_results: Vec::new(),
            quality_score: 0,
        },
        |project: ProjectState, qa: QAState| ProjectState {
            qa_output: Some(QAOutput {
                test_results: qa.test_results,
                quality_score: qa.quality_score,
            }),
            ..project
        },
    )?;

    // Add finalization node
    graph.add_node_from_fn("finalize", |state| Box::pin(finalize_project(state)));

    // Connect the workflow: init → research → engineering → qa → finalize
    graph.add_edge("init", "research_team");
    graph.add_edge("research_team", "engineering_team");
    graph.add_edge("engineering_team", "qa_team");
    graph.add_edge("qa_team", "finalize");
    graph.add_edge("finalize", END);

    graph.set_entry_point("init");

    Ok(graph)
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("\n=== Multi-Team Collaboration Example ===\n");
    println!("This example shows how subgraphs enable modular, team-based workflows.\n");
    println!("Each team (Research, Engineering, QA) has its own:");
    println!("  - State type (team-specific data)");
    println!("  - Workflow (nodes and edges)");
    println!("  - Independent execution\n");
    println!("State mapping functions integrate team outputs back into the main project.\n");
    println!("{}", "=".repeat(50));

    // Create main project graph
    let graph = create_main_graph()?;

    // Compile the graph
    let app = graph.compile()?;

    // Initialize project state
    let initial_state = ProjectState {
        requirements: "Build a scalable e-commerce platform".to_string(),
        research_output: None,
        engineering_output: None,
        qa_output: None,
        status: "Pending".to_string(),
    };

    // Execute the workflow
    let result = app.invoke(initial_state).await?;

    // Print final status
    println!("\n=== Final Project Status ===");
    println!("Status: {}", result.final_state.status);
    println!("Nodes executed: {}", result.nodes_executed.len());
    println!("Workflow: {:?}\n", result.nodes_executed);

    Ok(())
}
