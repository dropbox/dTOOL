//! Tool-Using Workflow Example
//!
//! Demonstrates how to use DashFlow Tools within a DashFlow workflow.
//! This example shows a research workflow where tools are used to gather
//! information from multiple sources.

use dashflow::core::tools::{Tool, ToolInput};
use dashflow::{MergeableState, StateGraph, ToolNode, END};
use serde::{Deserialize, Serialize};

/// State for our tool-using research workflow
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResearchState {
    query: String,
    search_results: Vec<String>,
    wikipedia_summary: String,
    calculator_result: String,
    final_report: String,
}

impl MergeableState for ResearchState {
    fn merge(&mut self, other: &Self) {
        if !other.query.is_empty() {
            self.query = other.query.clone();
        }
        self.search_results.extend(other.search_results.clone());
        if !other.wikipedia_summary.is_empty() {
            if self.wikipedia_summary.is_empty() {
                self.wikipedia_summary = other.wikipedia_summary.clone();
            } else {
                self.wikipedia_summary.push('\n');
                self.wikipedia_summary.push_str(&other.wikipedia_summary);
            }
        }
        if !other.calculator_result.is_empty() {
            if self.calculator_result.is_empty() {
                self.calculator_result = other.calculator_result.clone();
            } else {
                self.calculator_result.push('\n');
                self.calculator_result.push_str(&other.calculator_result);
            }
        }
        if !other.final_report.is_empty() {
            if self.final_report.is_empty() {
                self.final_report = other.final_report.clone();
            } else {
                self.final_report.push('\n');
                self.final_report.push_str(&other.final_report);
            }
        }
    }
}

/// Mock search tool that simulates web search
struct SearchTool;

#[async_trait::async_trait]
impl Tool for SearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for information on a given topic. Returns a list of relevant results."
    }

    async fn _call(&self, input: ToolInput) -> dashflow::core::Result<String> {
        let query = match input {
            ToolInput::String(s) => s,
            _ => {
                return Err(dashflow::core::Error::Other(
                    "Expected string input".to_string(),
                ))
            }
        };

        println!("🔍 [WEB SEARCH] Searching for: '{}'", query);

        // Simulate search results
        let results = vec![
            format!("Result 1: {} is a popular topic", query),
            format!("Result 2: Recent studies on {}", query),
            format!("Result 3: {} applications and use cases", query),
        ];

        // Return as JSON array string
        Ok(serde_json::to_string(&results)?)
    }
}

/// Mock Wikipedia tool that simulates Wikipedia API
struct WikipediaTool;

#[async_trait::async_trait]
impl Tool for WikipediaTool {
    fn name(&self) -> &str {
        "wikipedia"
    }

    fn description(&self) -> &str {
        "Get a summary from Wikipedia about a topic."
    }

    async fn _call(&self, input: ToolInput) -> dashflow::core::Result<String> {
        let topic = match input {
            ToolInput::String(s) => s,
            _ => {
                return Err(dashflow::core::Error::Other(
                    "Expected string input".to_string(),
                ))
            }
        };

        println!("📚 [WIKIPEDIA] Fetching summary for: '{}'", topic);

        // Simulate Wikipedia summary
        let summary = format!(
            "{} is a significant subject with wide-ranging applications. \
             It has been studied extensively and continues to evolve. \
             Key aspects include its theoretical foundations and practical implementations.",
            topic
        );

        Ok(summary)
    }
}

/// Mock calculator tool for numeric computations
struct CalculatorTool;

#[async_trait::async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Perform basic arithmetic calculations. Input should be a math expression."
    }

    async fn _call(&self, input: ToolInput) -> dashflow::core::Result<String> {
        let expression = match input {
            ToolInput::String(s) => s,
            _ => {
                return Err(dashflow::core::Error::Other(
                    "Expected string input".to_string(),
                ))
            }
        };

        println!("🧮 [CALCULATOR] Calculating: '{}'", expression);

        // Simple mock calculation - in real world would parse and evaluate
        let result = if expression.contains("count") {
            "42" // The answer to everything
        } else {
            "100"
        };

        Ok(format!("Result: {}", result))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tool-Using Research Workflow ===\n");

    // Create tools
    let search_tool = SearchTool;
    let wikipedia_tool = WikipediaTool;
    let calculator_tool = CalculatorTool;

    // Wrap tools as graph nodes
    let search_node = ToolNode::new(
        search_tool,
        |state: ResearchState| ToolInput::String(state.query),
        |mut state, output| {
            // Parse JSON array back to Vec<String>
            if let Ok(results) = serde_json::from_str::<Vec<String>>(&output) {
                state.search_results = results;
            }
            state
        },
    );

    let wikipedia_node = ToolNode::new(
        wikipedia_tool,
        |state: ResearchState| ToolInput::String(state.query),
        |mut state, output| {
            state.wikipedia_summary = output;
            state
        },
    );

    let calculator_node = ToolNode::new(
        calculator_tool,
        |_state: ResearchState| ToolInput::String("count results".to_string()),
        |mut state, output| {
            state.calculator_result = output;
            state
        },
    );

    // Create aggregation node (regular function node)
    let aggregation_node = |state: ResearchState| {
        Box::pin(async move {
            println!("\n📊 [AGGREGATOR] Compiling final report...");

            let report = format!(
                "# Research Report: {}\n\n\
                 ## Web Search Results\n{}\n\n\
                 ## Wikipedia Summary\n{}\n\n\
                 ## Statistics\n{}\n\n\
                 Report compiled successfully.",
                state.query,
                state.search_results.join("\n"),
                state.wikipedia_summary,
                state.calculator_result
            );

            let mut final_state = state;
            final_state.final_report = report;
            Ok(final_state)
        })
            as std::pin::Pin<
                Box<dyn std::future::Future<Output = dashflow::Result<ResearchState>> + Send>,
            >
    };

    // Build the workflow graph
    println!("Building research workflow with tools...\n");
    let mut graph = StateGraph::new();

    // Add tool nodes
    graph.add_node("search", search_node);
    graph.add_node("wikipedia", wikipedia_node);
    graph.add_node("calculator", calculator_node);
    graph.add_node_from_fn("aggregate", aggregation_node);

    // Define workflow: run all tools in sequence, then aggregate
    graph.add_edge("search", "wikipedia");
    graph.add_edge("wikipedia", "calculator");
    graph.add_edge("calculator", "aggregate");
    graph.add_edge("aggregate", END);
    graph.set_entry_point("search");

    // Compile the graph
    let app = graph.compile()?;

    // Execute workflow
    println!("Executing research workflow...\n");
    let initial_state = ResearchState {
        query: "Rust Programming Language".to_string(),
        search_results: vec![],
        wikipedia_summary: String::new(),
        calculator_result: String::new(),
        final_report: String::new(),
    };

    let result = app.invoke(initial_state).await?;

    // Display final report
    println!("\n{}", "=".repeat(60));
    println!("FINAL REPORT");
    println!("{}\n", "=".repeat(60));
    println!("{}", result.final_state.final_report);

    println!("\n✅ Research workflow complete!");

    Ok(())
}
