//! DashFlow StateGraph definition for the agent workflow
//!
//! This module defines the agent loop as a DashFlow StateGraph:
//! UserInput → Reasoning → ToolSelection → ToolExecution → ResultAnalysis → (loop or complete)
//!
//! ## Graph Registry
//!
//! The agent graph is registered in a global `GraphRegistry` for version tracking
//! and AI self-awareness. Use [`get_graph_registry()`] to query registered graphs.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use dashflow::graph_registry::{GraphRegistry, RegistryMetadata};
use dashflow::introspection::{EdgeManifest, GraphManifest, NodeManifest, NodeType};
use dashflow::{CompiledGraph, StateGraph, END};

use crate::nodes::{
    reasoning::reasoning_node, result_analysis::result_analysis_node,
    tool_execution::tool_execution_node, tool_selection::tool_selection_node,
    user_input::user_input_node,
};
use crate::state::AgentState;
use crate::Result;

/// Global graph registry singleton
static GRAPH_REGISTRY: OnceLock<Arc<RwLock<GraphRegistry>>> = OnceLock::new();

/// Agent graph version - follows semantic versioning
pub const AGENT_GRAPH_VERSION: &str = "0.1.0";

/// Agent graph name for registry
pub const AGENT_GRAPH_NAME: &str = "codex_dashflow_agent";

/// Get the global graph registry
///
/// The registry tracks all compiled graphs and their metadata, enabling:
/// - AI self-awareness (querying available graphs)
/// - Version tracking for deployed agents
/// - Discovery by tags and criteria
///
/// # Example
///
/// ```rust,ignore
/// use codex_dashflow_core::graph::get_graph_registry;
///
/// let registry = get_graph_registry();
/// let graphs = registry.read().unwrap().list_graphs();
/// for entry in graphs {
///     println!("Graph: {} v{}", entry.metadata.name, entry.metadata.version);
/// }
/// ```
pub fn get_graph_registry() -> Arc<RwLock<GraphRegistry>> {
    GRAPH_REGISTRY
        .get_or_init(|| Arc::new(RwLock::new(GraphRegistry::new())))
        .clone()
}

/// Get registry metadata for the agent graph
///
/// Returns the metadata used to register the agent graph, including
/// version, tags, and description.
pub fn get_agent_graph_metadata() -> RegistryMetadata {
    RegistryMetadata::new(AGENT_GRAPH_NAME, AGENT_GRAPH_VERSION)
        .with_description(
            "DashFlow-powered coding agent with 5-node workflow: \
             user_input → reasoning → tool_selection → tool_execution → result_analysis",
        )
        .with_tags(["coding", "agent", "dashflow", "production"])
        .with_author("codex_dashflow")
}

/// Route after reasoning node based on whether there are pending tool calls
fn route_after_reasoning(state: &AgentState) -> String {
    if state.pending_tool_calls.is_empty() {
        "complete".to_string()
    } else {
        "tool_selection".to_string()
    }
}

/// Route after result analysis to either continue the loop or complete
///
/// Audit #33: Check for accumulated tool errors - if all recent tool results
/// failed, we should continue to reasoning to let the LLM react to failures.
///
/// Audit #34: Guard against empty pending tools with InProgress status.
/// If we have no pending tools and no recent tool results, but status is
/// InProgress, route to reasoning to prevent a stall.
fn route_after_analysis(state: &AgentState) -> String {
    if !state.should_continue() {
        "complete".to_string()
    } else if state.has_pending_tool_calls() {
        // More tool calls to process
        "tool_selection".to_string()
    } else {
        // Audit #33: Check if all recent tool results were failures
        // This doesn't change routing, but helps with observability
        // (The LLM needs to see failures to adjust strategy)
        let all_failed =
            !state.tool_results.is_empty() && state.tool_results.iter().all(|r| !r.success);
        if all_failed {
            tracing::warn!(
                session_id = %state.session_id,
                failed_count = state.tool_results.len(),
                "All tool executions failed, continuing to reasoning for error recovery"
            );
        }

        // Audit #34: Guard against empty pending tools with InProgress status
        // If we have tool results, reasoning should process them
        // If we have no tool results and no pending tools, this might be a
        // mid-turn state that needs reasoning to generate new tool calls
        if state.tool_results.is_empty() && state.pending_tool_calls.is_empty() {
            tracing::debug!(
                session_id = %state.session_id,
                "No pending tools or results, routing to reasoning for new plan"
            );
        }

        // Continue reasoning loop
        "reasoning".to_string()
    }
}

/// Build the agent StateGraph (uncompiled) for inspection or mermaid export
fn build_agent_graph_uncompiled() -> StateGraph<AgentState> {
    let mut graph: StateGraph<AgentState> = StateGraph::new();

    // Add nodes
    graph.add_node_from_fn("user_input", user_input_node);
    graph.add_node_from_fn("reasoning", reasoning_node);
    graph.add_node_from_fn("tool_selection", tool_selection_node);
    graph.add_node_from_fn("tool_execution", tool_execution_node);
    graph.add_node_from_fn("result_analysis", result_analysis_node);

    // Set entry point
    graph.set_entry_point("user_input");

    // Define edges
    // UserInput → Reasoning (always)
    graph.add_edge("user_input", "reasoning");

    // Reasoning → ToolSelection or END (conditional)
    let mut reasoning_routes = HashMap::new();
    reasoning_routes.insert("tool_selection".to_string(), "tool_selection".to_string());
    reasoning_routes.insert("complete".to_string(), END.to_string());
    graph.add_conditional_edges("reasoning", route_after_reasoning, reasoning_routes);

    // ToolSelection → ToolExecution (always)
    graph.add_edge("tool_selection", "tool_execution");

    // ToolExecution → ResultAnalysis (always)
    graph.add_edge("tool_execution", "result_analysis");

    // ResultAnalysis → Reasoning, ToolSelection, or END (conditional)
    let mut analysis_routes = HashMap::new();
    analysis_routes.insert("reasoning".to_string(), "reasoning".to_string());
    analysis_routes.insert("tool_selection".to_string(), "tool_selection".to_string());
    analysis_routes.insert("complete".to_string(), END.to_string());
    graph.add_conditional_edges("result_analysis", route_after_analysis, analysis_routes);

    graph
}

/// Export the agent graph as a Mermaid diagram
///
/// # Returns
/// A string containing the Mermaid diagram definition
pub fn get_agent_graph_mermaid() -> String {
    let graph = build_agent_graph_uncompiled();
    graph.to_mermaid()
}

/// Build the agent StateGraph
///
/// The graph implements the following workflow:
/// ```text
/// UserInput → Reasoning → ToolSelection → ToolExecution → ResultAnalysis
///                ↑                                              │
///                └──────────────────────────────────────────────┘
/// ```
///
/// The compiled graph is automatically registered in the global [`GraphRegistry`]
/// with metadata for AI self-awareness and version tracking.
///
/// # Returns
/// A compiled DashFlow graph ready for execution
pub fn build_agent_graph() -> Result<CompiledGraph<AgentState>> {
    let graph = build_agent_graph_uncompiled();

    // Compile the graph
    let compiled = graph
        .compile()
        .map_err(|e| crate::Error::GraphCompilation(e.to_string()))?;

    // Register in the global graph registry for AI self-awareness
    // GraphRegistry uses internal locking, so we only need to check existence first
    let registry = get_graph_registry();
    let registry_guard = registry.read().expect("Registry lock poisoned");
    // Only register if not already registered (avoid duplicate entries on multiple builds)
    if registry_guard.get(AGENT_GRAPH_NAME).is_none() {
        let metadata = get_agent_graph_metadata();
        let manifest = build_agent_graph_manifest();
        registry_guard.register(AGENT_GRAPH_NAME, manifest, metadata);
        tracing::debug!(
            graph = AGENT_GRAPH_NAME,
            version = AGENT_GRAPH_VERSION,
            "Registered agent graph in registry"
        );
    }

    Ok(compiled)
}

/// Build the graph manifest for AI introspection
///
/// This creates a `GraphManifest` that describes the agent's structure,
/// enabling the AI to understand and query its own workflow.
///
/// # Example
///
/// ```rust,ignore
/// let manifest = build_agent_graph_manifest();
///
/// // AI can ask: "What nodes do I have?"
/// for (name, node) in &manifest.nodes {
///     println!("Node: {} - {:?}", name, node.description);
/// }
///
/// // Export as JSON for AI consumption
/// let json = manifest.to_json().unwrap();
/// ```
pub fn build_agent_graph_manifest() -> GraphManifest {
    // Use builder with the correct DashFlow API
    // NodeType variants: Function, Agent, ToolExecutor, Subgraph, Approval, Custom(String)
    GraphManifest::builder()
        .graph_name("codex_dashflow_agent")
        .graph_id("codex-dashflow-v0.1")
        .entry_point("user_input")
        // Define nodes with descriptions for AI understanding
        .add_node(
            "user_input",
            NodeManifest::new("user_input", NodeType::Function)
                .with_description("Processes user input and prepares it for reasoning. Validates state and handles session resume for mid-turn checkpoints."),
        )
        .add_node(
            "reasoning",
            NodeManifest::new("reasoning", NodeType::Agent)
                .with_description("Core LLM reasoning node. Generates responses and decides whether to call tools. Uses the configured model (GPT-4, Claude, etc.) to process the conversation and emit tool calls or final responses.")
                .with_tools(vec!["shell".to_string(), "read_file".to_string(), "write_file".to_string(), "list_dir".to_string(), "search_files".to_string(), "apply_patch".to_string()]),
        )
        .add_node(
            "tool_selection",
            NodeManifest::new("tool_selection", NodeType::Function)
                .with_description("Validates and routes tool calls from reasoning. Checks execution policy, handles approval requirements, and filters forbidden tools."),
        )
        .add_node(
            "tool_execution",
            NodeManifest::new("tool_execution", NodeType::ToolExecutor)
                .with_description("Executes approved tool calls (shell, file operations, MCP tools). Handles sandbox enforcement, timeouts, and output capture."),
        )
        .add_node(
            "result_analysis",
            NodeManifest::new("result_analysis", NodeType::Function)
                .with_description("Analyzes tool execution results and decides next step. Routes back to reasoning for more work, to tool_selection for more tools, or completes the turn."),
        )
        // Define edges
        .add_edge("user_input", EdgeManifest::simple("user_input", "reasoning").with_description("Process user input"))
        .add_edge("reasoning", EdgeManifest::conditional("reasoning", "tool_selection", "has_tool_calls").with_description("When reasoning produces tool calls"))
        .add_edge("reasoning", EdgeManifest::conditional("reasoning", "__end__", "complete").with_description("When reasoning produces final response without tool calls"))
        .add_edge("tool_selection", EdgeManifest::simple("tool_selection", "tool_execution").with_description("Execute approved tools"))
        .add_edge("tool_execution", EdgeManifest::simple("tool_execution", "result_analysis").with_description("Analyze execution results"))
        .add_edge("result_analysis", EdgeManifest::conditional("result_analysis", "reasoning", "continue").with_description("Continue processing with tool results"))
        .add_edge("result_analysis", EdgeManifest::conditional("result_analysis", "tool_selection", "more_tools").with_description("More tool calls pending"))
        .add_edge("result_analysis", EdgeManifest::conditional("result_analysis", "__end__", "complete").with_description("Task complete or max turns reached"))
        .build()
        .expect("Agent graph manifest is statically defined and should always be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mermaid_export() {
        let mermaid = get_agent_graph_mermaid();
        // Verify it's valid mermaid
        assert!(mermaid.starts_with("flowchart TD"));
        assert!(mermaid.contains("user_input"));
        assert!(mermaid.contains("reasoning"));
        assert!(mermaid.contains("tool_selection"));
        assert!(mermaid.contains("tool_execution"));
        assert!(mermaid.contains("result_analysis"));
        // Print for visual inspection
        println!(
            "\n=== AGENT GRAPH MERMAID ===\n{}\n===========================",
            mermaid
        );
    }

    #[test]
    fn test_build_agent_graph() {
        let result = build_agent_graph();
        assert!(result.is_ok());
        let graph = result.unwrap();
        assert_eq!(graph.entry_point(), "user_input");
        assert_eq!(graph.node_count(), 5);
    }

    #[test]
    fn test_route_after_reasoning_no_tools() {
        let state = AgentState::new();
        assert_eq!(route_after_reasoning(&state), "complete");
    }

    #[test]
    fn test_route_after_reasoning_with_tools() {
        use crate::state::ToolCall;
        let mut state = AgentState::new();
        state
            .pending_tool_calls
            .push(ToolCall::new("shell", serde_json::json!({})));
        assert_eq!(route_after_reasoning(&state), "tool_selection");
    }

    #[test]
    fn test_route_after_analysis_complete() {
        use crate::state::CompletionStatus;
        let mut state = AgentState::new();
        state.status = CompletionStatus::Complete;
        assert_eq!(route_after_analysis(&state), "complete");
    }

    #[test]
    fn test_route_after_analysis_turn_limit() {
        use crate::state::CompletionStatus;
        let mut state = AgentState::new();
        state.status = CompletionStatus::InProgress;
        state.max_turns = 5;
        state.turn_count = 5; // At limit
        assert_eq!(route_after_analysis(&state), "complete");
    }

    #[test]
    fn test_route_after_analysis_continue_reasoning() {
        use crate::state::CompletionStatus;
        let mut state = AgentState::new();
        state.status = CompletionStatus::InProgress;
        state.max_turns = 10;
        state.turn_count = 3;
        // No pending tool calls, should continue to reasoning
        assert_eq!(route_after_analysis(&state), "reasoning");
    }

    #[test]
    fn test_route_after_analysis_pending_tools() {
        use crate::state::{CompletionStatus, ToolCall};
        let mut state = AgentState::new();
        state.status = CompletionStatus::InProgress;
        state.max_turns = 10;
        state.turn_count = 3;
        // Add pending tool call
        state
            .pending_tool_calls
            .push(ToolCall::new("shell", serde_json::json!({})));
        assert_eq!(route_after_analysis(&state), "tool_selection");
    }

    #[test]
    fn test_route_after_analysis_error_status() {
        use crate::state::CompletionStatus;
        let mut state = AgentState::new();
        state.status = CompletionStatus::Error("test error".to_string());
        assert_eq!(route_after_analysis(&state), "complete");
    }

    #[test]
    fn test_route_after_analysis_all_tools_failed() {
        use crate::state::{CompletionStatus, ToolResult};
        let mut state = AgentState::new();
        state.status = CompletionStatus::InProgress;
        state.max_turns = 10;
        state.turn_count = 3;
        // Add failed tool results
        state.tool_results.push(ToolResult {
            tool_call_id: "call1".to_string(),
            tool: "shell".to_string(),
            output: "command failed".to_string(),
            success: false,
            duration_ms: 100,
        });
        state.tool_results.push(ToolResult {
            tool_call_id: "call2".to_string(),
            tool: "shell".to_string(),
            output: "another failure".to_string(),
            success: false,
            duration_ms: 50,
        });
        // Should still route to reasoning for error recovery (audit #33)
        assert_eq!(route_after_analysis(&state), "reasoning");
    }

    #[test]
    fn test_route_after_analysis_empty_pending_tools_guard() {
        use crate::state::CompletionStatus;
        let mut state = AgentState::new();
        state.status = CompletionStatus::InProgress;
        state.max_turns = 10;
        state.turn_count = 3;
        // No pending tools and no results (audit #34)
        assert!(state.pending_tool_calls.is_empty());
        assert!(state.tool_results.is_empty());
        // Should route to reasoning
        assert_eq!(route_after_analysis(&state), "reasoning");
    }

    #[test]
    fn test_build_agent_graph_manifest() {
        let manifest = build_agent_graph_manifest();

        // Verify basic structure
        assert_eq!(
            manifest.graph_name,
            Some("codex_dashflow_agent".to_string())
        );
        assert_eq!(manifest.graph_id, Some("codex-dashflow-v0.1".to_string()));
        assert_eq!(manifest.entry_point, "user_input");

        // Verify nodes
        assert_eq!(manifest.nodes.len(), 5);
        assert!(manifest.nodes.contains_key("user_input"));
        assert!(manifest.nodes.contains_key("reasoning"));
        assert!(manifest.nodes.contains_key("tool_selection"));
        assert!(manifest.nodes.contains_key("tool_execution"));
        assert!(manifest.nodes.contains_key("result_analysis"));

        // Verify reasoning node has tools listed
        let reasoning = manifest.nodes.get("reasoning").unwrap();
        assert!(reasoning.tools_available.contains(&"shell".to_string()));
        assert!(reasoning.tools_available.contains(&"read_file".to_string()));

        // Verify edges exist
        assert!(!manifest.edges.is_empty());

        // Verify JSON export works
        let json = manifest.to_json().unwrap();
        assert!(json.contains("codex_dashflow_agent"));
        assert!(json.contains("user_input"));
        assert!(json.contains("reasoning"));

        println!(
            "\n=== AGENT GRAPH MANIFEST JSON ===\n{}\n=================================",
            json
        );
    }

    #[test]
    fn test_graph_manifest_json_export() {
        let manifest = build_agent_graph_manifest();
        let json = manifest.to_json().unwrap();

        // Verify it's valid JSON by parsing it back
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_object());
        assert!(parsed.get("graph_name").is_some());
        assert!(parsed.get("nodes").is_some());
        assert!(parsed.get("edges").is_some());
    }

    // ========================================================================
    // Graph Registry Tests
    // ========================================================================

    #[test]
    fn test_get_graph_registry() {
        let registry = get_graph_registry();
        // Registry should be accessible
        let guard = registry.read().expect("Lock should be available");
        // Initially may be empty or have graphs from other tests
        drop(guard);
    }

    #[test]
    fn test_get_agent_graph_metadata() {
        let metadata = get_agent_graph_metadata();
        assert_eq!(metadata.name, AGENT_GRAPH_NAME);
        assert_eq!(metadata.version, AGENT_GRAPH_VERSION);
        assert!(!metadata.description.is_empty());
        assert!(metadata.tags.contains(&"coding".to_string()));
        assert!(metadata.tags.contains(&"agent".to_string()));
        assert!(metadata.tags.contains(&"dashflow".to_string()));
        assert_eq!(metadata.author, Some("codex_dashflow".to_string()));
    }

    #[test]
    fn test_agent_graph_version_constant() {
        // Version should be semantic versioning format
        let parts: Vec<&str> = AGENT_GRAPH_VERSION.split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "Version should have 3 parts (major.minor.patch)"
        );
        for part in parts {
            part.parse::<u32>()
                .expect("Each version part should be a number");
        }
    }

    #[test]
    fn test_agent_graph_name_constant() {
        // Verify constant has expected value (not just non-empty, which clippy warns about)
        assert_eq!(AGENT_GRAPH_NAME, "codex_dashflow_agent");
        assert!(AGENT_GRAPH_NAME.contains("agent"));
    }

    #[test]
    fn test_build_agent_graph_registers_in_registry() {
        // Build the graph - this should register it in the registry
        let result = build_agent_graph();
        assert!(result.is_ok());

        // Check registry contains the graph
        let registry = get_graph_registry();
        let guard = registry.read().expect("Lock should be available");
        let entry = guard.get(AGENT_GRAPH_NAME);
        assert!(entry.is_some(), "Graph should be registered after build");

        let entry = entry.unwrap();
        assert_eq!(entry.metadata.name, AGENT_GRAPH_NAME);
        assert_eq!(entry.metadata.version, AGENT_GRAPH_VERSION);
        assert!(entry.active);
    }
}
