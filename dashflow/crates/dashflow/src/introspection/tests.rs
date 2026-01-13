// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Tests for the introspection module.
//!
//! This module contains unit tests for all introspection types including:
//! - GraphManifest and graph structure tests
//! - ExecutionContext and execution state tests
//! - CapabilityManifest and capability introspection tests
//! - StateIntrospection tests
//! - ExecutionTrace tests
//! - Optimization and performance analysis tests
//! - Pattern analysis tests
//! - Resource usage tests
//! - Configuration recommendation tests

use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[path = "tests/bottleneck_tests.rs"]
mod bottleneck_tests;

#[path = "tests/pattern_learning_tests.rs"]
mod pattern_learning_tests;

#[path = "tests/config_recommendations_tests.rs"]
mod config_recommendations_tests;

#[test]
fn test_graph_manifest_builder() {
    let manifest = GraphManifest::builder()
        .graph_id("test-graph")
        .graph_name("Test Graph")
        .entry_point("start")
        .add_node(
            "start",
            NodeManifest::new("start", NodeType::Function).with_description("Entry point"),
        )
        .add_node(
            "process",
            NodeManifest::new("process", NodeType::Agent).with_description("Processing node"),
        )
        .add_edge("start", EdgeManifest::simple("start", "process"))
        .add_edge("process", EdgeManifest::simple("process", "__end__"))
        .build()
        .unwrap();

    assert_eq!(manifest.entry_point, "start");
    assert_eq!(manifest.node_count(), 2);
    assert_eq!(manifest.edge_count(), 2);
    assert!(manifest.has_node("start"));
    assert!(manifest.has_node("process"));
    assert!(!manifest.has_node("nonexistent"));
}

#[test]
fn test_manifest_to_json() {
    let manifest = GraphManifest::builder()
        .entry_point("node1")
        .add_node("node1", NodeManifest::new("node1", NodeType::Function))
        .add_edge("node1", EdgeManifest::simple("node1", "__end__"))
        .build()
        .unwrap();

    let json = manifest.to_json().unwrap();
    assert!(json.contains("node1"));
    assert!(json.contains("entry_point"));

    // Round-trip
    let parsed = GraphManifest::from_json(&json).unwrap();
    assert_eq!(parsed.entry_point, manifest.entry_point);
}

#[test]
fn test_manifest_to_schema() {
    // Test GraphManifest -> GraphSchema conversion
    let manifest = GraphManifest::builder()
        .graph_name("test-graph")
        .entry_point("start")
        .add_node(
            "start",
            NodeManifest::new("start", NodeType::Function).with_description("Entry node"),
        )
        .add_node(
            "llm",
            NodeManifest::new("llm", NodeType::Agent).with_description("LLM node"),
        )
        .add_node(
            "tool",
            NodeManifest::new("tool", NodeType::ToolExecutor).with_description("Tool node"),
        )
        .add_edge("start", EdgeManifest::simple("start", "llm"))
        .add_edge(
            "llm",
            EdgeManifest::conditional("llm", "tool", "needs_tool"),
        )
        .add_edge("tool", EdgeManifest::simple("tool", "__end__"))
        .build()
        .unwrap();

    let schema = manifest.to_schema();

    // Verify schema structure
    assert_eq!(schema.name, "test-graph");
    assert_eq!(schema.entry_point, "start");
    assert_eq!(schema.nodes.len(), 3);
    assert_eq!(schema.edges.len(), 3);

    // Verify node type conversions
    let start_node = schema.nodes.iter().find(|n| n.name == "start").unwrap();
    assert!(matches!(
        start_node.node_type,
        crate::schema::NodeType::Transform
    ));

    let llm_node = schema.nodes.iter().find(|n| n.name == "llm").unwrap();
    assert!(matches!(llm_node.node_type, crate::schema::NodeType::Llm));

    let tool_node = schema.nodes.iter().find(|n| n.name == "tool").unwrap();
    assert!(matches!(tool_node.node_type, crate::schema::NodeType::Tool));

    // Verify edges (should be flattened from map to array)
    let conditional_edge = schema.edges.iter().find(|e| e.from == "llm").unwrap();
    assert_eq!(
        conditional_edge.edge_type,
        crate::schema::EdgeType::Conditional
    );
    assert_eq!(conditional_edge.label, Some("needs_tool".to_string()));
}

#[test]
fn test_compute_schema_id() {
    // Test content-addressed schema ID computation
    let manifest1 = GraphManifest::builder()
        .entry_point("start")
        .add_node("start", NodeManifest::new("start", NodeType::Function))
        .add_node("end", NodeManifest::new("end", NodeType::Function))
        .add_edge("start", EdgeManifest::simple("start", "end"))
        .build()
        .unwrap();

    let manifest2 = GraphManifest::builder()
        .entry_point("start")
        .add_node("start", NodeManifest::new("start", NodeType::Function))
        .add_node("end", NodeManifest::new("end", NodeType::Function))
        .add_edge("start", EdgeManifest::simple("start", "end"))
        .build()
        .unwrap();

    // Same structure should produce same ID
    let id1 = manifest1.compute_schema_id();
    let id2 = manifest2.compute_schema_id();
    assert_eq!(id1, id2);
    assert_eq!(id1.len(), 16); // 8 bytes = 16 hex chars

    // Different structure should produce different ID
    let manifest3 = GraphManifest::builder()
        .entry_point("start")
        .add_node("start", NodeManifest::new("start", NodeType::Function))
        .add_node("middle", NodeManifest::new("middle", NodeType::Function))
        .add_node("end", NodeManifest::new("end", NodeType::Function))
        .add_edge("start", EdgeManifest::simple("start", "middle"))
        .add_edge("middle", EdgeManifest::simple("middle", "end"))
        .build()
        .unwrap();

    let id3 = manifest3.compute_schema_id();
    assert_ne!(id1, id3);
}

#[test]
fn test_decision_points() {
    let manifest = GraphManifest::builder()
        .entry_point("router")
        .add_node("router", NodeManifest::new("router", NodeType::Function))
        .add_node("path_a", NodeManifest::new("path_a", NodeType::Function))
        .add_node("path_b", NodeManifest::new("path_b", NodeType::Function))
        .add_edge(
            "router",
            EdgeManifest::conditional("router", "path_a", "option_a"),
        )
        .add_edge(
            "router",
            EdgeManifest::conditional("router", "path_b", "option_b"),
        )
        .add_edge("path_a", EdgeManifest::simple("path_a", "__end__"))
        .add_edge("path_b", EdgeManifest::simple("path_b", "__end__"))
        .build()
        .unwrap();

    let decisions = manifest.decision_points();
    assert_eq!(decisions.len(), 1);
    assert!(decisions.contains(&"router"));
}

#[test]
fn test_parallel_points() {
    let manifest = GraphManifest::builder()
        .entry_point("fan_out")
        .add_node("fan_out", NodeManifest::new("fan_out", NodeType::Function))
        .add_node("worker1", NodeManifest::new("worker1", NodeType::Function))
        .add_node("worker2", NodeManifest::new("worker2", NodeType::Function))
        .add_edge("fan_out", EdgeManifest::parallel("fan_out", "worker1"))
        .add_edge("fan_out", EdgeManifest::parallel("fan_out", "worker2"))
        .add_edge("worker1", EdgeManifest::simple("worker1", "__end__"))
        .add_edge("worker2", EdgeManifest::simple("worker2", "__end__"))
        .build()
        .unwrap();

    let parallel = manifest.parallel_points();
    assert_eq!(parallel.len(), 1);
    assert!(parallel.contains(&"fan_out"));
}

#[test]
fn test_terminal_nodes() {
    let manifest = GraphManifest::builder()
        .entry_point("start")
        .add_node("start", NodeManifest::new("start", NodeType::Function))
        .add_node("middle", NodeManifest::new("middle", NodeType::Function))
        .add_node("final", NodeManifest::new("final", NodeType::Function))
        .add_edge("start", EdgeManifest::simple("start", "middle"))
        .add_edge("middle", EdgeManifest::simple("middle", "final"))
        .add_edge("final", EdgeManifest::simple("final", "__end__"))
        .build()
        .unwrap();

    let terminals = manifest.terminal_nodes();
    assert_eq!(terminals.len(), 1);
    assert!(terminals.contains(&"final"));
}

#[test]
fn test_reachable_from_entry() {
    let manifest = GraphManifest::builder()
        .entry_point("start")
        .add_node("start", NodeManifest::new("start", NodeType::Function))
        .add_node(
            "reachable",
            NodeManifest::new("reachable", NodeType::Function),
        )
        .add_node(
            "unreachable",
            NodeManifest::new("unreachable", NodeType::Function),
        )
        .add_edge("start", EdgeManifest::simple("start", "reachable"))
        .add_edge("reachable", EdgeManifest::simple("reachable", "__end__"))
        // unreachable has no incoming edge
        .build()
        .unwrap();

    let reachable = manifest.reachable_from_entry();
    assert!(reachable.contains(&"start"));
    assert!(reachable.contains(&"reachable"));
    // Note: unreachable won't be found because it has no edges TO it
    // The algorithm only follows forward edges
}

#[test]
fn test_node_manifest() {
    let node = NodeManifest::new("tool_node", NodeType::ToolExecutor)
        .with_description("Executes tools")
        .with_tools(vec!["search".to_string(), "calculate".to_string()])
        .with_metadata("priority", serde_json::json!(1));

    assert_eq!(node.name, "tool_node");
    assert_eq!(node.node_type, NodeType::ToolExecutor);
    assert_eq!(node.tools_available.len(), 2);
    assert!(node.tools_available.contains(&"search".to_string()));
}

#[test]
fn test_edge_manifest_types() {
    let simple = EdgeManifest::simple("a", "b");
    assert!(!simple.is_conditional);
    assert!(!simple.is_parallel);

    let conditional = EdgeManifest::conditional("a", "b", "condition");
    assert!(conditional.is_conditional);
    assert!(!conditional.is_parallel);
    assert_eq!(conditional.condition_label, Some("condition".to_string()));

    let parallel = EdgeManifest::parallel("a", "b");
    assert!(!parallel.is_conditional);
    assert!(parallel.is_parallel);
}

#[test]
fn test_state_schema() {
    let schema = StateSchema::new("AgentState")
        .with_description("Main agent state")
        .with_field(
            FieldSchema::new("messages", "Vec<String>").with_description("Conversation history"),
        )
        .with_field(
            FieldSchema::new("context", "Option<String>")
                .optional()
                .with_description("Optional context"),
        );

    assert_eq!(schema.type_name, "AgentState");
    assert_eq!(schema.fields.len(), 2);
    assert!(!schema.fields[0].optional);
    assert!(schema.fields[1].optional);
}

#[test]
fn test_graph_metadata() {
    let metadata = GraphMetadata::new()
        .with_version("1.0.0")
        .with_author("test")
        .with_cycles(true)
        .with_parallel_edges(false)
        .with_custom("custom_key", serde_json::json!("custom_value"));

    assert_eq!(metadata.version, Some("1.0.0".to_string()));
    assert_eq!(metadata.author, Some("test".to_string()));
    assert!(metadata.has_cycles);
    assert!(!metadata.has_parallel_edges);
    assert!(metadata.custom.contains_key("custom_key"));
}

#[test]
fn test_builder_missing_entry_point() {
    let result = GraphManifest::builder().graph_name("Test").build();

    assert!(result.is_err());
}

#[test]
fn test_node_type_default() {
    let default_type: NodeType = Default::default();
    assert_eq!(default_type, NodeType::Function);
}

#[test]
fn test_node_type_custom() {
    let custom = NodeType::Custom("my_custom_type".to_string());
    if let NodeType::Custom(name) = custom {
        assert_eq!(name, "my_custom_type");
    } else {
        panic!("Expected Custom variant");
    }
}

// ========================================================================
// ExecutionContext Tests
// ========================================================================

#[test]
fn test_execution_context_new() {
    let ctx = ExecutionContext::new("node_a", 5);
    assert_eq!(ctx.current_node, "node_a");
    assert_eq!(ctx.iteration, 5);
    assert!(ctx.nodes_executed.is_empty());
    assert!(ctx.available_next_nodes.is_empty());
    assert!(ctx.state_snapshot.is_none());
}

#[test]
fn test_execution_context_builder() {
    let ctx = ExecutionContext::builder()
        .current_node("processor")
        .iteration(3)
        .nodes_executed(vec!["start".to_string(), "validate".to_string()])
        .available_next_nodes(vec!["output".to_string(), "error".to_string()])
        .state_snapshot(serde_json::json!({"key": "value"}))
        .thread_id("thread-123")
        .is_interrupted(false)
        .recursion_limit(25)
        .started_at("2025-12-07T10:00:00Z")
        .elapsed_ms(1500)
        .build()
        .unwrap();

    assert_eq!(ctx.current_node, "processor");
    assert_eq!(ctx.iteration, 3);
    assert_eq!(ctx.nodes_executed.len(), 2);
    assert_eq!(ctx.available_next_nodes.len(), 2);
    assert!(ctx.state_snapshot.is_some());
    assert_eq!(ctx.thread_id, Some("thread-123".to_string()));
    assert!(!ctx.is_interrupted);
    assert_eq!(ctx.recursion_limit, 25);
    assert_eq!(ctx.started_at, Some("2025-12-07T10:00:00Z".to_string()));
    assert_eq!(ctx.elapsed_ms, Some(1500));
}

#[test]
fn test_execution_context_builder_add_executed() {
    let ctx = ExecutionContext::builder()
        .current_node("node_c")
        .add_executed_node("node_a")
        .add_executed_node("node_b")
        .build()
        .unwrap();

    assert_eq!(ctx.nodes_executed, vec!["node_a", "node_b"]);
}

#[test]
fn test_execution_context_builder_missing_node() {
    let result = ExecutionContext::builder().iteration(1).build();

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "current_node is required");
}

#[test]
fn test_execution_context_to_json() {
    let ctx = ExecutionContext::new("test_node", 1);
    let json = ctx.to_json().unwrap();

    assert!(json.contains("test_node"));
    assert!(json.contains("iteration"));

    // Round-trip
    let parsed = ExecutionContext::from_json(&json).unwrap();
    assert_eq!(parsed.current_node, ctx.current_node);
    assert_eq!(parsed.iteration, ctx.iteration);
}

#[test]
fn test_execution_context_is_first_iteration() {
    let ctx1 = ExecutionContext::new("node", 1);
    assert!(ctx1.is_first_iteration());

    let ctx2 = ExecutionContext::new("node", 2);
    assert!(!ctx2.is_first_iteration());

    let ctx0 = ExecutionContext::new("node", 0);
    assert!(!ctx0.is_first_iteration());
}

#[test]
fn test_execution_context_is_near_limit() {
    // No limit (0) - never near limit
    let ctx0 = ExecutionContext::builder()
        .current_node("node")
        .iteration(100)
        .recursion_limit(0)
        .build()
        .unwrap();
    assert!(!ctx0.is_near_limit());

    // At 80% of limit (20 of 25)
    let ctx_at_80 = ExecutionContext::builder()
        .current_node("node")
        .iteration(20)
        .recursion_limit(25)
        .build()
        .unwrap();
    assert!(ctx_at_80.is_near_limit());

    // Below 80% of limit (10 of 25)
    let ctx_below = ExecutionContext::builder()
        .current_node("node")
        .iteration(10)
        .recursion_limit(25)
        .build()
        .unwrap();
    assert!(!ctx_below.is_near_limit());
}

#[test]
fn test_execution_context_remaining_iterations() {
    // No limit
    let ctx0 = ExecutionContext::builder()
        .current_node("node")
        .iteration(10)
        .recursion_limit(0)
        .build()
        .unwrap();
    assert_eq!(ctx0.remaining_iterations(), None);

    // With limit
    let ctx = ExecutionContext::builder()
        .current_node("node")
        .iteration(15)
        .recursion_limit(25)
        .build()
        .unwrap();
    assert_eq!(ctx.remaining_iterations(), Some(10));

    // At limit
    let ctx_at = ExecutionContext::builder()
        .current_node("node")
        .iteration(25)
        .recursion_limit(25)
        .build()
        .unwrap();
    assert_eq!(ctx_at.remaining_iterations(), Some(0));
}

#[test]
fn test_execution_context_has_executed() {
    let ctx = ExecutionContext::builder()
        .current_node("node_c")
        .nodes_executed(vec!["node_a".to_string(), "node_b".to_string()])
        .build()
        .unwrap();

    assert!(ctx.has_executed("node_a"));
    assert!(ctx.has_executed("node_b"));
    assert!(!ctx.has_executed("node_c"));
    assert!(!ctx.has_executed("unknown"));
}

#[test]
fn test_execution_context_execution_count() {
    let ctx = ExecutionContext::builder()
        .current_node("node_c")
        .nodes_executed(vec![
            "node_a".to_string(),
            "node_b".to_string(),
            "node_a".to_string(),
            "node_a".to_string(),
        ])
        .build()
        .unwrap();

    assert_eq!(ctx.execution_count("node_a"), 3);
    assert_eq!(ctx.execution_count("node_b"), 1);
    assert_eq!(ctx.execution_count("unknown"), 0);
}

#[test]
fn test_execution_context_can_go_to() {
    let ctx = ExecutionContext::builder()
        .current_node("router")
        .available_next_nodes(vec!["path_a".to_string(), "path_b".to_string()])
        .build()
        .unwrap();

    assert!(ctx.can_go_to("path_a"));
    assert!(ctx.can_go_to("path_b"));
    assert!(!ctx.can_go_to("path_c"));
}

#[test]
fn test_execution_context_recent_history() {
    let ctx = ExecutionContext::builder()
        .current_node("node_e")
        .nodes_executed(vec![
            "node_a".to_string(),
            "node_b".to_string(),
            "node_c".to_string(),
            "node_d".to_string(),
        ])
        .build()
        .unwrap();

    let recent2 = ctx.recent_history(2);
    assert_eq!(recent2, vec!["node_d", "node_c"]);

    let recent10 = ctx.recent_history(10);
    assert_eq!(recent10.len(), 4);
}

#[test]
fn test_execution_context_detect_loop() {
    // No loop
    let ctx_no_loop = ExecutionContext::builder()
        .current_node("node")
        .nodes_executed(vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ])
        .build()
        .unwrap();
    assert_eq!(ctx_no_loop.detect_loop(4), None);

    // Loop detected
    let ctx_loop = ExecutionContext::builder()
        .current_node("node")
        .nodes_executed(vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "b".to_string(),
        ])
        .build()
        .unwrap();
    assert_eq!(ctx_loop.detect_loop(4), Some("b"));

    // Loop outside window
    let ctx_outside = ExecutionContext::builder()
        .current_node("node")
        .nodes_executed(vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
        ])
        .build()
        .unwrap();
    // Window of 3: only sees e, d, c - no duplicates
    assert_eq!(ctx_outside.detect_loop(3), None);
}

#[test]
fn test_execution_context_default() {
    let ctx = ExecutionContext::default();
    assert_eq!(ctx.current_node, "");
    assert_eq!(ctx.iteration, 0);
    assert!(ctx.nodes_executed.is_empty());
    assert!(!ctx.is_interrupted);
    assert_eq!(ctx.recursion_limit, 0);
}

// ========================================================================
// Capability Introspection Tests
// ========================================================================

#[test]
fn test_capability_manifest_new() {
    let caps = CapabilityManifest::new();
    assert!(caps.tools.is_empty());
    assert!(caps.models.is_empty());
    assert!(caps.storage.is_empty());
    assert!(caps.custom.is_empty());
}

#[test]
fn test_capability_manifest_builder() {
    let caps = CapabilityManifest::builder()
        .add_tool(ToolManifest::new("search", "Search the web"))
        .add_tool(ToolManifest::new("calculate", "Perform calculations"))
        .add_model(ModelCapability::new("gpt-4", "OpenAI GPT-4"))
        .add_storage(StorageBackend::new("memory", StorageType::Memory))
        .custom("version", serde_json::json!("1.0.0"))
        .build();

    assert_eq!(caps.tool_count(), 2);
    assert_eq!(caps.model_count(), 1);
    assert!(caps.has_tools());
    assert!(caps.has_models());
    assert!(caps.has_storage());
}

#[test]
fn test_capability_manifest_has_tool() {
    let caps = CapabilityManifest::builder()
        .add_tool(ToolManifest::new("search", "Search"))
        .add_tool(ToolManifest::new("write_file", "Write a file"))
        .build();

    assert!(caps.has_tool("search"));
    assert!(caps.has_tool("write_file"));
    assert!(!caps.has_tool("read_file"));
}

#[test]
fn test_capability_manifest_get_tool() {
    let caps = CapabilityManifest::builder()
        .add_tool(ToolManifest::new("search", "Search the web").with_category("web"))
        .build();

    let tool = caps.get_tool("search");
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().description, "Search the web");

    assert!(caps.get_tool("nonexistent").is_none());
}

#[test]
fn test_capability_manifest_tools_in_category() {
    let caps = CapabilityManifest::builder()
        .add_tool(ToolManifest::new("search", "Search").with_category("web"))
        .add_tool(ToolManifest::new("fetch", "Fetch URL").with_category("web"))
        .add_tool(ToolManifest::new("calculate", "Calculate").with_category("math"))
        .build();

    let web_tools = caps.tools_in_category("web");
    assert_eq!(web_tools.len(), 2);

    let math_tools = caps.tools_in_category("math");
    assert_eq!(math_tools.len(), 1);

    let unknown_tools = caps.tools_in_category("unknown");
    assert!(unknown_tools.is_empty());
}

#[test]
fn test_capability_manifest_models() {
    let caps = CapabilityManifest::builder()
        .add_model(ModelCapability::new("gpt-4", "GPT-4").with_provider("openai"))
        .add_model(ModelCapability::new("claude-3", "Claude 3").with_provider("anthropic"))
        .add_model(ModelCapability::new("gpt-3.5", "GPT-3.5").with_provider("openai"))
        .build();

    assert!(caps.has_model("gpt-4"));
    assert!(caps.has_model("claude-3"));
    assert!(!caps.has_model("gemini"));

    let openai_models = caps.models_by_provider("openai");
    assert_eq!(openai_models.len(), 2);

    let anthropic_models = caps.models_by_provider("anthropic");
    assert_eq!(anthropic_models.len(), 1);
}

#[test]
fn test_capability_manifest_storage_type() {
    let caps = CapabilityManifest::builder()
        .add_storage(StorageBackend::new("memory", StorageType::Memory))
        .add_storage(StorageBackend::new("sqlite", StorageType::Database))
        .build();

    assert!(caps.has_storage_type(StorageType::Memory));
    assert!(caps.has_storage_type(StorageType::Database));
    assert!(!caps.has_storage_type(StorageType::FileSystem));
}

#[test]
fn test_capability_manifest_to_json() {
    let caps = CapabilityManifest::builder()
        .add_tool(ToolManifest::new("test", "Test tool"))
        .build();

    let json = caps.to_json().unwrap();
    assert!(json.contains("test"));
    assert!(json.contains("Test tool"));

    // Round-trip
    let parsed = CapabilityManifest::from_json(&json).unwrap();
    assert_eq!(parsed.tool_count(), 1);
}

#[test]
fn test_capability_manifest_names() {
    let caps = CapabilityManifest::builder()
        .add_tool(ToolManifest::new("tool_a", "Tool A"))
        .add_tool(ToolManifest::new("tool_b", "Tool B"))
        .add_model(ModelCapability::new("model_x", "Model X"))
        .build();

    let tool_names = caps.tool_names();
    assert!(tool_names.contains(&"tool_a"));
    assert!(tool_names.contains(&"tool_b"));

    let model_names = caps.model_names();
    assert!(model_names.contains(&"model_x"));
}

#[test]
fn test_tool_manifest() {
    let tool = ToolManifest::new("write_file", "Write content to a file")
        .with_category("filesystem")
        .with_parameter("path", "string", "File path", true)
        .with_parameter("content", "string", "File content", true)
        .with_parameter_default(
            "mode",
            "string",
            "Write mode",
            serde_json::json!("overwrite"),
        )
        .with_returns("boolean")
        .with_side_effects()
        .with_confirmation()
        .with_metadata("dangerous", serde_json::json!(true));

    assert_eq!(tool.name, "write_file");
    assert_eq!(tool.category, Some("filesystem".to_string()));
    assert_eq!(tool.parameters.len(), 3);
    assert!(tool.parameters[0].required);
    assert!(!tool.parameters[2].required);
    assert!(tool.parameters[2].default_value.is_some());
    assert_eq!(tool.returns, Some("boolean".to_string()));
    assert!(tool.has_side_effects);
    assert!(tool.requires_confirmation);
}

#[test]
fn test_model_capability() {
    let model = ModelCapability::new("gpt-4-turbo", "OpenAI GPT-4 Turbo")
        .with_provider("openai")
        .with_context_window(128000)
        .with_max_output(4096)
        .with_feature(ModelFeature::Chat)
        .with_feature(ModelFeature::FunctionCalling)
        .with_feature(ModelFeature::Vision)
        .with_input_cost(0.01)
        .with_output_cost(0.03)
        .with_metadata("version", serde_json::json!("0125"));

    assert_eq!(model.name, "gpt-4-turbo");
    assert_eq!(model.provider, Some("openai".to_string()));
    assert_eq!(model.context_window, Some(128000));
    assert_eq!(model.max_output_tokens, Some(4096));
    assert!(model.supports(&ModelFeature::Chat));
    assert!(model.supports(&ModelFeature::FunctionCalling));
    assert!(model.supports(&ModelFeature::Vision));
    assert!(!model.supports(&ModelFeature::Embeddings));
    assert_eq!(model.cost_per_1k_input, Some(0.01));
    assert_eq!(model.cost_per_1k_output, Some(0.03));
}

#[test]
fn test_storage_backend() {
    let storage = StorageBackend::new("checkpoint_db", StorageType::Database)
        .with_description("SQLite checkpoint storage")
        .with_feature(StorageFeature::Persistent)
        .with_feature(StorageFeature::Acid)
        .with_feature(StorageFeature::Concurrent)
        .with_max_capacity(1_000_000_000)
        .with_metadata("engine", serde_json::json!("sqlite"));

    assert_eq!(storage.name, "checkpoint_db");
    assert_eq!(storage.storage_type, StorageType::Database);
    assert!(storage.supports(&StorageFeature::Persistent));
    assert!(storage.supports(&StorageFeature::Acid));
    assert!(!storage.supports(&StorageFeature::Encrypted));
    assert_eq!(storage.max_capacity_bytes, Some(1_000_000_000));
}

#[test]
fn test_storage_type_default() {
    let default_type: StorageType = Default::default();
    assert_eq!(default_type, StorageType::Memory);
}

#[test]
fn test_storage_type_custom() {
    let custom = StorageType::Custom("custom_backend".to_string());
    if let StorageType::Custom(name) = custom {
        assert_eq!(name, "custom_backend");
    } else {
        panic!("Expected Custom variant");
    }
}

#[test]
fn test_model_feature_custom() {
    let custom = ModelFeature::Custom("reasoning".to_string());
    if let ModelFeature::Custom(name) = custom {
        assert_eq!(name, "reasoning");
    } else {
        panic!("Expected Custom variant");
    }
}

#[test]
fn test_storage_feature_custom() {
    let custom = StorageFeature::Custom("compression".to_string());
    if let StorageFeature::Custom(name) = custom {
        assert_eq!(name, "compression");
    } else {
        panic!("Expected Custom variant");
    }
}

#[test]
fn test_capability_manifest_empty_checks() {
    let empty = CapabilityManifest::new();
    assert!(!empty.has_tools());
    assert!(!empty.has_models());
    assert!(!empty.has_storage());

    let with_tool = CapabilityManifest::builder()
        .add_tool(ToolManifest::new("t", "t"))
        .build();
    assert!(with_tool.has_tools());
    assert!(!with_tool.has_models());
    assert!(!with_tool.has_storage());
}

#[test]
fn test_capability_manifest_builder_batch() {
    let tools = vec![ToolManifest::new("a", "A"), ToolManifest::new("b", "B")];
    let models = vec![
        ModelCapability::new("m1", "M1"),
        ModelCapability::new("m2", "M2"),
    ];
    let storage = vec![StorageBackend::new("s1", StorageType::Memory)];

    let caps = CapabilityManifest::builder()
        .tools(tools)
        .models(models)
        .storage(storage)
        .build();

    assert_eq!(caps.tool_count(), 2);
    assert_eq!(caps.model_count(), 2);
    assert_eq!(caps.storage.len(), 1);
}

// ========================================================================
// State Introspection Tests
// ========================================================================

// Test struct for introspection tests
#[derive(Clone, Serialize, Deserialize)]
struct TestState {
    messages: Vec<String>,
    iteration: u32,
    active: bool,
    metadata: Option<serde_json::Value>,
}

#[derive(Clone, Serialize, Deserialize)]
struct NestedState {
    user: UserInfo,
    settings: Settings,
    tags: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct UserInfo {
    name: String,
    email: String,
    age: u32,
}

#[derive(Clone, Serialize, Deserialize)]
struct Settings {
    theme: String,
    notifications: bool,
    preferences: Preferences,
}

#[derive(Clone, Serialize, Deserialize)]
struct Preferences {
    language: String,
    timezone: String,
}

#[test]
fn test_state_introspection_get_field() {
    let state = TestState {
        messages: vec!["Hello".to_string(), "World".to_string()],
        iteration: 5,
        active: true,
        metadata: Some(serde_json::json!({"key": "value"})),
    };

    // Get simple fields
    let iteration = state.get_field("iteration");
    assert!(iteration.is_some());
    assert_eq!(iteration.unwrap(), serde_json::json!(5));

    let active = state.get_field("active");
    assert!(active.is_some());
    assert_eq!(active.unwrap(), serde_json::json!(true));

    // Get array field
    let messages = state.get_field("messages");
    assert!(messages.is_some());

    // Non-existent field
    assert!(state.get_field("nonexistent").is_none());
}

#[test]
fn test_state_introspection_nested_get_field() {
    let state = NestedState {
        user: UserInfo {
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
            age: 30,
        },
        settings: Settings {
            theme: "dark".to_string(),
            notifications: true,
            preferences: Preferences {
                language: "en".to_string(),
                timezone: "UTC".to_string(),
            },
        },
        tags: vec!["tag1".to_string(), "tag2".to_string()],
    };

    // Nested field access
    let name = state.get_field("user.name");
    assert!(name.is_some());
    assert_eq!(name.unwrap(), serde_json::json!("Alice"));

    let email = state.get_field("user.email");
    assert_eq!(email.unwrap(), serde_json::json!("alice@example.com"));

    // Deep nested access
    let language = state.get_field("settings.preferences.language");
    assert_eq!(language.unwrap(), serde_json::json!("en"));

    let timezone = state.get_field("settings.preferences.timezone");
    assert_eq!(timezone.unwrap(), serde_json::json!("UTC"));

    // Non-existent nested path
    assert!(state.get_field("user.nonexistent").is_none());
    assert!(state
        .get_field("settings.preferences.nonexistent")
        .is_none());
}

#[test]
fn test_state_introspection_array_index_access() {
    let state = TestState {
        messages: vec![
            "First".to_string(),
            "Second".to_string(),
            "Third".to_string(),
        ],
        iteration: 0,
        active: false,
        metadata: None,
    };

    // Array index access
    let first = state.get_field("messages.0");
    assert!(first.is_some());
    assert_eq!(first.unwrap(), serde_json::json!("First"));

    let second = state.get_field("messages.1");
    assert_eq!(second.unwrap(), serde_json::json!("Second"));

    // Out of bounds
    assert!(state.get_field("messages.10").is_none());
}

#[test]
fn test_state_introspection_has_field() {
    let state = TestState {
        messages: vec![],
        iteration: 0,
        active: false,
        metadata: None,
    };

    assert!(state.has_field("messages"));
    assert!(state.has_field("iteration"));
    assert!(state.has_field("active"));
    assert!(state.has_field("metadata"));
    assert!(!state.has_field("nonexistent"));
}

#[test]
fn test_state_introspection_has_field_nested() {
    let state = NestedState {
        user: UserInfo {
            name: "Bob".to_string(),
            email: "bob@example.com".to_string(),
            age: 25,
        },
        settings: Settings {
            theme: "light".to_string(),
            notifications: false,
            preferences: Preferences {
                language: "de".to_string(),
                timezone: "CET".to_string(),
            },
        },
        tags: vec![],
    };

    assert!(state.has_field("user"));
    assert!(state.has_field("user.name"));
    assert!(state.has_field("user.email"));
    assert!(state.has_field("settings.preferences.language"));
    assert!(!state.has_field("user.missing"));
}

#[test]
fn test_state_introspection_list_fields() {
    let state = TestState {
        messages: vec![],
        iteration: 0,
        active: false,
        metadata: None,
    };

    let fields = state.list_fields();
    assert_eq!(fields.len(), 4);
    assert!(fields.contains(&"messages".to_string()));
    assert!(fields.contains(&"iteration".to_string()));
    assert!(fields.contains(&"active".to_string()));
    assert!(fields.contains(&"metadata".to_string()));
}

#[test]
fn test_state_introspection_list_all_fields() {
    let state = NestedState {
        user: UserInfo {
            name: "Charlie".to_string(),
            email: "charlie@example.com".to_string(),
            age: 35,
        },
        settings: Settings {
            theme: "auto".to_string(),
            notifications: true,
            preferences: Preferences {
                language: "fr".to_string(),
                timezone: "PST".to_string(),
            },
        },
        tags: vec!["a".to_string()],
    };

    // Depth 0: only top-level fields
    let fields_0 = state.list_all_fields(0);
    assert!(fields_0.contains(&"user".to_string()));
    assert!(fields_0.contains(&"settings".to_string()));
    assert!(fields_0.contains(&"tags".to_string()));
    // Should not contain nested paths at depth 0
    let nested_at_0 = fields_0.iter().any(|f| f.contains('.'));
    assert!(!nested_at_0);

    // Depth 1: top-level + one level of nesting
    let fields_1 = state.list_all_fields(1);
    assert!(fields_1.contains(&"user".to_string()));
    assert!(fields_1.contains(&"user.name".to_string()));
    assert!(fields_1.contains(&"user.email".to_string()));
    assert!(fields_1.contains(&"settings.theme".to_string()));
    assert!(fields_1.contains(&"settings.preferences".to_string()));

    // Depth 2: should include preferences.language
    let fields_2 = state.list_all_fields(2);
    assert!(fields_2.contains(&"settings.preferences.language".to_string()));
    assert!(fields_2.contains(&"settings.preferences.timezone".to_string()));
}

#[test]
fn test_state_introspection_state_size_bytes() {
    let small_state = TestState {
        messages: vec![],
        iteration: 0,
        active: false,
        metadata: None,
    };

    let size_small = small_state.state_size_bytes();
    assert!(size_small > 0);
    assert!(size_small < 100); // Should be small

    let large_state = TestState {
        messages: (0..100).map(|i| format!("Message number {}", i)).collect(),
        iteration: 0,
        active: false,
        metadata: Some(serde_json::json!({
            "large_data": "x".repeat(1000)
        })),
    };

    let size_large = large_state.state_size_bytes();
    assert!(size_large > size_small);
    assert!(size_large > 1000); // Should be reasonably large
}

#[test]
fn test_state_introspection_field_type() {
    let state = TestState {
        messages: vec!["test".to_string()],
        iteration: 42,
        active: true,
        metadata: Some(serde_json::json!({"nested": "object"})),
    };

    assert_eq!(state.field_type("messages"), Some("array".to_string()));
    assert_eq!(state.field_type("iteration"), Some("number".to_string()));
    assert_eq!(state.field_type("active"), Some("boolean".to_string()));
    assert_eq!(state.field_type("metadata"), Some("object".to_string()));
    assert_eq!(state.field_type("nonexistent"), None);
}

#[test]
fn test_state_introspection_field_type_nested() {
    let state = NestedState {
        user: UserInfo {
            name: "Dave".to_string(),
            email: "dave@example.com".to_string(),
            age: 40,
        },
        settings: Settings {
            theme: "dark".to_string(),
            notifications: true,
            preferences: Preferences {
                language: "es".to_string(),
                timezone: "EST".to_string(),
            },
        },
        tags: vec![],
    };

    assert_eq!(state.field_type("user"), Some("object".to_string()));
    assert_eq!(state.field_type("user.name"), Some("string".to_string()));
    assert_eq!(state.field_type("user.age"), Some("number".to_string()));
    assert_eq!(
        state.field_type("settings.notifications"),
        Some("boolean".to_string())
    );
    assert_eq!(state.field_type("tags"), Some("array".to_string()));
}

#[test]
fn test_state_introspection_to_introspection_value() {
    let state = TestState {
        messages: vec!["hello".to_string()],
        iteration: 1,
        active: true,
        metadata: None,
    };

    let value = state.to_introspection_value();
    assert!(value.is_object());
    assert_eq!(value["iteration"], serde_json::json!(1));
    assert_eq!(value["active"], serde_json::json!(true));
}

#[test]
fn test_state_introspection_null_metadata() {
    let state = TestState {
        messages: vec![],
        iteration: 0,
        active: false,
        metadata: None,
    };

    // None serializes to null
    let meta_type = state.field_type("metadata");
    assert_eq!(meta_type, Some("null".to_string()));
}

#[test]
fn test_state_introspection_with_option_some() {
    let state = TestState {
        messages: vec![],
        iteration: 0,
        active: false,
        metadata: Some(serde_json::json!({"key": "value"})),
    };

    let meta = state.get_field("metadata");
    assert!(meta.is_some());
    let meta_val = meta.unwrap();
    assert_eq!(meta_val["key"], serde_json::json!("value"));
}

#[test]
fn test_state_introspection_empty_array() {
    let state = TestState {
        messages: vec![],
        iteration: 0,
        active: false,
        metadata: None,
    };

    let messages = state.get_field("messages");
    assert!(messages.is_some());
    assert_eq!(messages.unwrap(), serde_json::json!([]));

    // Can't access index on empty array
    assert!(state.get_field("messages.0").is_none());
}

#[test]
fn test_get_nested_value_helper() {
    let value = serde_json::json!({
        "level1": {
            "level2": {
                "level3": "deep"
            }
        },
        "array": [1, 2, 3]
    });

    assert_eq!(
        get_nested_value(&value, "level1.level2.level3"),
        Some(serde_json::json!("deep"))
    );
    assert_eq!(
        get_nested_value(&value, "array.0"),
        Some(serde_json::json!(1))
    );
    assert_eq!(
        get_nested_value(&value, "array.2"),
        Some(serde_json::json!(3))
    );
    assert_eq!(get_nested_value(&value, "nonexistent"), None);
}

#[test]
#[allow(clippy::approx_constant)] // 3.14 is test data, not PI
fn test_json_type_name_helper() {
    assert_eq!(json_type_name(&serde_json::json!(null)), "null");
    assert_eq!(json_type_name(&serde_json::json!(true)), "boolean");
    assert_eq!(json_type_name(&serde_json::json!(false)), "boolean");
    assert_eq!(json_type_name(&serde_json::json!(42)), "number");
    assert_eq!(json_type_name(&serde_json::json!(3.14)), "number");
    assert_eq!(json_type_name(&serde_json::json!("string")), "string");
    assert_eq!(json_type_name(&serde_json::json!([1, 2, 3])), "array");
    assert_eq!(
        json_type_name(&serde_json::json!({"key": "value"})),
        "object"
    );
}

#[test]
fn test_state_introspection_trait_object_safety() {
    // Verify trait can be used as a trait object (if object-safe)
    // This is a compile-time check - the test passes if it compiles
    fn _use_trait<T: StateIntrospection>(state: &T) {
        let _ = state.list_fields();
        let _ = state.has_field("test");
        let _ = state.state_size_bytes();
    }

    let state = TestState {
        messages: vec![],
        iteration: 0,
        active: false,
        metadata: None,
    };
    _use_trait(&state);
}

// ========================================================================
// Execution Tracing API Tests
// ========================================================================

#[test]
fn test_execution_trace_new() {
    let trace = ExecutionTrace::new();
    assert!(trace.nodes_executed.is_empty());
    assert_eq!(trace.total_duration_ms, 0);
    assert_eq!(trace.total_tokens, 0);
    assert!(trace.errors.is_empty());
    assert!(!trace.completed);
    assert!(trace.thread_id.is_none());
}

#[test]
fn test_execution_trace_builder() {
    let trace = ExecutionTrace::builder()
        .thread_id("thread-123")
        .execution_id("exec-456")
        .total_duration_ms(5000)
        .total_tokens(1500)
        .completed(true)
        .started_at("2025-12-07T10:00:00Z")
        .ended_at("2025-12-07T10:00:05Z")
        .final_state(serde_json::json!({"result": "success"}))
        .metadata("environment", serde_json::json!("production"))
        .add_node_execution(NodeExecution::new("node_a", 100))
        .add_node_execution(NodeExecution::new("node_b", 200))
        .build();

    assert_eq!(trace.thread_id, Some("thread-123".to_string()));
    assert_eq!(trace.execution_id, Some("exec-456".to_string()));
    assert_eq!(trace.total_duration_ms, 5000);
    assert_eq!(trace.total_tokens, 1500);
    assert!(trace.completed);
    assert_eq!(trace.node_count(), 2);
    assert!(trace.final_state.is_some());
}

#[test]
fn test_execution_trace_to_json() {
    let trace = ExecutionTrace::builder()
        .thread_id("test")
        .completed(true)
        .add_node_execution(NodeExecution::new("node", 100))
        .build();

    let json = trace.to_json().unwrap();
    assert!(json.contains("test"));
    assert!(json.contains("completed"));

    // Round-trip
    let parsed = ExecutionTrace::from_json(&json).unwrap();
    assert_eq!(parsed.thread_id, trace.thread_id);
    assert_eq!(parsed.completed, trace.completed);
}

#[test]
fn test_execution_trace_node_queries() {
    let trace = ExecutionTrace::builder()
        .add_node_execution(NodeExecution::new("node_a", 100).with_tokens(500))
        .add_node_execution(NodeExecution::new("node_b", 300).with_tokens(200))
        .add_node_execution(NodeExecution::new("node_a", 150).with_tokens(300))
        .total_duration_ms(550)
        .total_tokens(1000)
        .build();

    // Count executions
    assert_eq!(trace.node_execution_count("node_a"), 2);
    assert_eq!(trace.node_execution_count("node_b"), 1);
    assert_eq!(trace.node_execution_count("unknown"), 0);

    // Get all executions for a node
    let node_a_execs = trace.get_all_node_executions("node_a");
    assert_eq!(node_a_execs.len(), 2);

    // Get first execution
    let first = trace.get_node_execution("node_a");
    assert!(first.is_some());
    assert_eq!(first.unwrap().duration_ms, 100);

    // Total time in node
    assert_eq!(trace.total_time_in_node("node_a"), 250); // 100 + 150
    assert_eq!(trace.total_time_in_node("node_b"), 300);

    // Total tokens in node
    assert_eq!(trace.total_tokens_in_node("node_a"), 800); // 500 + 300
    assert_eq!(trace.total_tokens_in_node("node_b"), 200);
}

#[test]
fn test_execution_trace_slowest_and_most_expensive() {
    let trace = ExecutionTrace::builder()
        .add_node_execution(NodeExecution::new("slow", 500).with_tokens(100))
        .add_node_execution(NodeExecution::new("expensive", 100).with_tokens(1000))
        .add_node_execution(NodeExecution::new("normal", 200).with_tokens(200))
        .build();

    let slowest = trace.slowest_node().unwrap();
    assert_eq!(slowest.node, "slow");
    assert_eq!(slowest.duration_ms, 500);

    let expensive = trace.most_expensive_node().unwrap();
    assert_eq!(expensive.node, "expensive");
    assert_eq!(expensive.tokens_used, 1000);
}

#[test]
fn test_execution_trace_unique_nodes() {
    let trace = ExecutionTrace::builder()
        .add_node_execution(NodeExecution::new("a", 100))
        .add_node_execution(NodeExecution::new("b", 100))
        .add_node_execution(NodeExecution::new("a", 100))
        .add_node_execution(NodeExecution::new("c", 100))
        .add_node_execution(NodeExecution::new("b", 100))
        .build();

    let unique = trace.unique_nodes();
    assert_eq!(unique, vec!["a", "b", "c"]);
}

#[test]
fn test_execution_trace_average_duration() {
    let trace = ExecutionTrace::builder()
        .add_node_execution(NodeExecution::new("a", 100))
        .add_node_execution(NodeExecution::new("b", 200))
        .add_node_execution(NodeExecution::new("c", 300))
        .build();

    let avg = trace.average_node_duration_ms();
    assert!((avg - 200.0).abs() < 0.001);

    // Empty trace
    let empty = ExecutionTrace::new();
    assert_eq!(empty.average_node_duration_ms(), 0.0);
}

#[test]
fn test_execution_trace_time_breakdown() {
    let trace = ExecutionTrace::builder()
        .add_node_execution(NodeExecution::new("a", 200))
        .add_node_execution(NodeExecution::new("b", 300))
        .total_duration_ms(1000)
        .build();

    let breakdown = trace.time_breakdown();
    assert!((breakdown["a"] - 20.0).abs() < 0.001);
    assert!((breakdown["b"] - 30.0).abs() < 0.001);

    // Zero duration
    let zero = ExecutionTrace::new();
    assert!(zero.time_breakdown().is_empty());
}

#[test]
fn test_execution_trace_token_breakdown() {
    let trace = ExecutionTrace::builder()
        .add_node_execution(NodeExecution::new("a", 100).with_tokens(250))
        .add_node_execution(NodeExecution::new("b", 100).with_tokens(750))
        .total_tokens(1000)
        .build();

    let breakdown = trace.token_breakdown();
    assert!((breakdown["a"] - 25.0).abs() < 0.001);
    assert!((breakdown["b"] - 75.0).abs() < 0.001);

    // Zero tokens
    let zero = ExecutionTrace::new();
    assert!(zero.token_breakdown().is_empty());
}

#[test]
fn test_execution_trace_error_handling() {
    let trace = ExecutionTrace::builder()
        .add_error(ErrorTrace::new("node_a", "Connection timeout"))
        .add_error(ErrorTrace::new("node_b", "Validation failed"))
        .add_error(ErrorTrace::new("node_a", "Retry failed"))
        .completed(false)
        .build();

    assert!(trace.has_errors());
    assert_eq!(trace.error_count(), 3);
    assert!(!trace.is_successful());

    let node_a_errors = trace.errors_for_node("node_a");
    assert_eq!(node_a_errors.len(), 2);

    let node_b_errors = trace.errors_for_node("node_b");
    assert_eq!(node_b_errors.len(), 1);
}

#[test]
fn test_execution_trace_is_successful() {
    // Successful: completed without errors
    let successful = ExecutionTrace::builder().completed(true).build();
    assert!(successful.is_successful());

    // Not successful: not completed
    let not_completed = ExecutionTrace::builder().completed(false).build();
    assert!(!not_completed.is_successful());

    // Not successful: completed but has errors
    let has_errors = ExecutionTrace::builder()
        .completed(true)
        .add_error(ErrorTrace::new("node", "error"))
        .build();
    assert!(!has_errors.is_successful());
}

#[test]
fn test_node_execution_new() {
    let exec = NodeExecution::new("test_node", 150);
    assert_eq!(exec.node, "test_node");
    assert_eq!(exec.duration_ms, 150);
    assert_eq!(exec.tokens_used, 0);
    assert!(exec.success);
    assert!(exec.error_message.is_none());
    assert!(exec.tools_called.is_empty());
}

#[test]
fn test_node_execution_builder_pattern() {
    let exec = NodeExecution::new("tool_executor", 200)
        .with_tokens(500)
        .with_state_before(serde_json::json!({"count": 0}))
        .with_state_after(serde_json::json!({"count": 1}))
        .with_tool("search")
        .with_tool("calculate")
        .with_index(3)
        .with_started_at("2025-12-07T10:00:00Z")
        .with_metadata("model", serde_json::json!("gpt-4"));

    assert_eq!(exec.node, "tool_executor");
    assert_eq!(exec.duration_ms, 200);
    assert_eq!(exec.tokens_used, 500);
    assert!(exec.success);
    assert!(exec.state_before.is_some());
    assert!(exec.state_after.is_some());
    assert_eq!(exec.tools_called.len(), 2);
    assert!(exec.tools_called.contains(&"search".to_string()));
    assert_eq!(exec.index, 3);
}

#[test]
fn test_node_execution_with_error() {
    let exec = NodeExecution::new("failed_node", 50).with_error("Connection refused");

    assert!(!exec.success);
    assert_eq!(exec.error_message, Some("Connection refused".to_string()));
}

#[test]
fn test_node_execution_tools() {
    let exec = NodeExecution::new("node", 100).with_tools(vec![
        "tool1".to_string(),
        "tool2".to_string(),
        "tool3".to_string(),
    ]);

    assert!(exec.called_tools());
    assert_eq!(exec.tool_count(), 3);

    let no_tools = NodeExecution::new("node", 100);
    assert!(!no_tools.called_tools());
    assert_eq!(no_tools.tool_count(), 0);
}

#[test]
fn test_node_execution_state_changed() {
    // State changed
    let changed = NodeExecution::new("node", 100)
        .with_state_before(serde_json::json!({"x": 1}))
        .with_state_after(serde_json::json!({"x": 2}));
    assert!(changed.state_changed());

    // State unchanged
    let unchanged = NodeExecution::new("node", 100)
        .with_state_before(serde_json::json!({"x": 1}))
        .with_state_after(serde_json::json!({"x": 1}));
    assert!(!unchanged.state_changed());

    // Missing state snapshots
    let no_before = NodeExecution::new("node", 100).with_state_after(serde_json::json!({"x": 1}));
    assert!(!no_before.state_changed());

    let no_after = NodeExecution::new("node", 100).with_state_before(serde_json::json!({"x": 1}));
    assert!(!no_after.state_changed());
}

#[test]
fn test_node_execution_changed_keys() {
    let exec = NodeExecution::new("node", 100)
        .with_state_before(serde_json::json!({
            "unchanged": "same",
            "modified": "old",
            "removed": "value"
        }))
        .with_state_after(serde_json::json!({
            "unchanged": "same",
            "modified": "new",
            "added": "value"
        }));

    let changed = exec.changed_keys();
    assert!(changed.contains(&"modified".to_string()));
    assert!(changed.contains(&"removed".to_string()));
    assert!(changed.contains(&"added".to_string()));
    assert!(!changed.contains(&"unchanged".to_string()));
    assert_eq!(changed.len(), 3);
}

#[test]
fn test_node_execution_changed_keys_non_object() {
    // Non-object states
    let exec = NodeExecution::new("node", 100)
        .with_state_before(serde_json::json!([1, 2, 3]))
        .with_state_after(serde_json::json!([4, 5, 6]));

    let changed = exec.changed_keys();
    assert!(changed.is_empty());
}

#[test]
fn test_error_trace_new() {
    let error = ErrorTrace::new("node_a", "Something went wrong");
    assert_eq!(error.node, "node_a");
    assert_eq!(error.message, "Something went wrong");
    assert!(error.error_type.is_none());
    assert!(!error.recoverable);
    assert!(!error.retry_attempted);
}

#[test]
fn test_error_trace_builder_pattern() {
    let error = ErrorTrace::new("tool_executor", "API rate limit exceeded")
        .with_error_type("RateLimitError")
        .with_state_at_error(serde_json::json!({"pending_calls": 5}))
        .with_timestamp("2025-12-07T10:00:00Z")
        .with_execution_index(7)
        .recoverable()
        .with_retry_attempted()
        .with_context("Attempted 3 retries with exponential backoff")
        .with_metadata("api", serde_json::json!("openai"));

    assert_eq!(error.node, "tool_executor");
    assert_eq!(error.message, "API rate limit exceeded");
    assert_eq!(error.error_type, Some("RateLimitError".to_string()));
    assert!(error.state_at_error.is_some());
    assert_eq!(error.execution_index, Some(7));
    assert!(error.recoverable);
    assert!(error.retry_attempted);
    assert!(error.context.is_some());
}

#[test]
fn test_error_trace_recoverable() {
    let recoverable = ErrorTrace::new("node", "Timeout").recoverable();
    assert!(recoverable.recoverable);

    let non_recoverable = ErrorTrace::new("node", "Fatal error");
    assert!(!non_recoverable.recoverable);
}

#[test]
fn test_execution_trace_builder_batch_operations() {
    let nodes = vec![
        NodeExecution::new("a", 100),
        NodeExecution::new("b", 200),
        NodeExecution::new("c", 300),
    ];

    let errors = vec![
        ErrorTrace::new("a", "error1"),
        ErrorTrace::new("b", "error2"),
    ];

    let trace = ExecutionTrace::builder()
        .nodes_executed(nodes)
        .errors(errors)
        .build();

    assert_eq!(trace.node_count(), 3);
    assert_eq!(trace.error_count(), 2);
}

#[test]
fn test_execution_trace_empty_queries() {
    let empty = ExecutionTrace::new();

    assert!(empty.slowest_node().is_none());
    assert!(empty.most_expensive_node().is_none());
    assert!(empty.get_node_execution("any").is_none());
    assert!(empty.get_all_node_executions("any").is_empty());
    assert!(empty.unique_nodes().is_empty());
    assert!(empty.errors_for_node("any").is_empty());
}

#[test]
fn test_node_execution_default() {
    let default = NodeExecution::default();
    assert_eq!(default.node, "");
    assert_eq!(default.duration_ms, 0);
    assert_eq!(default.tokens_used, 0);
    // Default::default() for bool is false, but NodeExecution::new() sets success=true
    // This tests the derive(Default) behavior, not the recommended constructor
    assert!(!default.success);
    assert_eq!(default.index, 0);
}

#[test]
fn test_error_trace_default() {
    let default = ErrorTrace::default();
    assert_eq!(default.node, "");
    assert_eq!(default.message, "");
    assert!(!default.recoverable);
    assert!(!default.retry_attempted);
}

#[test]
fn test_execution_trace_json_compact() {
    let trace = ExecutionTrace::builder().thread_id("test").build();

    let compact = trace.to_json_compact().unwrap();
    let pretty = trace.to_json().unwrap();

    // Compact should have no newlines (except in strings)
    assert!(!compact.contains('\n'));
    // Pretty should have newlines
    assert!(pretty.contains('\n'));
}

// ========================================================================
// Decision Explanation Tests
// ========================================================================

#[test]
fn test_decision_log_new() {
    let decision = DecisionLog::new("router", "has_tool_calls()");
    assert_eq!(decision.node, "router");
    assert_eq!(decision.condition, "has_tool_calls()");
    assert_eq!(decision.chosen_path, "");
    assert!(decision.alternative_paths.is_empty());
    assert!(decision.state_values.is_empty());
    assert!(decision.reasoning.is_none());
    assert!(!decision.is_default);
    assert!(decision.confidence.is_none());
}

#[test]
fn test_decision_log_with_methods() {
    let decision = DecisionLog::new("router", "message_type()")
        .with_chosen_path("tool_executor")
        .with_alternative("respond")
        .with_alternative("end")
        .with_state_value("tool_calls_count", serde_json::json!(3))
        .with_reasoning("State contains pending tool calls")
        .with_timestamp("2025-12-07T10:00:00Z")
        .with_execution_index(5)
        .with_confidence(0.95)
        .with_metadata("model", serde_json::json!("gpt-4"));

    assert_eq!(decision.node, "router");
    assert_eq!(decision.condition, "message_type()");
    assert_eq!(decision.chosen_path, "tool_executor");
    assert_eq!(decision.alternative_paths, vec!["respond", "end"]);
    assert_eq!(
        decision.state_values.get("tool_calls_count"),
        Some(&serde_json::json!(3))
    );
    assert_eq!(
        decision.reasoning,
        Some("State contains pending tool calls".to_string())
    );
    assert_eq!(decision.timestamp, Some("2025-12-07T10:00:00Z".to_string()));
    assert_eq!(decision.execution_index, Some(5));
    assert_eq!(decision.confidence, Some(0.95));
    assert!(!decision.is_default);
}

#[test]
fn test_decision_log_as_default() {
    let decision = DecisionLog::new("router", "condition")
        .with_chosen_path("fallback")
        .as_default();

    assert!(decision.is_default);
}

#[test]
fn test_decision_log_confidence_clamped() {
    let too_high = DecisionLog::new("n", "c").with_confidence(1.5);
    assert_eq!(too_high.confidence, Some(1.0));

    let too_low = DecisionLog::new("n", "c").with_confidence(-0.5);
    assert_eq!(too_low.confidence, Some(0.0));

    let valid = DecisionLog::new("n", "c").with_confidence(0.75);
    assert_eq!(valid.confidence, Some(0.75));
}

#[test]
fn test_decision_log_with_alternatives() {
    let decision = DecisionLog::new("n", "c").with_alternatives(vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
    ]);

    assert_eq!(decision.alternative_paths.len(), 3);
    assert!(decision.alternative_paths.contains(&"a".to_string()));
    assert!(decision.alternative_paths.contains(&"b".to_string()));
    assert!(decision.alternative_paths.contains(&"c".to_string()));
}

#[test]
fn test_decision_log_with_state_values() {
    let mut values = HashMap::new();
    values.insert("key1".to_string(), serde_json::json!("value1"));
    values.insert("key2".to_string(), serde_json::json!(42));

    let decision = DecisionLog::new("n", "c").with_state_values(values);

    assert_eq!(decision.state_values.len(), 2);
    assert_eq!(
        decision.state_values.get("key1"),
        Some(&serde_json::json!("value1"))
    );
}

#[test]
fn test_decision_log_to_json() {
    let decision = DecisionLog::new("router", "condition")
        .with_chosen_path("path_a")
        .with_reasoning("Test reason");

    let json = decision.to_json().unwrap();
    assert!(json.contains("router"));
    assert!(json.contains("condition"));
    assert!(json.contains("path_a"));
    assert!(json.contains("Test reason"));

    // Round-trip
    let parsed = DecisionLog::from_json(&json).unwrap();
    assert_eq!(parsed.node, decision.node);
    assert_eq!(parsed.condition, decision.condition);
    assert_eq!(parsed.chosen_path, decision.chosen_path);
    assert_eq!(parsed.reasoning, decision.reasoning);
}

#[test]
fn test_decision_log_compact_json() {
    let decision = DecisionLog::new("n", "c");
    let compact = decision.to_json_compact().unwrap();
    let pretty = decision.to_json().unwrap();

    assert!(!compact.contains('\n'));
    assert!(pretty.contains('\n'));
}

#[test]
fn test_decision_log_had_alternatives() {
    let no_alts = DecisionLog::new("n", "c");
    assert!(!no_alts.had_alternatives());

    let with_alts = DecisionLog::new("n", "c").with_alternative("x");
    assert!(with_alts.had_alternatives());
}

#[test]
fn test_decision_log_total_paths() {
    let decision = DecisionLog::new("n", "c")
        .with_chosen_path("chosen")
        .with_alternative("alt1")
        .with_alternative("alt2");

    assert_eq!(decision.total_paths(), 3); // chosen + 2 alternatives
}

#[test]
fn test_decision_log_has_reasoning() {
    let no_reason = DecisionLog::new("n", "c");
    assert!(!no_reason.has_reasoning());

    let with_reason = DecisionLog::new("n", "c").with_reasoning("Because...");
    assert!(with_reason.has_reasoning());
}

#[test]
fn test_decision_log_get_state_value() {
    let decision = DecisionLog::new("n", "c")
        .with_state_value("count", serde_json::json!(5))
        .with_state_value("name", serde_json::json!("test"));

    assert_eq!(
        decision.get_state_value("count"),
        Some(&serde_json::json!(5))
    );
    assert_eq!(
        decision.get_state_value("name"),
        Some(&serde_json::json!("test"))
    );
    assert_eq!(decision.get_state_value("missing"), None);
}

#[test]
fn test_decision_log_explain() {
    // Basic explanation
    let basic = DecisionLog::new("router", "has_tools()");
    let explain = basic.explain();
    assert!(explain.contains("router"));
    assert!(explain.contains("has_tools()"));

    // With chosen path
    let with_path = DecisionLog::new("router", "has_tools()").with_chosen_path("tool_executor");
    let explain = with_path.explain();
    assert!(explain.contains("chose path 'tool_executor'"));

    // With default
    let with_default = DecisionLog::new("router", "fallback")
        .with_chosen_path("default_path")
        .as_default();
    let explain = with_default.explain();
    assert!(explain.contains("(default)"));

    // With alternatives
    let with_alts = DecisionLog::new("router", "cond")
        .with_chosen_path("a")
        .with_alternative("b")
        .with_alternative("c");
    let explain = with_alts.explain();
    assert!(explain.contains("over alternatives"));
    assert!(explain.contains("b, c"));

    // With reasoning
    let with_reason = DecisionLog::new("router", "cond")
        .with_chosen_path("a")
        .with_reasoning("This is why");
    let explain = with_reason.explain();
    assert!(explain.contains("Reason: This is why"));

    // With confidence
    let with_conf = DecisionLog::new("router", "cond")
        .with_chosen_path("a")
        .with_confidence(0.85);
    let explain = with_conf.explain();
    assert!(explain.contains("confidence: 85.0%"));
}

#[test]
fn test_decision_log_builder() {
    let decision = DecisionLog::builder()
        .node("router")
        .condition("has_tool_calls()")
        .chosen_path("tool_executor")
        .add_alternative("respond")
        .add_alternative("end")
        .state_value("count", serde_json::json!(3))
        .reasoning("Pending tool calls exist")
        .timestamp("2025-12-07T10:00:00Z")
        .execution_index(5)
        .is_default(false)
        .confidence(0.92)
        .metadata("source", serde_json::json!("llm"))
        .build()
        .unwrap();

    assert_eq!(decision.node, "router");
    assert_eq!(decision.condition, "has_tool_calls()");
    assert_eq!(decision.chosen_path, "tool_executor");
    assert_eq!(decision.alternative_paths, vec!["respond", "end"]);
    assert!(decision.state_values.contains_key("count"));
    assert_eq!(
        decision.reasoning,
        Some("Pending tool calls exist".to_string())
    );
    assert_eq!(decision.execution_index, Some(5));
    assert!(!decision.is_default);
    assert_eq!(decision.confidence, Some(0.92));
}

#[test]
fn test_decision_log_builder_alternatives_batch() {
    let decision = DecisionLog::builder()
        .node("n")
        .condition("c")
        .alternatives(vec!["a".to_string(), "b".to_string()])
        .build()
        .unwrap();

    assert_eq!(decision.alternative_paths.len(), 2);
}

#[test]
fn test_decision_log_builder_state_values_batch() {
    let mut values = HashMap::new();
    values.insert("k1".to_string(), serde_json::json!(1));
    values.insert("k2".to_string(), serde_json::json!(2));

    let decision = DecisionLog::builder()
        .node("n")
        .condition("c")
        .state_values(values)
        .build()
        .unwrap();

    assert_eq!(decision.state_values.len(), 2);
}

#[test]
fn test_decision_log_builder_missing_node() {
    let result = DecisionLog::builder().condition("c").build();

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "node is required");
}

#[test]
fn test_decision_log_builder_missing_condition() {
    let result = DecisionLog::builder().node("n").build();

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "condition is required");
}

#[test]
fn test_decision_log_builder_confidence_clamped() {
    let decision = DecisionLog::builder()
        .node("n")
        .condition("c")
        .confidence(2.0)
        .build()
        .unwrap();

    assert_eq!(decision.confidence, Some(1.0));
}

#[test]
fn test_decision_log_default() {
    let default = DecisionLog::default();
    assert_eq!(default.node, "");
    assert_eq!(default.condition, "");
    assert_eq!(default.chosen_path, "");
    assert!(default.alternative_paths.is_empty());
    assert!(!default.is_default);
}

// ========================================================================
// DecisionHistory Tests
// ========================================================================

#[test]
fn test_decision_history_new() {
    let history = DecisionHistory::new();
    assert!(history.is_empty());
    assert_eq!(history.len(), 0);
    assert!(history.thread_id.is_none());
    assert!(history.execution_id.is_none());
}

#[test]
fn test_decision_history_with_thread_id() {
    let history = DecisionHistory::with_thread_id("thread-123").with_execution_id("exec-456");

    assert_eq!(history.thread_id, Some("thread-123".to_string()));
    assert_eq!(history.execution_id, Some("exec-456".to_string()));
}

#[test]
fn test_decision_history_add() {
    let mut history = DecisionHistory::new();
    history.add(DecisionLog::new("n1", "c1"));
    history.add(DecisionLog::new("n2", "c2"));

    assert_eq!(history.len(), 2);
    assert!(!history.is_empty());
}

#[test]
fn test_decision_history_with_decision() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("n1", "c1"))
        .with_decision(DecisionLog::new("n2", "c2"));

    assert_eq!(history.len(), 2);
}

#[test]
fn test_decision_history_all() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("n1", "c1"))
        .with_decision(DecisionLog::new("n2", "c2"));

    let all = history.all();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].node, "n1");
    assert_eq!(all[1].node, "n2");
}

#[test]
fn test_decision_history_to_json() {
    let history = DecisionHistory::with_thread_id("test")
        .with_decision(DecisionLog::new("n", "c").with_chosen_path("p"));

    let json = history.to_json().unwrap();
    assert!(json.contains("test"));
    assert!(json.contains("decisions"));

    // Round-trip
    let parsed = DecisionHistory::from_json(&json).unwrap();
    assert_eq!(parsed.thread_id, history.thread_id);
    assert_eq!(parsed.len(), history.len());
}

#[test]
fn test_decision_history_decisions_at_node() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("router", "c1"))
        .with_decision(DecisionLog::new("other", "c2"))
        .with_decision(DecisionLog::new("router", "c3"));

    let router_decisions = history.decisions_at_node("router");
    assert_eq!(router_decisions.len(), 2);

    let other_decisions = history.decisions_at_node("other");
    assert_eq!(other_decisions.len(), 1);

    let missing_decisions = history.decisions_at_node("missing");
    assert!(missing_decisions.is_empty());
}

#[test]
fn test_decision_history_decisions_choosing_path() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("n1", "c").with_chosen_path("path_a"))
        .with_decision(DecisionLog::new("n2", "c").with_chosen_path("path_b"))
        .with_decision(DecisionLog::new("n3", "c").with_chosen_path("path_a"));

    let path_a = history.decisions_choosing_path("path_a");
    assert_eq!(path_a.len(), 2);

    let path_b = history.decisions_choosing_path("path_b");
    assert_eq!(path_b.len(), 1);
}

#[test]
fn test_decision_history_decisions_for_condition() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("n1", "has_tools()"))
        .with_decision(DecisionLog::new("n2", "is_done()"))
        .with_decision(DecisionLog::new("n3", "has_tools()"));

    let tools_decisions = history.decisions_for_condition("has_tools()");
    assert_eq!(tools_decisions.len(), 2);
}

#[test]
fn test_decision_history_default_decisions() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("n1", "c"))
        .with_decision(DecisionLog::new("n2", "c").as_default())
        .with_decision(DecisionLog::new("n3", "c").as_default())
        .with_decision(DecisionLog::new("n4", "c"));

    assert_eq!(history.default_decision_count(), 2);
    assert!((history.default_decision_percentage() - 50.0).abs() < 0.001);
}

#[test]
fn test_decision_history_default_percentage_empty() {
    let empty = DecisionHistory::new();
    assert_eq!(empty.default_decision_percentage(), 0.0);
}

#[test]
fn test_decision_history_decisions_with_reasoning() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("n1", "c").with_reasoning("reason1"))
        .with_decision(DecisionLog::new("n2", "c"))
        .with_decision(DecisionLog::new("n3", "c").with_reasoning("reason2"));

    let with_reasoning = history.decisions_with_reasoning();
    assert_eq!(with_reasoning.len(), 2);
}

#[test]
fn test_decision_history_confidence_stats() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("n1", "c").with_confidence(0.8))
        .with_decision(DecisionLog::new("n2", "c")) // No confidence
        .with_decision(DecisionLog::new("n3", "c").with_confidence(0.6))
        .with_decision(DecisionLog::new("n4", "c").with_confidence(1.0));

    let avg = history.average_confidence().unwrap();
    assert!((avg - 0.8).abs() < 0.001); // (0.8 + 0.6 + 1.0) / 3 = 0.8

    let min = history.min_confidence().unwrap();
    assert!((min - 0.6).abs() < 0.001);

    let max = history.max_confidence().unwrap();
    assert!((max - 1.0).abs() < 0.001);
}

#[test]
fn test_decision_history_confidence_stats_none() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("n1", "c"))
        .with_decision(DecisionLog::new("n2", "c"));

    assert!(history.average_confidence().is_none());
    assert!(history.min_confidence().is_none());
    assert!(history.max_confidence().is_none());
}

#[test]
fn test_decision_history_unique_decision_nodes() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("router", "c"))
        .with_decision(DecisionLog::new("checker", "c"))
        .with_decision(DecisionLog::new("router", "c"));

    let nodes = history.unique_decision_nodes();
    assert_eq!(nodes.len(), 2);
    assert!(nodes.contains(&"router"));
    assert!(nodes.contains(&"checker"));
}

#[test]
fn test_decision_history_unique_chosen_paths() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("n", "c").with_chosen_path("a"))
        .with_decision(DecisionLog::new("n", "c").with_chosen_path("b"))
        .with_decision(DecisionLog::new("n", "c").with_chosen_path("a"))
        .with_decision(DecisionLog::new("n", "c")); // Empty path

    let paths = history.unique_chosen_paths();
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&"a"));
    assert!(paths.contains(&"b"));
}

#[test]
fn test_decision_history_path_choice_frequency() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("n", "c").with_chosen_path("a"))
        .with_decision(DecisionLog::new("n", "c").with_chosen_path("b"))
        .with_decision(DecisionLog::new("n", "c").with_chosen_path("a"))
        .with_decision(DecisionLog::new("n", "c").with_chosen_path("a"));

    let freq = history.path_choice_frequency();
    assert_eq!(freq.get("a"), Some(&3));
    assert_eq!(freq.get("b"), Some(&1));
}

#[test]
fn test_decision_history_most_frequent_path() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("n", "c").with_chosen_path("rare"))
        .with_decision(DecisionLog::new("n", "c").with_chosen_path("common"))
        .with_decision(DecisionLog::new("n", "c").with_chosen_path("common"))
        .with_decision(DecisionLog::new("n", "c").with_chosen_path("common"));

    let (path, count) = history.most_frequent_path().unwrap();
    assert_eq!(path, "common".to_string());
    assert_eq!(count, 3);
}

#[test]
fn test_decision_history_most_frequent_path_empty() {
    let empty = DecisionHistory::new();
    assert!(empty.most_frequent_path().is_none());
}

#[test]
fn test_decision_history_chronological() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("n3", "c").with_execution_index(3))
        .with_decision(DecisionLog::new("n1", "c").with_execution_index(1))
        .with_decision(DecisionLog::new("n2", "c").with_execution_index(2));

    let sorted = history.chronological();
    assert_eq!(sorted[0].node, "n1");
    assert_eq!(sorted[1].node, "n2");
    assert_eq!(sorted[2].node, "n3");
}

#[test]
fn test_decision_history_last_decision() {
    let empty = DecisionHistory::new();
    assert!(empty.last_decision().is_none());

    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("first", "c"))
        .with_decision(DecisionLog::new("last", "c"));

    let last = history.last_decision().unwrap();
    assert_eq!(last.node, "last");
}

#[test]
fn test_decision_history_decision_at_index() {
    let history = DecisionHistory::new()
        .with_decision(DecisionLog::new("n1", "c").with_execution_index(1))
        .with_decision(DecisionLog::new("n2", "c").with_execution_index(5))
        .with_decision(DecisionLog::new("n3", "c").with_execution_index(10));

    let at_5 = history.decision_at_index(5).unwrap();
    assert_eq!(at_5.node, "n2");

    assert!(history.decision_at_index(7).is_none());
}

#[test]
fn test_decision_history_summarize() {
    let history = DecisionHistory::new()
        .with_decision(
            DecisionLog::new("router", "c")
                .with_chosen_path("a")
                .with_confidence(0.9),
        )
        .with_decision(
            DecisionLog::new("router", "c")
                .with_chosen_path("a")
                .as_default(),
        )
        .with_decision(
            DecisionLog::new("checker", "c")
                .with_chosen_path("b")
                .with_confidence(0.7),
        );

    let summary = history.summarize();
    assert!(summary.contains("3 decisions"));
    assert!(summary.contains("Decision points: 2"));
    assert!(summary.contains("Default decisions:"));
    assert!(summary.contains("Average confidence:"));
    assert!(summary.contains("Most chosen path: 'a' (2 times)"));
}

#[test]
fn test_decision_history_summarize_empty() {
    let empty = DecisionHistory::new();
    let summary = empty.summarize();
    assert_eq!(summary, "No decisions recorded.");
}

#[test]
fn test_decision_history_default() {
    let default = DecisionHistory::default();
    assert!(default.is_empty());
    assert!(default.thread_id.is_none());
    assert!(default.execution_id.is_none());
}

// ========================================================================
// Real-Time Performance Metrics Tests
// ========================================================================

#[test]
fn test_performance_metrics_new() {
    let metrics = PerformanceMetrics::new();
    assert_eq!(metrics.current_latency_ms, 0.0);
    assert_eq!(metrics.tokens_per_second, 0.0);
    assert_eq!(metrics.error_rate, 0.0);
    assert_eq!(metrics.memory_usage_mb, 0.0);
    assert!(metrics.timestamp.is_none());
}

#[test]
fn test_performance_metrics_with_methods() {
    let metrics = PerformanceMetrics::new()
        .with_current_latency_ms(250.0)
        .with_average_latency_ms(200.0)
        .with_p95_latency_ms(500.0)
        .with_p99_latency_ms(1000.0)
        .with_tokens_per_second(45.0)
        .with_error_rate(0.05)
        .with_memory_usage_mb(512.0)
        .with_cpu_usage_percent(60.0)
        .with_sample_count(100)
        .with_sample_window_secs(60.0)
        .with_timestamp("2025-12-07T10:00:00Z")
        .with_thread_id("thread-123");

    assert_eq!(metrics.current_latency_ms, 250.0);
    assert_eq!(metrics.average_latency_ms, 200.0);
    assert_eq!(metrics.p95_latency_ms, 500.0);
    assert_eq!(metrics.p99_latency_ms, 1000.0);
    assert_eq!(metrics.tokens_per_second, 45.0);
    assert_eq!(metrics.error_rate, 0.05);
    assert_eq!(metrics.memory_usage_mb, 512.0);
    assert_eq!(metrics.cpu_usage_percent, 60.0);
    assert_eq!(metrics.sample_count, 100);
    assert_eq!(metrics.sample_window_secs, 60.0);
    assert_eq!(metrics.timestamp, Some("2025-12-07T10:00:00Z".to_string()));
    assert_eq!(metrics.thread_id, Some("thread-123".to_string()));
}

#[test]
fn test_performance_metrics_clamping() {
    // Negative values clamped to 0
    let metrics = PerformanceMetrics::new()
        .with_current_latency_ms(-100.0)
        .with_tokens_per_second(-50.0);
    assert_eq!(metrics.current_latency_ms, 0.0);
    assert_eq!(metrics.tokens_per_second, 0.0);

    // Error rate clamped to 0-1
    let high_error = PerformanceMetrics::new().with_error_rate(1.5);
    assert_eq!(high_error.error_rate, 1.0);

    let low_error = PerformanceMetrics::new().with_error_rate(-0.5);
    assert_eq!(low_error.error_rate, 0.0);

    // CPU clamped to 0-100
    let high_cpu = PerformanceMetrics::new().with_cpu_usage_percent(150.0);
    assert_eq!(high_cpu.cpu_usage_percent, 100.0);
}

#[test]
fn test_performance_metrics_custom_metrics() {
    let metrics = PerformanceMetrics::new()
        .with_custom_metric("queue_depth", 25.0)
        .with_custom_metric("pending_requests", 10.0);

    assert_eq!(metrics.get_custom_metric("queue_depth"), Some(25.0));
    assert_eq!(metrics.get_custom_metric("pending_requests"), Some(10.0));
    assert_eq!(metrics.get_custom_metric("missing"), None);
}

#[test]
fn test_performance_metrics_threshold_checks() {
    let metrics = PerformanceMetrics::new()
        .with_current_latency_ms(1500.0)
        .with_error_rate(0.08)
        .with_memory_usage_mb(3000.0)
        .with_cpu_usage_percent(85.0)
        .with_tokens_per_second(5.0);

    assert!(metrics.is_latency_high(1000.0));
    assert!(!metrics.is_latency_high(2000.0));

    assert!(metrics.is_error_rate_high(0.05));
    assert!(!metrics.is_error_rate_high(0.1));

    assert!(metrics.is_memory_high(2000.0));
    assert!(!metrics.is_memory_high(4000.0));

    assert!(metrics.is_cpu_high(80.0));
    assert!(!metrics.is_cpu_high(90.0));

    assert!(metrics.is_throughput_low(10.0));
    assert!(!metrics.is_throughput_low(1.0));
}

#[test]
fn test_performance_metrics_is_healthy() {
    // Healthy system
    let healthy = PerformanceMetrics::new()
        .with_current_latency_ms(100.0)
        .with_error_rate(0.01)
        .with_memory_usage_mb(512.0)
        .with_cpu_usage_percent(50.0);
    assert!(healthy.is_healthy());

    // Unhealthy - high latency
    let high_latency = PerformanceMetrics::new().with_current_latency_ms(10000.0);
    assert!(!high_latency.is_healthy());

    // Unhealthy - high error rate
    let high_error = PerformanceMetrics::new().with_error_rate(0.15);
    assert!(!high_error.is_healthy());

    // Unhealthy - high memory
    let high_memory = PerformanceMetrics::new().with_memory_usage_mb(5000.0);
    assert!(!high_memory.is_healthy());

    // Unhealthy - high CPU
    let high_cpu = PerformanceMetrics::new().with_cpu_usage_percent(95.0);
    assert!(!high_cpu.is_healthy());
}

#[test]
fn test_performance_metrics_is_healthy_with_thresholds() {
    let metrics = PerformanceMetrics::new()
        .with_current_latency_ms(2000.0)
        .with_error_rate(0.05)
        .with_memory_usage_mb(2048.0)
        .with_cpu_usage_percent(80.0);

    // With default thresholds: healthy
    assert!(metrics.is_healthy_with_thresholds(&PerformanceThresholds::default()));

    // With strict thresholds: not healthy
    assert!(!metrics.is_healthy_with_thresholds(&PerformanceThresholds::strict()));

    // With lenient thresholds: healthy
    assert!(metrics.is_healthy_with_thresholds(&PerformanceThresholds::lenient()));
}

#[test]
fn test_performance_metrics_check_thresholds() {
    let metrics = PerformanceMetrics::new()
        .with_current_latency_ms(6000.0) // High
        .with_error_rate(0.15) // High
        .with_memory_usage_mb(5000.0) // High
        .with_cpu_usage_percent(92.0) // High
        .with_tokens_per_second(0.5); // Low

    let alerts = metrics.check_thresholds(&PerformanceThresholds::default());

    assert_eq!(alerts.len(), 5);
    assert!(alerts
        .iter()
        .any(|a| a.alert_type == AlertType::HighLatency));
    assert!(alerts
        .iter()
        .any(|a| a.alert_type == AlertType::HighErrorRate));
    assert!(alerts.iter().any(|a| a.alert_type == AlertType::HighMemory));
    assert!(alerts.iter().any(|a| a.alert_type == AlertType::HighCpu));
    assert!(alerts
        .iter()
        .any(|a| a.alert_type == AlertType::LowThroughput));
}

#[test]
fn test_performance_metrics_error_rate_percent() {
    let metrics = PerformanceMetrics::new().with_error_rate(0.05);
    assert!((metrics.error_rate_percent() - 5.0).abs() < 0.001);
}

#[test]
fn test_performance_metrics_summarize() {
    let metrics = PerformanceMetrics::new()
        .with_current_latency_ms(250.0)
        .with_average_latency_ms(200.0)
        .with_p95_latency_ms(500.0)
        .with_tokens_per_second(45.0)
        .with_error_rate(0.02)
        .with_memory_usage_mb(512.0)
        .with_cpu_usage_percent(60.0);

    let summary = metrics.summarize();
    assert!(summary.contains("Current latency: 250.0ms"));
    assert!(summary.contains("Average latency: 200.0ms"));
    assert!(summary.contains("P95 latency: 500.0ms"));
    assert!(summary.contains("Throughput: 45.0 tokens/s"));
    assert!(summary.contains("Error rate: 2.00%"));
    assert!(summary.contains("Memory: 512.0MB"));
    assert!(summary.contains("CPU: 60.0%"));
    assert!(summary.contains("Status: HEALTHY"));
}

#[test]
fn test_performance_metrics_json() {
    let metrics = PerformanceMetrics::new()
        .with_current_latency_ms(100.0)
        .with_error_rate(0.01)
        .with_thread_id("test");

    let json = metrics.to_json().unwrap();
    assert!(json.contains("current_latency_ms"));
    assert!(json.contains("100"));

    // Round-trip
    let parsed = PerformanceMetrics::from_json(&json).unwrap();
    assert_eq!(parsed.current_latency_ms, metrics.current_latency_ms);
    assert_eq!(parsed.error_rate, metrics.error_rate);
    assert_eq!(parsed.thread_id, metrics.thread_id);
}

#[test]
fn test_performance_metrics_builder() {
    let metrics = PerformanceMetrics::builder()
        .current_latency_ms(100.0)
        .average_latency_ms(90.0)
        .p95_latency_ms(200.0)
        .p99_latency_ms(300.0)
        .tokens_per_second(50.0)
        .error_rate(0.01)
        .memory_usage_mb(256.0)
        .cpu_usage_percent(40.0)
        .sample_count(1000)
        .sample_window_secs(60.0)
        .timestamp("now")
        .thread_id("t1")
        .custom_metric("extra", 42.0)
        .build();

    assert_eq!(metrics.current_latency_ms, 100.0);
    assert_eq!(metrics.average_latency_ms, 90.0);
    assert_eq!(metrics.p95_latency_ms, 200.0);
    assert_eq!(metrics.p99_latency_ms, 300.0);
    assert_eq!(metrics.tokens_per_second, 50.0);
    assert_eq!(metrics.error_rate, 0.01);
    assert_eq!(metrics.memory_usage_mb, 256.0);
    assert_eq!(metrics.cpu_usage_percent, 40.0);
    assert_eq!(metrics.sample_count, 1000);
    assert_eq!(metrics.sample_window_secs, 60.0);
    assert_eq!(metrics.timestamp, Some("now".to_string()));
    assert_eq!(metrics.thread_id, Some("t1".to_string()));
    assert_eq!(metrics.get_custom_metric("extra"), Some(42.0));
}

#[test]
fn test_performance_thresholds_default() {
    let thresholds = PerformanceThresholds::default();
    assert_eq!(thresholds.max_latency_ms, 5000.0);
    assert_eq!(thresholds.max_error_rate, 0.1);
    assert_eq!(thresholds.max_memory_mb, 4096.0);
    assert_eq!(thresholds.max_cpu_percent, 90.0);
    assert_eq!(thresholds.min_tokens_per_second, 1.0);
}

#[test]
fn test_performance_thresholds_strict() {
    let thresholds = PerformanceThresholds::strict();
    assert_eq!(thresholds.max_latency_ms, 1000.0);
    assert_eq!(thresholds.max_error_rate, 0.01);
    assert_eq!(thresholds.max_memory_mb, 1024.0);
    assert_eq!(thresholds.max_cpu_percent, 70.0);
    assert_eq!(thresholds.min_tokens_per_second, 10.0);
}

#[test]
fn test_performance_thresholds_lenient() {
    let thresholds = PerformanceThresholds::lenient();
    assert_eq!(thresholds.max_latency_ms, 30000.0);
    assert_eq!(thresholds.max_error_rate, 0.25);
    assert_eq!(thresholds.max_memory_mb, 8192.0);
    assert_eq!(thresholds.max_cpu_percent, 95.0);
    assert_eq!(thresholds.min_tokens_per_second, 0.1);
}

#[test]
fn test_performance_thresholds_with_methods() {
    let thresholds = PerformanceThresholds::new()
        .with_max_latency_ms(2000.0)
        .with_max_error_rate(0.05)
        .with_max_memory_mb(2048.0)
        .with_max_cpu_percent(80.0)
        .with_min_tokens_per_second(5.0);

    assert_eq!(thresholds.max_latency_ms, 2000.0);
    assert_eq!(thresholds.max_error_rate, 0.05);
    assert_eq!(thresholds.max_memory_mb, 2048.0);
    assert_eq!(thresholds.max_cpu_percent, 80.0);
    assert_eq!(thresholds.min_tokens_per_second, 5.0);
}

#[test]
fn test_alert_severity() {
    assert!(!AlertSeverity::Info.is_severe());
    assert!(!AlertSeverity::Warning.is_severe());
    assert!(AlertSeverity::Error.is_severe());
    assert!(AlertSeverity::Critical.is_severe());
}

#[test]
fn test_performance_alert_excess_ratio() {
    let latency_alert = PerformanceAlert {
        alert_type: AlertType::HighLatency,
        metric_name: "latency".to_string(),
        current_value: 2000.0,
        threshold_value: 1000.0,
        severity: AlertSeverity::Warning,
        message: "test".to_string(),
    };
    assert!((latency_alert.excess_ratio() - 2.0).abs() < 0.001);

    // Throughput is inverse (below threshold)
    let throughput_alert = PerformanceAlert {
        alert_type: AlertType::LowThroughput,
        metric_name: "throughput".to_string(),
        current_value: 5.0,
        threshold_value: 10.0,
        severity: AlertSeverity::Warning,
        message: "test".to_string(),
    };
    assert!((throughput_alert.excess_ratio() - 2.0).abs() < 0.001);
}

#[test]
fn test_performance_alert_is_severe() {
    let warning = PerformanceAlert {
        alert_type: AlertType::HighLatency,
        metric_name: "test".to_string(),
        current_value: 0.0,
        threshold_value: 0.0,
        severity: AlertSeverity::Warning,
        message: "test".to_string(),
    };
    assert!(!warning.is_severe());

    let critical = PerformanceAlert {
        alert_type: AlertType::HighLatency,
        metric_name: "test".to_string(),
        current_value: 0.0,
        threshold_value: 0.0,
        severity: AlertSeverity::Critical,
        message: "test".to_string(),
    };
    assert!(critical.is_severe());
}

#[test]
fn test_performance_history_new() {
    let history = PerformanceHistory::new(100);
    assert!(history.is_empty());
    assert_eq!(history.len(), 0);
    assert_eq!(history.max_snapshots, 100);
}

#[test]
fn test_performance_history_add() {
    let mut history = PerformanceHistory::new(5);

    for i in 0..7 {
        history.add(PerformanceMetrics::new().with_current_latency_ms(i as f64 * 100.0));
    }

    // Should be trimmed to max_snapshots
    assert_eq!(history.len(), 5);
    // First two should be removed, so first latency should be 200ms
    assert_eq!(history.snapshots[0].current_latency_ms, 200.0);
}

#[test]
fn test_performance_history_latest() {
    let mut history = PerformanceHistory::new(10);
    assert!(history.latest().is_none());

    history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
    history.add(PerformanceMetrics::new().with_current_latency_ms(200.0));

    let latest = history.latest().unwrap();
    assert_eq!(latest.current_latency_ms, 200.0);
}

#[test]
fn test_performance_history_averages() {
    let mut history = PerformanceHistory::new(10);

    assert!(history.average_latency().is_none());
    assert!(history.average_error_rate().is_none());
    assert!(history.average_throughput().is_none());

    history.add(
        PerformanceMetrics::new()
            .with_current_latency_ms(100.0)
            .with_error_rate(0.01)
            .with_tokens_per_second(50.0),
    );
    history.add(
        PerformanceMetrics::new()
            .with_current_latency_ms(200.0)
            .with_error_rate(0.03)
            .with_tokens_per_second(40.0),
    );
    history.add(
        PerformanceMetrics::new()
            .with_current_latency_ms(300.0)
            .with_error_rate(0.02)
            .with_tokens_per_second(60.0),
    );

    let avg_latency = history.average_latency().unwrap();
    assert!((avg_latency - 200.0).abs() < 0.001);

    let avg_error = history.average_error_rate().unwrap();
    assert!((avg_error - 0.02).abs() < 0.001);

    let avg_tps = history.average_throughput().unwrap();
    assert!((avg_tps - 50.0).abs() < 0.001);
}

#[test]
fn test_performance_history_min_max_latency() {
    let mut history = PerformanceHistory::new(10);

    assert!(history.min_latency().is_none());
    assert!(history.max_latency().is_none());

    history.add(PerformanceMetrics::new().with_current_latency_ms(200.0));
    history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
    history.add(PerformanceMetrics::new().with_current_latency_ms(300.0));

    assert_eq!(history.min_latency(), Some(100.0));
    assert_eq!(history.max_latency(), Some(300.0));
}

#[test]
fn test_performance_history_latency_trending() {
    // Not enough data
    let mut short = PerformanceHistory::new(10);
    short.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
    short.add(PerformanceMetrics::new().with_current_latency_ms(200.0));
    assert!(!short.is_latency_trending_up());

    // Trending up
    let mut trending_up = PerformanceHistory::new(10);
    trending_up.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
    trending_up.add(PerformanceMetrics::new().with_current_latency_ms(110.0));
    trending_up.add(PerformanceMetrics::new().with_current_latency_ms(200.0));
    trending_up.add(PerformanceMetrics::new().with_current_latency_ms(220.0));
    assert!(trending_up.is_latency_trending_up());

    // Not trending up
    let mut stable = PerformanceHistory::new(10);
    stable.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
    stable.add(PerformanceMetrics::new().with_current_latency_ms(105.0));
    stable.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
    stable.add(PerformanceMetrics::new().with_current_latency_ms(102.0));
    assert!(!stable.is_latency_trending_up());
}

#[test]
fn test_performance_history_error_rate_trending() {
    // Not enough data
    let mut short = PerformanceHistory::new(10);
    short.add(PerformanceMetrics::new().with_error_rate(0.01));
    assert!(!short.is_error_rate_trending_up());

    // Trending up (50% increase required)
    let mut trending = PerformanceHistory::new(10);
    trending.add(PerformanceMetrics::new().with_error_rate(0.01));
    trending.add(PerformanceMetrics::new().with_error_rate(0.01));
    trending.add(PerformanceMetrics::new().with_error_rate(0.02));
    trending.add(PerformanceMetrics::new().with_error_rate(0.03));
    assert!(trending.is_error_rate_trending_up());
}

#[test]
fn test_performance_history_health_summary() {
    let empty = PerformanceHistory::new(10);
    assert_eq!(empty.health_summary(), "No data available");

    let mut healthy = PerformanceHistory::new(10);
    healthy.add(PerformanceMetrics::new().with_current_latency_ms(100.0));
    assert!(healthy.health_summary().contains("HEALTHY"));

    let mut degraded = PerformanceHistory::new(10);
    degraded.add(PerformanceMetrics::new().with_current_latency_ms(10000.0));
    assert!(degraded.health_summary().contains("DEGRADED"));
}

#[test]
fn test_performance_history_json() {
    let mut history = PerformanceHistory::new(10).with_thread_id("test-thread");
    history.add(PerformanceMetrics::new().with_current_latency_ms(100.0));

    let json = history.to_json().unwrap();
    assert!(json.contains("test-thread"));
    assert!(json.contains("snapshots"));

    let parsed = PerformanceHistory::from_json(&json).unwrap();
    assert_eq!(parsed.thread_id, Some("test-thread".to_string()));
    assert_eq!(parsed.len(), 1);
}

#[test]
fn test_performance_metrics_default() {
    let default = PerformanceMetrics::default();
    assert_eq!(default.current_latency_ms, 0.0);
    assert_eq!(default.error_rate, 0.0);
    assert!(default.custom.is_empty());
}

#[test]
fn test_performance_history_default() {
    let default = PerformanceHistory::default();
    assert!(default.is_empty());
    assert_eq!(default.max_snapshots, 0);
    assert!(default.thread_id.is_none());
}

// ========================================================================
// Resource Usage Awareness Tests
// ========================================================================

#[test]
fn test_resource_usage_new() {
    let usage = ResourceUsage::new();
    assert_eq!(usage.tokens_used, 0);
    assert_eq!(usage.tokens_budget, 0);
    assert_eq!(usage.api_calls, 0);
    assert_eq!(usage.cost_usd, 0.0);
    assert_eq!(usage.execution_time_ms, 0);
}

#[test]
fn test_resource_usage_builder_pattern() {
    let usage = ResourceUsage::new()
        .with_tokens_used(5000)
        .with_tokens_budget(10000)
        .with_input_tokens(3000)
        .with_output_tokens(2000)
        .with_api_calls(25)
        .with_api_calls_budget(100)
        .with_cost_usd(0.15)
        .with_cost_budget_usd(1.0)
        .with_execution_time_ms(5000)
        .with_execution_time_budget_ms(60000)
        .with_thread_id("test-thread")
        .with_execution_id("exec-001")
        .with_started_at("2024-01-01T00:00:00Z")
        .with_updated_at("2024-01-01T00:01:00Z")
        .with_custom("memory_mb", 512.0);

    assert_eq!(usage.tokens_used, 5000);
    assert_eq!(usage.tokens_budget, 10000);
    assert_eq!(usage.input_tokens, 3000);
    assert_eq!(usage.output_tokens, 2000);
    assert_eq!(usage.api_calls, 25);
    assert_eq!(usage.api_calls_budget, 100);
    assert!((usage.cost_usd - 0.15).abs() < f64::EPSILON);
    assert!((usage.cost_budget_usd - 1.0).abs() < f64::EPSILON);
    assert_eq!(usage.execution_time_ms, 5000);
    assert_eq!(usage.execution_time_budget_ms, 60000);
    assert_eq!(usage.thread_id, Some("test-thread".to_string()));
    assert_eq!(usage.execution_id, Some("exec-001".to_string()));
    assert_eq!(usage.get_custom("memory_mb"), Some(512.0));
}

#[test]
fn test_resource_usage_builder_struct() {
    let usage = ResourceUsageBuilder::new()
        .tokens_used(1000)
        .tokens_budget(5000)
        .input_tokens(600)
        .output_tokens(400)
        .api_calls(10)
        .api_calls_budget(50)
        .cost_usd(0.05)
        .cost_budget_usd(0.5)
        .execution_time_ms(2000)
        .execution_time_budget_ms(10000)
        .thread_id("builder-thread")
        .execution_id("builder-exec")
        .started_at("2024-01-01T00:00:00Z")
        .updated_at("2024-01-01T00:00:30Z")
        .custom("requests", 100.0)
        .build();

    assert_eq!(usage.tokens_used, 1000);
    assert_eq!(usage.tokens_budget, 5000);
    assert_eq!(usage.api_calls, 10);
    assert_eq!(usage.thread_id, Some("builder-thread".to_string()));
    assert_eq!(usage.get_custom("requests"), Some(100.0));
}

#[test]
fn test_resource_usage_negative_cost_clamped() {
    let usage = ResourceUsage::new().with_cost_usd(-10.0);
    assert_eq!(usage.cost_usd, 0.0);

    let builder_usage = ResourceUsageBuilder::new().cost_usd(-5.0).build();
    assert_eq!(builder_usage.cost_usd, 0.0);
}

#[test]
fn test_resource_usage_token_budget_monitoring() {
    let usage = ResourceUsage::new()
        .with_tokens_used(7500)
        .with_tokens_budget(10000);

    assert_eq!(usage.remaining_tokens(), 2500);
    assert!((usage.token_usage_percentage() - 75.0).abs() < f64::EPSILON);
    assert!(!usage.is_over_token_budget());
    assert!(usage.has_token_budget());
    assert!(usage.is_near_token_limit(0.7));
    assert!(!usage.is_near_token_limit(0.8));
}

#[test]
fn test_resource_usage_token_over_budget() {
    let usage = ResourceUsage::new()
        .with_tokens_used(15000)
        .with_tokens_budget(10000);

    assert_eq!(usage.remaining_tokens(), 0);
    assert!(usage.is_over_token_budget());
    assert!(usage.is_near_token_limit(0.5));
}

#[test]
fn test_resource_usage_no_token_budget() {
    let usage = ResourceUsage::new().with_tokens_used(10000);

    assert_eq!(usage.remaining_tokens(), u64::MAX);
    assert_eq!(usage.token_usage_percentage(), 0.0);
    assert!(!usage.is_over_token_budget());
    assert!(!usage.has_token_budget());
    assert!(!usage.is_near_token_limit(0.9));
}

#[test]
fn test_resource_usage_api_call_monitoring() {
    let usage = ResourceUsage::new()
        .with_api_calls(45)
        .with_api_calls_budget(50);

    assert_eq!(usage.remaining_api_calls(), 5);
    assert!((usage.api_call_usage_percentage() - 90.0).abs() < f64::EPSILON);
    assert!(usage.is_near_api_call_limit(0.9));
    assert!(!usage.is_over_api_call_budget());
    assert!(usage.has_api_call_budget());
}

#[test]
fn test_resource_usage_api_over_budget() {
    let usage = ResourceUsage::new()
        .with_api_calls(60)
        .with_api_calls_budget(50);

    assert_eq!(usage.remaining_api_calls(), 0);
    assert!(usage.is_over_api_call_budget());
}

#[test]
fn test_resource_usage_no_api_budget() {
    let usage = ResourceUsage::new().with_api_calls(100);

    assert_eq!(usage.remaining_api_calls(), u64::MAX);
    assert_eq!(usage.api_call_usage_percentage(), 0.0);
    assert!(!usage.is_over_api_call_budget());
    assert!(!usage.has_api_call_budget());
}

#[test]
fn test_resource_usage_cost_monitoring() {
    let usage = ResourceUsage::new()
        .with_cost_usd(0.80)
        .with_cost_budget_usd(1.0);

    assert!((usage.remaining_cost_usd() - 0.20).abs() < 0.001);
    assert!((usage.cost_usage_percentage() - 80.0).abs() < f64::EPSILON);
    assert!(usage.is_near_cost_limit(0.8));
    assert!(!usage.is_over_cost_budget());
    assert!(usage.has_cost_budget());
}

#[test]
fn test_resource_usage_cost_over_budget() {
    let usage = ResourceUsage::new()
        .with_cost_usd(1.50)
        .with_cost_budget_usd(1.0);

    assert_eq!(usage.remaining_cost_usd(), 0.0);
    assert!(usage.is_over_cost_budget());
}

#[test]
fn test_resource_usage_no_cost_budget() {
    let usage = ResourceUsage::new().with_cost_usd(10.0);

    assert_eq!(usage.remaining_cost_usd(), f64::MAX);
    assert_eq!(usage.cost_usage_percentage(), 0.0);
    assert!(!usage.is_over_cost_budget());
    assert!(!usage.has_cost_budget());
}

#[test]
fn test_resource_usage_time_monitoring() {
    let usage = ResourceUsage::new()
        .with_execution_time_ms(50000)
        .with_execution_time_budget_ms(60000);

    assert_eq!(usage.remaining_time_ms(), 10000);
    assert!((usage.time_usage_percentage() - 83.333).abs() < 0.1);
    assert!(usage.is_near_time_limit(0.8));
    assert!(!usage.is_over_time_budget());
    assert!(usage.has_time_budget());
}

#[test]
fn test_resource_usage_time_over_budget() {
    let usage = ResourceUsage::new()
        .with_execution_time_ms(70000)
        .with_execution_time_budget_ms(60000);

    assert_eq!(usage.remaining_time_ms(), 0);
    assert!(usage.is_over_time_budget());
}

#[test]
fn test_resource_usage_no_time_budget() {
    let usage = ResourceUsage::new().with_execution_time_ms(100000);

    assert_eq!(usage.remaining_time_ms(), u64::MAX);
    assert_eq!(usage.time_usage_percentage(), 0.0);
    assert!(!usage.is_over_time_budget());
    assert!(!usage.has_time_budget());
}

#[test]
fn test_resource_usage_any_budget_exceeded() {
    let normal = ResourceUsage::new()
        .with_tokens_used(5000)
        .with_tokens_budget(10000);
    assert!(!normal.is_any_budget_exceeded());

    let token_over = ResourceUsage::new()
        .with_tokens_used(15000)
        .with_tokens_budget(10000);
    assert!(token_over.is_any_budget_exceeded());

    let cost_over = ResourceUsage::new()
        .with_cost_usd(2.0)
        .with_cost_budget_usd(1.0);
    assert!(cost_over.is_any_budget_exceeded());
}

#[test]
fn test_resource_usage_near_any_limit() {
    let normal = ResourceUsage::new()
        .with_tokens_used(5000)
        .with_tokens_budget(10000);
    assert!(!normal.is_near_any_limit(0.9));

    let near_token = ResourceUsage::new()
        .with_tokens_used(9500)
        .with_tokens_budget(10000);
    assert!(near_token.is_near_any_limit(0.9));

    let near_cost = ResourceUsage::new()
        .with_cost_usd(0.95)
        .with_cost_budget_usd(1.0);
    assert!(near_cost.is_near_any_limit(0.9));
}

#[test]
fn test_resource_usage_check_budgets_no_alerts() {
    let usage = ResourceUsage::new()
        .with_tokens_used(5000)
        .with_tokens_budget(10000);
    let alerts = usage.check_budgets(0.8);
    assert!(alerts.is_empty());
}

#[test]
fn test_resource_usage_check_budgets_warning() {
    let usage = ResourceUsage::new()
        .with_tokens_used(8500)
        .with_tokens_budget(10000);
    let alerts = usage.check_budgets(0.8);

    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].alert_type, BudgetAlertType::TokensNearLimit);
    assert_eq!(alerts[0].severity, BudgetAlertSeverity::Warning);
    assert!(!alerts[0].is_critical());
}

#[test]
fn test_resource_usage_check_budgets_critical() {
    let usage = ResourceUsage::new()
        .with_tokens_used(15000)
        .with_tokens_budget(10000);
    let alerts = usage.check_budgets(0.8);

    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].alert_type, BudgetAlertType::TokensExceeded);
    assert_eq!(alerts[0].severity, BudgetAlertSeverity::Critical);
    assert!(alerts[0].is_critical());
}

#[test]
fn test_resource_usage_check_budgets_multiple() {
    let usage = ResourceUsage::new()
        .with_tokens_used(15000)
        .with_tokens_budget(10000)
        .with_cost_usd(0.95)
        .with_cost_budget_usd(1.0)
        .with_api_calls(45)
        .with_api_calls_budget(50);

    let alerts = usage.check_budgets(0.9);

    // Token exceeded (critical) + cost near limit (warning) + api near limit (warning)
    assert_eq!(alerts.len(), 3);
    let critical_count = alerts.iter().filter(|a| a.is_critical()).count();
    assert_eq!(critical_count, 1);
}

#[test]
fn test_resource_usage_cost_per_token() {
    let usage = ResourceUsage::new()
        .with_tokens_used(10000)
        .with_cost_usd(0.10);
    assert!((usage.cost_per_token() - 0.00001).abs() < 0.0000001);

    let zero = ResourceUsage::new();
    assert_eq!(zero.cost_per_token(), 0.0);
}

#[test]
fn test_resource_usage_tokens_per_api_call() {
    let usage = ResourceUsage::new()
        .with_tokens_used(10000)
        .with_api_calls(20);
    assert!((usage.tokens_per_api_call() - 500.0).abs() < f64::EPSILON);

    let zero = ResourceUsage::new();
    assert_eq!(zero.tokens_per_api_call(), 0.0);
}

#[test]
fn test_resource_usage_summarize() {
    let usage = ResourceUsage::new()
        .with_tokens_used(5000)
        .with_tokens_budget(10000)
        .with_api_calls(25)
        .with_cost_usd(0.15);

    let summary = usage.summarize();
    assert!(summary.contains("Tokens: 5000 / 10000"));
    assert!(summary.contains("API calls: 25"));
    assert!(summary.contains("Cost: $0.1500"));
    assert!(summary.contains("Status: OK"));
}

#[test]
fn test_resource_usage_summarize_near_limits() {
    let usage = ResourceUsage::new()
        .with_tokens_used(9500)
        .with_tokens_budget(10000);

    let summary = usage.summarize();
    assert!(summary.contains("Status: NEAR LIMITS"));
}

#[test]
fn test_resource_usage_summarize_over_budget() {
    let usage = ResourceUsage::new()
        .with_tokens_used(15000)
        .with_tokens_budget(10000);

    let summary = usage.summarize();
    assert!(summary.contains("Status: OVER BUDGET"));
}

#[test]
fn test_resource_usage_json() {
    let usage = ResourceUsage::new()
        .with_tokens_used(5000)
        .with_tokens_budget(10000)
        .with_cost_usd(0.15)
        .with_thread_id("test-thread")
        .with_custom("memory", 256.0);

    let json = usage.to_json().unwrap();
    assert!(json.contains("\"tokens_used\": 5000"));
    assert!(json.contains("\"test-thread\""));
    assert!(json.contains("\"memory\""));

    let parsed = ResourceUsage::from_json(&json).unwrap();
    assert_eq!(parsed.tokens_used, 5000);
    assert_eq!(parsed.thread_id, Some("test-thread".to_string()));
    assert_eq!(parsed.get_custom("memory"), Some(256.0));
}

#[test]
fn test_resource_usage_json_compact() {
    let usage = ResourceUsage::new()
        .with_tokens_used(1000)
        .with_api_calls(10);

    let compact = usage.to_json_compact().unwrap();
    assert!(!compact.contains('\n'));
    assert!(compact.contains("\"tokens_used\":1000"));
}

#[test]
fn test_budget_alert_usage_ratio() {
    let alert = BudgetAlert {
        alert_type: BudgetAlertType::TokensNearLimit,
        resource_name: "tokens".to_string(),
        current_value: 8000.0,
        budget_value: 10000.0,
        severity: BudgetAlertSeverity::Warning,
        message: "Test".to_string(),
    };
    assert!((alert.usage_ratio() - 0.8).abs() < f64::EPSILON);
    assert_eq!(alert.over_budget_percentage(), 0.0);
}

#[test]
fn test_budget_alert_over_budget_percentage() {
    let alert = BudgetAlert {
        alert_type: BudgetAlertType::TokensExceeded,
        resource_name: "tokens".to_string(),
        current_value: 12000.0,
        budget_value: 10000.0,
        severity: BudgetAlertSeverity::Critical,
        message: "Test".to_string(),
    };
    assert!((alert.over_budget_percentage() - 20.0).abs() < 0.001);
}

#[test]
fn test_budget_alert_severity_is_critical() {
    assert!(!BudgetAlertSeverity::Info.is_critical());
    assert!(!BudgetAlertSeverity::Warning.is_critical());
    assert!(BudgetAlertSeverity::Critical.is_critical());
}

#[test]
fn test_resource_usage_history_new() {
    let history = ResourceUsageHistory::new(100);
    assert!(history.is_empty());
    assert_eq!(history.max_snapshots, 100);
    assert!(history.thread_id.is_none());
}

#[test]
fn test_resource_usage_history_add() {
    let mut history = ResourceUsageHistory::new(3);

    history.add(ResourceUsage::new().with_tokens_used(1000));
    assert_eq!(history.len(), 1);

    history.add(ResourceUsage::new().with_tokens_used(2000));
    history.add(ResourceUsage::new().with_tokens_used(3000));
    assert_eq!(history.len(), 3);

    // Exceed max, oldest should be removed
    history.add(ResourceUsage::new().with_tokens_used(4000));
    assert_eq!(history.len(), 3);
    assert_eq!(history.snapshots[0].tokens_used, 2000);
}

#[test]
fn test_resource_usage_history_latest() {
    let mut history = ResourceUsageHistory::new(10);
    assert!(history.latest().is_none());

    history.add(ResourceUsage::new().with_tokens_used(1000));
    history.add(ResourceUsage::new().with_tokens_used(2000));

    let latest = history.latest().unwrap();
    assert_eq!(latest.tokens_used, 2000);
}

#[test]
fn test_resource_usage_history_totals() {
    let mut history = ResourceUsageHistory::new(10);
    history.add(
        ResourceUsage::new()
            .with_tokens_used(1000)
            .with_cost_usd(0.10)
            .with_api_calls(5),
    );
    history.add(
        ResourceUsage::new()
            .with_tokens_used(3000)
            .with_cost_usd(0.30)
            .with_api_calls(15),
    );
    history.add(
        ResourceUsage::new()
            .with_tokens_used(2000)
            .with_cost_usd(0.20)
            .with_api_calls(10),
    );

    assert_eq!(history.total_tokens(), 3000);
    assert!((history.total_cost() - 0.30).abs() < 0.001);
    assert_eq!(history.total_api_calls(), 15);
}

#[test]
fn test_resource_usage_history_rates() {
    let mut history = ResourceUsageHistory::new(10);
    history.add(
        ResourceUsage::new()
            .with_tokens_used(1000)
            .with_cost_usd(0.10)
            .with_execution_time_ms(0),
    );
    history.add(
        ResourceUsage::new()
            .with_tokens_used(5000)
            .with_cost_usd(0.50)
            .with_execution_time_ms(1000),
    );

    // 4000 tokens in 1000ms = 4 tokens/ms
    assert!((history.token_rate() - 4.0).abs() < f64::EPSILON);
    // 0.40 USD in 1000ms = 0.0004 USD/ms
    assert!((history.cost_rate() - 0.0004).abs() < 0.00001);
}

#[test]
fn test_resource_usage_history_estimate_time_to_limit() {
    let mut history = ResourceUsageHistory::new(10);
    history.add(
        ResourceUsage::new()
            .with_tokens_used(1000)
            .with_tokens_budget(10000)
            .with_execution_time_ms(0),
    );
    history.add(
        ResourceUsage::new()
            .with_tokens_used(2000)
            .with_tokens_budget(10000)
            .with_execution_time_ms(1000),
    );

    // Rate is 1 token/ms, 8000 remaining, estimate 8000ms
    let estimate = history.estimate_time_to_token_limit_ms().unwrap();
    assert_eq!(estimate, 8000);
}

#[test]
fn test_resource_usage_history_estimate_cost_limit() {
    let mut history = ResourceUsageHistory::new(10);
    history.add(
        ResourceUsage::new()
            .with_cost_usd(0.10)
            .with_cost_budget_usd(1.0)
            .with_execution_time_ms(0),
    );
    history.add(
        ResourceUsage::new()
            .with_cost_usd(0.20)
            .with_cost_budget_usd(1.0)
            .with_execution_time_ms(1000),
    );

    // Rate is 0.0001 USD/ms, 0.80 remaining, estimate 8000ms
    let estimate = history.estimate_time_to_cost_limit_ms().unwrap();
    assert_eq!(estimate, 8000);
}

#[test]
fn test_resource_usage_history_no_estimate_without_rate() {
    let history = ResourceUsageHistory::new(10);
    assert!(history.estimate_time_to_token_limit_ms().is_none());
    assert!(history.estimate_time_to_cost_limit_ms().is_none());
}

#[test]
fn test_resource_usage_history_usage_summary() {
    let empty = ResourceUsageHistory::new(10);
    assert_eq!(empty.usage_summary(), "No usage data available");

    let mut with_data = ResourceUsageHistory::new(10);
    with_data.add(
        ResourceUsage::new()
            .with_tokens_used(5000)
            .with_cost_usd(0.15)
            .with_api_calls(25),
    );

    let summary = with_data.usage_summary();
    assert!(summary.contains("Tokens: 5000"));
    assert!(summary.contains("Cost: $0.1500"));
    assert!(summary.contains("Status: OK"));
}

#[test]
fn test_resource_usage_history_json() {
    let mut history = ResourceUsageHistory::new(10).with_thread_id("test-thread");
    history.add(ResourceUsage::new().with_tokens_used(1000));

    let json = history.to_json().unwrap();
    assert!(json.contains("test-thread"));
    assert!(json.contains("snapshots"));

    let parsed = ResourceUsageHistory::from_json(&json).unwrap();
    assert_eq!(parsed.thread_id, Some("test-thread".to_string()));
    assert_eq!(parsed.len(), 1);
}

#[test]
fn test_resource_usage_default() {
    let default = ResourceUsage::default();
    assert_eq!(default.tokens_used, 0);
    assert_eq!(default.cost_usd, 0.0);
    assert!(default.custom.is_empty());
}

#[test]
fn test_resource_usage_history_default() {
    let default = ResourceUsageHistory::default();
    assert!(default.is_empty());
    assert_eq!(default.max_snapshots, 0);
    assert!(default.thread_id.is_none());
}

#[test]
fn test_resource_usage_threshold_clamping() {
    let usage = ResourceUsage::new()
        .with_tokens_used(5000)
        .with_tokens_budget(10000);

    // Threshold should be clamped to [0, 1]
    // At 50% usage:
    // - threshold -0.5 clamped to 0.0: 0.5 >= 0.0 is true
    // - threshold 1.5 clamped to 1.0: 0.5 >= 1.0 is false
    assert!(usage.is_near_token_limit(-0.5)); // Clamped to 0.0, 50% >= 0% = true
    assert!(!usage.is_near_token_limit(1.5)); // Clamped to 1.0, 50% >= 100% = false
}

#[test]
fn test_resource_usage_input_output_tokens_in_summary() {
    let usage = ResourceUsage::new()
        .with_input_tokens(3000)
        .with_output_tokens(2000);

    let summary = usage.summarize();
    assert!(summary.contains("Input: 3000"));
    assert!(summary.contains("Output: 2000"));
}

// ========================================================================
// Optimization Suggestions Tests
// ========================================================================

#[test]
fn test_optimization_category_display() {
    assert_eq!(format!("{}", OptimizationCategory::Caching), "caching");
    assert_eq!(
        format!("{}", OptimizationCategory::Parallelization),
        "parallelization"
    );
    assert_eq!(
        format!("{}", OptimizationCategory::ModelChoice),
        "model_choice"
    );
    assert_eq!(
        format!("{}", OptimizationCategory::TokenOptimization),
        "token_optimization"
    );
    assert_eq!(
        format!("{}", OptimizationCategory::ErrorHandling),
        "error_handling"
    );
    assert_eq!(
        format!("{}", OptimizationCategory::FrequencyReduction),
        "frequency_reduction"
    );
    assert_eq!(
        format!("{}", OptimizationCategory::Stabilization),
        "stabilization"
    );
    assert_eq!(
        format!("{}", OptimizationCategory::Performance),
        "performance"
    );
}

#[test]
fn test_optimization_priority_display() {
    assert_eq!(format!("{}", OptimizationPriority::Low), "low");
    assert_eq!(format!("{}", OptimizationPriority::Medium), "medium");
    assert_eq!(format!("{}", OptimizationPriority::High), "high");
    assert_eq!(format!("{}", OptimizationPriority::Critical), "critical");
}

#[test]
fn test_optimization_priority_ordering() {
    assert!(OptimizationPriority::Low < OptimizationPriority::Medium);
    assert!(OptimizationPriority::Medium < OptimizationPriority::High);
    assert!(OptimizationPriority::High < OptimizationPriority::Critical);
}

#[test]
fn test_optimization_suggestion_new() {
    let suggestion = OptimizationSuggestion::new(
        OptimizationCategory::Caching,
        vec!["node1".to_string()],
        "Add caching",
        "50% faster",
        "Use Redis cache",
    );

    assert_eq!(suggestion.category, OptimizationCategory::Caching);
    assert_eq!(suggestion.target_nodes, vec!["node1".to_string()]);
    assert_eq!(suggestion.description, "Add caching");
    assert_eq!(suggestion.expected_improvement, "50% faster");
    assert_eq!(suggestion.implementation, "Use Redis cache");
    assert_eq!(suggestion.priority, OptimizationPriority::Medium);
    assert_eq!(suggestion.effort, 3);
    assert_eq!(suggestion.confidence, 0.5);
    assert!(suggestion.related_bottleneck.is_none());
    assert!(suggestion.evidence.is_empty());
}

#[test]
fn test_optimization_suggestion_builder() {
    let suggestion = OptimizationSuggestion::builder()
        .category(OptimizationCategory::Parallelization)
        .target_node("node_a")
        .target_node("node_b")
        .description("Run in parallel")
        .expected_improvement("40% time reduction")
        .implementation("Use async execution")
        .priority(OptimizationPriority::High)
        .effort(2)
        .confidence(0.8)
        .related_bottleneck(BottleneckMetric::Latency)
        .evidence("Sequential execution detected")
        .build()
        .unwrap();

    assert_eq!(suggestion.category, OptimizationCategory::Parallelization);
    assert_eq!(suggestion.target_nodes.len(), 2);
    assert_eq!(suggestion.priority, OptimizationPriority::High);
    assert_eq!(suggestion.effort, 2);
    assert_eq!(suggestion.confidence, 0.8);
    assert_eq!(
        suggestion.related_bottleneck,
        Some(BottleneckMetric::Latency)
    );
    assert_eq!(suggestion.evidence.len(), 1);
}

#[test]
fn test_optimization_suggestion_builder_missing_fields() {
    let result = OptimizationSuggestion::builder()
        .description("Missing category")
        .build();
    assert!(result.is_err());

    let result = OptimizationSuggestion::builder()
        .category(OptimizationCategory::Caching)
        .build();
    assert!(result.is_err()); // missing description

    let result = OptimizationSuggestion::builder()
        .category(OptimizationCategory::Caching)
        .description("test")
        .expected_improvement("better")
        .implementation("do something")
        .build();
    assert!(result.is_err()); // missing target nodes
}

#[test]
fn test_optimization_suggestion_with_methods() {
    let suggestion = OptimizationSuggestion::new(
        OptimizationCategory::Caching,
        vec!["node1".to_string()],
        "desc",
        "improvement",
        "impl",
    )
    .with_priority(OptimizationPriority::Critical)
    .with_effort(1)
    .with_confidence(0.95)
    .with_related_bottleneck(BottleneckMetric::HighFrequency)
    .with_evidence("evidence 1")
    .with_evidence("evidence 2");

    assert_eq!(suggestion.priority, OptimizationPriority::Critical);
    assert_eq!(suggestion.effort, 1);
    assert_eq!(suggestion.confidence, 0.95);
    assert_eq!(
        suggestion.related_bottleneck,
        Some(BottleneckMetric::HighFrequency)
    );
    assert_eq!(suggestion.evidence.len(), 2);
}

#[test]
fn test_optimization_suggestion_effort_clamping() {
    let suggestion = OptimizationSuggestion::new(
        OptimizationCategory::Caching,
        vec!["n".to_string()],
        "d",
        "i",
        "p",
    )
    .with_effort(10); // should clamp to 5
    assert_eq!(suggestion.effort, 5);

    let suggestion = suggestion.with_effort(0); // should clamp to 1
    assert_eq!(suggestion.effort, 1);
}

#[test]
fn test_optimization_suggestion_confidence_clamping() {
    let suggestion = OptimizationSuggestion::new(
        OptimizationCategory::Caching,
        vec!["n".to_string()],
        "d",
        "i",
        "p",
    )
    .with_confidence(1.5); // should clamp to 1.0
    assert_eq!(suggestion.confidence, 1.0);

    let suggestion = suggestion.with_confidence(-0.5); // should clamp to 0.0
    assert_eq!(suggestion.confidence, 0.0);
}

#[test]
fn test_optimization_suggestion_is_high_priority() {
    let low = OptimizationSuggestion::new(
        OptimizationCategory::Caching,
        vec!["n".to_string()],
        "d",
        "i",
        "p",
    )
    .with_priority(OptimizationPriority::Low);
    assert!(!low.is_high_priority());

    let medium = low.clone().with_priority(OptimizationPriority::Medium);
    assert!(!medium.is_high_priority());

    let high = low.clone().with_priority(OptimizationPriority::High);
    assert!(high.is_high_priority());

    let critical = low.with_priority(OptimizationPriority::Critical);
    assert!(critical.is_high_priority());
}

#[test]
fn test_optimization_suggestion_is_low_effort() {
    let low_effort = OptimizationSuggestion::new(
        OptimizationCategory::Caching,
        vec!["n".to_string()],
        "d",
        "i",
        "p",
    )
    .with_effort(2);
    assert!(low_effort.is_low_effort());

    let high_effort = low_effort.with_effort(3);
    assert!(!high_effort.is_low_effort());
}

#[test]
fn test_optimization_suggestion_quick_win_score() {
    // High priority, low effort, high confidence = good quick win
    let good_quick_win = OptimizationSuggestion::new(
        OptimizationCategory::Caching,
        vec!["n".to_string()],
        "d",
        "i",
        "p",
    )
    .with_priority(OptimizationPriority::Critical)
    .with_effort(1)
    .with_confidence(1.0);

    // Low priority, high effort, low confidence = bad quick win
    let bad_quick_win = OptimizationSuggestion::new(
        OptimizationCategory::Caching,
        vec!["n".to_string()],
        "d",
        "i",
        "p",
    )
    .with_priority(OptimizationPriority::Low)
    .with_effort(5)
    .with_confidence(0.2);

    assert!(good_quick_win.quick_win_score() > bad_quick_win.quick_win_score());
}

#[test]
fn test_optimization_suggestion_summary() {
    let suggestion = OptimizationSuggestion::new(
        OptimizationCategory::Caching,
        vec!["node1".to_string(), "node2".to_string()],
        "Add caching layer",
        "50% faster",
        "impl",
    )
    .with_priority(OptimizationPriority::High);

    let summary = suggestion.summary();
    assert!(summary.contains("[high]"));
    assert!(summary.contains("caching"));
    assert!(summary.contains("node1, node2"));
    assert!(summary.contains("Add caching layer"));
    assert!(summary.contains("50% faster"));
}

#[test]
fn test_optimization_suggestion_json_roundtrip() {
    let suggestion = OptimizationSuggestion::new(
        OptimizationCategory::Parallelization,
        vec!["node1".to_string()],
        "Parallelize operations",
        "30% faster",
        "Use async",
    )
    .with_priority(OptimizationPriority::High)
    .with_effort(2)
    .with_confidence(0.7);

    let json = suggestion.to_json().unwrap();
    let parsed = OptimizationSuggestion::from_json(&json).unwrap();

    assert_eq!(parsed.category, suggestion.category);
    assert_eq!(parsed.target_nodes, suggestion.target_nodes);
    assert_eq!(parsed.priority, suggestion.priority);
}

#[test]
fn test_optimization_analysis_new() {
    let analysis = OptimizationAnalysis::new();
    assert!(!analysis.has_suggestions());
    assert_eq!(analysis.suggestion_count(), 0);
    assert_eq!(analysis.patterns_analyzed, 0);
    assert_eq!(analysis.health_score, 1.0);
}

#[test]
fn test_optimization_analysis_by_category() {
    let mut analysis = OptimizationAnalysis::new();
    analysis.suggestions.push(OptimizationSuggestion::new(
        OptimizationCategory::Caching,
        vec!["n1".to_string()],
        "d",
        "i",
        "p",
    ));
    analysis.suggestions.push(OptimizationSuggestion::new(
        OptimizationCategory::Caching,
        vec!["n2".to_string()],
        "d",
        "i",
        "p",
    ));
    analysis.suggestions.push(OptimizationSuggestion::new(
        OptimizationCategory::Parallelization,
        vec!["n3".to_string()],
        "d",
        "i",
        "p",
    ));

    assert_eq!(
        analysis.by_category(&OptimizationCategory::Caching).len(),
        2
    );
    assert_eq!(
        analysis
            .by_category(&OptimizationCategory::Parallelization)
            .len(),
        1
    );
    assert_eq!(
        analysis
            .by_category(&OptimizationCategory::ErrorHandling)
            .len(),
        0
    );
}

#[test]
fn test_optimization_analysis_by_priority() {
    let mut analysis = OptimizationAnalysis::new();
    analysis.suggestions.push(
        OptimizationSuggestion::new(
            OptimizationCategory::Caching,
            vec!["n".to_string()],
            "d",
            "i",
            "p",
        )
        .with_priority(OptimizationPriority::High),
    );
    analysis.suggestions.push(
        OptimizationSuggestion::new(
            OptimizationCategory::Caching,
            vec!["n".to_string()],
            "d",
            "i",
            "p",
        )
        .with_priority(OptimizationPriority::High),
    );
    analysis.suggestions.push(
        OptimizationSuggestion::new(
            OptimizationCategory::Caching,
            vec!["n".to_string()],
            "d",
            "i",
            "p",
        )
        .with_priority(OptimizationPriority::Low),
    );

    assert_eq!(analysis.by_priority(OptimizationPriority::High).len(), 2);
    assert_eq!(analysis.by_priority(OptimizationPriority::Low).len(), 1);
    assert_eq!(analysis.high_priority().len(), 2);
}

#[test]
fn test_optimization_analysis_for_node() {
    let mut analysis = OptimizationAnalysis::new();
    analysis.suggestions.push(OptimizationSuggestion::new(
        OptimizationCategory::Caching,
        vec!["node_a".to_string()],
        "d",
        "i",
        "p",
    ));
    analysis.suggestions.push(OptimizationSuggestion::new(
        OptimizationCategory::Parallelization,
        vec!["node_a".to_string(), "node_b".to_string()],
        "d",
        "i",
        "p",
    ));
    analysis.suggestions.push(OptimizationSuggestion::new(
        OptimizationCategory::ErrorHandling,
        vec!["node_c".to_string()],
        "d",
        "i",
        "p",
    ));

    assert_eq!(analysis.for_node("node_a").len(), 2);
    assert_eq!(analysis.for_node("node_b").len(), 1);
    assert_eq!(analysis.for_node("node_c").len(), 1);
    assert_eq!(analysis.for_node("node_d").len(), 0);
}

#[test]
fn test_optimization_analysis_quick_wins() {
    let mut analysis = OptimizationAnalysis::new();
    // Add multiple suggestions with different quick win scores
    for i in 0..10 {
        let priority = match i % 4 {
            0 => OptimizationPriority::Low,
            1 => OptimizationPriority::Medium,
            2 => OptimizationPriority::High,
            _ => OptimizationPriority::Critical,
        };
        analysis.suggestions.push(
            OptimizationSuggestion::new(
                OptimizationCategory::Caching,
                vec![format!("n{}", i)],
                "d",
                "i",
                "p",
            )
            .with_priority(priority)
            .with_effort((i % 5 + 1) as u8)
            .with_confidence(0.5 + (i as f64 * 0.05)),
        );
    }

    let quick_wins = analysis.quick_wins();
    assert_eq!(quick_wins.len(), 5); // Returns top 5
                                     // First should have highest quick win score
    assert!(quick_wins[0].quick_win_score() >= quick_wins[4].quick_win_score());
}

#[test]
fn test_optimization_analysis_json_roundtrip() {
    let mut analysis = OptimizationAnalysis::new();
    analysis.patterns_analyzed = 7;
    analysis.health_score = 0.85;
    analysis.summary = "Test summary".to_string();
    analysis.suggestions.push(OptimizationSuggestion::new(
        OptimizationCategory::Caching,
        vec!["n".to_string()],
        "d",
        "i",
        "p",
    ));

    let json = analysis.to_json().unwrap();
    let parsed = OptimizationAnalysis::from_json(&json).unwrap();

    assert_eq!(parsed.patterns_analyzed, 7);
    assert_eq!(parsed.health_score, 0.85);
    assert_eq!(parsed.suggestion_count(), 1);
}

#[test]
fn test_suggest_optimizations_empty_trace() {
    let trace = ExecutionTrace::new();
    let analysis = trace.suggest_optimizations();

    assert!(!analysis.has_suggestions());
    assert_eq!(analysis.patterns_analyzed, 7);
    assert_eq!(analysis.health_score, 1.0);
}

#[test]
fn test_suggest_optimizations_healthy_trace() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("node_a", 100),
            NodeExecution::new("node_b", 100),
        ],
        total_duration_ms: 200,
        total_tokens: 200,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.suggest_optimizations();
    // Healthy trace should have few or no suggestions
    assert!(analysis.health_score > 0.8);
}

#[test]
fn test_suggest_optimizations_caching_opportunity() {
    // Node executed 5 times with consistent duration = caching opportunity
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("repeated_node", 100),
            NodeExecution::new("repeated_node", 95),
            NodeExecution::new("repeated_node", 105),
            NodeExecution::new("repeated_node", 100),
            NodeExecution::new("repeated_node", 100),
        ],
        total_duration_ms: 500,
        total_tokens: 500,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.suggest_optimizations();
    let caching = analysis.by_category(&OptimizationCategory::Caching);
    assert!(!caching.is_empty());
    assert!(caching
        .iter()
        .any(|s| s.target_nodes.contains(&"repeated_node".to_string())));
}

#[test]
fn test_suggest_optimizations_error_handling() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("error_prone", 100).with_error("Failed"),
            NodeExecution::new("error_prone", 100).with_error("Failed"),
            NodeExecution::new("error_prone", 100),
            NodeExecution::new("error_prone", 100),
        ],
        total_duration_ms: 400,
        total_tokens: 400,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.suggest_optimizations();
    let error_handling = analysis.by_category(&OptimizationCategory::ErrorHandling);
    assert!(!error_handling.is_empty());
    // 50% error rate should be high/critical priority
    assert!(error_handling.iter().any(|s| s.is_high_priority()));
}

#[test]
fn test_suggest_optimizations_token_optimization() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("token_hog", 1000).with_tokens(5000),
            NodeExecution::new("efficient", 1000).with_tokens(500),
        ],
        total_duration_ms: 2000,
        total_tokens: 5500,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.suggest_optimizations();
    let token_opts = analysis.by_category(&OptimizationCategory::TokenOptimization);
    // token_hog uses 5000/5500 = 90%+ of total tokens
    assert!(!token_opts.is_empty());
    assert!(token_opts
        .iter()
        .any(|s| s.target_nodes.contains(&"token_hog".to_string())));
}

#[test]
fn test_suggest_optimizations_frequency_reduction() {
    // Node executed 25 times = frequency reduction opportunity
    let mut nodes = Vec::new();
    for i in 0..25 {
        nodes.push(NodeExecution::new("loop_node", 10).with_index(i));
    }

    let trace = ExecutionTrace {
        nodes_executed: nodes,
        total_duration_ms: 250,
        total_tokens: 250,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.suggest_optimizations();
    let freq = analysis.by_category(&OptimizationCategory::FrequencyReduction);
    assert!(!freq.is_empty());
}

#[test]
fn test_suggest_optimizations_stabilization() {
    // Node with high variance in execution time
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("unstable", 10),
            NodeExecution::new("unstable", 100),
            NodeExecution::new("unstable", 500),
            NodeExecution::new("unstable", 50),
        ],
        total_duration_ms: 660,
        total_tokens: 400,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.suggest_optimizations();
    let stabilization = analysis.by_category(&OptimizationCategory::Stabilization);
    // High coefficient of variation should trigger stabilization suggestion
    assert!(!stabilization.is_empty());
}

#[test]
fn test_suggest_optimizations_model_choice() {
    // Node with high tokens but fast execution = possible model over-provisioning
    let trace = ExecutionTrace {
        nodes_executed: vec![NodeExecution::new("simple_task", 200).with_tokens(2000)],
        total_duration_ms: 200,
        total_tokens: 2000,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.suggest_optimizations();
    let model_choice = analysis.by_category(&OptimizationCategory::ModelChoice);
    assert!(!model_choice.is_empty());
}

#[test]
fn test_suggest_optimizations_health_score_degradation() {
    // Trace with multiple issues should have lower health score
    let mut nodes = Vec::new();
    for i in 0..50 {
        let node = if i % 3 == 0 {
            NodeExecution::new("loop_node", 100).with_error("Failed")
        } else {
            NodeExecution::new("loop_node", 100)
        };
        nodes.push(node);
    }

    let trace = ExecutionTrace {
        nodes_executed: nodes,
        total_duration_ms: 5000,
        total_tokens: 5000,
        completed: false,
        ..Default::default()
    };

    let analysis = trace.suggest_optimizations();
    // Multiple issues should reduce health score
    assert!(analysis.health_score < 0.9);
    assert!(analysis.has_suggestions());
}

#[test]
fn test_suggest_optimizations_summary_generation() {
    let trace = ExecutionTrace {
        nodes_executed: vec![NodeExecution::new("error_node", 100).with_error("Failed")],
        total_duration_ms: 100,
        total_tokens: 100,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.suggest_optimizations();
    assert!(!analysis.summary.is_empty());
    // Summary should contain info about suggestions found
    if analysis.has_suggestions() {
        assert!(analysis.summary.contains("optimization suggestions"));
    }
}

#[test]
fn test_suggest_optimizations_sorted_by_priority() {
    // Create trace that triggers multiple suggestions
    let mut nodes = Vec::new();
    // Add error-prone node (will be high priority)
    nodes.push(NodeExecution::new("error_node", 100).with_error("Failed"));
    // Add repeated node (will be lower priority)
    for _ in 0..5 {
        nodes.push(NodeExecution::new("repeated", 10));
    }

    let trace = ExecutionTrace {
        nodes_executed: nodes,
        total_duration_ms: 150,
        total_tokens: 150,
        completed: true,
        ..Default::default()
    };

    let analysis = trace.suggest_optimizations();
    // Verify suggestions are sorted by priority (highest first)
    if analysis.suggestions.len() >= 2 {
        for i in 0..analysis.suggestions.len() - 1 {
            assert!(analysis.suggestions[i].priority >= analysis.suggestions[i + 1].priority);
        }
    }
}

// ========================================================================
// Prompt Self-Evolution Integration Tests
// ========================================================================

#[test]
fn test_execution_trace_analyze_prompt_effectiveness_empty() {
    let trace = ExecutionTrace::new();
    let analyses = trace.analyze_prompt_effectiveness();
    assert!(analyses.is_empty());
}

#[test]
fn test_execution_trace_analyze_prompt_effectiveness_healthy() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("reasoning", 100).with_index(0),
            NodeExecution::new("tool_call", 200).with_index(1),
            NodeExecution::new("output", 50).with_index(2),
        ],
        total_duration_ms: 350,
        total_tokens: 1000,
        completed: true,
        ..Default::default()
    };

    let analyses = trace.analyze_prompt_effectiveness();
    assert_eq!(analyses.len(), 3);

    // No issues expected for healthy execution
    for analysis in &analyses {
        assert_eq!(analysis.execution_count, 1);
        // With only 1 execution, min_executions (3) threshold won't be met
        // so issues won't be flagged
    }
}

#[test]
fn test_execution_trace_analyze_prompt_effectiveness_with_errors() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("flaky_node", 100)
                .with_index(0)
                .with_error("Error 1"),
            NodeExecution::new("flaky_node", 110)
                .with_index(1)
                .with_error("Error 2"),
            NodeExecution::new("flaky_node", 105).with_index(2),
            NodeExecution::new("flaky_node", 108).with_index(3),
        ],
        total_duration_ms: 423,
        total_tokens: 500,
        completed: true,
        ..Default::default()
    };

    let analyses = trace.analyze_prompt_effectiveness();
    assert_eq!(analyses.len(), 1);

    let analysis = &analyses[0];
    assert_eq!(analysis.node, "flaky_node");
    assert_eq!(analysis.execution_count, 4);
    assert_eq!(analysis.failure_count, 2);
    assert_eq!(analysis.error_rate, 0.5); // 50% error rate
    assert!(analysis.has_issues());
}

#[test]
fn test_execution_trace_evolve_prompts_empty() {
    let trace = ExecutionTrace::new();
    let evolutions = trace.evolve_prompts();
    assert!(evolutions.is_empty());
}

#[test]
fn test_execution_trace_evolve_prompts_generates_suggestions() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            // High token usage node
            NodeExecution::new("verbose_node", 1000)
                .with_tokens(10000)
                .with_index(0),
            NodeExecution::new("verbose_node", 1100)
                .with_tokens(11000)
                .with_index(1),
            NodeExecution::new("verbose_node", 1050)
                .with_tokens(10500)
                .with_index(2),
        ],
        total_duration_ms: 3150,
        total_tokens: 31500,
        completed: true,
        ..Default::default()
    };

    let evolutions = trace.evolve_prompts();
    // Should generate evolutions for token inefficiency
    assert!(!evolutions.is_empty());
}

#[test]
fn test_execution_trace_has_prompt_issues() {
    // Healthy trace
    let healthy_trace = ExecutionTrace {
        nodes_executed: vec![NodeExecution::new("node_a", 100).with_index(0)],
        total_duration_ms: 100,
        completed: true,
        ..Default::default()
    };
    // Single execution won't trigger issues (need min 3)
    assert!(!healthy_trace.has_prompt_issues());

    // Unhealthy trace with errors
    let unhealthy_trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("node_a", 100)
                .with_index(0)
                .with_error("Error"),
            NodeExecution::new("node_a", 100)
                .with_index(1)
                .with_error("Error"),
            NodeExecution::new("node_a", 100)
                .with_index(2)
                .with_error("Error"),
        ],
        total_duration_ms: 300,
        completed: true,
        ..Default::default()
    };
    assert!(unhealthy_trace.has_prompt_issues());
}

#[test]
fn test_execution_trace_prompt_health_summary_healthy() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("reasoning", 100).with_index(0),
            NodeExecution::new("output", 50).with_index(1),
        ],
        total_duration_ms: 150,
        completed: true,
        ..Default::default()
    };

    let summary = trace.prompt_health_summary();
    assert!(summary.contains("healthy prompts"));
    assert!(summary.contains("no detected issues"));
}

#[test]
fn test_execution_trace_prompt_health_summary_unhealthy() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("failing_node", 100)
                .with_index(0)
                .with_error("Error"),
            NodeExecution::new("failing_node", 100)
                .with_index(1)
                .with_error("Error"),
            NodeExecution::new("failing_node", 100)
                .with_index(2)
                .with_error("Error"),
        ],
        total_duration_ms: 300,
        completed: true,
        ..Default::default()
    };

    let summary = trace.prompt_health_summary();
    assert!(summary.contains("prompt issues"));
    assert!(summary.contains("failing_node"));
}

#[test]
fn test_execution_trace_analyze_with_custom_thresholds() {
    let trace = ExecutionTrace {
        // Non-consecutive indices to avoid retry detection
        // 5 executions to meet strict min_executions threshold
        nodes_executed: vec![
            NodeExecution::new("node_a", 100)
                .with_tokens(2500)
                .with_index(0),
            NodeExecution::new("node_a", 110)
                .with_tokens(2600)
                .with_index(5), // Not consecutive
            NodeExecution::new("node_a", 105)
                .with_tokens(2550)
                .with_index(10), // Not consecutive
            NodeExecution::new("node_a", 95)
                .with_tokens(2450)
                .with_index(15), // Not consecutive
            NodeExecution::new("node_a", 115)
                .with_tokens(2700)
                .with_index(20), // Not consecutive
        ],
        total_duration_ms: 525,
        total_tokens: 12800,
        completed: true,
        ..Default::default()
    };

    // With default thresholds (5000 tokens), no issue
    let default_analyses = trace.analyze_prompt_effectiveness();
    let default_issues: Vec<_> = default_analyses.iter().filter(|a| a.has_issues()).collect();
    assert!(default_issues.is_empty());

    // With strict thresholds (2000 tokens, min 5 executions), should flag
    let strict = crate::prompt_evolution::PromptThresholds::strict();
    let strict_analyses = trace.analyze_prompt_effectiveness_with_thresholds(&strict);
    let strict_issues: Vec<_> = strict_analyses.iter().filter(|a| a.has_issues()).collect();
    assert!(!strict_issues.is_empty());
}

// ========================================================================
// Adaptive Timeout Integration Tests
// ========================================================================

#[test]
fn test_execution_trace_collect_latency_stats() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("node_a", 100),
            NodeExecution::new("node_a", 120),
            NodeExecution::new("node_a", 110),
            NodeExecution::new("node_b", 200),
            NodeExecution::new("node_b", 220),
        ],
        total_duration_ms: 750,
        completed: true,
        ..Default::default()
    };

    let stats = trace.collect_latency_stats();
    assert_eq!(stats.len(), 2); // Two unique nodes

    // Find stats for node_a
    let node_a_stats = stats.iter().find(|s| s.node == "node_a").unwrap();
    assert_eq!(node_a_stats.sample_count, 3);
    assert_eq!(node_a_stats.min_ms, 100);
    assert_eq!(node_a_stats.max_ms, 120);

    // Find stats for node_b
    let node_b_stats = stats.iter().find(|s| s.node == "node_b").unwrap();
    assert_eq!(node_b_stats.sample_count, 2);
}

#[test]
fn test_execution_trace_collect_latency_stats_empty() {
    let trace = ExecutionTrace::default();
    let stats = trace.collect_latency_stats();
    assert!(stats.is_empty());
}

#[test]
fn test_execution_trace_calculate_optimal_timeouts() {
    // Create a trace with enough samples (10+) for high confidence recommendations
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("stable_node", 100),
            NodeExecution::new("stable_node", 102),
            NodeExecution::new("stable_node", 98),
            NodeExecution::new("stable_node", 101),
            NodeExecution::new("stable_node", 99),
            NodeExecution::new("stable_node", 103),
            NodeExecution::new("stable_node", 97),
            NodeExecution::new("stable_node", 104),
            NodeExecution::new("stable_node", 96),
            NodeExecution::new("stable_node", 100),
        ],
        total_duration_ms: 1000,
        completed: true,
        ..Default::default()
    };

    let recommendations = trace.calculate_optimal_timeouts();
    // Should have recommendation since we have 10 samples (confidence >= 0.5)
    assert!(recommendations.has_recommendations());
    assert!(!recommendations.recommendations.is_empty());

    let rec = &recommendations.recommendations[0];
    assert_eq!(rec.node, "stable_node");
    // Timeout should be p95 * 1.5 (buffer), at minimum 100ms
    assert!(rec.recommended_timeout_ms >= 100);
}

#[test]
fn test_execution_trace_calculate_optimal_timeouts_insufficient_data() {
    // Only 2 samples - below default min_samples of 5
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("rare_node", 100),
            NodeExecution::new("rare_node", 110),
        ],
        total_duration_ms: 210,
        completed: true,
        ..Default::default()
    };

    let recommendations = trace.calculate_optimal_timeouts();
    // Should not have recommendations for rare_node
    assert!(recommendations
        .insufficient_data_nodes
        .contains(&"rare_node".to_string()));
}

#[test]
fn test_execution_trace_get_timeout_mutations() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("test_node", 100),
            NodeExecution::new("test_node", 105),
            NodeExecution::new("test_node", 102),
            NodeExecution::new("test_node", 98),
            NodeExecution::new("test_node", 101),
            NodeExecution::new("test_node", 103),
            NodeExecution::new("test_node", 99),
            NodeExecution::new("test_node", 104),
            NodeExecution::new("test_node", 97),
            NodeExecution::new("test_node", 100),
        ],
        total_duration_ms: 1009,
        completed: true,
        ..Default::default()
    };

    let mutations = trace.get_timeout_mutations(0.5);
    // Should get at least one mutation for the well-sampled node
    assert!(!mutations.is_empty());

    // Verify mutation type
    match &mutations[0].mutation_type {
        crate::graph_reconfiguration::MutationType::AdjustTimeout { node, .. } => {
            assert_eq!(node, "test_node");
        }
        _ => panic!("Expected AdjustTimeout mutation"),
    }
}

#[test]
fn test_execution_trace_has_timeout_optimization_opportunities() {
    // With enough data
    let trace = ExecutionTrace {
        nodes_executed: (0..10)
            .map(|i| NodeExecution::new("node", 100 + i as u64 % 5))
            .collect(),
        total_duration_ms: 1025,
        completed: true,
        ..Default::default()
    };
    assert!(trace.has_timeout_optimization_opportunities());

    // Without enough data
    let sparse_trace = ExecutionTrace {
        nodes_executed: vec![NodeExecution::new("node", 100)],
        total_duration_ms: 100,
        completed: true,
        ..Default::default()
    };
    assert!(!sparse_trace.has_timeout_optimization_opportunities());
}

#[test]
fn test_execution_trace_timeout_optimization_summary() {
    let trace = ExecutionTrace {
        nodes_executed: (0..10)
            .map(|i| NodeExecution::new("api_call", 100 + i as u64 * 5))
            .collect(),
        total_duration_ms: 1225,
        completed: true,
        ..Default::default()
    };

    let summary = trace.timeout_optimization_summary();
    assert!(summary.contains("api_call") || summary.contains("Analyzed"));
}

#[test]
fn test_execution_trace_timeout_optimization_summary_no_opportunities() {
    let trace = ExecutionTrace {
        nodes_executed: vec![NodeExecution::new("single", 100)],
        total_duration_ms: 100,
        completed: true,
        ..Default::default()
    };

    let summary = trace.timeout_optimization_summary();
    assert!(summary.contains("No timeout optimization opportunities"));
}

#[test]
fn test_execution_trace_calculate_optimal_timeouts_with_config() {
    // Create trace with enough samples for confidence >= aggressive min_confidence (0.4)
    // Need ~8 samples for 0.4 confidence (8/20 = 0.4)
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("fast_node", 50),
            NodeExecution::new("fast_node", 55),
            NodeExecution::new("fast_node", 52),
            NodeExecution::new("fast_node", 48),
            NodeExecution::new("fast_node", 53),
            NodeExecution::new("fast_node", 51),
            NodeExecution::new("fast_node", 54),
            NodeExecution::new("fast_node", 49),
        ],
        total_duration_ms: 412,
        completed: true,
        ..Default::default()
    };

    // With aggressive config (min_samples = 3, min_confidence = 0.4)
    let aggressive_config = crate::adaptive_timeout::TimeoutConfig::aggressive();
    let recommendations = trace.calculate_optimal_timeouts_with_config(&aggressive_config);

    // Should have recommendation with aggressive config
    assert!(recommendations.has_recommendations());
}

// =========================================================================
// Telemetry Unification Tests (DashOpt Integration)
// =========================================================================

#[test]
fn test_execution_trace_to_examples() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("llm_node", 100)
                .with_state_before(serde_json::json!({"input": "What is Rust?"}))
                .with_state_after(serde_json::json!({"output": "A systems programming language"})),
            NodeExecution::new("no_state_node", 50),
            NodeExecution::new("partial_state", 75).with_state_before(serde_json::json!({"x": 1})),
        ],
        completed: true,
        ..Default::default()
    };

    let examples = trace.to_examples();

    // Only nodes with both state_before and state_after produce examples
    assert_eq!(examples.len(), 1);

    let example = &examples[0];
    assert_eq!(
        example.get("input_input"),
        Some(&serde_json::json!("What is Rust?"))
    );
    assert_eq!(
        example.get("output_output"),
        Some(&serde_json::json!("A systems programming language"))
    );
    assert_eq!(example.get("_node"), Some(&serde_json::json!("llm_node")));
    assert_eq!(example.get("_success"), Some(&serde_json::json!(true)));
}

#[test]
#[allow(deprecated)] // Tests use deprecated TraceEntry for backward compatibility verification
fn test_execution_trace_to_trace_entries() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("node1", 100)
                .with_state_before(serde_json::json!({"query": "test"}))
                .with_state_after(serde_json::json!({"result": "success"})),
            NodeExecution::new("node2", 50).with_error("Timeout"),
        ],
        completed: true,
        ..Default::default()
    };

    let entries = trace.to_trace_entries();

    assert_eq!(entries.len(), 2);

    // First entry should be successful
    assert_eq!(entries[0].predictor_name, "node1");
    assert_eq!(
        entries[0].inputs.get("query"),
        Some(&serde_json::json!("test"))
    );
    assert!(entries[0].outputs.is_success());

    // Second entry should be failed
    assert_eq!(entries[1].predictor_name, "node2");
    assert!(entries[1].outputs.is_failed());
    if let crate::optimize::PredictionOrFailed::Failed(f) = &entries[1].outputs {
        assert_eq!(f.error, "Timeout");
    }
}

#[test]
fn test_execution_trace_to_trace_data() {
    let trace = ExecutionTrace {
        nodes_executed: vec![NodeExecution::new("calc", 50)
            .with_state_before(serde_json::json!({"x": 6, "y": 7}))
            .with_state_after(serde_json::json!({"result": 42}))],
        completed: true,
        final_state: Some(serde_json::json!({"result": 42})),
        ..Default::default()
    };

    let example = crate::optimize::Example::new().with("question", "What is 6 * 7?");

    let trace_data = trace.to_trace_data(example, 5, Some(1.0));

    assert_eq!(trace_data.example_ind, 5);
    assert_eq!(trace_data.score, Some(1.0));
    assert_eq!(trace_data.trace.len(), 1);
    assert!(trace_data.is_success());
}

#[test]
fn test_execution_trace_final_prediction_success() {
    let trace = ExecutionTrace {
        nodes_executed: vec![NodeExecution::new("node", 50)],
        completed: true,
        final_state: Some(serde_json::json!({"answer": 42, "confidence": 0.95})),
        ..Default::default()
    };

    let prediction = trace.final_prediction();
    assert!(prediction.is_success());

    if let crate::optimize::PredictionOrFailed::Success(p) = &prediction {
        assert_eq!(p.get("answer"), Some(&serde_json::json!(42)));
        assert_eq!(p.get("confidence"), Some(&serde_json::json!(0.95)));
    }
}

#[test]
fn test_execution_trace_final_prediction_failure() {
    let trace = ExecutionTrace {
        nodes_executed: vec![NodeExecution::new("node", 50)],
        completed: false,
        errors: vec![ErrorTrace {
            node: "node".to_string(),
            message: "Connection refused".to_string(),
            error_type: Some("NetworkError".to_string()),
            state_at_error: None,
            timestamp: None,
            execution_index: Some(0),
            recoverable: false,
            retry_attempted: false,
            context: None,
            metadata: HashMap::new(),
        }],
        ..Default::default()
    };

    let prediction = trace.final_prediction();
    assert!(prediction.is_failed());

    if let crate::optimize::PredictionOrFailed::Failed(f) = &prediction {
        assert_eq!(f.error, "Connection refused");
    }
}

#[test]
fn test_execution_trace_has_training_data() {
    // No state snapshots
    let trace1 = ExecutionTrace {
        nodes_executed: vec![NodeExecution::new("node", 50)],
        completed: true,
        ..Default::default()
    };
    assert!(!trace1.has_training_data());

    // With state snapshots
    let trace2 = ExecutionTrace {
        nodes_executed: vec![NodeExecution::new("node", 50)
            .with_state_before(serde_json::json!({"x": 1}))
            .with_state_after(serde_json::json!({"y": 2}))],
        completed: true,
        ..Default::default()
    };
    assert!(trace2.has_training_data());
}

#[test]
fn test_execution_trace_training_example_count() {
    let trace = ExecutionTrace {
        nodes_executed: vec![
            NodeExecution::new("n1", 50)
                .with_state_before(serde_json::json!({}))
                .with_state_after(serde_json::json!({})),
            NodeExecution::new("n2", 50), // No state
            NodeExecution::new("n3", 50)
                .with_state_before(serde_json::json!({}))
                .with_state_after(serde_json::json!({})),
        ],
        completed: true,
        ..Default::default()
    };

    assert_eq!(trace.training_example_count(), 2);
}

// =========================================================================
// OptimizationTrace Tests (Meta-Learning Telemetry)
// =========================================================================

#[test]
fn test_optimization_trace_new() {
    let trace = OptimizationTrace::new("opt_001");
    assert_eq!(trace.optimization_id, "opt_001");
    assert!(trace.started_at.is_some());
    assert!(trace.variants_tested.is_empty());
    assert!(!trace.found_improvement());
}

#[test]
fn test_optimization_trace_builder() {
    let trace = OptimizationTrace::new("opt_002")
        .with_strategy_name("Joint")
        .with_target_node("llm_node")
        .with_target_param("temperature")
        .with_duration_ms(60000)
        .with_improvement_delta(0.15)
        .with_initial_score(0.70)
        .with_termination_reason(TerminationReason::ConvergenceThreshold(0.01))
        .complete();

    assert_eq!(trace.target_node, "llm_node");
    assert_eq!(trace.target_param, "temperature");
    assert_eq!(trace.total_duration_ms, 60000);
    assert!((trace.improvement_delta - 0.15).abs() < 0.001);
    assert!(trace.ended_at.is_some());
}

#[test]
fn test_optimization_trace_with_variants() {
    let variant1 = VariantResult::new("v1")
        .with_score(0.75)
        .with_execution_trace_id("exec_001")
        .with_metric("latency_ms", 250.0);

    let variant2 = VariantResult::new("v2")
        .with_score(0.85)
        .with_execution_trace_id("exec_002");

    let trace = OptimizationTrace::new("opt_003")
        .with_target_node("test_node")
        .with_target_param("prompt")
        .with_variant(variant1)
        .with_variant(variant2.clone())
        .with_best_variant(variant2)
        .with_improvement_delta(0.10);

    assert_eq!(trace.variant_count(), 2);
    assert!(trace.found_improvement());
    assert_eq!(trace.best_score(), Some(0.85));
}

#[test]
fn test_optimization_trace_summary() {
    let variant = VariantResult::new("v1").with_score(0.90);

    let trace = OptimizationTrace::new("opt_004")
        .with_strategy_name("Sequential")
        .with_target_node("agent")
        .with_target_param("system_prompt")
        .with_variant(variant.clone())
        .with_best_variant(variant)
        .with_duration_ms(30000)
        .with_improvement_delta(0.20)
        .with_termination_reason(TerminationReason::MaxIterations(10));

    let summary = trace.summary();
    assert!(summary.contains("opt_004"));
    assert!(summary.contains("agent.system_prompt"));
    assert!(summary.contains("1 variants"));
    assert!(summary.contains("Sequential"));
    assert!(summary.contains("20.0%"));
    assert!(summary.contains("max iterations"));
}

#[test]
fn test_optimization_trace_to_json() {
    let trace = OptimizationTrace::new("opt_005")
        .with_target_node("node")
        .with_target_param("param");

    let json = trace.to_json().unwrap();
    assert!(json.contains("opt_005"));
    assert!(json.contains("node"));
}

#[test]
fn test_variant_result_new() {
    let variant = VariantResult::new("var_001");
    assert_eq!(variant.variant_id, "var_001");
    assert!(variant.tested_at.is_some());
    assert_eq!(variant.score, 0.0);
}

#[test]
fn test_variant_result_with_config() {
    let config =
        NodeConfig::new("test", "llm.chat").with_config(serde_json::json!({"temperature": 0.7}));

    let variant = VariantResult::new("var_002")
        .with_config(config)
        .with_score(0.88)
        .with_evaluation_duration_ms(5000);

    assert!(variant.config.is_some());
    assert!(variant.config_hash().is_some());
    assert_eq!(variant.config_version(), Some(1));
    assert_eq!(variant.score, 0.88);
}

#[test]
fn test_variant_result_metrics() {
    let mut metrics = std::collections::HashMap::new();
    metrics.insert("accuracy".to_string(), 0.95);
    metrics.insert("latency".to_string(), 150.0);

    let variant = VariantResult::new("var_003")
        .with_metrics(metrics)
        .with_metric("throughput", 1000.0);

    assert_eq!(variant.get_metric("accuracy"), Some(0.95));
    assert_eq!(variant.get_metric("latency"), Some(150.0));
    assert_eq!(variant.get_metric("throughput"), Some(1000.0));
    assert_eq!(variant.get_metric("missing"), None);
}

#[test]
fn test_termination_reason_descriptions() {
    assert_eq!(
        TerminationReason::MaxIterations(10).description(),
        "max iterations (10)"
    );
    assert!(TerminationReason::ConvergenceThreshold(0.01)
        .description()
        .contains("converged"));
    assert!(TerminationReason::TimeLimit(60000)
        .description()
        .contains("60.0s"));
    assert_eq!(
        TerminationReason::NoImprovement { iterations: 5 }.description(),
        "no improvement (5 iterations)"
    );
    assert_eq!(TerminationReason::UserStopped.description(), "user stopped");
    assert!(TerminationReason::Error("test".to_string())
        .description()
        .contains("error"));
    assert_eq!(TerminationReason::Unknown.description(), "unknown");
}

#[test]
fn test_termination_reason_is_success() {
    assert!(TerminationReason::MaxIterations(10).is_success());
    assert!(TerminationReason::ConvergenceThreshold(0.01).is_success());
    assert!(TerminationReason::UserStopped.is_success());
    assert!(!TerminationReason::Error("fail".to_string()).is_success());
    assert!(!TerminationReason::Unknown.is_success());
}

#[test]
fn test_termination_reason_converged() {
    assert!(TerminationReason::ConvergenceThreshold(0.01).converged());
    assert!(!TerminationReason::MaxIterations(10).converged());
    assert!(!TerminationReason::UserStopped.converged());
}

#[test]
fn test_optimization_trace_no_improvement() {
    let trace = OptimizationTrace::new("opt_no_improve")
        .with_target_node("node")
        .with_target_param("param")
        .with_improvement_delta(0.0)
        .with_termination_reason(TerminationReason::NoImprovement { iterations: 5 });

    assert!(!trace.found_improvement());
    assert!(trace.best_variant.is_none());

    let summary = trace.summary();
    assert!(summary.contains("no improvement found"));
}
